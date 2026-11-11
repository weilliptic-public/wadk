# Financial Advisor

The adk-recipes Java sample
[`contrib/java/financial-advisor`](https://github.com/google/adk-recipes/tree/main/contrib/java/financial-advisor),
with a Weil identity. Same files, agent graph and prompts (Apache-2.0, Google's
license header kept):

- `FinancialAdvisorAgent`: the coordinator, `financial_coordinator`. It calls
  the four specialists as `AgentTool`s.
- `DataAnalystAgent` (`market_data_analysis_output`): searches the web for a ticker.
- `TradingAnalystAgent` (`proposed_trading_strategies_output`)
- `ExecutionAnalystAgent` (`execution_plan_output`)
- `RiskAnalystAgent` (`final_risk_assessment_output`)
- `LegalDisclaimer`, `Models`

What changes for Weil:

- **Model**: `Models` uses `WeilAgentSetup.model()`, so `MODEL_PROVIDER`
  (anthropic or openai) picks the model instead of Gemini's `MODEL_NAME`.
- **Search**: Gemini's `GoogleSearchTool` runs on Google's side, where no
  permission check sees it. The data analyst uses `web_search`
  (`examples/WebSearchTools.java`) instead. Any model can call it, and it runs
  through the systemic search applet (`websearch::weil`, or
  `WEIL_SEARCH_APPLET`). Its method carries `@WeilSecured("engg.weil")`, so only
  a wallet with Execution or Management in `engg.weil` can search.
- **Audit**: `main` runs the coordinator with `Weil.chat`. Each specialist runs
  as an `AgentTool` made by `Weil.agentTool`, with the coordinator's plugins
  (`includePlugins`, which ADK
  leaves off by default). So every model call, agent-tool call and search, the
  specialists' included, is audited under the agent's wallet, and `web_search`'s
  `@WeilSecured` gate applies inside the data analyst.
- **Specialist gates**: an agent has no Java method to carry `@WeilSecured`, so
  each specialist is made with `Weil.agentTool(agent, service)`, and the tool
  carries its service instead. `SPECIALIST_SERVICES` in `FinancialAdvisorAgent`
  maps each one to its service (all `engg.weil` here).
  A wallet without Execution or Management there is refused before the
  specialist runs, and the coordinator gets "Permission denied" as the result.
  Each verdict is published to `security`, as for any gated tool.

## Run

Copy `.env.example` to `.env.financial` and fill in `WEIL_AGENT_KEY`,
`SENTINEL_HOST` and the model's key. Then, from `java/`:

Against a dev Sentinel, first point Java at the truststore that trusts its
certificate, once per terminal (see "Local Sentinel certificate" in
`adk/README.md`):

```bash
export JAVA_TOOL_OPTIONS="-Djavax.net.ssl.trustStore=$HOME/.weilliptic/weil-truststore.p12 -Djavax.net.ssl.trustStorePassword=changeit"
```

Then:

```bash
(set -a; source .env.financial; set +a; mvn -q exec:java -Dexec.mainClass=com.weilliptic.weilwallet.examples.financialadvisor.FinancialAdvisorAgent)
```

With no arguments it runs a default conversation: analyse NVDA, develop
strategies for a moderate, long-term investor, plan the execution, then assess
the risk. To ask your own question, pass it with `-Dexec.args="..."`. To
chat with it, typing each message at the `>` prompt in one session (`exit`,
`quit` or Ctrl+D ends it):

```bash
(set -a; source .env.financial; set +a; mvn -q exec:java -Dexec.mainClass=com.weilliptic.weilwallet.examples.financialadvisor.FinancialAdvisorAgent -Dexec.args="-i")
```
