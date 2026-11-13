package com.weilliptic.weilwallet.adk;

import java.util.EnumSet;
import java.util.List;
import java.util.Objects;
import java.util.Set;
import java.util.function.Function;
import java.util.function.Predicate;

/**
 * Binds an applet call to ADK hooks: <em>when</em> to fire, <em>what</em> to
 * call, and <em>how</em> to treat the answer.
 *
 * <p>Mirrors {@code weiladk.Action} in the Go SDK. This is the main thing you
 * write. {@code build} always runs inside the hook, so it may read the event's
 * payload freely; for a non-blocking action only the call itself is deferred.</p>
 *
 * <pre>{@code
 * WeilAction.named("spend-cap")
 *     .on(Hook.BEFORE_TOOL)
 *     .when(ev -> "pay_invoice".equals(ev.toolName()))
 *     .blocking()
 *     .failClosed()
 *     .build(ev -> Call.to(ledger, "balance_for")
 *                      .args(Map.of("addr", ev.identity().address()))
 *                      .build())
 *     .onResult((ev, res) -> {
 *         long balance = res.decode().asLong();
 *         long amount = ((Number) ev.toolArgs().get("amount")).longValue();
 *         if (amount > balance / 4) {
 *             throw new VetoException("POLICY: exceeds 25% of treasury");
 *         }
 *     })
 *     .done();
 * }</pre>
 */
public final class WeilAction {

    /**
     * Thrown from a result handler to veto the hook.
     *
     * <p>A blocking before-hook that vetoes skips the step entirely: the tool
     * never runs, and the model is handed the reason as the tool's error.</p>
     */
    public static final class VetoException extends RuntimeException {
        public VetoException(String message) {
            super(message);
        }
    }

    /** Builds the call to make from the event, or returns null to skip. */
    @FunctionalInterface
    public interface CallBuilder {
        Call build(WeilEvent event) throws Exception;
    }

    /** Sees a successful call's result. Throw {@link VetoException} to veto. */
    @FunctionalInterface
    public interface ResultHandler {
        void accept(WeilEvent event, WeilResult result) throws Exception;
    }

    /** Picks the identity for an event, overriding the plugin's default. */
    @FunctionalInterface
    public interface IdentityResolver {
        WeilIdentity resolve(WeilEvent event);
    }

    private final String name;
    private final Set<Hook> hooks;
    private final Mode mode;
    private final FailurePolicy failurePolicy;
    private final Predicate<WeilEvent> when;
    private final CallBuilder callBuilder;
    private final ResultHandler onResult;
    private final IdentityResolver identity;

    private WeilAction(Builder b) {
        this.name = b.name != null ? b.name : "action";
        this.hooks = b.hooks.isEmpty() ? EnumSet.noneOf(Hook.class) : EnumSet.copyOf(b.hooks);
        this.mode = b.mode;
        this.failurePolicy = b.failurePolicy;
        this.when = b.when;
        this.callBuilder = Objects.requireNonNull(b.callBuilder, "build(...) is required");
        this.onResult = b.onResult;
        this.identity = b.identity;
    }

    public static Builder named(String name) {
        return new Builder().name(name);
    }

    public String name() {
        return name;
    }

    public Set<Hook> hooks() {
        return hooks;
    }

    public Mode mode() {
        return mode;
    }

    public FailurePolicy failurePolicy() {
        return failurePolicy;
    }

    boolean appliesTo(WeilEvent event) {
        return when == null || when.test(event);
    }

    Call buildCall(WeilEvent event) throws Exception {
        return callBuilder.build(event);
    }

    boolean hasResultHandler() {
        return onResult != null;
    }

    void handleResult(WeilEvent event, WeilResult result) throws Exception {
        if (onResult != null) {
            onResult.accept(event, result);
        }
    }

    IdentityResolver identityResolver() {
        return identity;
    }

    public static final class Builder {
        private String name;
        private final Set<Hook> hooks = EnumSet.noneOf(Hook.class);
        private Mode mode = Mode.NON_BLOCKING;
        private FailurePolicy failurePolicy = FailurePolicy.FAIL_OPEN;
        private Predicate<WeilEvent> when;
        private CallBuilder callBuilder;
        private ResultHandler onResult;
        private IdentityResolver identity;

        public Builder name(String name) {
            this.name = name;
            return this;
        }

        /** The hooks this action fires on. */
        public Builder on(Hook... hooks) {
            for (Hook h : hooks) {
                this.hooks.add(h);
            }
            return this;
        }

        public Builder on(List<Hook> hooks) {
            this.hooks.addAll(hooks);
            return this;
        }

        /** Filter the events this action fires for. */
        public Builder when(Predicate<WeilEvent> when) {
            this.when = when;
            return this;
        }

        public Builder blocking() {
            this.mode = Mode.BLOCKING;
            return this;
        }

        public Builder nonBlocking() {
            this.mode = Mode.NON_BLOCKING;
            return this;
        }

        public Builder failClosed() {
            this.failurePolicy = FailurePolicy.FAIL_CLOSED;
            return this;
        }

        public Builder failOpen() {
            this.failurePolicy = FailurePolicy.FAIL_OPEN;
            return this;
        }

        public Builder build(CallBuilder callBuilder) {
            this.callBuilder = callBuilder;
            return this;
        }

        /** Shorthand for an action that always calls the same applet method. */
        public Builder callTo(Applet applet, String method, Function<WeilEvent, java.util.Map<String, ?>> args) {
            this.callBuilder = ev -> {
                Call.Builder c = Call.to(applet, method);
                if (args != null) {
                    c.args(args.apply(ev));
                }
                return c.build();
            };
            return this;
        }

        public Builder onResult(ResultHandler onResult) {
            this.onResult = onResult;
            return this;
        }

        /** Override which identity signs this action's call. */
        public Builder identity(IdentityResolver identity) {
            this.identity = identity;
            return this;
        }

        public WeilAction done() {
            return new WeilAction(this);
        }
    }
}
