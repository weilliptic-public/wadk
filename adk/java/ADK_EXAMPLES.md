# Running the examples

Google's ADK Java examples, the city-time-weather tutorial and the adk-recipes
financial advisor, ported with a WeilChain wallet, plus this repo's own
examples. Each one audits every step and gates its secured tools.

## Before you start

- Run every command from `java/`.
- Each command reads an env file, `.env.<name>`. Make one from `.env.example`
  and fill in `WEIL_AGENT_KEY` (or `WEIL_WALLET_FILE`), `SENTINEL_HOST` and the
  model: `ANTHROPIC_API_KEY`, or `MODEL_PROVIDER=openai` with `OPENAI_API_KEY`
  and `OPENAI_MODEL`. The `.env.*` files are git-ignored.
- Secured tools need the wallet to hold Execution or Management in their
  subgroup (`engg.weil` unless noted); otherwise the model is told "Permission
  denied".
- A development Sentinel uses a certificate Java does not trust. Either build
  the truststore once (see "Local Sentinel certificate" in
  `src/main/java/com/weilliptic/weilwallet/adk/README.md`) and run
  `export JAVA_TOOL_OPTIONS="-Djavax.net.ssl.trustStore=$HOME/.weilliptic/weil-truststore.p12 -Djavax.net.ssl.trustStorePassword=changeit"`,
  or, against a Sentinel you run yourself, put `WEIL_TLS_VERIFY=0` in the env
  file.
- The agents open in ADK's dev UI at http://localhost:8080: pick the agent and
  chat. `PORT` in the env file changes the port. Ctrl+C stops it and sends any
  records still queued. The wallet loads in the background, so the page opens
  at once; a wallet that cannot load shows up as a `[weil]` line.

## Agents in `examples/`

### helloworld: dice and primes, with memory

```bash
(set -a; source .env.hello; set +a; mvn -q compile exec:java -Dexec.mainClass=com.weilliptic.weilwallet.examples.helloworld.HelloWorldAgent)
```

Pick `hello_world_agent`.

### city_time_weather: ADK's quickstart tutorial

```bash
(set -a; source .env.city; set +a; mvn -q compile exec:java -Dexec.mainClass=com.weilliptic.weilwallet.examples.city_time_weather.CityTimeWeather)
```

Pick `multi_tool_agent`.

### contextcaching: a weather assistant with a cached instruction

Prints how much of each prompt the cache served.

```bash
(set -a; source .env.hello; set +a; mvn -q compile exec:java -Dexec.mainClass=com.weilliptic.weilwallet.examples.contextcaching.ContextCachingAgent)
```

Pick `weather_assistant`.

### mcpfilesystem: a file assistant over MCP

Needs Node.js. `MCP_FS_ROOT` is the only folder it may touch.

```bash
(set -a; source .env.mcp; set +a; mvn -q compile exec:java -Dexec.mainClass=com.weilliptic.weilwallet.examples.mcpfilesystem.McpFilesystemAgent)
```

Pick `filesystem_agent`.

### financialadvisor: a coordinator with four specialist agents

From adk-recipes. Searches through `websearch::weil` (or `WEIL_SEARCH_APPLET`).
The wallet needs `engg.weil` for the search and each specialist's subgroup.

```bash
(set -a; source .env.financial; set +a; mvn -q compile exec:java -Dexec.mainClass=com.weilliptic.weilwallet.examples.financialadvisor.FinancialAdvisorAgent)
```

Pick `financial_coordinator`.

### WeilAdkPluginExample: a treasurer with a spending cap

A plugin built by hand: digest-only audits and a cap on payments, read from the
`wusd` applet. Paying needs Management in `engg.weil`.

```bash
(set -a; source .env.hello; set +a; mvn -q compile exec:java -Dexec.mainClass=com.weilliptic.weilwallet.examples.WeilAdkPluginExample)
```

Pick `treasurer`.

### a2a_server and a2a_basic: dice locally, primes over A2A

Start the prime agent first. It serves on `PORT` (default 8003) and admits only
signed callers:

```bash
(set -a; source .env.prime; set +a; mvn -q compile exec:java -Dexec.mainClass=com.weilliptic.weilwallet.examples.a2a_server.agent.Agent)
```

Then, in another terminal, the dice agent, which calls it at `PRIME_URL`
(default `http://127.0.0.1:8003`):

```bash
(set -a; source .env.dice; set +a; mvn -q compile exec:java -Dexec.mainClass=com.weilliptic.weilwallet.examples.a2a_basic.A2AAgent)
```

Pick `root_agent`.

### Travel: a planner calling a flights agent and a hotels agent over A2A

Start the two specialists, each in its own terminal (ports 8001 and 8002):

```bash
(set -a; source .env.flights; set +a; mvn -q compile exec:java -Dexec.mainClass=com.weilliptic.weilwallet.examples.WeilTravelFlights)
```

```bash
(set -a; source .env.hotels; set +a; mvn -q compile exec:java -Dexec.mainClass=com.weilliptic.weilwallet.examples.WeilTravelHotels)
```

Then the planner, which calls both as tools at `FLIGHTS_URL` and `HOTELS_URL`:

```bash
(set -a; source .env.web; set +a; mvn -q compile exec:java -Dexec.mainClass=com.weilliptic.weilwallet.examples.WeilTravelPlanner)
```

Pick `travel_planner`. Give each agent its own `WEIL_AGENT_KEY`: with one key
they all sign as the same wallet, so the specialists' permission check always
passes.

## Apps in `examples-web/`

A separate module that uses the SDK from the local Maven repository, so install
the SDK first, and again after changing it:

```bash
mvn -q install -DskipTests
```

### WeilTravelPlannerWeb: the travel planner, from its own module

The same planner as `WeilTravelPlanner`; start the flights and hotels agents
first.

```bash
(set -a; source .env.web; set +a; cd examples-web && mvn -q compile exec:java -Dexec.mainClass=com.weilliptic.weilwallet.examples.web.WeilTravelPlannerWeb)
```

### spring_chat: a chat page backed by an agent with memory

ADK's `examples/spring_chat`. Each page load is a new session; graph memory
carries what the user said across reloads.

```bash
(set -a; source .env.hello; set +a; cd examples-web && mvn -q compile exec:java -Dexec.mainClass=com.weilliptic.weilwallet.examples.web.spring_chat.SpringChatApplication)
```

### spring_chat_advanced: streaming order support with secured tools

ADK's `examples/spring_chat_advanced`. Try "When do orders 1001 and 1002
arrive?" (both lookups run in one turn) or "Can I talk to a human?". The order
tools need `engg.weil`; the hand-off to a human does not.

```bash
(set -a; source .env.hello; set +a; cd examples-web && mvn -q compile exec:java -Dexec.mainClass=com.weilliptic.weilwallet.examples.web.spring_chat_advanced.SpringChatAdvancedApplication)
```

Both apps listen on `localhost:8080` only, since their chat endpoint has no
login.

## More

- Every way to run, plug in and secure an agent: `AGENT_GUIDE.md`.
- The SDK: `src/main/java/com/weilliptic/weilwallet/adk/README.md`; hooks and
  records: `HOOKS.md`.
- The financial advisor's changes from the recipe:
  `src/main/java/com/weilliptic/weilwallet/examples/financialadvisor/README.md`.
