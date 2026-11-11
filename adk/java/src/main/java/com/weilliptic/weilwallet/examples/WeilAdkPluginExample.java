package com.weilliptic.weilwallet.examples;

import com.google.adk.agents.LlmAgent;
import com.google.adk.tools.Annotations.Schema;
import com.google.adk.tools.FunctionTool;

import com.weilliptic.weilwallet.adk.Applet;
import com.weilliptic.weilwallet.adk.AuditAction;
import com.weilliptic.weilwallet.adk.Call;
import com.weilliptic.weilwallet.adk.Hook;
import com.weilliptic.weilwallet.adk.PermissionGate;
import com.weilliptic.weilwallet.adk.Weil;
import com.weilliptic.weilwallet.adk.WeilAction;
import com.weilliptic.weilwallet.adk.WeilAgentSetup;
import com.weilliptic.weilwallet.adk.WeilPlugin;
import com.weilliptic.weilwallet.adk.WeilSecured;

import java.util.List;
import java.util.Map;

/**
 * An ADK agent with a WeilChain identity: every step it takes is recorded on
 * chain under its own wallet, and a payment tool is gated by on-chain state.
 *
 * <p>Environment:</p>
 * <pre>
 *   WEIL_AGENT_KEY     Agent Registry API key   (or WEIL_WALLET_FILE)
 *   WEIL_WALLET_FILE   path to a wallet.wc      (alternative to the key)
 *   SENTINEL_HOST      Sentinel URL             (optional)
 *   ANTHROPIC_API_KEY  for the model
 *   CLAUDE_MODEL       model id                 (optional)
 * </pre>
 *
 * <p>Run with:</p>
 * <pre>
 *   mvn compile
 *   mvn exec:java -Dexec.mainClass=com.weilliptic.weilwallet.examples.WeilAdkPluginExample
 * </pre>
 *
 * <p>Served in ADK's dev UI: open http://localhost:8080 (or {@code PORT}), pick
 * {@code treasurer}, and ask for the treasury balance, then to pay 40000 WUSD to
 * wallet acme-treasury.</p>
 */
public final class WeilAdkPluginExample {

    /** The subgroup whose purposes decide who may read and who may pay. */
    static final String TREASURY_SERVICE = "engg.weil";

    /** A tool the model may call freely. */
    public static class Tools {

        @WeilSecured(TREASURY_SERVICE)
        @Schema(description = "Return the treasury's balance in WUSD.")
        public static Map<String, Object> getBalance() {
            return Map.of("balance", 100_000);
        }

        @WeilSecured(value = TREASURY_SERVICE, admin = true)
        @Schema(description = "Pay an invoice from the treasury.")
        public static Map<String, Object> payInvoice(
            @Schema(name = "payee", description = "Payee wallet address") String payee,
            @Schema(name = "amount", description = "Amount in WUSD") int amount) {
            return Map.of("paid", amount, "to", payee);
        }
    }

    public static void main(String[] args) throws Exception {
        // A blocking guard: ask the chain before any payment, and veto from its
        // answer. The tool never runs when this throws.
        WeilAction spendCap = WeilAction.named("spend-cap")
            .on(Hook.BEFORE_TOOL)
            .when(ev -> "payInvoice".equals(ev.toolName()))
            .blocking()
            .failClosed()
            .build(ev -> Call.to(Applet.systemic("wusd"), "balance_for")
                .args(Map.of("addr", ev.identity().address()))
                .build())
            .onResult((ev, result) -> {
                long balance = result.decode() == null ? 0 : result.decode().asLong();
                Object raw = ev.toolArgs().get("amount");
                long amount = raw instanceof Number ? ((Number) raw).longValue() : 0L;
                long limit = balance / 4;
                if (amount > limit) {
                    throw new WeilAction.VetoException(
                        "POLICY: " + amount + " exceeds the single-payment cap of " + limit
                            + " (25% of treasury balance " + balance + ")");
                }
                System.out.println("  [gate] " + amount + " is within the cap of " + limit);
            })
            .done();

        // Built here rather than taken from Weil: this example records digests
        // only (AuditAction.defaults(), no stream), and adds the spend cap. The
        // @WeilSecured gates on the tools come with PermissionGate.annotated:
        // reading the balance needs Execution or Management in
        // TREASURY_SERVICE, paying needs Management.
        WeilPlugin plugin = WeilPlugin.builder(Weil.identity())
            .name("weil-audit")
            .withActions(AuditAction.defaults(),
                PermissionGate.annotated(Weil.identity(), System.getenv("SENTINEL_HOST")),
                spendCap)
            .flushAfterRun(60_000L)
            .onError((ev, action, err) ->
                System.out.println("  [weil] " + action + " failed on " + ev.hook() + ": " + err))
            .build();

        System.out.println("model: " + WeilAgentSetup.modelId());

        LlmAgent agent = LlmAgent.builder()
            .name("treasurer")
            .model(WeilAgentSetup.model())
            .description("Pays invoices from the treasury.")
            .instruction("""
                You manage a treasury. Check the balance before paying anything.
                If a payment is refused, explain the reason to the user instead of
                retrying.
                """)
            .tools(
                FunctionTool.create(Tools.class, "getBalance"),
                FunctionTool.create(Tools.class, "payInvoice"))
            .build();

        Weil.web(agent, List.of(plugin), args);
    }
}
