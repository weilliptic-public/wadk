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
 * The hotels specialist, served to other agents over A2A, every call signed
 * by the calling agent's Weil wallet. Its card is at
 * http://127.0.0.1:8002/.well-known/agent-card.json.
 *
 * <pre>
 *   mvn exec:java -Dexec.mainClass=com.weilliptic.weilwallet.examples.WeilTravelHotels
 * </pre>
 */
public final class WeilTravelHotels {

    /** The subgroup a caller must be permitted in to reach this agent at all. */
    static final String AGENT_SERVICE = "engg.weil";

    /** The subgroup a caller must be permitted in to use searchHotels. */
    static final String HOTEL_SERVICE = "engg.weil";

    public static class Tools {
        @WeilSecured(HOTEL_SERVICE)
        @Schema(description = "Search for hotels in a city for a date range.")
        public static Map<String, Object> searchHotels(
            @Schema(name = "city", description = "City to stay in") String city,
            @Schema(name = "checkIn", description = "Check-in date") String checkIn,
            @Schema(name = "checkOut", description = "Check-out date") String checkOut) {
            return Map.of(
                "city", city,
                "checkIn", checkIn,
                "checkOut", checkOut,
                "hotels", List.of(
                    Map.of("name", "The Grand Hotel", "rating", 4.6, "pricePerNight", 5500,
                        "location", "City Center"),
                    Map.of("name", "Hotel Central", "rating", 4.3, "pricePerNight", 3800,
                        "location", "Near Railway Station"),
                    Map.of("name", "The Royal Palace", "rating", 4.8, "pricePerNight", 7200,
                        "location", "Downtown")));
        }
    }

    public static void main(String[] args) throws Exception {
        LlmAgent agent = LlmAgent.builder()
            .name("hotel_agent")
            .model(WeilAgentSetup.model())
            .description("Finds hotels and accommodation.")
            .instruction("""
                You are a hotel search specialist.
                Use the searchHotels tool for every accommodation question, and
                report exactly what it returns. Do not invent hotels or prices.
                Answer only about hotels.
                """)
            .tools(FunctionTool.create(Tools.class, "searchHotels"))
            .build();

        // Wallet and audit trail from the env file, the searchHotels gate from
        // its annotation; callers need Execution or Management in AGENT_SERVICE
        // to reach the agent. Serves on PORT if set, else 8002.
        Weil.toSecureA2a(agent, Weil.port(8002), AGENT_SERVICE);
    }

}
