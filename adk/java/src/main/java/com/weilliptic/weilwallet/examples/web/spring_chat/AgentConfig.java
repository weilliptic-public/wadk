package com.weilliptic.weilwallet.examples.web.spring_chat;

import com.google.adk.agents.BaseAgent;
import com.google.adk.agents.LlmAgent;
import com.google.adk.runner.Runner;

import com.weilliptic.weilwallet.adk.Weil;
import com.weilliptic.weilwallet.adk.WeilAgentSetup;

import org.springframework.context.annotation.Bean;
import org.springframework.context.annotation.Configuration;

/** Spring configuration for the ADK agent and its runner. */
@Configuration(proxyBeanMethods = false)
class AgentConfig {

    @Bean
    BaseAgent chatAgent() {
        return LlmAgent.builder()
            .name("spring_chat_agent")
            .description("A friendly general-purpose chat assistant.")
            .model(WeilAgentSetup.model())
            .instruction("You are a friendly, concise assistant. Answer the user's questions clearly.")
            .tools(Weil.memory())
            .build();
    }

    /** ADK's {@code InMemoryRunner}, with the Weil plugins on every run. */
    @Bean
    Runner runner(BaseAgent chatAgent) {
        return Weil.runner(chatAgent);
    }
}
