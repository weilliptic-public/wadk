"""Venues agent: a Strands agent served over secured A2A.

Run from examples/strands/event-a2a:

    uvicorn venues.agent:a2a_app --port 8201 --env-file .env
"""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[4]))
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import settings
import weil_ai
import weil_ai.strands as weil_strands
from strands import Agent, tool


SERVICE = settings.WEIL_VENUES_SERVICE


@tool
def search_venues(city: str, guests: int) -> str:
    """List every venue in a city that seats at least the given number of guests.

    One call returns all of them; searching again gives the same list.
    """
    venues = [
        ("The Glasshouse", 40, "Rs 60,000/day"),
        ("Riverside Hall", 120, "Rs 1,40,000/day"),
        ("Courtyard Loft", 80, "Rs 95,000/day"),
    ]
    fits = [f"- {name} (seats {seats}, {price})" for name, seats, price in venues if seats >= guests]
    return (
        f"All venues in {city} that seat {guests} or more:\n"
        + ("\n".join(fits) or "none")
        + "\nThis is the complete list. Name, capacity and price are all that is "
        "known: availability, AV, layout and other details are not available."
    )


@tool
@weil_ai.secured_user(SERVICE)
def hold_venue(venue: str, date: str) -> str:
    """Place a 48-hour hold on a venue for a date. Only permitted wallets may hold."""
    return f"Held {venue} for {date}. Hold reference VH-{abs(hash((venue, date))) % 10000:04d}."


@weil_ai.agent(
    identity=settings.required("WEIL_VENUES_IDENTITY"),
    sentinel_host=settings.SENTINEL_HOST,
    verify=False,
)
def create_venues_agent():
    # A plain Strands agent; to_secure_a2a installs Weil's hooks on it.
    return Agent(
        name="venues_agent",
        description="Finds event venues and places holds on them.",
        model=settings.openai_model(),
        tools=[search_venues, hold_venue],
        system_prompt=(
            "You are a venue specialist. Call search_venues once per request and "
            "answer from its result: it returns every venue there is, so never "
            "search again with other inputs. Name, capacity and price are all you "
            "know; if asked about availability, AV or anything else, say it is "
            "not available. Call hold_venue only when the request explicitly asks "
            "you to hold a named venue for a date; otherwise only recommend. "
            "Answer only about venues, briefly."
        ),
    )


# A fresh agent per A2A conversation, each with Weil's hooks installed
# (ADK: setup_secure_a2a + to_secure_a2a). Callers must sign their requests and
# hold Execution or Management on the service.
a2a_app = weil_strands.to_secure_a2a(
    create_venues_agent, service=SERVICE, host="127.0.0.1", port=8201
)
