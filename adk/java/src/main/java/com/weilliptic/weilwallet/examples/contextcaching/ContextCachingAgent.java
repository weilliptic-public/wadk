package com.weilliptic.weilwallet.examples.contextcaching;

import com.google.adk.agents.CallbackContext;
import com.google.adk.agents.ContextCacheConfig;
import com.google.adk.agents.LlmAgent;
import com.google.adk.apps.App;
import com.google.adk.models.BaseLlm;
import com.google.adk.models.LlmResponse;
import com.google.adk.plugins.BasePlugin;

import com.weilliptic.weilwallet.adk.Weil;
import com.weilliptic.weilwallet.adk.WeilAgentSetup;

import io.reactivex.rxjava3.core.Maybe;

import java.time.Duration;
import java.util.List;

/**
 * ADK's {@code examples/contextcaching}: a weather assistant whose long
 * instruction, a reference of several thousand tokens, is the same on every
 * turn, so it is cached rather than processed afresh each time. With a Weil
 * identity, every turn is audited under the agent's wallet.
 *
 * <p>Caching happens where the model is served:</p>
 * <ul>
 *   <li>on Gemini, through the App's {@link ContextCacheConfig}, as in ADK's
 *       sample;</li>
 *   <li>on Claude, through Anthropic's prompt cache, which
 *       {@link com.weilliptic.weilwallet.adk.ClaudeModel} marks the
 *       instruction for (on for Anthropic's own API; CLAUDE_PROMPT_CACHE
 *       turns it on or off);</li>
 *   <li>on OpenAI, automatically, for a prompt that starts the same way.</li>
 * </ul>
 *
 * <p>After each model call, {@link CacheReport} prints the prompt's size and
 * how much of it came from the cache: nothing on the first turn, most of it
 * after. It is one of the App's own plugins, which {@link Weil#chat(App,
 * String[], String...)} keeps, running the Weil plugin before it.</p>
 *
 * <pre>
 *   export WEIL_AGENT_KEY=weil_...          # or WEIL_WALLET_FILE
 *   export ANTHROPIC_API_KEY=sk-ant-...
 *   mvn exec:java -Dexec.mainClass=com.weilliptic.weilwallet.examples.contextcaching.ContextCachingAgent
 * </pre>
 *
 * <p>Pass a question as an argument, or let it ask ADK's six in one session.</p>
 */
public final class ContextCachingAgent {

    /** How long Gemini keeps the cache: the shortest Vertex AI accepts, as in ADK's sample. */
    static final Duration CACHE_TTL = Duration.ofMinutes(1);

    /** ADK's sample questions, each answered from a different part of the reference. */
    static final String[] PROMPTS = {
        "In one sentence, what should I do during a thunderstorm?",
        "How is wind chill defined, and when does it apply?",
        "What units does the reference use for wind speed?",
        "What is the average annual rainfall listed for Verdant?",
        "Which named city is the driest?",
        "What is the July average temperature for Puerto Brisa?",
    };

    /**
     * The instruction, which is almost all of what is cached. Caching starts
     * only past the model's minimum (about 1,000 to 2,000 tokens for Claude,
     * 2,048 for Gemini 2.5), so station rows pad it well past that.
     */
    private static final String WEATHER_REFERENCE = buildWeatherReference();

    private ContextCachingAgent() {}

    /** An App that caches for all its agents, with {@link CacheReport} among its plugins. */
    public static App createApp(BaseLlm model) {
        LlmAgent agent = LlmAgent.builder()
            .name("weather_assistant")
            .description("Answers weather questions from a large shared reference.")
            .model(model)
            .instruction(WEATHER_REFERENCE)
            .build();

        return App.builder()
            .name("context_caching_demo")
            .rootAgent(agent)
            .plugins(new CacheReport())
            .contextCacheConfig(new ContextCacheConfig(/* cacheIntervals= */ 10, CACHE_TTL, /* minTokens= */ 0))
            .build();
    }

    public static void main(String[] args) {
        System.out.println("agent wallet: " + Weil.identity().address());
        System.out.println("model: " + WeilAgentSetup.modelId());
        Weil.chat(createApp(WeilAgentSetup.model()), args, PROMPTS);
    }

    /** Prints each model call's prompt size and the share the cache served. */
    static final class CacheReport extends BasePlugin {

        CacheReport() {
            super("cache-report");
        }

        @Override
        public Maybe<LlmResponse> afterModelCallback(CallbackContext ctx, LlmResponse response) {
            response.usageMetadata().ifPresent(usage -> {
                int prompt = usage.promptTokenCount().orElse(0);
                int cached = usage.cachedContentTokenCount().orElse(0);
                System.out.printf("  [cache] prompt %d tokens, %d served from cache%s%n",
                    prompt, cached, cached > 0 ? "" : " (first use, or caching off)");
            });
            return Maybe.empty();
        }
    }

    private static String buildWeatherReference() {
        StringBuilder reference = new StringBuilder();
        reference.append("You are a weather assistant. Answer questions using ONLY the weather"
            + " reference below, keep answers to one or two sentences, and cite the section or"
            + " city you used.\n\n");

        List<String> guidelines = List.of(
            "Forecasting basics: a forecast combines current observations, numerical model"
                + " guidance, and local climatology; always state the valid time window and your"
                + " confidence.",
            "Wind chill: how cold the air feels once wind is accounted for. It is only defined for"
                + " temperatures at or below 10C and wind above 5 km/h.",
            "Heat index: the apparent temperature, combining air temperature and humidity. It is"
                + " meaningful above about 27C when relative humidity is high.",
            "Precipitation types: rain, drizzle, sleet, freezing rain, snow, and hail are"
                + " distinguished by the temperature profile between the cloud base and the"
                + " ground.",
            "Storm safety: during a thunderstorm, move indoors, avoid open fields and tall isolated"
                + " trees, and stay off corded electronics until 30 minutes after the last"
                + " thunder.",
            "Units: temperatures are in degrees Celsius, wind speed in km/h, and precipitation in"
                + " millimeters unless a value states otherwise.");
        for (int i = 0; i < guidelines.size(); i++) {
            reference.append("Section ").append(i + 1).append(": ").append(guidelines.get(i)).append('\n');
        }

        reference.append("\nClimate normals for named cities:\n");
        List<String> cities = List.of(
            "Marisol: coastal and mild; January average 12C, July average 24C, annual rainfall 640 mm.",
            "Fjordheim: cold maritime; January average -6C, July average 15C, annual rainfall 900 mm.",
            "Solara: hot desert; January average 18C, July average 41C, annual rainfall 90 mm.",
            "Verdant: temperate rainforest; January average 7C, July average 19C, annual rainfall"
                + " 2400 mm.",
            "Highpoint: alpine; January average -9C, July average 12C, annual rainfall 1100 mm.",
            "Puerto Brisa: tropical; January average 26C, July average 29C, annual rainfall 1800 mm.",
            "Windgate: windy plains; January average -2C, July average 27C, annual rainfall 520 mm.",
            "Frosthollow: subarctic; January average -22C, July average 16C, annual rainfall 380 mm.");
        cities.forEach(city -> reference.append("  ").append(city).append('\n'));

        reference.append("\nAutomated station normals:\n");
        List<String> windDirections = List.of("N", "NE", "E", "SE", "S", "SW", "W", "NW");
        for (int i = 1; i <= 100; i++) {
            reference.append(String.format(
                "  Station S%d (sector %c%d): January average %dC, July average %dC, annual rainfall"
                    + " %d mm, prevailing wind %s at %d km/h.\n",
                i,
                (char) ('A' + i % 8),
                i % 12,
                -12 + (i * 7) % 34,
                14 + (i * 3) % 22,
                250 + (i * 37) % 2300,
                windDirections.get(i % 8),
                6 + (i * 5) % 38));
        }
        return reference.toString();
    }
}
