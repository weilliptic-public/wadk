package com.weilliptic.weilwallet.adk;

import com.google.adk.agents.BaseAgent;
import com.google.adk.apps.App;
import com.google.adk.artifacts.BaseArtifactService;
import com.google.adk.memory.BaseMemoryService;
import com.google.adk.plugins.BasePlugin;
import com.google.adk.runner.Runner;
import com.google.adk.sessions.BaseSessionService;
import com.google.adk.web.AdkWebServer;
import com.google.adk.web.AgentLoader;
import com.google.adk.web.AgentStaticLoader;
import com.google.adk.web.service.RunnerService;

import org.springframework.beans.factory.support.AbstractBeanDefinition;
import org.springframework.beans.factory.support.DefaultListableBeanFactory;
import org.springframework.boot.SpringApplication;
import org.springframework.context.ConfigurableApplicationContext;

import java.util.List;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

/**
 * ADK's dev UI for {@link Weil#web}: the same Spring app {@code AdkWebServer}
 * starts, given the agents to serve and the Weil plugins for every run.
 *
 * <p>Kept apart from {@link Weil} because it needs {@code google-adk-dev},
 * which the SDK only declares as optional: an agent that never calls
 * {@code Weil.web} never loads this class, so it does not need the dev UI's
 * Spring Boot on its classpath.</p>
 */
final class WeilWeb {

    /** A class from {@code google-adk-dev}, to tell whether it is on the classpath. */
    private static final String DEV_UI_CLASS = "com.google.adk.web.AdkWebServer";

    private WeilWeb() {}

    /** Fail with what to add, rather than a NoClassDefFoundError, without the dev UI. */
    static void requireDevUi() {
        try {
            Class.forName(DEV_UI_CLASS, false, Weil.class.getClassLoader());
        } catch (ClassNotFoundException e) {
            throw new IllegalStateException(
                "Weil.web needs ADK's dev UI: add the dependency"
                    + " com.google.adk:google-adk-dev, the same version as google-adk");
        }
    }

    /** Serve {@code agents}, each run with {@code plugins}. */
    static ConfigurableApplicationContext serve(
        String[] args, List<BasePlugin> plugins, BaseAgent... agents) {
        return start(args, plugins, null, agents);
    }

    /**
     * Serve {@code app}'s root agent as the App itself: its context cache,
     * compaction and resumability settings, and its plugins after
     * {@code plugins}. ADK's dev UI serves agents, not Apps, so its runner
     * service is swapped for {@link AppRunnerService}.
     */
    static ConfigurableApplicationContext serve(
        String[] args, List<BasePlugin> plugins, App app) {
        return start(args, plugins, app, app.rootAgent());
    }

    private static ConfigurableApplicationContext start(
        String[] args, List<BasePlugin> plugins, App app, BaseAgent... agents) {
        // ADK serves these agents instead of scanning a source folder for them.
        System.setProperty("adk.agents.loader", "static");

        SpringApplication spring = new SpringApplication(AdkWebServer.class);
        // localhost unless SERVER_ADDRESS says otherwise, since the UI has no
        // login; the port from PORT, as containers set it.
        spring.setDefaultProperties(Map.of(
            "server.address", "${SERVER_ADDRESS:localhost}",
            "server.port", "${PORT:8080}"));
        spring.addInitializers(context -> {
            DefaultListableBeanFactory beans =
                (DefaultListableBeanFactory) context.getBeanFactory();
            beans.registerSingleton("agentLoader", new AgentStaticLoader(agents));
            // ADK's runner service reads this bean; without it the UI would
            // run the agents with no audit trail.
            beans.registerSingleton("extraPlugins", plugins);
            if (app != null) {
                beans.registerSingleton("weilApp", app);
                // Runs after ADK's components are scanned: build its runner
                // service as AppRunnerService instead.
                context.addBeanFactoryPostProcessor(factory -> ((AbstractBeanDefinition)
                    factory.getBeanDefinition("runnerService")).setBeanClass(AppRunnerService.class));
            }
        });
        return spring.run(args);
    }

    /** ADK's runner service, but each runner runs the served App with its settings kept. */
    public static final class AppRunnerService extends RunnerService {

        private final App app;
        private final BaseArtifactService artifactService;
        private final BaseSessionService sessionService;
        private final BaseMemoryService memoryService;
        private final List<BasePlugin> extraPlugins;
        private final Map<String, Runner> runners = new ConcurrentHashMap<>();

        public AppRunnerService(
            AgentLoader agentLoader,
            BaseArtifactService artifactService,
            BaseSessionService sessionService,
            BaseMemoryService memoryService,
            List<BasePlugin> extraPlugins,
            App weilApp) {
            super(agentLoader, artifactService, sessionService, memoryService, extraPlugins);
            this.app = weilApp;
            this.artifactService = artifactService;
            this.sessionService = sessionService;
            this.memoryService = memoryService;
            this.extraPlugins = extraPlugins;
        }

        /** The App, named as the UI names it (after its root agent, so the UI's sessions match). */
        @Override
        public Runner getRunner(String appName) {
            return runners.computeIfAbsent(appName, name -> Runner.builder()
                .app(Weil.withPlugins(app, name, extraPlugins))
                .artifactService(artifactService)
                .sessionService(sessionService)
                .memoryService(memoryService)
                .build());
        }
    }
}
