import weil_ai
from google.adk.agents import Agent
from google.adk.models.lite_llm import LiteLlm


FLIGHT_AGENT_API_KEY = "weil_***"
FLIGHT_SERVICE = "engg.weil"


@weil_ai.secured_user("engg.weil")
async def search_flights(origin: str, destination: str, date: str) -> str:
    return f"""
Available flights from {origin} to {destination} on {date}:

1. Air India
   Departure: 08:30
   Arrival: 10:45
   Price: Rs 7,200

2. IndiGo
   Departure: 13:15
   Arrival: 15:30
   Price: Rs 6,400

3. Vistara
   Departure: 18:20
   Arrival: 20:35
   Price: Rs 8,100
"""


@weil_ai.agent(identity=FLIGHT_AGENT_API_KEY, verify=False)
def create_flight_agent():
    return Agent(
        name="flight_agent",
        model=LiteLlm(model="openai/gpt-4o"),
        description="Finds flights between cities.",
        instruction="""
You are a flight search specialist.

When the user asks about flights, use the search_flights tool.
Return the available flights clearly.
Do not answer questions outside your expertise.
""",
        tools=[search_flights],
    )


weil_flight_agent = create_flight_agent()
weil_flight_agent.setup_secure_a2a(service=FLIGHT_SERVICE)

# ADK tools expect the module-level root_agent to be the native ADK object.
root_agent = weil_flight_agent.inner_agent

# Run with: uvicorn flights.agent:a2a_app --port 8001
a2a_app = weil_flight_agent.to_secure_a2a(host="127.0.0.1", port=8001)
