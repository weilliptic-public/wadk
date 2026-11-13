"""Settings for the product-launch agents.

Each setting is taken from the environment variable of the same name when it
is set, and otherwise from the value written below. Do not commit real keys.
"""

import os

from strands.models.openai import OpenAIModel


def _setting(name: str, value: str) -> str:
    return os.environ.get(name) or value


# Each agent's Weil identity (its Agent Registry API key, "weil_...").
WEIL_DIRECTOR_IDENTITY = _setting("WEIL_DIRECTOR_IDENTITY", "")
WEIL_COMPLIANCE_IDENTITY = _setting("WEIL_COMPLIANCE_IDENTITY", "")
WEIL_RESEARCH_IDENTITY = _setting("WEIL_RESEARCH_IDENTITY", "")
WEIL_MEDIA_IDENTITY = _setting("WEIL_MEDIA_IDENTITY", "")

# The service the servers gate callers on, and the @secured tools check.
WEIL_SERVICE = _setting("WEIL_SERVICE", "engg.acme")

# The Sentinel every agent talks to. Its TLS certificate is not verified.
SENTINEL_HOST = _setting("SENTINEL_HOST", "https://sentinel.weilliptic.ai")

OPENAI_API_KEY = _setting("OPENAI_API_KEY", "")
OPENAI_MODEL = _setting("OPENAI_MODEL", "gpt-5")

# Where the director finds the two servers.
RESEARCH_URL = _setting("RESEARCH_URL", "http://127.0.0.1:8211")
MEDIA_URL = _setting("MEDIA_URL", "http://127.0.0.1:8212")


def required(name: str) -> str:
    """The setting *name*, or a clear error when it is set nowhere."""
    value = globals()[name]
    if not value:
        raise RuntimeError(f"{name} is not set: export it, put it in .env, or write it in settings.py")
    return value


def openai_model() -> OpenAIModel:
    # GPT-5 and the o-series reject "max_tokens"; this one covers their
    # hidden reasoning too, so keep it generous.
    return OpenAIModel(
        client_args={"api_key": required("OPENAI_API_KEY")},
        model_id=OPENAI_MODEL,
        params={"max_completion_tokens": 8000},
    )
