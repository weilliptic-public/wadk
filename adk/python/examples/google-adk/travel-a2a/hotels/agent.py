from google.adk.agents import Agent
from google.adk.a2a.utils.agent_to_a2a import to_a2a


def search_hotels(city: str, check_in: str, check_out: str) -> str:
    """
    Search for hotels in a city.

    This is a mock database for the example.
    """

    return f"""
Hotels in {city}:

1. The Grand Hotel
   Rating: 4.6
   Price: ₹5,500/night
   Location: City Center

2. Hotel Central
   Rating: 4.3
   Price: ₹3,800/night
   Location: Near Railway Station

3. The Royal Palace
   Rating: 4.8
   Price: ₹7,200/night
   Location: Downtown
"""


root_agent = Agent(
    name="hotel_agent",
    model="gemini-2.5-flash",
    description="Finds hotels and accommodation.",
    instruction="""
You are a hotel search specialist.

When the user asks about accommodation, use the search_hotels tool.

Return useful hotel options with prices and ratings.
Do not answer questions outside your expertise.
""",
    tools=[search_hotels],
)


a2a_app = to_a2a(root_agent)