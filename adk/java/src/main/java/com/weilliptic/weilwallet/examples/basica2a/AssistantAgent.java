package com.weilliptic.weilwallet.examples.basica2a;

import com.google.adk.agents.LlmAgent;
import com.google.adk.agents.ParallelAgent;
import com.google.adk.agents.SequentialAgent;
import com.google.adk.tools.Annotations.Schema;
import com.google.adk.tools.FunctionTool;

import com.weilliptic.weilwallet.adk.Weil;
import com.weilliptic.weilwallet.adk.WeilAgentSetup;
import com.weilliptic.weilwallet.adk.WeilSecured;

import java.time.LocalDate;
import java.util.Map;

/**
 * The calling agent of the basic A2A set, in ADK's dev UI on {@code PORT}
 * (default 8080). It has one tool of its own and reaches two agents
 * over A2A, {@link WeatherAgent} and {@link CurrencyAgent}, as the travel
 * planner reaches its flights and hotels agents, in one of two ways:
 *
 * <ul>
 *   <li>{@code A2A_MODE=tool} (default): both are tools ({@link Weil#remoteTool});
 *       the assistant asks them, both in one step when a question needs both,
 *       and combines the answers.</li>
 *   <li>{@code A2A_MODE=agent}: both are sub-agents ({@link Weil#remoteAgent});
 *       the assistant hands the conversation to one of them.</li>
 *   <li>{@code A2A_MODE=chain}: one tool, {@link TravelDeskAgent}, which in
 *       turn calls the other two: assistant → travel desk → weather and
 *       currency, three agents deep in one trace.</li>
 *   <li>{@code A2A_MODE=chain-agent}: the same chain by hand-over all the
 *       way: the assistant hands the conversation to {@link TravelDeskAgent}
 *       (a {@link Weil#remoteAgent}), which hands it to the weather or
 *       currency agent. Start the travel desk with {@code A2A_MODE=agent}.</li>
 *   <li>{@code A2A_MODE=parallel}: both agents as remote sub-agents of an
 *       ADK {@code ParallelAgent}, so both run on every prompt at the same
 *       time, then a summarizer combines their replies. No model chooses.</li>
 * </ul>
 *
 * <p>Every call is signed by this agent's wallet, checked at the called
 * agent's door, and carries this run's trace, so one prompt is one trace.
 * {@code today} runs only if this agent's own wallet holds Execution or
 * Management in {@code engg.weil}.</p>
 *
 * <p>{@code WEATHER_URL}, {@code CURRENCY_URL} and {@code TRAVEL_DESK_URL} are
 * the agents' addresses (default {@code http://127.0.0.1:8011},
 * {@code http://127.0.0.1:8012} and {@code http://127.0.0.1:8013}).</p>
 */
public final class AssistantAgent {

    public static class Tools {
        @WeilSecured("engg.weil")
        @Schema(description = "Today's date.")
        public static Map<String, Object> today() {
            return Map.of("date", LocalDate.now().toString());
        }
    }

    public static void main(String[] args) throws Exception {
        String weatherUrl = System.getenv().getOrDefault("WEATHER_URL", "http://127.0.0.1:8011");
        String currencyUrl = System.getenv().getOrDefault("CURRENCY_URL", "http://127.0.0.1:8012");
        String deskUrl = System.getenv().getOrDefault("TRAVEL_DESK_URL", "http://127.0.0.1:8013");
        String mode = System.getenv().getOrDefault("A2A_MODE", "tool").toLowerCase(java.util.Locale.ROOT);

        if (mode.equals("parallel")) {
            // Both remote agents run every time, at the same time.
            ParallelAgent lookups = ParallelAgent.builder()
                .name("lookups")
                .description("Asks the weather and currency agents together.")
                .subAgents(
                    Weil.remoteAgent("weather_agent", weatherUrl, "Tells today's weather in a city."),
                    Weil.remoteAgent("currency_agent", currencyUrl, "Converts amounts between currencies."))
                .build();

            // Runs after both have answered, and combines their replies.
            LlmAgent summarizer = LlmAgent.builder()
                .name("summarizer")
                .model(WeilAgentSetup.model())
                .description("Combines the weather and currency answers.")
                .instruction("""
                    The weather agent and the currency agent have just answered
                    the user's question; their replies are in the conversation.
                    Combine them into one short answer, using only what they
                    said. If one of them refused for a permission reason, quote
                    the reason. Use today for the date if it is needed.
                    """)
                .tools(FunctionTool.create(Tools.class, "today"))
                .build();

            // The root: first both lookups, then the summary.
            SequentialAgent assistant = SequentialAgent.builder()
                .name("assistant")
                .description("A travel helper that checks the weather and currency rates together.")
                .subAgents(lookups, summarizer)
                .build();

            Weil.web(assistant, args);
            return;
        }

        LlmAgent.Builder builder = LlmAgent.builder()
            .name("assistant")
            .model(WeilAgentSetup.model())
            .description("A travel helper that checks the date, the weather and currency rates.");

        if (mode.equals("chain-agent")) {
            builder
                .instruction("""
                    You are a travel helper. Use today for the date.
                    For any question about a city's weather or about converting
                    money, transfer to travel_desk.
                    """)
                .tools(FunctionTool.create(Tools.class, "today"))
                .subAgents(Weil.remoteAgent("travel_desk", deskUrl,
                    "Answers questions about a city's weather and currency conversions."));
        } else if (mode.equals("chain")) {
            builder
                .instruction("""
                    You are a travel helper with two tools:
                    - today: today's date.
                    - travel_desk: answers questions about a city's weather and
                      currency conversions.
                    The travel desk cannot see this conversation, so put every
                    detail in your request, and ask it everything about weather
                    and money in one request. If a tool or agent refuses for a
                    permission reason, quote the reason.
                    """)
                .tools(
                    FunctionTool.create(Tools.class, "today"),
                    Weil.remoteTool("travel_desk", deskUrl,
                        "Answers questions about a city's weather and currency conversions."));
        } else if (mode.equals("agent")) {
            builder
                .instruction("""
                    You are a travel helper. Use today for the date.
                    For a weather question, transfer to weather_agent.
                    For a currency question, transfer to currency_agent.
                    If a tool refuses for a permission reason, quote the reason.
                    """)
                .tools(FunctionTool.create(Tools.class, "today"))
                .subAgents(
                    Weil.remoteAgent("weather_agent", weatherUrl, "Tells today's weather in a city."),
                    Weil.remoteAgent("currency_agent", currencyUrl, "Converts amounts between currencies."));
        } else {
            builder
                .instruction("""
                    You are a travel helper with three tools:
                    - today: today's date.
                    - weather_agent: today's weather in a city.
                    - currency_agent: converts an amount between currencies.
                    The agents cannot see this conversation, so put every detail
                    in your request. Call every tool the question needs, in the
                    same step when they do not depend on each other, then combine
                    the answers. If a tool or agent refuses for a permission
                    reason, quote the reason.
                    """)
                .tools(
                    FunctionTool.create(Tools.class, "today"),
                    Weil.remoteTool("weather_agent", weatherUrl, "Tells today's weather in a city."),
                    Weil.remoteTool("currency_agent", currencyUrl, "Converts amounts between currencies."));
        }

        Weil.web(builder.build(), args);
    }

    private AssistantAgent() {}
}
