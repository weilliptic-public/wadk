import weil_ai
from google.adk.agents import Agent
from google.adk.models.lite_llm import LiteLlm
from google.adk.agents.remote_a2a_agent import (
    AGENT_CARD_WELL_KNOWN_PATH,
    RemoteA2aAgent,
)


PLANNER_AGENT_API_KEY = "weil_2XbeHBLdxkvbyZ0SBUlYZWt4G1y2JyZH19-MgOwtlKVQSAvM"


flight_agent = RemoteA2aAgent(
    name="flight_agent",
    description="Searches for flights.",
    agent_card=f"http://127.0.0.1:8001{AGENT_CARD_WELL_KNOWN_PATH}",
)


hotel_agent = RemoteA2aAgent(
    name="hotel_agent",
    description="Searches for hotels.",
    agent_card=f"http://127.0.0.1:8002{AGENT_CARD_WELL_KNOWN_PATH}",
)


@weil_ai.agent(identity=PLANNER_AGENT_API_KEY, verify=False)
def create_planner_agent():
    return Agent(
        name="travel_planner",
        model=LiteLlm(model="openai/gpt-4o"),
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
""",
        sub_agents=[
            flight_agent,
            hotel_agent,
        ],
    )


weil_root_agent = create_planner_agent()
weil_root_agent.setup_secure_a2a()

# ADK runners expect the module-level root_agent to be the native ADK object.
root_agent = weil_root_agent.inner_agent
