package com.weilliptic.weilwallet.examples.web.spring_chat_advanced;


import org.springframework.boot.SpringApplication;
import org.springframework.boot.autoconfigure.SpringBootApplication;

import java.util.Map;

/**
 * ADK's {@code examples/spring_chat_advanced}: an order-support chat whose
 * agent uses tools from Spring beans and streams its replies to the browser,
 * with a Weil identity.
 *
 * <p>On top of ADK's sample, the order tools are {@link
 * com.weilliptic.weilwallet.adk.WeilSecured @WeilSecured}, so the agent's
 * wallet must be permitted in {@value OrderService#SUPPORT_SERVICE} to look
 * an order up, while handing over to a human stays open to everyone. Every
 * turn is audited and traced under the wallet, and asking about two orders at
 * once shows up in the trace as two sibling tool calls.</p>
 *
 * <p>ADK's {@code ShippingTools} are Spring AI {@code @Tool} methods given to
 * the agent through {@code SpringAiToolset}, which no released ADK has yet.
 * Here they are ADK tools like {@link OrderService}'s, and Spring AI's
 * {@code returnDirect} is ADK's own skip-summarization.</p>
 *
 * <pre>
 *   mvn -q compile exec:java \
 *       -Dexec.mainClass=com.weilliptic.weilwallet.examples.web.spring_chat_advanced.SpringChatAdvancedApplication
 * </pre>
 *
 * <p>Then open <a href="http://localhost:8080">http://localhost:8080</a> and
 * ask, for example, "When do orders 1001 and 1002 arrive?". Override the port
 * with {@code --server.port=9000}.</p>
 */
@SpringBootApplication
public class SpringChatAdvancedApplication {

    public static void main(String[] args) {
        SpringApplication app = new SpringApplication(SpringChatAdvancedApplication.class);
        app.setDefaultProperties(Map.of(
            // This module holds several web apps, so each serves its own page.
            "spring.web.resources.static-locations", "classpath:/com/weilliptic/weilwallet/examples/web/spring_chat_advanced/",
            // The chat endpoint has no authentication, so listen on loopback
            // only unless told otherwise.
            "server.address", "${SERVER_ADDRESS:localhost}"));
        app.run(args);
    }
}
