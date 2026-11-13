package com.weilliptic.weilwallet.examples;

import com.google.adk.tools.Annotations.Schema;
import com.google.adk.tools.FunctionTool;
import com.google.adk.utils.ComponentRegistry;

import com.weilliptic.weilwallet.ContractId;
import com.weilliptic.weilwallet.adk.Applet;
import com.weilliptic.weilwallet.adk.Call;
import com.weilliptic.weilwallet.adk.Weil;
import com.weilliptic.weilwallet.adk.WeilIdentity;
import com.weilliptic.weilwallet.adk.WeilResult;
import com.weilliptic.weilwallet.adk.WeilSecured;

import com.fasterxml.jackson.databind.JsonNode;

import java.util.Map;

/**
 * Web search through a deployed search applet, as a function tool any model
 * can call and Weil can secure: the applet's {@code search(search_query)}
 * (the Tavily and Brave search applets share it) runs the query and returns
 * the results as text.
 *
 * <p>The applet is the systemic {@code websearch::weil}, the one every
 * agent searches through whatever its organisation, as in the Go SDK.
 * WEIL_SEARCH_APPLET names another, by service name or contract address. The
 * call is signed by this agent's wallet ({@link Weil#identity()}); the
 * permission check, on {@link #SEARCH_SERVICE}, is of the caller's.</p>
 *
 * <pre>{@code
 * .tools(FunctionTool.create(WebSearchTools.class, "webSearch"))
 * }</pre>
 *
 * <p>YAML agents list it as {@code - name: weil.web_search}, after
 * {@link #register()}.</p>
 */
public final class WebSearchTools {

    /** The subgroup a wallet must be permitted in to search. */
    static final String SEARCH_SERVICE = "engg.weil";

    /** The search applet when WEIL_SEARCH_APPLET is unset: {@code websearch::weil}. */
    static final Applet SEARCH_APPLET = Applet.systemic("websearch");

    /** The applet's search method, as its WIDL declares it. */
    static final String SEARCH_METHOD = "search";

    private WebSearchTools() {}

    @WeilSecured(SEARCH_SERVICE)
    @Schema(name = "web_search",
        description = "Search the web and return the most relevant results for a query.")
    public static Map<String, Object> webSearch(
        @Schema(name = "query", description = "What to search the web for") String query) {
        if (query == null || query.isBlank()) {
            return Map.of("status", "error", "error", "query is empty");
        }
        try {
            WeilIdentity identity = Weil.identity();
            WeilResult result = identity.invoke(searchCall(searchApplet(), query.trim()));
            JsonNode ok = result.decode();
            return Map.of("status", "success",
                "results", ok == null ? "" : ok.isTextual() ? ok.asText() : ok.toString());
        } catch (WeilResult.AppletException e) {
            return Map.of("status", "error", "error", e.getMessage());
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            return Map.of("status", "error", "error", "interrupted");
        } catch (Exception e) {
            return Map.of("status", "error", "error", "web search failed: " + e.getMessage());
        }
    }
    /** The applet call for {@code query}: read-only, so its arguments are left visible. */
    static Call searchCall(Applet applet, String query) {
        return Call.to(applet, SEARCH_METHOD)
            .args(Map.of("search_query", query))
            .hideArgs(false)
            .fireAndForget(false)
            .build();
    }

    /**
     * The applet WEIL_SEARCH_APPLET names, by service name or contract
     * address, or else {@link #SEARCH_APPLET}.
     */
    static Applet searchApplet() {
        String applet = System.getenv("WEIL_SEARCH_APPLET");
        if (applet != null && !applet.isBlank()) {
            return applet(applet.trim());
        }
        return SEARCH_APPLET;
    }

    static Applet applet(String nameOrAddress) {
        return nameOrAddress.contains("::")
            ? Applet.byName(nameOrAddress)
            : Applet.byId(new ContractId(nameOrAddress));
    }

    /** Register the tool for YAML agents as {@code weil.web_search}. */
    public static void register() {
        ComponentRegistry.getInstance().register("weil.web_search",
            FunctionTool.create(WebSearchTools.class, "webSearch"));
    }
}
