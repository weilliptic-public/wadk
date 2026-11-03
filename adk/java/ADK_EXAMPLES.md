# Running the ADK Java ports

Every agent ported from Google's ADK Java (`adk-java` examples, its YAML
`configagent` samples, the city-time-weather tutorial and the adk-recipes
financial advisor), with the command that runs it. Each one has a WeilChain
wallet, audits every step, and gates its secured tools.

## Before you start

- Run every command from `java/`.
- Each command reads an env file (`.env.<name>`). Copy `.env.example` to make
  one, and fill in `WEIL_AGENT_KEY` (or `WEIL_WALLET_FILE`), `SENTINEL_HOST`,
  and the model: `MODEL_PROVIDER=anthropic` with `ANTHROPIC_API_KEY`, or
  `MODEL_PROVIDER=openai` with `OPENAI_API_KEY` and `OPENAI_MODEL`. The
  `.env.*` files are git-ignored.
- The Sentinel node in `SENTINEL_HOST` must be running and reachable;
  otherwise a run stops at "cannot load the agent's wallet".
- Secured tools need the wallet to hold Execution or Management in their
  service (`engg.weil` unless noted). Without it the model gets
  "Permission denied" instead of a result.
- A development Sentinel's certificate is signed by the cluster's own CA,
  which Java does not trust, and the SDK always checks certificates. Build
  the truststore once per machine (see "Local Sentinel certificate" in
  `src/main/java/com/weilliptic/weilwallet/adk/README.md`), then name it once
  per terminal before running anything below:

  ```bash
  export JAVA_TOOL_OPTIONS="-Djavax.net.ssl.trustStore=$HOME/.weilliptic/weil-truststore.p12 -Djavax.net.ssl.trustStorePassword=changeit"
  ```

  For a quick local run without the truststore, put `WEIL_TLS_VERIFY=0` in
  the env file (or in front of `mvn`) instead: the SDK then accepts any
  certificate on its own connections to Sentinel and other agents. Permission
  checks can be faked that way, so keep it to a Sentinel you run yourself
  (see "Turning verification off instead" in the ADK README).
- Run a command with no question and it plays its default conversation. To
  ask your own question, add `-Dexec.args="..."` to a coded agent, or put the
  question after the YAML path for a YAML agent.

## Coded agents (`examples/`)

### helloworld: dice and primes, with memory

```bash
(set -a; source .env.hello; set +a; mvn -q exec:java -Dexec.mainClass=com.weilliptic.weilwallet.examples.helloworld.HelloWorldAgent)
```

### city_time_weather: the ADK quickstart tutorial

```bash
(set -a; source .env.city; set +a; mvn -q exec:java -Dexec.mainClass=com.weilliptic.weilwallet.examples.city_time_weather.CityTimeWeather)
```

### contextcaching: a weather assistant with a cached instruction

Any env file with a wallet and a model works; it prints how much of each
prompt came from the cache.

```bash
(set -a; source .env.hello; set +a; mvn -q exec:java -Dexec.mainClass=com.weilliptic.weilwallet.examples.contextcaching.ContextCachingAgent)
```

### mcpfilesystem: a file assistant over MCP

Needs Node.js. `MCP_FS_ROOT` in `.env.mcp` is the only folder it may touch.

```bash
(set -a; source .env.mcp; set +a; mvn -q exec:java -Dexec.mainClass=com.weilliptic.weilwallet.examples.mcpfilesystem.McpFilesystemAgent)
```

### a2a_server + a2a_basic: dice locally, primes over A2A

Two terminals, each with its own wallet. Start the prime agent first; it
serves on `PORT` (default 8003) and admits only signed callers.

```bash
(set -a; source .env.prime; set +a; mvn -q exec:java -Dexec.mainClass=com.weilliptic.weilwallet.examples.a2a_server.agent.Agent)
```

Then the dice agent, which calls it at `PRIME_URL` (default
`http://127.0.0.1:8003`):

```bash
(set -a; source .env.dice; set +a; mvn -q exec:java -Dexec.mainClass=com.weilliptic.weilwallet.examples.a2a_basic.A2AAgent)
```

### financialadvisor: a coordinator with four specialist agents

From adk-recipes. Needs a wallet in an organisation with a search applet
(`websearch::<org>`, or `WEIL_SEARCH_APPLET`), and Execution or Management in
`engg.weil` for the specialists and web search. `.env.web` has such a wallet;
or copy it to `.env.financial`.

```bash
(set -a; source .env.web; set +a; mvn -q exec:java -Dexec.mainClass=com.weilliptic.weilwallet.examples.financialadvisor.FinancialAdvisorAgent)
```

## Web agents (`examples-web/`)

Spring Boot 4 apps, in a module of their own (see its `pom.xml` for why). It
uses the SDK from the local Maven repository, so install the SDK first, and
again after changing it:

```bash
mvn -q install -DskipTests
```

Any env file with a wallet and a model works. Each app listens on
`localhost:8080` only, since its chat endpoint has no login; pass
`-Dexec.args=--server.port=9000` for another port, and set
`SERVER_ADDRESS=0.0.0.0` only on a trusted network.

### spring_chat: a chat page backed by an agent with memory

ADK's `examples/spring_chat`. Each page load is a new session, and graph
memory carries what the user said across reloads.

```bash
(set -a; source .env.hello; set +a; cd examples-web && mvn -q compile exec:java -Dexec.mainClass=com.weilliptic.weilwallet.examples.web.spring_chat.SpringChatApplication)
```

### spring_chat_advanced: streaming order support with secured tools

ADK's `examples/spring_chat_advanced`. Ask "When do orders 1001 and 1002
arrive?" to see both lookups run in one turn, "Track order 1001", or "Can I
talk to a human?". The order tools need the wallet in `engg.weil`; the hand-off
to a human does not.

```bash
(set -a; source .env.hello; set +a; cd examples-web && mvn -q compile exec:java -Dexec.mainClass=com.weilliptic.weilwallet.examples.web.spring_chat_advanced.SpringChatAdvancedApplication)
```

## YAML agents (`examples/configagent/`)

All run through `WeilYamlAgent`. The first argument is the root agent's file;
anything after it is one question. With no question it reads questions from
the terminal, one per line, until an empty line.

### core_basic_config: plain question answering

```bash
(set -a; source .env.hello; set +a; mvn -q exec:java -Dexec.mainClass=com.weilliptic.weilwallet.examples.WeilYamlAgent -Dexec.args="src/main/java/com/weilliptic/weilwallet/examples/configagent/core_basic_config/root_agent.yaml What can you do?")
```

### core_callback_config: dice and primes, printing every callback

```bash
(set -a; source .env.hello; set +a; mvn -q exec:java -Dexec.mainClass=com.weilliptic.weilwallet.examples.WeilYamlAgent -Dexec.args="src/main/java/com/weilliptic/weilwallet/examples/configagent/core_callback_config/root_agent.yaml Roll a 6-sided die and check if it is prime")
```

### core_generate_content_config_config: web search with generation limits

```bash
(set -a; source .env.web; set +a; mvn -q exec:java -Dexec.mainClass=com.weilliptic.weilwallet.examples.WeilYamlAgent -Dexec.args="src/main/java/com/weilliptic/weilwallet/examples/configagent/core_generate_content_config_config/root_agent.yaml What is the latest news on WeilChain?")
```

### multi_agent_basic_config: a learning assistant with code and math tutors

```bash
(set -a; source .env.learning; set +a; mvn -q exec:java -Dexec.mainClass=com.weilliptic.weilwallet.examples.WeilYamlAgent -Dexec.args="src/main/java/com/weilliptic/weilwallet/examples/configagent/multi_agent_basic_config/root_agent.yaml Explain recursion with a short Java example")
```

### multi_agent_llm_config: a root routing to a roll agent and a prime agent

```bash
(set -a; source .env.multidice; set +a; mvn -q exec:java -Dexec.mainClass=com.weilliptic.weilwallet.examples.WeilYamlAgent -Dexec.args="src/main/java/com/weilliptic/weilwallet/examples/configagent/multi_agent_llm_config/root_agent.yaml Roll a 20-sided die and check if it is prime")
```

### sub_agents_config: a root routing to a work agent and a life agent

```bash
(set -a; source .env.hello; set +a; mvn -q exec:java -Dexec.mainClass=com.weilliptic.weilwallet.examples.WeilYamlAgent -Dexec.args="src/main/java/com/weilliptic/weilwallet/examples/configagent/sub_agents_config/root_agent.yaml How do I balance work and rest this week?")
```

### tool_builtin_config: web search

```bash
(set -a; source .env.web; set +a; mvn -q exec:java -Dexec.mainClass=com.weilliptic.weilwallet.examples.WeilYamlAgent -Dexec.args="src/main/java/com/weilliptic/weilwallet/examples/configagent/tool_builtin_config/root_agent.yaml Who won the last Cricket World Cup?")
```

### tool_functions_config: dice and primes as function tools

```bash
(set -a; source .env.hello; set +a; mvn -q exec:java -Dexec.mainClass=com.weilliptic.weilwallet.examples.WeilYamlAgent -Dexec.args="src/main/java/com/weilliptic/weilwallet/examples/configagent/tool_functions_config/root_agent.yaml Roll a 20-sided die and check if it is prime")
```

### tool_mcp_stdio_file_system_config: files in /tmp/mcp-demo over MCP

Needs Node.js.

```bash
(set -a; source .env.mcp; set +a; mvn -q exec:java -Dexec.mainClass=com.weilliptic.weilwallet.examples.WeilYamlAgent -Dexec.args="src/main/java/com/weilliptic/weilwallet/examples/configagent/tool_mcp_stdio_file_system_config/root_agent.yaml List the files you can see")
```

### dice: dice and primes, saving every roll to memory

This repo's own YAML agent.

```bash
(set -a; source .env.hello; set +a; mvn -q exec:java -Dexec.mainClass=com.weilliptic.weilwallet.examples.WeilYamlAgent -Dexec.args="src/main/java/com/weilliptic/weilwallet/examples/configagent/dice/root_agent.yaml")
```

### travel: a planner calling a flights agent and a hotels agent over A2A

This repo's own YAML agents. Three terminals, each with its own wallet. Start
the two specialists first:

```bash
(set -a; source .env.flights; set +a; mvn -q exec:java -Dexec.mainClass=com.weilliptic.weilwallet.examples.WeilYamlAgent -Dexec.args="--serve 8001 src/main/java/com/weilliptic/weilwallet/examples/configagent/travel/flights_agent.yaml")
```

```bash
(set -a; source .env.hotels; set +a; mvn -q exec:java -Dexec.mainClass=com.weilliptic.weilwallet.examples.WeilYamlAgent -Dexec.args="--serve 8002 src/main/java/com/weilliptic/weilwallet/examples/configagent/travel/hotels_agent.yaml")
```

Then the planner:

```bash
(set -a; source .env.web; set +a; mvn -q exec:java -Dexec.mainClass=com.weilliptic.weilwallet.examples.WeilYamlAgent -Dexec.args="src/main/java/com/weilliptic/weilwallet/examples/configagent/travel/planner_agent.yaml")
```

## More

- Every way to run an agent, attach the Weil plugin, and secure agents and
  tools, side by side: `AGENT_GUIDE.md`.
- What each YAML agent secures, and the `weil:` permission block:
  `examples/configagent/README.md`.
- The financial advisor's changes from the recipe:
  `examples/financialadvisor/README.md`.
- The SDK itself, hooks and the hand-written travel examples:
  `src/main/java/com/weilliptic/weilwallet/adk/README.md` and `HOOKS.md`.
