from google.adk.agents import Agent
from google.adk.agents.remote_a2a_agent import (
    RemoteA2aAgent,
    AGENT_CARD_WELL_KNOWN_PATH,
)


flight_agent = RemoteA2aAgent(
    name="flight_agent",
    description="Searches for flights.",
    agent_card=(
        f"http://127.0.0.1:8001"
        f"{AGENT_CARD_WELL_KNOWN_PATH}"
    ),
)


hotel_agent = RemoteA2aAgent(
    name="hotel_agent",
    description="Searches for hotels.",
    agent_card=(
        f"http://127.0.0.1:8002"
        f"{AGENT_CARD_WELL_KNOWN_PATH}"
    ),
)


root_agent = Agent(
    name="travel_planner",
    model="gemini-2.5-flash",

    instruction="""
You are a travel planning agent.

You have access to two specialist agents:

1. flight_agent
   - Searches for flights.

2. hotel_agent
   - Searches for hotels.

When the user asks you to plan a trip:

1. Determine the destination, origin and dates.
2. Ask flight_agent to find suitable flights.
3. Ask hotel_agent to find suitable hotels.
4. Combine their responses.
5. Present a concise travel plan to the user.

You MUST delegate flight-related work to flight_agent.

You MUST delegate hotel-related work to hotel_agent.

Do not pretend to have searched for flights or hotels yourself.

You are responsible for coordinating the specialists and
producing the final answer.
""",

    sub_agents=[
        flight_agent,
        hotel_agent,
    ],
)