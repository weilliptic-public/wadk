package com.weilliptic.weilwallet.adk;

import com.google.adk.a2a.agent.RemoteA2AAgent;
import com.google.adk.a2a.converters.A2AMetadataKey;
import com.google.genai.types.Content;
import com.google.genai.types.Part;
import com.google.adk.agents.BaseAgent;
import com.google.adk.agents.InvocationContext;
import com.google.adk.events.Event;

import com.weilliptic.weilwallet.TraceAction;

import io.reactivex.rxjava3.core.Flowable;

import java.lang.reflect.Method;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.stream.Collectors;

/**
 * A remote A2A agent as a sub-agent the conversation is handed to, made by
 * {@link Weil#remoteAgent}: ADK's {@link RemoteA2AAgent}, every call signed by
 * this agent's wallet, that also carries the caller's trace. The message it
 * sends names this run's trace and the hand-over's node, as {@code weil_trace}
 * in the signed metadata, so the remote agent adds its run to this trace
 * rather than starting one of its own, as a {@link Weil#remoteTool} call does.
 *
 * <p>It runs the remote agent's own logic under this agent's place in the
 * tree, so plugins see one agent, not two. Should that logic not be
 * reachable, the remote agent runs as itself and the call goes without the
 * trace: the remote agent then keeps a trace of its own.</p>
 */
public final class WeilRemoteAgent extends BaseAgent {

    private static final Method RUN_IMPL = runImpl();

    private final RemoteA2AAgent remote;

    WeilRemoteAgent(RemoteA2AAgent remote) {
        super(remote.name(), remote.description(), List.of(), null, null);
        this.remote = remote;
    }

    /** The ADK remote agent this sends through. */
    public RemoteA2AAgent remote() {
        return remote;
    }

    @Override
    protected Flowable<Event> runAsyncImpl(InvocationContext ctx) {
        return relayed(ctx).map(WeilRemoteAgent::withoutA2aLabels);
    }

    /**
     * The ADK A2A labels ({@code adk_type}, {@code adk_partial}, ...) the remote
     * reply's parts carry, taken off.
     *
     * <p>{@link RemoteA2AAgent} keeps them in each part's metadata when it turns
     * the reply into events. An agent served over A2A that hands over to this
     * one then sends those events on, and ADK's converter, adding the same
     * labels again to a function call or response part, refuses the duplicate
     * key: the served agent's whole run fails. Without them the events convert
     * as the agent's own do, and the labels are rebuilt from the part itself.</p>
     */
    static Event withoutA2aLabels(Event event) {
        Content content = event.content().orElse(null);
        if (content == null || content.parts().isEmpty()) {
            return event;
        }
        boolean changed = false;
        List<Part> parts = new ArrayList<>();
        for (Part part : content.parts().get()) {
            Map<String, Object> metadata = part.partMetadata().orElse(null);
            if (metadata == null || A2A_LABELS.stream().noneMatch(metadata::containsKey)) {
                parts.add(part);
                continue;
            }
            Map<String, Object> kept = new LinkedHashMap<>(metadata);
            kept.keySet().removeAll(A2A_LABELS);
            parts.add(kept.isEmpty()
                ? part.toBuilder().clearPartMetadata().build()
                : part.toBuilder().partMetadata(kept).build());
            changed = true;
        }
        return changed
            ? event.toBuilder().content(content.toBuilder().parts(parts).build()).build()
            : event;
    }

    private static final Set<String> A2A_LABELS = Arrays.stream(A2AMetadataKey.values())
        .map(A2AMetadataKey::getType)
        .collect(Collectors.toUnmodifiableSet());

    private Flowable<Event> relayed(InvocationContext ctx) {
        if (RUN_IMPL == null) {
            return remote.runAsync(ctx);
        }
        // Decided now, after the before-agent hooks, which give the hand-over
        // its node in the trace.
        Map<String, Object> trace = TraceAction.remoteAgentContext(ctx.invocationId(), name());
        Flowable<Event> send = Flowable.defer(() -> {
            try {
                @SuppressWarnings("unchecked")
                Flowable<Event> events = (Flowable<Event>) RUN_IMPL.invoke(remote, ctx);
                return events;
            } catch (java.lang.reflect.InvocationTargetException e) {
                return Flowable.error(e.getCause() != null ? e.getCause() : e);
            }
        });
        return WeilA2A.withOutgoingTrace(send, trace);
    }

    @Override
    protected Flowable<Event> runLiveImpl(InvocationContext ctx) {
        return remote.runLive(ctx);
    }

    /** ADK's remote agent's own run, or null when this ADK version hides it. */
    private static Method runImpl() {
        try {
            Method method = RemoteA2AAgent.class.getDeclaredMethod("runAsyncImpl", InvocationContext.class);
            method.setAccessible(true);
            return method;
        } catch (ReflectiveOperationException | RuntimeException e) {
            System.err.println("[weil] remote agents will not carry the trace: " + e);
            return null;
        }
    }
}
