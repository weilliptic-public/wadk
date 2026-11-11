package com.weilliptic.weilwallet.examples;

import com.google.adk.agents.LlmAgent;
import com.google.adk.tools.Annotations.Schema;
import com.google.adk.tools.FunctionTool;

import com.weilliptic.weilwallet.adk.Weil;
import com.weilliptic.weilwallet.adk.WeilAgentSetup;
import com.weilliptic.weilwallet.adk.WeilSecured;

import java.util.List;
import java.util.Map;

/**
 * The flights specialist, served to other agents over A2A, every call signed
 * by the calling agent's Weil wallet. Its card is at
 * http://127.0.0.1:8001/.well-known/agent-card.json.
 *
 * <pre>
 *   export WEIL_AGENT_KEY=weil_...          # or WEIL_WALLET_FILE
 *   export SENTINEL_HOST=https://...        # optional
 *   export ANTHROPIC_API_KEY=sk-ant-...
 *   mvn exec:java -Dexec.mainClass=com.weilliptic.weilwallet.examples.WeilTravelFlights
 * </pre>
 */
public final class WeilTravelFlights {

    /** The subgroup a caller must be permitted in to reach this agent at all. */
    static final String AGENT_SERVICE = "engg.weil";

    /** The subgroup a caller must be permitted in to use searchFlights. */
    static final String FLIGHT_SERVICE = "finance.weil";

    public static class Tools {
        @WeilSecured(FLIGHT_SERVICE)
        @Schema(description = "Search for flights between two cities on a date.")
        public static Map<String, Object> searchFlights(
            @Schema(name = "origin", description = "Departure city") String origin,
            @Schema(name = "destination", description = "Arrival city") String destination,
            @Schema(name = "date", description = "Travel date") String date) {
            return Map.of(
                "origin", origin,
                "destination", destination,
                "date", date,
                "flights", List.of(
                    Map.of("airline", "Air India", "departs", "08:30", "arrives", "10:45", "price", 7200),
                    Map.of("airline", "IndiGo", "departs", "13:15", "arrives", "15:30", "price", 6400),
                    Map.of("airline", "Vistara", "departs", "18:20", "arrives", "20:35", "price", 8100)));
        }
    }

    public static void main(String[] args) throws Exception {
        LlmAgent agent = LlmAgent.builder()
            .name("flight_agent")
            .model(WeilAgentSetup.model())
            .description("Finds flights between cities.")
            .instruction("""
                You are a flight search specialist.
                Use the searchFlights tool for every flight question, and report
                exactly what it returns. Do not invent flights or prices.
                Answer only about flights.
                """)
            .tools(FunctionTool.create(Tools.class, "searchFlights"))
            .build();

        // Wallet and audit trail from the env file, the searchFlights gate from
        // its annotation; callers need Execution or Management in AGENT_SERVICE
        // to reach the agent. Serves on PORT if set, else 8001.
        int port = Integer.parseInt(System.getenv().getOrDefault("PORT", "8001"));
        Weil.toSecureA2a(agent, port, AGENT_SERVICE);
    }

}
