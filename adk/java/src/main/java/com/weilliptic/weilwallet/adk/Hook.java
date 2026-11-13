package com.weilliptic.weilwallet.adk;

/**
 * A point in an ADK agent's lifecycle that a {@link WeilAction} can attach to.
 *
 * <p>These are the callbacks of ADK's {@code Plugin} interface, named as in the
 * Go SDK's {@code weiladk.Hook}. Listed in lifecycle order.</p>
 */
public enum Hook {

    /** A user message arrives. */
    USER_MESSAGE("user_message"),
    /** A whole run starts. */
    BEFORE_RUN("before_run"),
    /** An agent starts its turn. */
    BEFORE_AGENT("before_agent"),
    /** Before each LLM call. */
    BEFORE_MODEL("before_model"),
    /** After each LLM call. */
    AFTER_MODEL("after_model"),
    /** An LLM call failed. */
    MODEL_ERROR("model_error"),
    /** Before a tool runs. A blocking action here can stop the tool. */
    BEFORE_TOOL("before_tool"),
    /** After a tool runs, whether or not it failed. */
    AFTER_TOOL("after_tool"),
    /** A tool failed. */
    TOOL_ERROR("tool_error"),
    /** An agent finishes its turn. */
    AFTER_AGENT("after_agent"),
    /** Every event, before it is persisted. */
    EVENT("event"),
    /** A whole run ends. A blocking action here cannot veto anything. */
    AFTER_RUN("after_run"),
    /** A run failed. */
    RUN_ERROR("run_error");

    private final String wireName;

    Hook(String wireName) {
        this.wireName = wireName;
    }

    /** The name recorded in audit entries. Stable across SDKs. */
    public String wireName() {
        return wireName;
    }

    @Override
    public String toString() {
        return wireName;
    }
}
