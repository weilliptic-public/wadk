package com.weilliptic.weilwallet.examples.web.spring_chat_advanced;

import com.google.adk.tools.Annotations.Schema;

import com.weilliptic.weilwallet.adk.WeilSecured;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.stereotype.Service;

import java.util.Map;

/**
 * A Spring service whose methods {@link AgentConfig} exposes to the agent as
 * tools. A real app would back it with a database or another service.
 */
@Service
public class OrderService {

    // Spring's log line shows the thread, so tools the model asks for together
    // can be seen running on threads of their own.
    private static final Logger logger = LoggerFactory.getLogger(OrderService.class);

    /** The subgroup a wallet must be permitted in to look orders up. */
    static final String SUPPORT_SERVICE = "engg.weil";

    private record Order(String status, String item, String deliveryEstimate) {}

    private static final Map<String, Order> ORDERS = Map.of(
        "1001", new Order("shipped", "mechanical keyboard", "2 days"),
        "1002", new Order("processing", "USB-C hub", "5 days"),
        "1003", new Order("delivered", "monitor stand", "already delivered"));

    /** Returns the status and item of the order, or an {@code error} entry if there is no such order. */
    @WeilSecured(SUPPORT_SERVICE)
    @Schema(description = "Looks up the status and item of an order.")
    public Map<String, Object> getOrderStatus(
        @Schema(name = "orderId", description = "The order number, for example 1001.") String orderId) {
        logger.info("getOrderStatus {}", orderId);
        Order order = ORDERS.get(orderId);
        if (order == null) {
            return Map.of("error", "No order with this number.");
        }
        return Map.of("orderId", orderId, "status", order.status(), "item", order.item());
    }

    /** Returns the delivery estimate of the order, or an {@code error} entry if there is no such order. */
    @WeilSecured(SUPPORT_SERVICE)
    @Schema(description = "Returns the delivery estimate of an order.")
    public Map<String, Object> getDeliveryEstimate(
        @Schema(name = "orderId", description = "The order number, for example 1001.") String orderId) {
        logger.info("getDeliveryEstimate {}", orderId);
        Order order = ORDERS.get(orderId);
        if (order == null) {
            return Map.of("error", "No order with this number.");
        }
        return Map.of("orderId", orderId, "deliveryEstimate", order.deliveryEstimate());
    }
}
