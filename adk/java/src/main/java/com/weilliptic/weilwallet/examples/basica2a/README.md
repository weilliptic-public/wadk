# Basic A2A set: assistant → weather and currency

Three small agents for testing agent-to-agent calls, laid out like the travel
example: one agent that calls two others. Use them to check the door and tool
checks, and that a prompt is one trace however many agents it reaches.

| Agent | Role | Runs on |
|---|---|---|
| `WeatherAgent` | called over A2A; protected tool `getWeather` (needs its own wallet in `engg.weil`) | `WEATHER_PORT`, default 8011 |
| `CurrencyAgent` | called over A2A; protected tool `convert` (needs its own wallet in `finance.weil`) | `CURRENCY_PORT`, default 8012 |
| `AssistantAgent` | the caller, in ADK's dev UI; its own protected tool `today`, plus the two agents | `PORT`, default 8080 |

## Run

From `adk/java`, each in its own terminal, with your env file (wallet key,
`SENTINEL_HOST`, model settings). Start the two called agents first.

```bash
(set -a; source /path/to/.env; set +a; mvn -q compile exec:java -Dexec.mainClass=com.weilliptic.weilwallet.examples.basica2a.WeatherAgent)
```

```bash
(set -a; source /path/to/.env; set +a; mvn -q compile exec:java -Dexec.mainClass=com.weilliptic.weilwallet.examples.basica2a.CurrencyAgent)
```

The assistant, calling both as tools:

```bash
(set -a; source /path/to/.env; set +a; mvn -q compile exec:java -Dexec.mainClass=com.weilliptic.weilwallet.examples.basica2a.AssistantAgent)
```

Or handing the conversation over to one of them:

```bash
(set -a; source /path/to/.env; set +a; A2A_MODE=agent mvn -q compile exec:java -Dexec.mainClass=com.weilliptic.weilwallet.examples.basica2a.AssistantAgent)
```

To run any agent on another port, put its port setting in front of `mvn`:
`PORT` for the assistant, `WEATHER_PORT`, `CURRENCY_PORT` or
`TRAVEL_DESK_PORT` for the others. A caller then needs the new address,
through `WEATHER_URL`, `CURRENCY_URL` or `TRAVEL_DESK_URL`. For example, the
assistant on port 9000:

```bash
(set -a; source /path/to/.env; set +a; PORT=9000 mvn -q compile exec:java -Dexec.mainClass=com.weilliptic.weilwallet.examples.basica2a.AssistantAgent)
```

and the weather agent on port 9011, with the assistant pointed at it:

```bash
(set -a; source /path/to/.env; set +a; WEATHER_PORT=9011 mvn -q compile exec:java -Dexec.mainClass=com.weilliptic.weilwallet.examples.basica2a.WeatherAgent)
```

```bash
(set -a; source /path/to/.env; set +a; WEATHER_URL=http://127.0.0.1:9011 mvn -q compile exec:java -Dexec.mainClass=com.weilliptic.weilwallet.examples.basica2a.AssistantAgent)
```

To give each agent its own wallet, put a different `WEIL_AGENT_KEY=...` in
front of `mvn` for each one.

## Prompts

- "I'm going to Dubai today. What's the weather there, and how much is 10000 INR in AED?": both agents in one turn (tool mode).
- "What's the weather in Goa?": the weather agent only.
- "Convert 250 USD to INR.": the currency agent only.
- "What's the date?": the assistant's own tool only.

## What to check

| Where | Wallet checked |
|---|---|
| Weather agent's door (`WEATHER_SERVICE`, default `engg.weil`) | the **assistant's** |
| Currency agent's door (`CURRENCY_SERVICE`, default `engg.weil`) | the **assistant's** |
| `getWeather` (`engg.weil`) | the **weather agent's own** |
| `convert` (`finance.weil`) | the **currency agent's own** |
| `today` (`engg.weil`) | the **assistant's own** |

Each check's pass or fail goes to the `security` topic.

On the `audits` topic there is **one** trace per prompt, published by the
assistant:

```
prompt
 ├─ today
 ├─ weather_agent call
 │   └─ subtrace [weather agent's wallet]
 │       └─ getWeather
 └─ currency_agent call
     └─ subtrace [currency agent's wallet]
         └─ convert
```

In hand-over mode the call is a `transfer_to_agent` step followed by a
`remote_agent` step holding the reply. The called agents publish nothing of
their own.

## Three agents deep: assistant → travel desk → weather and currency

Start the weather and currency agents first, then one travel desk and the
assistant:

| Chain | Travel desk | Assistant |
|---|---|---|
| Remote tools all the way: the desk asks both agents and combines the answers | `TravelDeskAgent` (8013) | `A2A_MODE=chain` |
| Remote agents all the way: each question is handed on to one agent, so ask about weather or currency, not both | `TravelDeskAgent` (8013) with `A2A_MODE=agent` | `A2A_MODE=chain-agent` |
| Remote agents, desk asks both in parallel: the assistant hands over to the desk, which runs weather and currency at the same time and combines them | `TravelDeskAgent` (8013) with `A2A_MODE=parallel` | `A2A_MODE=chain-agent` |

## Both agents in parallel, every time

`A2A_MODE=parallel` puts the weather and currency agents in an ADK
`ParallelAgent` (as remote agents), followed by a summarizer: both run on every
prompt, at the same time, and the summarizer combines their replies. No travel
desk is needed.

```bash
(set -a; source /path/to/.env; set +a; A2A_MODE=parallel mvn -q compile exec:java -Dexec.mainClass=com.weilliptic.weilwallet.examples.basica2a.AssistantAgent)
```

To see a refusal at a door, start that agent with its service set to a group
the assistant's wallet is not in, such as `CURRENCY_SERVICE=hr.weil`. To see a
tool refusal, run the currency agent with a wallet that is not in
`finance.weil`.
