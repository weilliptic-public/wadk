"""Settings for the event-planning agents.

Each setting is taken from the environment variable of the same name when it
is set, and otherwise from the value written below. Fill in the values here,
or export them (or put them in .env) and leave these blank.

Do not commit real keys: anything written in this file ends up in git.
"""

import os

from strands.models.openai import OpenAIModel


def _setting(name: str, value: str) -> str:
    return os.environ.get(name) or value


# Each agent's Weil identity (its Agent Registry API key, "weil_...").
WEIL_VENUES_IDENTITY = _setting("WEIL_VENUES_IDENTITY", "")
WEIL_CATERING_IDENTITY = _setting("WEIL_CATERING_IDENTITY", "")
WEIL_PLANNER_IDENTITY = _setting("WEIL_PLANNER_IDENTITY", "")

# The service each server gates callers on, and its @secured tools check.
WEIL_VENUES_SERVICE = _setting("WEIL_VENUES_SERVICE", "engg.acme")
WEIL_CATERING_SERVICE = _setting("WEIL_CATERING_SERVICE", "engg.acme")

# The Sentinel every agent talks to: wallets, permission checks, audit, traces,
# memory. TLS certificates are not verified (the agents pass verify=False).
SENTINEL_HOST = _setting("SENTINEL_HOST", "https://sentinel.weilliptic.ai")

# The model every agent uses.
OPENAI_API_KEY = _setting("OPENAI_API_KEY", "")
# Any OpenAI model with tool calling, e.g. "gpt-5", "gpt-5-mini" (cheaper,
# faster) or "gpt-4o".
OPENAI_MODEL = _setting("OPENAI_MODEL", "gpt-5")
# How hard a reasoning model (GPT-5, the o-series) thinks before answering:
# "minimal", "low", "medium" or "high". Lower is faster and cheaper, and leaves
# more of each answer's token budget for the answer. Other models ignore it.
OPENAI_REASONING_EFFORT = _setting("OPENAI_REASONING_EFFORT", "low")

# Where the planner finds the two servers.
VENUES_URL = _setting("VENUES_URL", "http://127.0.0.1:8201")
CATERING_URL = _setting("CATERING_URL", "http://127.0.0.1:8202")


def required(name: str) -> str:
    """The setting *name*, or a clear error when it is set nowhere."""
    value = globals()[name]
    if not value:
        raise RuntimeError(
            f"{name} is not set: export it, put it in .env, or write it in settings.py"
        )
    return value


def openai_model(max_completion_tokens: int = 16000) -> OpenAIModel:
    """An OpenAI model for an agent, from the OPENAI_* settings above.

    ``max_completion_tokens`` caps each answer: the output limit every current
    model accepts (GPT-5 and the o-series reject the older ``max_tokens``). For
    a reasoning model it also covers the hidden reasoning, so keep it generous.
    """
    params: dict = {"max_completion_tokens": max_completion_tokens}
    if _is_reasoning_model(OPENAI_MODEL):
        params["reasoning_effort"] = OPENAI_REASONING_EFFORT
    return OpenAIModel(
        client_args={"api_key": required("OPENAI_API_KEY")},
        model_id=OPENAI_MODEL,
        params=params,
    )


def _is_reasoning_model(model: str) -> bool:
    # Only these accept reasoning_effort; gpt-4o and gpt-4.1 reject it.
    return model.startswith(("gpt-5", "o1", "o3", "o4"))
