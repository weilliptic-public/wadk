"""Event planner: calls the venues and catering agents over signed A2A.

Start both servers first (see the README), then from examples/strands/event-a2a
either chat with it:

    python planner/agent.py

or ask one question and exit:

    python planner/agent.py "Plan a team offsite in Pune for 60 people on 12 Dec"
"""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[4]))
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import settings
import weil_ai
import weil_ai.strands as weil_strands
from strands import Agent, tool


@tool
def estimate_total(venue_per_day: int, catering_total: int, days: int = 1) -> str:
    """Add up the venue and catering cost of an event, with 10% contingency."""
    subtotal = venue_per_day * days + catering_total
    return f"Subtotal Rs {subtotal:,}; with 10% contingency Rs {round(subtotal * 1.1):,}."


@weil_ai.agent(
    identity=settings.required("WEIL_PLANNER_IDENTITY"),
    sentinel_host=settings.SENTINEL_HOST,
    verify=False,
)
def create_planner_agent():
    # The remote agents, as tools. Created inside the factory so every request
    # to them is signed with this planner's wallet.
    venues_agent = weil_strands.remote_agent_tool(
        settings.VENUES_URL,
        name="venues_agent",
        description="Finds event venues and places holds on them.",
    )
    catering_agent = weil_strands.remote_agent_tool(
        settings.CATERING_URL,
        name="catering_agent",
        description="Suggests catering menus and books them.",
    )
    # A local sub-agent with no wallet of its own: it runs as the planner.
    budget_agent = Agent(
        name="budget_agent",
        description="Works out an event's total cost from venue and catering prices.",
        model=settings.openai_model(),
        tools=[estimate_total],
        callback_handler=None,
    )

    # A plain Strands agent; the remote agents and the sub-agent are passed in
    # `tools` (ADK: sub_agents=[...]).
    return Agent(
        name="event_planner",
        model=settings.openai_model(),
        tools=[venues_agent, catering_agent, budget_agent],
        system_prompt=(
            "You are an event planner. Ask venues_agent for venues that fit, "
            "catering_agent for menus, and budget_agent for the total cost, each "
            "once per question, with the city, date and guest count. They know "
            "only names, capacities and prices; do not ask them for availability "
            "or other details. Do not ask venues_agent to hold or catering_agent "
            "to book anything unless the user explicitly asked you to hold or "
            "book; when they did, name the exact venue or menu, date and guest "
            "count. Never invent venues, menus or prices yourself."
        ),
    )


weil_planner = create_planner_agent()
weil_planner.setup_secure_strands()  # ADK: setup_secure_a2a()

# The native Strands agent, with Weil's hooks installed.
planner = weil_planner.inner_agent


def ask(request: str) -> None:
    # Strands prints the answer, and each tool it calls, as it streams.
    planner(request)
    print()


def chat() -> None:
    """Ask questions one after another; the planner remembers the conversation."""
    print("Event planner. Ask a question; type 'exit' or press Ctrl+C to quit.")
    while True:
        try:
            request = input("\nYou: ").strip()
        except (EOFError, KeyboardInterrupt):
            print()
            return
        if request.lower() in ("exit", "quit"):
            return
        if request:
            print("Planner: ", end="", flush=True)
            try:
                ask(request)
            except KeyboardInterrupt:
                print("\n(stopped)")
                return


if __name__ == "__main__":
    if len(sys.argv) > 1:
        ask(" ".join(sys.argv[1:]))
    else:
        chat()
