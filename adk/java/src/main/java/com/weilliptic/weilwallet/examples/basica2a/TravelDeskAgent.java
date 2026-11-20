package com.weilliptic.weilwallet.examples.basica2a;

import com.google.adk.agents.BaseAgent;
import com.google.adk.agents.LlmAgent;
import com.google.adk.agents.ParallelAgent;
import com.google.adk.agents.SequentialAgent;

import com.weilliptic.weilwallet.adk.Weil;
import com.weilliptic.weilwallet.adk.WeilAgentSetup;

/**
 * The middle agent of the basic A2A set, for a chain of calls: it is served
 * over A2A like {@link WeatherAgent} and {@link CurrencyAgent}, and is itself
 * a caller of both. {@link AssistantAgent} with {@code A2A_MODE=chain} asks
 * only this agent, so one prompt goes assistant → travel desk → weather and
 * currency. Start the weather and currency agents first, then this one, then
 * the assistant.
 *
 * <ul>
 *   <li><b>Door check:</b> a caller's wallet (the assistant's) needs Execution
 *       or Management in {@code TRAVEL_DESK_SERVICE} (default {@code engg.weil}).</li>
 *   <li><b>Its own calls:</b> signed by this agent's wallet, so the weather and
 *       currency agents check <em>this</em> agent's wallet at their doors, not
 *       the assistant's.</li>
 *   <li><b>Trace:</b> it joins the assistant's trace and passes it on, so the
 *       weather and currency steps hang under this agent's calls, in the one
 *       trace the assistant publishes.</li>
 * </ul>
 *
 * <p>{@code A2A_MODE=agent} makes the weather and currency agents remote
 * sub-agents ({@link Weil#remoteAgent}) the desk hands the conversation to,
 * one per question, instead of tools; pair it with the assistant's
 * {@code A2A_MODE=chain-agent}.</p>
 *
 * <p>{@code A2A_MODE=parallel} runs both agents as remote sub-agents of an
 * ADK {@code ParallelAgent}, at the same time on every request, and then a
 * summarizer combines their replies into the desk's answer. Pair it with the
 * assistant's {@code A2A_MODE=chain-agent} (the assistant hands over to the
 * desk) or {@code A2A_MODE=chain} (the assistant asks the desk).</p>
 *
 * <p>Serves on {@code TRAVEL_DESK_PORT} (default 8013). {@code WEATHER_URL}
 * and {@code CURRENCY_URL} are where it finds the other two (default
 * {@code http://127.0.0.1:8011} and {@code http://127.0.0.1:8012}).</p>
 */
public final class TravelDeskAgent {

    /** Who may call this agent: a subgroup or a rule, such as "engg.weil | finance.weil". */
    static final String AGENT_SERVICE = System.getenv().getOrDefault("TRAVEL_DESK_SERVICE", "engg.weil");

    /**
     * Serves the desk on {@code TRAVEL_DESK_PORT} (default 8013), its way of
     * reaching the weather and currency agents set by {@code A2A_MODE}:
     * {@code tool} (default), {@code agent} or {@code parallel}.
     */
    public static void main(String[] args) throws Exception {
        String mode = System.getenv().getOrDefault("A2A_MODE", "tool").toLowerCase(java.util.Locale.ROOT);
        int port = Integer.parseInt(System.getenv().getOrDefault("TRAVEL_DESK_PORT", "8013"));
        String weatherUrl = System.getenv().getOrDefault("WEATHER_URL", "http://127.0.0.1:8011");
        String currencyUrl = System.getenv().getOrDefault("CURRENCY_URL", "http://127.0.0.1:8012");

        BaseAgent desk = mode.equals("parallel")
            ? parallelDesk(weatherUrl, currencyUrl)
            : llmDesk(mode.equals("agent"), weatherUrl, currencyUrl);
        Weil.toSecureA2a(desk, port, AGENT_SERVICE);
    }

    /**
     * Both agents at once, every time, then a summary: the desk's answer is
     * the summarizer's.
     */
    static BaseAgent parallelDesk(String weatherUrl, String currencyUrl) {
        ParallelAgent lookups = ParallelAgent.builder()
            .name("lookups")
            .description("Asks the weather and currency agents together.")
            .subAgents(
                Weil.remoteAgent("weather_agent", weatherUrl, "Tells today's weather in a city."),
                Weil.remoteAgent("currency_agent", currencyUrl, "Converts amounts between currencies."))
            .build();

        LlmAgent summarizer = LlmAgent.builder()
            .name("desk_summarizer")
            .model(WeilAgentSetup.model())
            .description("Combines the weather and currency answers.")
            .instruction("""
                The weather agent and the currency agent have just answered the
                request; their replies are in the conversation. Combine them
                into one short answer, using only what they said. If one of
                them refused for a permission reason, quote the reason.
                """)
            .build();

        return SequentialAgent.builder()
            .name("travel_desk")
            .description("Answers travel questions about a city's weather and money.")
            .subAgents(lookups, summarizer)
            .build();
    }

    /**
     * An LLM desk: with {@code handOver}, the weather and currency agents are
     * remote sub-agents it hands the conversation to; otherwise remote tools
     * it asks and combines.
     */
    static BaseAgent llmDesk(boolean handOver, String weatherUrl, String currencyUrl) {

        LlmAgent.Builder builder = LlmAgent.builder()
            .name("travel_desk")
            .model(WeilAgentSetup.model())
            .description("Answers travel questions about a city's weather and money.");

        if (handOver) {
            // The conversation is handed on to one agent, whose answer is this
            // agent's answer: one topic per question.
            builder
                .instruction("""
                    You are a travel desk. For a weather question, transfer to
                    weather_agent. For a currency question, transfer to
                    currency_agent.
                    """)
                .subAgents(
                    Weil.remoteAgent("weather_agent", weatherUrl, "Tells today's weather in a city."),
                    Weil.remoteAgent("currency_agent", currencyUrl, "Converts amounts between currencies."));
        } else {
            builder
                .instruction("""
                    You are a travel desk with two tools:
                    - weather_agent: today's weather in a city.
                    - currency_agent: converts an amount between currencies.
                    The agents cannot see this conversation, so put every detail
                    in your request. Call every tool the question needs, in the
                    same step when they do not depend on each other, then combine
                    the answers. If an agent refuses for a permission reason,
                    quote the reason.
                    """)
                .tools(
                    Weil.remoteTool("weather_agent", weatherUrl, "Tells today's weather in a city."),
                    Weil.remoteTool("currency_agent", currencyUrl, "Converts amounts between currencies."));
        }
        return builder.build();
    }

    private TravelDeskAgent() {}
}
