# Product Launch A2A (Strands)

A launch director plans the launch of **Aurora**, a sunrise alarm lamp, with four
helper agents: two remote and two local.

```
                     launch_director   (director wallet)
                            │
        ┌─── remote (A2A) ──┴───────────┬────── local (same process) ──────┐
        │                 │             │                                  │
  research_agent    media_agent    compliance_agent                copywriter_agent
  :8211             :8212          (its OWN wallet)                (no wallet: runs
  research wallet   media wallet                                   as the director)
                    book_ads 🔒      approve_headline 🔒
```

🔒 = `@secured_user`: the tool runs only if the acting wallet has permission.

| Agent | Where | Runs as |
|---|---|---|
| `research_agent` | remote, `:8211` | its own wallet |
| `media_agent` | remote, `:8212` | its own wallet |
| `compliance_agent` | local | its own wallet (director recorded as caller) |
| `copywriter_agent` | local | the director's wallet |

## Run

```bash
pip install -e "../../../weil_ai[strands]" "strands-agents[a2a,openai]"
```

Put the settings in `examples/strands/launch-a2a/.env` (or in `settings.py`):

```bash
WEIL_DIRECTOR_IDENTITY=weil_...
WEIL_COMPLIANCE_IDENTITY=weil_...
WEIL_RESEARCH_IDENTITY=weil_...
WEIL_MEDIA_IDENTITY=weil_...
OPENAI_API_KEY=sk-...
```

From `examples/strands/launch-a2a`, start the two remote agents, each in its own terminal:

```bash
uvicorn research.agent:a2a_app --port 8211 --env-file .env
uvicorn media.agent:a2a_app --port 8212 --env-file .env
```

Then run the director (the first line loads `.env`; skip it if the values are
in `settings.py`):

```bash
set -a; source .env; set +a
python director/agent.py
```

That starts a chat; the director remembers the conversation, so you can plan
first and book afterwards:

```
You: Plan the launch of Aurora with Rs 2,00,000 for ads
     (research → copywriter → compliance → media; nothing is booked)
You: Book Rs 1,20,000 on Instagram Reels
     (only media_agent is called, and book_ads 🔒 runs as the media wallet)
You: exit
```

The copywriter will write something like "Clinically proven to cure bad
mornings", compliance rejects it with the reason, and the director picks an
approved rewrite. Each question is its own run, with its own audit records and
trace. To ask one question and exit instead, put it in the command:

```bash
python director/agent.py "Plan the launch of Aurora with Rs 2,00,000 for ads"
```

## Try a refusal

- Remove the **director** wallet's permission on `engg.acme`: the remote
  servers answer 403 and the director reports the refusal.
- Remove only the **compliance** wallet's permission: `approve_headline` is
  refused, even though the director itself still has permission. Compliance
  checks run under compliance's own wallet.
