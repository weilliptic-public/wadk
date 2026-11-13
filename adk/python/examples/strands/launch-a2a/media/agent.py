"""Media agent: a remote agent, served over secured A2A, that can spend money.

Run from examples/strands/launch-a2a:

    uvicorn media.agent:a2a_app --port 8212 --env-file .env
"""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[4]))
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import settings
import weil_ai
import weil_ai.strands as weil_strands
from strands import Agent, tool


# (channel, cost per 1,000 impressions in Rs, minimum spend, best for)
CHANNELS = [
    ("Instagram Reels", 180, 25_000, "remote tech workers, students"),
    ("YouTube pre-roll", 240, 40_000, "broad reach, demo videos"),
    ("Spotify podcast spots", 320, 30_000, "night-shift workers, commuters"),
    ("Tech newsletter sponsorship", 900, 15_000, "remote tech workers"),
]


@tool
def channel_quotes(budget: int) -> str:
    """Every ad channel with its rate, minimum spend and the reach the budget buys.

    One call returns all of them; asking again gives the same list.
    """
    lines = []
    for name, cpm, minimum, audience in CHANNELS:
        if budget >= minimum:
            reach = f"~{budget * 1000 // cpm:,} impressions for the whole budget"
        else:
            reach = f"below the Rs {minimum:,} minimum"
        lines.append(f"- {name}: Rs {cpm}/1k impressions, {reach}; best for {audience}")
    return (
        f"All channels for a Rs {budget:,} budget:\n" + "\n".join(lines) + "\n"
        "This is the complete list. Rates and reach are all that is known."
    )


@tool
@weil_ai.secured_user(settings.WEIL_SERVICE)
def book_ads(channel: str, amount: int) -> str:
    """Book ads on a channel. Spends money, so only permitted wallets may call it."""
    return f"Booked Rs {amount:,} of ads on {channel}."


@weil_ai.agent(
    identity=settings.required("WEIL_MEDIA_IDENTITY"),
    sentinel_host=settings.SENTINEL_HOST,
    verify=False,
)
def create_media_agent():
    return Agent(
        name="media_agent",
        description="Suggests ad channels and books ads.",
        model=settings.openai_model(),
        tools=[channel_quotes, book_ads],
        system_prompt=(
            "You are a media buyer. Call channel_quotes once per request and "
            "answer from its result; never call it again with other inputs. "
            "Propose a split of the budget across at most two channels that suit "
            "the target audience. Call book_ads only when the request explicitly "
            "asks you to book and gives the channel and amount. Answer only about "
            "media, briefly."
        ),
    )


a2a_app = weil_strands.to_secure_a2a(
    create_media_agent, service=settings.WEIL_SERVICE, host="127.0.0.1", port=8212
)
