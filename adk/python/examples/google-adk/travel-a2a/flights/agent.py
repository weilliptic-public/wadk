from google.adk.agents import Agent
from google.adk.a2a.utils.agent_to_a2a import to_a2a


def search_flights(origin: str, destination: str, date: str) -> str:
    """
    Search for flights between two cities.

    This is a mock database for the example.
    """

    return f"""
Available flights from {origin} to {destination} on {date}:

1. Air India
   Departure: 08:30
   Arrival: 10:45
   Price: ₹7,200

2. IndiGo
   Departure: 13:15
   Arrival: 15:30
   Price: ₹6,400

3. Vistara
   Departure: 18:20
   Arrival: 20:35
   Price: ₹8,100
"""


root_agent = Agent(
    name="flight_agent",
    model="gemini-2.5-flash",
    description="Finds flights between cities.",
    instruction="""
You are a flight search specialist.

When the user asks about flights, use the search_flights tool.

Return the available flights clearly.
Do not answer questions outside your expertise.
""",
    tools=[search_flights],
)


# Expose this ADK agent through A2A.
a2a_app = to_a2a(root_agent)