"""Catering agent: a Strands agent served over secured A2A.

Run from examples/strands/event-a2a:

    uvicorn catering.agent:a2a_app --port 8202 --env-file .env
"""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[4]))
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import settings
import weil_ai
import weil_ai.strands as weil_strands
from strands import Agent, tool


SERVICE = settings.WEIL_CATERING_SERVICE


@tool
def catering_menus(guests: int, diet: str = "any") -> str:
    """List every catering menu with its price for the given number of guests.

    One call returns all of them; asking again gives the same list.
    """
    menus = [("Working lunch", 650), ("South Indian thali", 900), ("Plated dinner", 1800)]
    lines = [f"- {name}: Rs {per_head}/head, Rs {per_head * guests:,} total" for name, per_head in menus]
    return (
        f"All menus for {guests} guests (diet requested: {diet}):\n"
        + "\n".join(lines)
        + "\nThis is the complete list. Name and price are all that is known: "
        "dishes, dietary details and availability are not available."
    )


@tool
@weil_ai.secured_user(SERVICE)
def book_catering(menu: str, guests: int, date: str) -> str:
    """Book a catering menu for a date. Only permitted wallets may book."""
    return f"Booked {menu} for {guests} guests on {date}. Order CT-{abs(hash((menu, date))) % 10000:04d}."


@weil_ai.agent(
    identity=settings.required("WEIL_CATERING_IDENTITY"),
    sentinel_host=settings.SENTINEL_HOST,
    verify=False,
)
def create_catering_agent():
    # A plain Strands agent; to_secure_a2a installs Weil's hooks on it.
    return Agent(
        name="catering_agent",
        description="Suggests catering menus and books them.",
        model=settings.openai_model(),
        tools=[catering_menus, book_catering],
        system_prompt=(
            "You are a catering specialist. Call catering_menus once per request "
            "and answer from its result: it returns every menu there is, so never "
            "ask again with other inputs. Name and price are all you know; if "
            "asked about dishes, dietary details or anything else, say it is not "
            "available. Call book_catering only when the request explicitly asks "
            "you to book a named menu for a date and guest count; otherwise only "
            "recommend. Answer only about catering, briefly."
        ),
    )


a2a_app = weil_strands.to_secure_a2a(
    create_catering_agent, service=SERVICE, host="127.0.0.1", port=8202
)
