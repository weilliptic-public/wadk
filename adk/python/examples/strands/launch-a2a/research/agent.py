"""Research agent: a remote agent, served over secured A2A.

Run from examples/strands/launch-a2a:

    uvicorn research.agent:a2a_app --port 8211 --env-file .env
"""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[4]))
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import settings
import weil_ai
import weil_ai.strands as weil_strands
from strands import Agent, tool


COMPETITORS = [
    ("Philips Wake-up Light HF3520", 11_999, "4.3", "premium, clinical look"),
    ("Hatch Restore 2", 14_500, "4.1", "app-heavy, subscription for sounds"),
    ("Lumie Bodyclock Shine 300", 16_900, "4.5", "UK import, slow delivery"),
    ("Generic Amazon sunrise lamp", 2_499, "3.6", "cheap plastic, harsh light"),
]
SEGMENTS = [
    ("Night-shift healthcare workers", "18%", "irregular sleep, wake in the dark"),
    ("Early-rising students", "27%", "price-sensitive, phone-alarm fatigue"),
    ("Remote tech workers", "34%", "design-led, want app + Alexa control"),
    ("New parents", "21%", "want a gentle night light more than an alarm"),
]


@tool
def market_snapshot(product: str) -> str:
    """Competitor prices and buyer segments for sunrise alarm lamps in India.

    One call returns everything known; asking again gives the same answer.
    """
    rivals = [f"- {n}: Rs {p:,}, rated {r}, {note}" for n, p, r, note in COMPETITORS]
    segments = [f"- {n} ({share} of buyers): {need}" for n, share, need in SEGMENTS]
    return (
        f"Market snapshot for {product} (sunrise alarm lamps, India):\n"
        "Competitors:\n" + "\n".join(rivals) + "\n"
        "Buyer segments:\n" + "\n".join(segments) + "\n"
        "This is everything known: sales volumes, ad costs and anything else are "
        "not available."
    )


@weil_ai.agent(
    identity=settings.required("WEIL_RESEARCH_IDENTITY"),
    sentinel_host=settings.SENTINEL_HOST,
    verify=False,
)
def create_research_agent():
    return Agent(
        name="research_agent",
        description="Suggests a price from competitor prices.",
        model=settings.openai_model(),
        tools=[market_snapshot],
        system_prompt=(
            "You are a market research analyst. Call market_snapshot once per "
            "request and answer from its result; never call it again with other "
            "inputs. Recommend a price band and the segment to target first, with "
            "a one-line reason each. Answer only about the market, briefly."
        ),
    )


# Callers must sign their requests and hold Execution or Management on the service.
a2a_app = weil_strands.to_secure_a2a(
    create_research_agent, service=settings.WEIL_SERVICE, host="127.0.0.1", port=8211
)
