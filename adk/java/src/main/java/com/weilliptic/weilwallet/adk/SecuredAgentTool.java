package com.weilliptic.weilwallet.adk;

import com.google.adk.agents.BaseAgent;
import com.google.adk.tools.AgentTool;

import java.util.Objects;

/**
 * A local agent as a tool, run with the caller's plugins, that a wallet may
 * call only with Execution or Management in {@link #service}: the agent-level
 * counterpart of {@link WeilSecured}, made by {@link Weil#agentTool(BaseAgent,
 * String)}. {@link PermissionGate#annotated} reads the service off the tool,
 * as it reads the annotation off a method.
 */
final class SecuredAgentTool extends AgentTool {

    /** The subgroup and organisation, {@code "<subgroup>.<org>"}, such as {@code "finance.weil"}. */
    final String service;

    SecuredAgentTool(BaseAgent agent, String service) {
        super(agent, /* skipSummarization= */ false, /* includePlugins= */ true);
        this.service = Objects.requireNonNull(service, "service");
    }
}
