package com.weilliptic.weilwallet.adk;

import com.google.adk.agents.BaseAgent;
import com.google.adk.agents.CallbackContext;
import com.google.adk.agents.InvocationContext;
import com.google.adk.agents.LlmAgent;
import com.google.adk.events.Event;
import com.google.adk.models.BaseLlm;
import com.google.adk.models.LlmRequest;
import com.google.adk.models.LlmResponse;
import com.google.adk.models.Model;
import com.google.adk.plugins.BasePlugin;
import com.google.adk.tools.BaseTool;
import com.google.adk.tools.ToolContext;
import com.google.genai.types.Content;

import io.reactivex.rxjava3.core.Completable;
import io.reactivex.rxjava3.core.Maybe;

import java.util.ArrayList;
import java.util.EnumMap;
import java.util.List;
import java.util.Map;

/**
 * Plugs {@link WeilAction}s into Google ADK's lifecycle, so an agent's activity
 * calls applets on WeilChain as that agent.
 *
 * <p>Mirrors the Go SDK's {@code weiladk.NewPlugin}. One plugin covers every
 * agent the runner executes, including sub-agents.</p>
 *
 * <pre>{@code
 * WeilPlugin plugin = WeilPlugin.builder(identity)
 *     .withActions(AuditAction.defaults())
 *     .flushAfterRun(60_000L)
 *     .onError((ev, action, err) -> log.warn("{} failed on {}", action, ev.hook(), err))
 *     .build();
 *
 * Runner runner = new InMemoryRunner(agent, "my-app", List.of(plugin));
 * }</pre>
 */
public final class WeilPlugin extends BasePlugin {

    /** Receives failures that do not fail a hook. */
    @FunctionalInterface
    public interface ErrorHandler {
        void accept(WeilEvent event, String actionName, Throwable error);
    }

    private final WeilIdentity identity;
    private final Map<Hook, List<WeilAction>> byHook;
    private final Dispatcher dispatcher;
    private final boolean ownsDispatcher;
    private final long flushAfterRunMillis;
    private final ErrorHandler onError;


    


    public static Builder builder(WeilIdentity identity) {
        return new Builder(identity);
    }

    public Dispatcher dispatcher() {
        return dispatcher;
    }

    // ------------------------------------------------------------------
    // ADK Plugin callbacks
    // ------------------------------------------------------------------

    @Override
    public Maybe<Content> onUserMessageCallback(InvocationContext ctx, Content content) {
        return fireVoid(event(Hook.USER_MESSAGE).invocationContext(ctx).userMessage(content));
    }

    @Override
    public Maybe<Content> beforeRunCallback(InvocationContext ctx) {
        return fireVoid(event(Hook.BEFORE_RUN).invocationContext(ctx));
    }

    @Override
    public Maybe<Content> beforeAgentCallback(BaseAgent agent, CallbackContext ctx) {
        return fireVoid(event(Hook.BEFORE_AGENT).agent(agent).callbackContext(ctx));
    }

    @Override
    public Maybe<Content> afterAgentCallback(BaseAgent agent, CallbackContext ctx) {
        return fireVoid(event(Hook.AFTER_AGENT).agent(agent).callbackContext(ctx));
    }

    @Override
    public Maybe<LlmResponse> beforeModelCallback(CallbackContext ctx, LlmRequest.Builder request) {
        return fireVoidAs(modelOf(event(Hook.BEFORE_MODEL).callbackContext(ctx), ctx));
    }

    @Override
    public Maybe<LlmResponse> afterModelCallback(CallbackContext ctx, LlmResponse response) {
        // A streamed run reports each chunk of a reply, then the whole reply;
        // only the whole one is a model response to audit and trace.
        if (response.partial().orElse(false)) {
            return Maybe.empty();
        }
        return fireVoidAs(
            modelOf(event(Hook.AFTER_MODEL).callbackContext(ctx), ctx).llmResponse(response));
    }

    @Override
    public Maybe<LlmResponse> onModelErrorCallback(
        CallbackContext ctx, LlmRequest.Builder request, Throwable error) {
        return fireVoidAs(modelOf(event(Hook.MODEL_ERROR).callbackContext(ctx), ctx).error(error));
    }

    @Override
    public Maybe<Map<String, Object>> beforeToolCallback(
        BaseTool tool, Map<String, Object> args, ToolContext ctx) {
        WeilEvent ev = event(Hook.BEFORE_TOOL).callbackContext(ctx).tool(tool).toolArgs(args);
        if (!has(ev.hook())) {
            return Maybe.empty();
        }
        try {
            fire(ev);
        } catch (WeilAction.VetoException veto) {
            // A vetoed tool does not run, and the model reads the refusal as
            // its result. ADK takes a value from this callback as the tool's
            // response; an error would abort the whole run, leaving the user,
            // or an agent calling over A2A, with an empty answer.
            return Maybe.just(Map.of("error", String.valueOf(veto.getMessage())));
        }
        return Maybe.empty();
    }

    @Override
    public Maybe<Map<String, Object>> afterToolCallback(
        BaseTool tool, Map<String, Object> args, ToolContext ctx, Map<String, Object> result) {
        return fireVoidAsMap(
            event(Hook.AFTER_TOOL).callbackContext(ctx).tool(tool).toolArgs(args).toolResult(result));
    }

    @Override
    public Maybe<Map<String, Object>> onToolErrorCallback(
        BaseTool tool, Map<String, Object> args, ToolContext ctx, Throwable error) {
        return fireVoidAsMap(
            event(Hook.TOOL_ERROR).callbackContext(ctx).tool(tool).toolArgs(args).error(error));
    }

    @Override
    public Maybe<Event> onEventCallback(InvocationContext ctx, Event adkEvent) {
        return fireVoidAs(event(Hook.EVENT).invocationContext(ctx));
    }

    @Override
    public Completable afterRunCallback(InvocationContext ctx) {
        return Completable.fromAction(() -> {
            WeilEvent ev = event(Hook.AFTER_RUN).invocationContext(ctx);
            try {
                fire(ev);
            } catch (WeilAction.VetoException veto) {
                // After the run there is nothing left to stop; report only.
                report(ev, "after_run", veto);
            }
            if (flushAfterRunMillis > 0 && !dispatcher.flush(flushAfterRunMillis)) {
                report(ev, "flush", new IllegalStateException(
                    "queued calls did not drain in " + flushAfterRunMillis + "ms: "
                        + dispatcher.stats()));
            }
        });
    }

    @Override
    public Completable onRunErrorCallback(InvocationContext ctx, Throwable error) {
        return Completable.fromAction(() -> {
            WeilEvent ev = event(Hook.RUN_ERROR).invocationContext(ctx).error(error);
            try {
                fire(ev);
            } catch (WeilAction.VetoException veto) {
                report(ev, "run_error", veto);
            }
        });
    }

    /**
     * ADK's own close hook: drains queued calls, then shuts down a dispatcher
     * this plugin created. Only matters with a queued {@link Dispatcher}; with
     * the default inline one there is nothing to drain or stop.
     *
     * <p>Note this returns {@code Completable}, so {@code WeilPlugin} is not an
     * {@code AutoCloseable} and cannot be used in try-with-resources. Call
     * {@link #shutdown()} to drain it directly.</p>
     */
    @Override
    public Completable close() {
        return Completable.fromAction(this::shutdown);
    }

    /**
     * Send every queued call, however long that takes, and release the
     * dispatcher, synchronously. Only matters with a queued {@link Dispatcher},
     * whose calls would otherwise die with the process; with the default
     * inline one every call was sent before its hook returned, and this
     * returns at once. {@link Weil#shutdown} calls it for every plugin
     * {@link Weil} made.
     */
    public void shutdown() {
        dispatcher.flush();
        if (ownsDispatcher) {
            dispatcher.close();
        }
    }

    // ------------------------------------------------------------------
    // Firing
    // ------------------------------------------------------------------

    private WeilEvent event(Hook hook) {
        return new WeilEvent(hook, identity);
    }

    /**
     * Name the LLM and model behind the agent making this call. Best effort:
     * an agent that is not an {@link LlmAgent}, or a model that cannot be
     * resolved, leaves both unset rather than failing the hook.
     */
    private static WeilEvent modelOf(WeilEvent ev, CallbackContext ctx) {
        if (ctx == null) {
            return ev;
        }
        try {
            if (!(ctx.invocationContext().agent() instanceof LlmAgent agent)) {
                return ev;
            }
            Model resolved = agent.resolvedModel();
            BaseLlm llm = resolved.model().orElse(null);
            ev.model(resolved.modelName().orElse(llm != null ? llm.model() : null));
            if (llm instanceof ClaudeModel claude) {
                ev.llm(claude.provider());
            } else if (llm != null) {
                ev.llm(llm.getClass().getSimpleName());
            }
        } catch (RuntimeException e) {
            // unresolvable: record the call without naming its model
        }
        return ev;
    }

    /** No actions on this hook: skip the work entirely. */
    private boolean has(Hook hook) {
        List<WeilAction> actions = byHook.get(hook);
        return actions != null && !actions.isEmpty();
    }

    private Maybe<Content> fireVoid(WeilEvent ev) {
        if (!has(ev.hook())) {
            return Maybe.empty();
        }
        try {
            fire(ev);
        } catch (WeilAction.VetoException veto) {
            return Maybe.error(veto);
        }
        return Maybe.empty();
    }

    private <T> Maybe<T> fireVoidAs(WeilEvent ev) {
        if (!has(ev.hook())) {
            return Maybe.empty();
        }
        try {
            fire(ev);
        } catch (WeilAction.VetoException veto) {
            return Maybe.error(veto);
        }
        return Maybe.empty();
    }

    private Maybe<Map<String, Object>> fireVoidAsMap(WeilEvent ev) {
        return fireVoidAs(ev);
    }

    /**
     * Run every action registered for this event's hook, in registration order.
     *
     * @throws WeilAction.VetoException when an action vetoes the hook
     */
    private void fire(WeilEvent shared) {
        List<WeilAction> actions = byHook.get(shared.hook());
        if (actions == null) {
            return;
        }
        for (WeilAction action : actions) {
            if (!action.appliesTo(shared)) {
                continue;
            }
            runAction(action, shared);
        }
    }

    private void runAction(WeilAction action, WeilEvent shared) {
        // Each action sees its own copy, so a per-action identity never leaks.
        WeilIdentity acting = identity;
        if (action.identityResolver() != null) {
            acting = action.identityResolver().resolve(shared);
        }
        WeilEvent ev = shared.copyWith(acting);

        Call call;
        try {
            call = action.buildCall(ev);
        } catch (WeilAction.VetoException veto) {
            // A build step may decide the answer itself -- a permission gate
            // does exactly that. Pass the veto through unwrapped so the model
            // sees the reason, not a nested exception.
            throw veto;
        } catch (Exception e) {
            fail(action, ev, e);
            return;
        }
        // A build step that returns null has nothing to submit -- a gate that
        // decided for itself, or an event this action skips. No identity needed.
        if (call == null) {
            return;
        }

        // Only a call that will actually be submitted needs a signing identity.
        if (acting == null) {
            fail(action, ev, new IllegalStateException(
                "no identity bound for agent " + ev.agentName()));
            return;
        }

        if (action.mode() == Mode.BLOCKING) {
            runBlocking(action, ev, call);
        } else {
            runNonBlocking(action, ev, call);
        }
    }

    private void runBlocking(WeilAction action, WeilEvent ev, Call call) {
        WeilResult result;
        try {
            result = ev.identity().invoke(call);
        } catch (Exception e) {
            fail(action, ev, e);
            return;
        }
        if (!action.hasResultHandler()) {
            return;
        }
        try {
            action.handleResult(ev, result);
        } catch (WeilAction.VetoException veto) {
            // A deliberate rejection vetoes whatever the failure policy says.
            throw veto;
        } catch (Exception e) {
            throw new WeilAction.VetoException(
                "action \"" + action.name() + "\" on " + ev.hook() + ": " + e.getMessage());
        }
    }

    private void runNonBlocking(WeilAction action, WeilEvent ev, Call call) {
        dispatcher.submit(ev.identity(), call, (result, error) -> {
            if (error != null) {
                report(ev, action.name(), error);
                return;
            }
            if (action.hasResultHandler()) {
                try {
                    action.handleResult(ev, result);
                } catch (Exception e) {
                    report(ev, action.name(), e);
                }
            }
        });
    }

    /** An infrastructure failure: honour the action's failure policy. */
    private void fail(WeilAction action, WeilEvent ev, Throwable error) {
        boolean closed = action.mode() == Mode.BLOCKING
            && action.failurePolicy() == FailurePolicy.FAIL_CLOSED
            && ev.hook() != Hook.AFTER_RUN
            && ev.hook() != Hook.RUN_ERROR;
        if (closed) {
            throw new WeilAction.VetoException(
                "action \"" + action.name() + "\" on " + ev.hook() + ": " + error);
        }
        report(ev, action.name(), error);
    }

    private void report(WeilEvent ev, String actionName, Throwable error) {
        if (onError != null) {
            onError.accept(ev, actionName, error);
        }
    }

    private WeilPlugin(Builder b) {
        super(b.name);
        this.identity = b.identity;
        this.dispatcher = b.dispatcher != null ? b.dispatcher
            : b.inBackground ? Dispatcher.ordered()
            : Dispatcher.inline();
        this.ownsDispatcher = b.dispatcher == null;
        this.flushAfterRunMillis = b.flushAfterRunMillis;
        this.onError = b.onError;

        this.byHook = new EnumMap<>(Hook.class);
        for (WeilAction action : b.actions) {
            for (Hook hook : action.hooks()) {
                byHook.computeIfAbsent(hook, h -> new ArrayList<>()).add(action);
            }
        }
    }

    // ------------------------------------------------------------------

    public static final class Builder {
        private final WeilIdentity identity;
        private String name = "weil";
        private final List<WeilAction> actions = new ArrayList<>();
        private Dispatcher dispatcher;
        private boolean inBackground;
        private long flushAfterRunMillis;
        private ErrorHandler onError;

        private Builder(WeilIdentity identity) {
            this.identity = identity;
        }

        public Builder name(String name) {
            this.name = name;
            return this;
        }

        public Builder withActions(WeilAction... actions) {
            for (WeilAction a : actions) {
                this.actions.add(a);
            }
            return this;
        }

        public Builder withActions(List<WeilAction> actions) {
            this.actions.addAll(actions);
            return this;
        }

        /**
         * Share a dispatcher across plugins; otherwise an inline one is
         * created and owned, so every non-blocking call is made before its
         * hook returns and none can be lost. Pass a queued {@link Dispatcher}
         * to stop the agent waiting on those calls, at the risk of losing
         * whatever is still queued when the process stops.
         */
        public Builder dispatcher(Dispatcher dispatcher) {
            this.dispatcher = dispatcher;
            return this;
        }

        /**
         * Send non-blocking calls from a background queue this plugin owns,
         * {@link Dispatcher#ordered()}: one at a time, in the order made, so
         * the agent never waits on them. {@link #shutdown()} sends whatever
         * is still queued, so nothing is lost on a normal exit; a killed
         * process loses what was queued. Ignored when a {@link #dispatcher}
         * is given.
         */
        public Builder inBackground() {
            this.inBackground = true;
            return this;
        }

        /**
         * Make the end of each run wait, up to this long, for queued calls to
         * land. On-chain writes take roughly two seconds, so with a queued
         * {@link #dispatcher} a short run can finish before its audit entries
         * are recorded. Only matters with a queued dispatcher: the default
         * inline one has nothing to wait for, and the flush returns at once.
         */
        public Builder flushAfterRun(long millis) {
            this.flushAfterRunMillis = millis;
            return this;
        }

        public Builder onError(ErrorHandler onError) {
            this.onError = onError;
            return this;
        }

        public WeilPlugin build() {
            return new WeilPlugin(this);
        }
    }
}


