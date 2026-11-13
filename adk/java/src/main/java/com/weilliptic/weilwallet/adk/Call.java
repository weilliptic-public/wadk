package com.weilliptic.weilwallet.adk;

import com.fasterxml.jackson.databind.ObjectMapper;

import java.util.Map;
import java.util.Objects;

/**
 * One applet method invocation.
 *
 * <p>Mirrors {@code weiladk.Call} in the Go SDK.</p>
 */
public final class Call {

    private static final ObjectMapper JSON = new ObjectMapper();

    private final Applet applet;
    private final String method;
    private final String argsJson;
    private final boolean hideArgs;
    private final boolean fireAndForget;

    private Call(Builder b) {
        this.applet = Objects.requireNonNull(b.applet, "applet");
        this.method = Objects.requireNonNull(b.method, "method");
        this.argsJson = b.argsJson != null ? b.argsJson : "{}";
        this.hideArgs = b.hideArgs;
        this.fireAndForget = b.fireAndForget;
    }

    public static Builder to(Applet applet, String method) {
        return new Builder().applet(applet).method(method);
    }

    public Applet applet() {
        return applet;
    }

    public String method() {
        return method;
    }

    /** The method arguments, already JSON-encoded. */
    public String argsJson() {
        return argsJson;
    }

    /** Keep the arguments out of the public transaction record. */
    public boolean hideArgs() {
        return hideArgs;
    }

    /** Submit without waiting for the platform to run the method. */
    public boolean fireAndForget() {
        return fireAndForget;
    }

    public static final class Builder {
        private Applet applet;
        private String method;
        private String argsJson;
        private boolean hideArgs;
        private boolean fireAndForget;

        public Builder applet(Applet applet) {
            this.applet = applet;
            return this;
        }

        public Builder method(String method) {
            this.method = method;
            return this;
        }

        /** Set arguments from a map; encoded as a JSON object. */
        public Builder args(Map<String, ?> args) {
            try {
                this.argsJson = JSON.writeValueAsString(args);
            } catch (Exception e) {
                throw new IllegalArgumentException("cannot encode call args", e);
            }
            return this;
        }

        /** Set arguments from already-encoded JSON. */
        public Builder argsJson(String json) {
            this.argsJson = json;
            return this;
        }

        public Builder hideArgs(boolean hideArgs) {
            this.hideArgs = hideArgs;
            return this;
        }

        public Builder fireAndForget(boolean fireAndForget) {
            this.fireAndForget = fireAndForget;
            return this;
        }

        public Call build() {
            return new Call(this);
        }
    }
}
