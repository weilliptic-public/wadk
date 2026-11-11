package com.weilliptic.weilwallet.examples.web.spring_chat_advanced;

import com.google.adk.agents.BaseAgent;
import com.google.adk.agents.LlmAgent;
import com.google.adk.runner.Runner;
import com.google.adk.tools.FunctionTool;

import com.weilliptic.weilwallet.adk.Weil;
import com.weilliptic.weilwallet.adk.WeilAgentSetup;

import org.springframework.context.annotation.Bean;
import org.springframework.context.annotation.Configuration;

/** Spring configuration for the ADK agent, its tools and the runner. */
@Configuration(proxyBeanMethods = false)
class AgentConfig {

    /**
     * Returns the order support agent. Its tools are methods of Spring beans,
     * so they can use anything Spring injects. {@code FunctionTool} reads the
     * {@code @Schema} and {@code @WeilSecured} annotations from the bean's
     * class, so if an aspect such as {@code @Transactional} makes Spring proxy
     * a bean, pass the {@code Method} from its class instead of a name.
     */
    @Bean
    BaseAgent orderAgent(OrderService orderService, ShippingTools shippingTools) {
        return LlmAgent.builder()
            .name("order_support_agent")
            .description("Answers questions about the user's orders.")
            .model(WeilAgentSetup.model())
            .instruction("You are a concise customer support assistant. Use the tools to look up"
                + " orders; never guess order details. The order the user last tracked, if any: {"
                + ShippingTools.LAST_TRACKED_ORDER
                + "?}")
            .tools(Weil.withMemory(
                FunctionTool.create(orderService, "getOrderStatus"),
                FunctionTool.create(orderService, "getDeliveryEstimate"),
                FunctionTool.create(shippingTools, "getTracking"),
                FunctionTool.create(shippingTools, "talkToHuman")))
            .build();
    }

    /** ADK's {@code InMemoryRunner}, with the Weil plugins on every run. */
    @Bean
    Runner runner(BaseAgent orderAgent) {
        return Weil.runner(orderAgent);
    }
}
