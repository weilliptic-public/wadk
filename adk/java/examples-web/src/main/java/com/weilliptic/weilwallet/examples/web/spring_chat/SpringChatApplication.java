package com.weilliptic.weilwallet.examples.web.spring_chat;

import com.weilliptic.weilwallet.adk.Weil;

import org.springframework.boot.SpringApplication;
import org.springframework.boot.autoconfigure.SpringBootApplication;

import java.util.Map;

/**
 * ADK's {@code examples/spring_chat}: a Spring Boot app with a browser chat
 * window backed by an ADK agent, with a Weil identity.
 *
 * <p>The agent is ADK's, with three changes. Its model is
 * {@link com.weilliptic.weilwallet.adk.WeilAgentSetup#model()}, so it
 * follows {@code MODEL_PROVIDER} in {@code .env} like every other example,
 * where ADK's goes through a Spring AI starter. Its runner is
 * {@link Weil#runner}, so every turn is audited and traced under the agent's
 * wallet. And it has graph memory, so it remembers the user across page
 * reloads, where ADK's forgets everything with the session.</p>
 *
 * <pre>
 *   ./run.sh spring-chat
 * </pre>
 *
 * <p>Then open <a href="http://localhost:8080">http://localhost:8080</a>.
 * Override the port with {@code --server.port=9000}.</p>
 */
@SpringBootApplication
public class SpringChatApplication {

    public static void main(String[] args) {
        System.out.println("agent wallet: " + Weil.identity().address());

        SpringApplication app = new SpringApplication(SpringChatApplication.class);
        app.setDefaultProperties(Map.of(
            // This module holds several web apps, so each serves its own page.
            "spring.web.resources.static-locations", "classpath:/spring_chat/",
            // The chat endpoint has no authentication, so listen on loopback
            // only unless told otherwise.
            "server.address", "${SERVER_ADDRESS:localhost}"));
        app.run(args);
    }
}
