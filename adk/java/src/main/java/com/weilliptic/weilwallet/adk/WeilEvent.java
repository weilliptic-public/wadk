package com.weilliptic.weilwallet.adk;

import com.google.adk.agents.BaseAgent;
import com.google.adk.agents.CallbackContext;
import com.google.adk.agents.InvocationContext;
import com.google.adk.models.LlmResponse;
import com.google.adk.tools.BaseTool;
import com.google.adk.tools.ToolContext;
import com.google.genai.types.Content;

import java.time.Instant;
import java.util.Map;

/**
 * What a {@link WeilAction} sees when its hook fires: who is acting, where in
 * the run, and the hook's payload.
 *
 * <p>Mirrors {@code weiladk.Event} in the Go SDK. Fields not carried by a given
 * {@link Hook} are null.</p>
 */
public final class WeilEvent {

    private final Hook hook;
    private final Instant time;
    private final WeilIdentity identity;

    private String agentName;
    private String invocationId;
    private BaseAgent agent;
    private CallbackContext callbackContext;
    private InvocationContext invocationContext;
    private BaseTool tool;
    private Map<String, Object> toolArgs;
    private Map<String, Object> toolResult;
    private LlmResponse llmResponse;
    private Content userMessage;
    private String llm;
    private String model;
    private Throwable error;

    WeilEvent(Hook hook, WeilIdentity identity) {
        this.hook = hook;
        this.identity = identity;
        this.time = Instant.now();
    }

    public Hook hook() {
        return hook;
    }

    public Instant time() {
        return time;
    }

    /** The identity whose wallet signs this action's call. */
    public WeilIdentity identity() {
        return identity;
    }

    public String agentName() {
        return agentName;
    }

    public String invocationId() {
        return invocationId;
    }

    public BaseAgent agent() {
        return agent;
    }

    public CallbackContext callbackContext() {
        return callbackContext;
    }

    public InvocationContext invocationContext() {
        return invocationContext;
    }

    public BaseTool tool() {
        return tool;
    }

    /** The tool's name, or null outside tool hooks. */
    public String toolName() {
        return tool == null ? null : tool.name();
    }

    /**
     * The id the model gave this tool call, or null outside tool hooks or
     * when the model gave none. Tools called together each have their own.
     */
    public String toolCallId() {
        return callbackContext instanceof ToolContext tc ? tc.functionCallId().orElse(null) : null;
    }

    public Map<String, Object> toolArgs() {
        return toolArgs;
    }

    public Map<String, Object> toolResult() {
        return toolResult;
    }

    public LlmResponse llmResponse() {
        return llmResponse;
    }

    /** The user's message, or null outside {@link Hook#USER_MESSAGE}. */
    public Content userMessage() {
        return userMessage;
    }

    /**
     * Who served the model call, such as {@code api.anthropic.com} or
     * {@code api.moonshot.ai}, or the adapter's class name when it does not
     * say; null outside model hooks.
     */
    public String llm() {
        return llm;
    }

    /** The model the agent is configured with, or null outside model hooks. */
    public String model() {
        return model;
    }

    public Throwable error() {
        return error;
    }

    // ---- package-private setters, used by WeilPlugin while building events ----

    WeilEvent agentName(String v) {
        this.agentName = v;
        return this;
    }

    WeilEvent invocationId(String v) {
        this.invocationId = v;
        return this;
    }

    WeilEvent agent(BaseAgent v) {
        this.agent = v;
        if (v != null && this.agentName == null) {
            this.agentName = v.name();
        }
        return this;
    }

    WeilEvent callbackContext(CallbackContext v) {
        this.callbackContext = v;
        // Model and tool hooks carry no agent of their own; the context names
        // both, so every record says whose step it was and in which run.
        if (v != null) {
            if (this.agentName == null) {
                this.agentName = v.agentName();
            }
            if (this.invocationId == null) {
                this.invocationId = v.invocationId();
            }
        }
        return this;
    }

    WeilEvent invocationContext(InvocationContext v) {
        this.invocationContext = v;
        if (v != null) {
            if (this.invocationId == null) {
                this.invocationId = v.invocationId();
            }
            if (this.agentName == null && v.agent() != null) {
                this.agentName = v.agent().name();
            }
        }
        return this;
    }

    WeilEvent tool(BaseTool v) {
        this.tool = v;
        return this;
    }

    WeilEvent toolArgs(Map<String, Object> v) {
        this.toolArgs = v;
        return this;
    }

    WeilEvent toolResult(Map<String, Object> v) {
        this.toolResult = v;
        return this;
    }

    WeilEvent llmResponse(LlmResponse v) {
        this.llmResponse = v;
        return this;
    }

    WeilEvent userMessage(Content v) {
        this.userMessage = v;
        return this;
    }

    WeilEvent llm(String v) {
        this.llm = v;
        return this;
    }

    WeilEvent model(String v) {
        this.model = v;
        return this;
    }

    WeilEvent error(Throwable v) {
        this.error = v;
        return this;
    }

    /** Shallow copy, so a per-action identity never leaks into another action's view. */
    WeilEvent copyWith(WeilIdentity other) {
        WeilEvent e = new WeilEvent(hook, other);
        e.agentName = agentName;
        e.invocationId = invocationId;
        e.agent = agent;
        e.callbackContext = callbackContext;
        e.invocationContext = invocationContext;
        e.tool = tool;
        e.toolArgs = toolArgs;
        e.toolResult = toolResult;
        e.llmResponse = llmResponse;
        e.userMessage = userMessage;
        e.llm = llm;
        e.model = model;
        e.error = error;
        return e;
    }
}
