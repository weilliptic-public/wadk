package com.weilliptic.weilwallet.examples.web;

import com.google.adk.agents.LlmAgent;

import com.weilliptic.weilwallet.adk.Weil;
import com.weilliptic.weilwallet.adk.WeilAgentSetup;

/**
 * The planner behind ADK's dev UI, the Java counterpart of {@code adk web
 * planner/}.
 *
 * <p>Same agent as {@link WeilTravelPlanner} — same wallet, same two
 * specialists as signed tools, same audit plugin — but driven from a browser
 * instead of one hardcoded question, so a trip can be explored over several
 * turns and each turn's events inspected.</p>
 *
 * <p>Start the two specialists first, then:</p>
 *
 * <pre>
 *   mvn -q compile exec:java \
 *       -Dexec.mainClass=com.weilliptic.weilwallet.examples.web.WeilTravelPlannerWeb
 * </pre>
 *
 * <p>Then open <a href="http://127.0.0.1:8080">http://127.0.0.1:8080</a> and
 * pick {@code travel_planner}. Override the port with {@code PORT} or
 * {@code --server.port=9000}.</p>
 *
 * <p>{@link Weil#web} starts ADK's dev UI with the Weil plugins on every run;
 * without them the UI would run the agent with no audit trail.</p>
 */
public final class WeilTravelPlannerWeb {

    public static void main(String[] args) throws Exception {
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
            .tools(Weil.withMemory(
                Weil.remoteTool("flight_agent", flightsUrl,
                    "Searches for flights between cities. "
                        + "Ask for an origin, a destination and a date."),
                Weil.remoteTool("hotel_agent", hotelsUrl,
                    "Searches for hotels. "
                        + "Ask for a city, a check-in date and a check-out date.")))
            .build();

        // The plugin every agent runs with: an audit trail (with content) on
        // the auditor applet, plus a trace of each run on weilchain::weil,
        // published to publisher::<org>, topic "audits". No tool gates: the planner's tools
        // are its memory and the two specialists, which check their callers
        // themselves. Weil flushes it when the server stops, since the UI has
        // no end-of-run.
        System.setProperty(
            "org.apache.tomcat.websocket.DEFAULT_BUFFER_SIZE", String.valueOf(10 * 1024 * 1024));
        Weil.web(planner, args);
    }
}
