package com.weilliptic.weilwallet.examples.basica2a;

import com.google.adk.agents.LlmAgent;
import com.google.adk.tools.Annotations.Schema;
import com.google.adk.tools.FunctionTool;

import com.weilliptic.weilwallet.adk.Weil;
import com.weilliptic.weilwallet.adk.WeilAgentSetup;
import com.weilliptic.weilwallet.adk.WeilSecured;

import java.util.Map;

/**
 * The called agent of the basic A2A pair: a weather agent served to other
 * agents over A2A. Start it before {@link AssistantAgent}.
 *
 * <ul>
 *   <li><b>Door check:</b> a caller's wallet needs Execution or Management in
 *       {@code WEATHER_SERVICE} (default {@code engg.weil}) to reach it at all.</li>
 *   <li><b>Tool check:</b> {@code getWeather} runs only if this agent's own
 *       wallet holds Execution or Management in {@code WEATHER_TOOL_SERVICE}
 *       (default {@code engg.weil}).</li>
 *   <li><b>Trace:</b> a call from {@link AssistantAgent} carries its trace, so
 *       this agent's work is added to the assistant's trace rather than
 *       published on its own.</li>
 * </ul>
 *
 * <p>Serves on {@code WEATHER_PORT} (default 8011); its card is at
 * {@code http://127.0.0.1:8011/.well-known/agent-card.json}.</p>
 */
public final class WeatherAgent {

    /** Who may call this agent: a subgroup or a rule, such as "engg.weil | hr.weil". */
    static final String AGENT_SERVICE = System.getenv().getOrDefault("WEATHER_SERVICE", "engg.weil");

    /** What this agent's own wallet needs to run getWeather. */
    static final String TOOL_SERVICE = "engg.weil";

    public static class Tools {
        @WeilSecured(TOOL_SERVICE)
        @Schema(description = "Today's weather in a city.")
        public static Map<String, Object> getWeather(
            @Schema(name = "city", description = "The city") String city) {
            int temperature = 18 + Math.floorMod(city.toLowerCase().hashCode(), 17);
            String sky = switch (Math.floorMod(city.toLowerCase().hashCode(), 3)) {
                case 0 -> "sunny";
                case 1 -> "cloudy";
                default -> "light rain";
            };
            return Map.of("city", city, "temperature_c", temperature, "sky", sky);
        }
    }

    public static void main(String[] args) throws Exception {
        LlmAgent agent = LlmAgent.builder()
            .name("weather_agent")
            .model(WeilAgentSetup.model())
            .description("Tells today's weather in a city.")
            .instruction("""
                You report today's weather. Always call getWeather for the city
                asked about and report exactly what it returns. If it refuses for
                a permission reason, quote the reason. Answer only about weather.
                """)
            .tools(FunctionTool.create(Tools.class, "getWeather"))
            .build();

        int port = Integer.parseInt(System.getenv().getOrDefault("WEATHER_PORT", "8011"));
        Weil.toSecureA2a(agent, port, AGENT_SERVICE);
    }

    private WeatherAgent() {}
}
