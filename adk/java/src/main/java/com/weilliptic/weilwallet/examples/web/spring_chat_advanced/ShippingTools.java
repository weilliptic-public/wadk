package com.weilliptic.weilwallet.examples.web.spring_chat_advanced;

import com.google.adk.tools.Annotations.Schema;
import com.google.adk.tools.ToolContext;

import com.weilliptic.weilwallet.adk.WeilSecured;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.stereotype.Component;

import java.util.Map;

/** Shipping tools that {@link AgentConfig} gives the agent alongside {@link OrderService}'s. */
@Component
public class ShippingTools {

    // Spring's log line shows the thread, so tools the model asks for together
    // can be seen running on threads of their own.
    private static final Logger logger = LoggerFactory.getLogger(ShippingTools.class);

    /** Session state key that {@code getTracking} sets and the agent's instruction reads. */
    static final String LAST_TRACKED_ORDER = "lastTrackedOrder";

    private static final Map<String, String> TRACKING_NUMBERS =
        Map.of("1001", "DHL JD014600001001", "1003", "UPS 1Z999AA10000001003");

    /** Returns the tracking number of a shipped order and records the order in the ADK session. */
    @WeilSecured(OrderService.SUPPORT_SERVICE)
    @Schema(description = "Returns the carrier and tracking number of a shipped order.")
    public Map<String, Object> getTracking(
        @Schema(name = "orderId", description = "The order number, for example 1001.") String orderId,
        // ADK fills this in, and recognises it only by the name "toolContext";
        // this build does not keep parameter names, so it is given here.
        @Schema(name = "toolContext") ToolContext toolContext) {
        logger.info("getTracking {}", orderId);
        String trackingNumber = TRACKING_NUMBERS.get(orderId);
        if (trackingNumber == null) {
            return Map.of("error", "This order has no tracking number yet.");
        }
        toolContext.state().put(LAST_TRACKED_ORDER, orderId);
        return Map.of("orderId", orderId, "tracking", trackingNumber);
    }

    /**
     * Skips summarization, ADK's counterpart of Spring AI's {@code returnDirect}:
     * this result goes to the user without another model call.
     */
    @Schema(description = "Hands the conversation to a human support agent when the user asks for one.")
    public String talkToHuman(@Schema(name = "toolContext") ToolContext toolContext) {
        toolContext.actions().setSkipSummarization(true);
        return "A human support agent will join this chat within 5 minutes.";
    }
}
