package com.weilliptic.weilwallet.examples.helloworld;

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
import java.util.TreeSet;
import java.util.stream.Collectors;

/**
 * ADK's {@code examples/helloworld}: one agent that rolls dice and
 * checks primes, with a Weil identity. Every step is audited under its wallet,
 * and graph memory keeps its rolls past the end of the session, where the
 * sample's session state forgets them.
 *
 * <pre>
 *   export WEIL_AGENT_KEY=weil_...          # or WEIL_WALLET_FILE
 *   export ANTHROPIC_API_KEY=sk-ant-...
 *   mvn exec:java -Dexec.mainClass=com.weilliptic.weilwallet.examples.helloworld.HelloWorldAgent
 * </pre>
 *
 * <p>Served in ADK's dev UI: open http://localhost:8080 (or {@code PORT}) and pick
 * {@code hello_world_agent}.</p>
 */
public final class HelloWorldAgent {

    /** The subgroup a wallet must be permitted in to use this agent's tools. */
    static final String DICE_SERVICE = "engg.weil";

    private static final Random RANDOM = new Random();

    public static void main(String[] args) throws Exception {
        LlmAgent agent = LlmAgent.builder()
            .name("hello_world_agent")
            .model(WeilAgentSetup.model())
            .description("Hello world agent that can roll a die and check prime numbers.")
            .instruction("""
                You roll dice and answer questions about the outcome of the dice rolls.
                You can roll dice of different sizes.
                You can use multiple tools in parallel by calling functions in parallel (in one
                request and in one round).
                It is ok to discuss previous dice rolls, and comment on the dice rolls.
                When you are asked to roll a die, you must call the rollDie tool with the number
                of sides. Be sure to pass in an integer. Do not pass in a string. If you don't
                know how many sides the die has, use 6.
                You should never roll a die on your own.
                When checking prime numbers, call the checkPrime tool with a list of integers.
                Be sure to pass in a list of integers. You should never pass in a string.
                You should not check prime numbers before calling the tool.
                When you are asked to roll a die and check prime numbers, you should always make
                the following two function calls:
                1. You should first call the rollDie tool to get a roll. Wait for the function
                   response before calling the checkPrime tool.
                2. After you get the function response from the rollDie tool, you should call the
                   checkPrime tool with the rollDie result.
                   2.1 If the user asks you to check primes based on previous rolls, make sure
                       you include the previous rolls in the list.
                3. When you respond, you must include the rollDie result from step 1, and the
                   number of sides the die has.
                You should always perform the previous 3 steps when asked for a roll and to
                check prime numbers.
                You should not rely on the previous history for prime results.
                Remember every roll in memory, so rolls from earlier sessions can be checked too.
                """)
            .tools(Weil.withMemory(
                FunctionTool.create(HelloWorldAgent.class, "rollDie"),
                FunctionTool.create(HelloWorldAgent.class, "checkPrime")))
            .build();

        Weil.web(agent, args);
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

    private HelloWorldAgent() {}
}
