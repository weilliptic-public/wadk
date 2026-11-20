package com.weilliptic.weilwallet.examples.basica2a;

import com.google.adk.agents.LlmAgent;
import com.google.adk.tools.Annotations.Schema;
import com.google.adk.tools.FunctionTool;

import com.weilliptic.weilwallet.adk.Weil;
import com.weilliptic.weilwallet.adk.WeilAgentSetup;
import com.weilliptic.weilwallet.adk.WeilSecured;

import java.util.Locale;
import java.util.Map;

/**
 * The second called agent of the basic A2A set: a currency agent served to
 * other agents over A2A. Start it, with {@link WeatherAgent}, before
 * {@link AssistantAgent}.
 *
 * <ul>
 *   <li><b>Door check:</b> a caller's wallet needs Execution or Management in
 *       {@code CURRENCY_SERVICE} (default {@code engg.weil}).</li>
 *   <li><b>Tool check:</b> {@code convert} runs only if this agent's own wallet
 *       holds Execution or Management in {@code finance.weil}.</li>
 *   <li><b>Trace:</b> its work is added to the caller's trace.</li>
 * </ul>
 *
 * <p>Serves on {@code CURRENCY_PORT} (default 8012).</p>
 */
public final class CurrencyAgent {

    /** Who may call this agent: a subgroup or a rule, such as "engg.weil | finance.weil". */
    static final String AGENT_SERVICE = System.getenv().getOrDefault("CURRENCY_SERVICE", "engg.weil");

    /** What this agent's own wallet needs to run convert. */
    static final String TOOL_SERVICE = "finance.weil";

    /** Units of each currency in one US dollar: sample rates, not live ones. */
    private static final Map<String, Double> PER_USD = Map.of(
        "USD", 1.0, "INR", 84.0, "EUR", 0.92, "GBP", 0.79, "JPY", 151.0, "AED", 3.67);

    public static class Tools {
        @WeilSecured(TOOL_SERVICE)
        @Schema(description = "Convert an amount from one currency to another.")
        public static Map<String, Object> convert(
            @Schema(name = "amount", description = "The amount to convert") double amount,
            @Schema(name = "from", description = "Currency code to convert from, such as INR") String from,
            @Schema(name = "to", description = "Currency code to convert to, such as USD") String to) {
            Double fromRate = PER_USD.get(from.toUpperCase(Locale.ROOT));
            Double toRate = PER_USD.get(to.toUpperCase(Locale.ROOT));
            if (fromRate == null || toRate == null) {
                return Map.of("error", "unknown currency; known: " + PER_USD.keySet());
            }
            double converted = Math.round(amount / fromRate * toRate * 100.0) / 100.0;
            return Map.of("amount", amount, "from", from.toUpperCase(Locale.ROOT),
                "to", to.toUpperCase(Locale.ROOT), "converted", converted);
        }
    }

    public static void main(String[] args) throws Exception {
        LlmAgent agent = LlmAgent.builder()
            .name("currency_agent")
            .model(WeilAgentSetup.model())
            .description("Converts amounts between currencies.")
            .instruction("""
                You convert money between currencies. Always call convert and
                report exactly what it returns. If it refuses for a permission
                reason, quote the reason. Answer only about currency.
                """)
            .tools(FunctionTool.create(Tools.class, "convert"))
            .build();

        int port = Integer.parseInt(System.getenv().getOrDefault("CURRENCY_PORT", "8012"));
        Weil.toSecureA2a(agent, port, AGENT_SERVICE);
    }

    private CurrencyAgent() {}
}
