package com.weilliptic.weilwallet.examples.web;

import com.google.adk.agents.BaseAgent;
import com.google.adk.agents.LlmAgent;
import com.google.adk.plugins.BasePlugin;
import com.google.adk.web.AdkWebServer;
import com.google.adk.web.AgentStaticLoader;

import com.weilliptic.weilwallet.adk.Weil;
import com.weilliptic.weilwallet.adk.WeilAgentSetup;

import org.springframework.beans.factory.support.DefaultListableBeanFactory;
import org.springframework.boot.SpringApplication;
import org.springframework.context.ApplicationContextInitializer;
import org.springframework.context.ConfigurableApplicationContext;

import java.util.List;

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
 *   java -cp "target/classes:$(cat /tmp/cp.txt)" \
 *       com.weilliptic.weilwallet.examples.web.WeilTravelPlannerWeb
 * </pre>
 *
 * <p>Then open <a href="http://127.0.0.1:8080">http://127.0.0.1:8080</a> and
 * pick {@code travel_planner}. Override the port with
 * {@code --server.port=9000}.</p>
 *
 * <p>{@link AdkWebServer#start} registers agents but takes no plugins, so this
 * starts Spring the same way and adds the {@code extraPlugins} bean that
 * ADK's RunnerService looks for — without it the UI would run the agent with
 * no audit trail.</p>
 */
public final class WeilTravelPlannerWeb {

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
        serve(args, Weil.plugins(), planner);
    }

    /**
     * Start ADK's dev UI over {@code agents}, with {@code plugins} applied to
     * every run.
     */
    private static void serve(String[] args, List<BasePlugin> plugins, BaseAgent... agents) {
        // Tells ADK not to scan a source directory for agents; ours are built.
        System.setProperty("adk.agents.loader", "static");
        System.setProperty(
            "org.apache.tomcat.websocket.DEFAULT_BUFFER_SIZE", String.valueOf(10 * 1024 * 1024));

        SpringApplication app = new SpringApplication(AdkWebServer.class);
        app.addInitializers(new ApplicationContextInitializer<ConfigurableApplicationContext>() {
            @Override
            public void initialize(ConfigurableApplicationContext context) {
                DefaultListableBeanFactory beans =
                    (DefaultListableBeanFactory) context.getBeanFactory();
                beans.registerSingleton("agentLoader", new AgentStaticLoader(agents));
                beans.registerSingleton("extraPlugins", plugins);
            }
        });
        app.run(args);
    }
}
