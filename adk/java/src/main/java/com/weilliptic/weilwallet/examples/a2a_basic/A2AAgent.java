package com.weilliptic.weilwallet.examples.a2a_basic;

import com.google.adk.agents.LlmAgent;
import com.google.adk.tools.Annotations.Schema;
import com.google.adk.tools.FunctionTool;
import com.google.adk.tools.ToolContext;

import com.weilliptic.weilwallet.adk.Weil;
import com.weilliptic.weilwallet.adk.WeilSecured;
import com.weilliptic.weilwallet.adk.WeilAgentSetup;

import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.Random;

/**
 * ADK's {@code examples/a2a_basic}: a root agent that rolls dice with a
 * local sub-agent and hands prime checks to the remote prime agent,
 * {@link com.weilliptic.weilwallet.examples.a2a_server.agent.Agent}, over
 * A2A. Each A2A call is signed by this agent's wallet, which the prime server
 * verifies before its model runs.
 *
 * <p>Start the prime agent first, with its own key, then run this with a
 * different one:</p>
 *
 * <pre>
 *   export WEIL_AGENT_KEY=weil_...          # this agent's key, or WEIL_WALLET_FILE
 *   export ANTHROPIC_API_KEY=sk-ant-...
 *   export PRIME_URL=http://127.0.0.1:8003  # optional
 *   mvn exec:java -Dexec.mainClass=com.weilliptic.weilwallet.examples.a2a_basic.A2AAgent
 * </pre>
 *
 * <p>Served in ADK's dev UI: open http://localhost:8080 (or {@code PORT}) and pick
 * {@code root_agent}.</p>
 */
public final class A2AAgent {

    /** The subgroup a wallet must be permitted in to use this agent's tools. */
    static final String DICE_SERVICE = "engg.weil";

    private static final Random RANDOM = new Random();

    public static void main(String[] args) throws Exception {
        String primeUrl = System.getenv().getOrDefault("PRIME_URL", "http://127.0.0.1:8003");

        LlmAgent rollAgent = LlmAgent.builder()
            .name("roll_agent")
            .model(WeilAgentSetup.model())
            .description("Handles rolling dice of different sizes.")
            .instruction("""
                When asked to roll a die, always call the rollDie tool with the requested number
                of sides (default to 6 if unspecified). Do not fabricate results.
                """)
            .tools(FunctionTool.create(A2AAgent.class, "rollDie"))
            .build();

        LlmAgent root = LlmAgent.builder()
            .name("root_agent")
            .model(WeilAgentSetup.model())
            .description("Rolls dice locally and checks primes with a remote agent.")
            .instruction("""
                You can roll dice locally and delegate prime-checking to the remote prime_agent.
                1. When the user asks to roll a die, route the request to roll_agent.
                2. When the user asks to check primes, delegate to prime_agent.
                3. If the user asks to roll and then check, roll_agent first, then prime_agent
                   with the result.
                Always recap the die result before discussing primality.
                """)
            .subAgents(
                rollAgent,
                Weil.remoteAgent("prime_agent", primeUrl,
                    "Checks whether numbers are prime. Give it the integers to check."))
            .tools(Weil.memory())
            .build();

        Weil.web(root, args);
    }

    @WeilSecured(DICE_SERVICE)
    @Schema(description = "Roll a die with the given number of sides and return the result.")
    @SuppressWarnings("unchecked")
    public static Map<String, Object> rollDie(
        @Schema(name = "sides", description = "Number of sides on the die; use 6 if unknown") int sides,
        // ADK fills this in, and recognises it only by the name "toolContext";
        // this build does not keep parameter names, so it is given here.
        @Schema(name = "toolContext") ToolContext toolContext) {
        // This session's rolls, so the model can check earlier ones for primes.
        List<Integer> rolls = (List<Integer>) toolContext.state()
            .computeIfAbsent("rolls", k -> new ArrayList<Integer>());
        int result = RANDOM.nextInt(Math.max(sides, 1)) + 1;
        rolls.add(result);
        return Map.of("result", result, "sides", sides);
    }

    private A2AAgent() {}
}
