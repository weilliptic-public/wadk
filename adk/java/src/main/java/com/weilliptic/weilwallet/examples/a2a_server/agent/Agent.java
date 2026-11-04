package com.weilliptic.weilwallet.examples.a2a_server.agent;

import com.google.adk.agents.LlmAgent;
import com.google.adk.tools.Annotations.Schema;
import com.google.adk.tools.FunctionTool;

import com.weilliptic.weilwallet.adk.Weil;
import com.weilliptic.weilwallet.adk.WeilSecured;
import com.weilliptic.weilwallet.adk.WeilAgentSetup;

import java.util.List;
import java.util.Map;
import java.util.TreeSet;
import java.util.stream.Collectors;

/**
 * ADK's {@code examples/a2a_server}: a prime-checking agent served over
 * A2A, here with its own Weil identity. Every call must be signed by the
 * calling agent's wallet, and every step is audited under this one's.
 *
 * <p>The sample's before/after model callbacks only logged; the plugin's audit
 * hooks record the same steps on chain, so they are dropped. Its card is at
 * http://127.0.0.1:8003/.well-known/agent-card.json, and
 * {@link com.weilliptic.weilwallet.examples.a2a_basic.A2AAgent} calls it.</p>
 *
 * <pre>
 *   export WEIL_AGENT_KEY=weil_...          # this agent's key, or WEIL_WALLET_FILE
 *   export SENTINEL_HOST=https://...        # optional
 *   export ANTHROPIC_API_KEY=sk-ant-...
 *   export PORT=8003                        # optional
 *   mvn exec:java -Dexec.mainClass=com.weilliptic.weilwallet.examples.a2a_server.agent.Agent
 * </pre>
 */
public final class Agent {

    /** The subgroup a wallet must be permitted in to use this agent's tools. */
    static final String DICE_SERVICE = "engg.weil";

    /** The subgroup a caller must be permitted in to reach this agent at all. */
    static final String AGENT_SERVICE = "engg.weil";

    public static void main(String[] args) throws Exception {
        System.out.println("prime agent wallet: " + Weil.identity().address());

        LlmAgent agent = LlmAgent.builder()
            .name("check_prime_agent")
            .model(WeilAgentSetup.model())
            .description("Checks whether numbers are prime.")
            .instruction("""
                You check whether numbers are prime.
                If the last user message contains numbers, call checkPrime exactly once with
                exactly those integers as a list (e.g., [2]). Never add other numbers. Do not ask
                for clarification. Return only the tool's result.
                Always pass a list of integers to the tool (use a single-element list for one
                number). Never pass strings.
                """)
            .tools(FunctionTool.create(Agent.class, "checkPrime"))
            .build();

        // Wallet and audit trail from the env file, the checkPrime gate from
        // @WeilSecured on checkPrime; callers need Execution or
        // Management in AGENT_SERVICE to reach the agent. Serves on PORT if
        // set, else 8003.
        int port = Integer.parseInt(System.getenv().getOrDefault("PORT", "8003"));
        Weil.toSecureA2a(agent, port, AGENT_SERVICE);
    }

    @WeilSecured(DICE_SERVICE)
    @Schema(description = "Check which of the given integers are prime.")
    public static Map<String, Object> checkPrime(
        @Schema(name = "nums", description = "The integers to check") List<Integer> nums) {
        TreeSet<Integer> primes = new TreeSet<>();
        for (int num : nums) {
            if (isPrime(num)) {
                primes.add(num);
            }
        }
        String result;
        if (primes.isEmpty()) {
            result = "No prime numbers found.";
        } else if (primes.size() == 1) {
            result = primes.first() + " is a prime number.";
        } else {
            result = primes.stream().map(String::valueOf).collect(Collectors.joining(", "))
                + " are prime numbers.";
        }
        return Map.of("result", result);
    }

    private static boolean isPrime(int num) {
        if (num <= 1) {
            return false;
        }
        for (int i = 2; (long) i * i <= num; i++) {
            if (num % i == 0) {
                return false;
            }
        }
        return true;
    }

    private Agent() {}
}
