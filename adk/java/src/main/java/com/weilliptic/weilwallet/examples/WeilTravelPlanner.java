package com.weilliptic.weilwallet.examples;

import com.google.adk.agents.LlmAgent;

import com.weilliptic.weilwallet.adk.Weil;
import com.weilliptic.weilwallet.adk.WeilAgentSetup;

/**
 * The coordinator. Holds the two specialists as <em>tools</em>, so it can call
 * both in one turn and merge their answers; each call is signed by this agent's
 * own wallet and verified by the specialist before its model runs.
 *
 * <p>Start the two specialists first, then:</p>
 *
 * <pre>
 *   mvn exec:java -Dexec.mainClass=com.weilliptic.weilwallet.examples.WeilTravelPlanner
 * </pre>
 *
 * <p>Pass a question as an argument, or let it use the default trip.</p>
 */
public final class WeilTravelPlanner {

    public static void main(String[] args) throws Exception {
        System.out.println("planner wallet: " + Weil.identity().address());

        // The specialists' A2A base URLs; their agent cards are read from
        // there. A pre-A2A ".../invoke" URL still works: the suffix is dropped.
        String flightsUrl = System.getenv()
            .getOrDefault("FLIGHTS_URL", "http://127.0.0.1:8001")
            .replaceAll("/invoke/?$", "");
        String hotelsUrl = System.getenv()
            .getOrDefault("HOTELS_URL", "http://127.0.0.1:8002")
            .replaceAll("/invoke/?$", "");


        LlmAgent planner = LlmAgent.builder()
            .name("travel_planner")
            .model(WeilAgentSetup.model())
            .description("Plans trips by coordinating specialist agents.")
            .instruction("""
                You plan trips using two specialist agents, each available as a
                tool. Call them; they answer immediately. You are never waiting
                for a reply, and you must never claim to be.

                For a full trip, call both in the same turn, then combine their
                answers into one plan with a total cost.

                Each specialist is a separate process and cannot see this
                conversation, so put every detail it needs into the question:
                cities, dates, and what you want back.

                Report only what the tools return. Never invent a flight, a
                hotel or a price. If a tool returns an error, explain the reason
                to the user instead of retrying.
                """)
            // The two specialists as signed A2A tools, plus graph memory.
            .tools(Weil.withMemory(
                Weil.remoteTool("flight_agent", flightsUrl,
                    "Searches for flights between cities. "
                        + "Ask for an origin, a destination and a date."),
                Weil.remoteTool("hotel_agent", hotelsUrl,
                    "Searches for hotels. "
                        + "Ask for a city, a check-in date and a check-out date.")))
            .build();

        Weil.chat(planner, args,
            "I want to travel from Delhi to Bombay from October 20 to October 23. "
                + "Plan the trip for me, with a total cost.");
    }
}
