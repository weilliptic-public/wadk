package com.weilliptic.weilwallet.examples.city_time_weather;

import com.google.adk.agents.LlmAgent;
import com.google.adk.tools.Annotations.Schema;
import com.google.adk.tools.FunctionTool;

import com.weilliptic.weilwallet.adk.Weil;
import com.weilliptic.weilwallet.adk.WeilSecured;
import com.weilliptic.weilwallet.adk.WeilAgentSetup;

import java.text.Normalizer;
import java.time.ZoneId;
import java.time.ZonedDateTime;
import java.time.format.DateTimeFormatter;
import java.util.Map;

/**
 * ADK's {@code tutorials/city-time-weather} quickstart, with a Weil identity.
 * Every step is audited under the agent's wallet, and graph memory keeps the
 * cities a user asks about, so "my city" works in a later session.
 *
 * <pre>
 *   export WEIL_AGENT_KEY=weil_...          # or WEIL_WALLET_FILE
 *   export ANTHROPIC_API_KEY=sk-ant-...
 *   mvn exec:java -Dexec.mainClass=com.weilliptic.weilwallet.examples.city_time_weather.CityTimeWeather
 * </pre>
 *
 * <p>Served in ADK's dev UI: open http://localhost:8080 (or {@code PORT}) and pick
 * {@code multi_tool_agent}.</p>
 */
public final class CityTimeWeather {

    /** The subgroup a wallet must be permitted in to ask the time or weather. */
    static final String CITY_SERVICE = "engg.weil";

    @WeilSecured(CITY_SERVICE)
    @Schema(description = "Return the current time in a city.")
    public static Map<String, String> getCurrentTime(
        @Schema(name = "city",
            description = "The name of the city for which to retrieve the current time")
        String city) {
        String normalizedCity = Normalizer.normalize(city, Normalizer.Form.NFD)
            .trim()
            .toLowerCase()
            .replaceAll("(\\p{IsM}+|\\p{IsP}+)", "")
            .replaceAll("\\s+", "_");
        return ZoneId.getAvailableZoneIds().stream()
            .filter(zid -> zid.toLowerCase().endsWith("/" + normalizedCity))
            .findFirst()
            .map(zid -> Map.of(
                "status", "success",
                "report", "The current time in " + city + " is "
                    + ZonedDateTime.now(ZoneId.of(zid))
                        .format(DateTimeFormatter.ofPattern("HH:mm"))
                    + "."))
            .orElseGet(() -> Map.of(
                "status", "error",
                "report", "Sorry, I don't have timezone information for " + city + "."));
    }

    @WeilSecured(CITY_SERVICE)
    @Schema(description = "Return the weather report for a city.")
    public static Map<String, String> getWeather(
        @Schema(name = "city",
            description = "The name of the city for which to retrieve the weather report")
        String city) {
        if (city.equalsIgnoreCase("new york")) {
            return Map.of(
                "status", "success",
                "report", "The weather in New York is sunny with a temperature of 25 degrees"
                    + " Celsius (77 degrees Fahrenheit).");
        }
        return Map.of(
            "status", "error",
            "report", "Weather information for " + city + " is not available.");
    }

    public static void main(String[] args) throws Exception {
        LlmAgent agent = LlmAgent.builder()
            .name("multi_tool_agent")
            .model(WeilAgentSetup.model())
            .description("Agent to answer questions about the time and weather in a city.")
            .instruction("""
                You are a helpful agent who can answer user questions about the time and weather
                in a city. Report only what the tools return.
                Remember the cities the user lives in or asks about, so a question about
                "my city" can be answered without asking again.
                """)
            .tools(Weil.withMemory(
                FunctionTool.create(CityTimeWeather.class, "getCurrentTime"),
                FunctionTool.create(CityTimeWeather.class, "getWeather")))
            .build();

        Weil.web(agent, args);
    }

    private CityTimeWeather() {}
}
