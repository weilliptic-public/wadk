import weil_ai
from google.adk.agents import Agent
from google.adk.models.lite_llm import LiteLlm


HOTEL_AGENT_API_KEY = "weil_***"
HOTEL_SERVICE = "engg.weil"


@weil_ai.secured_user("engg.weil")
async def search_hotels(city: str, check_in: str, check_out: str) -> str:
    return f"""
Hotels in {city} from {check_in} to {check_out}:

1. The Grand Hotel
   Rating: 4.6
   Price: Rs 5,500/night
   Location: City Center

2. Hotel Central
   Rating: 4.3
   Price: Rs 3,800/night
   Location: Near Railway Station

3. The Royal Palace
   Rating: 4.8
   Price: Rs 7,200/night
   Location: Downtown
"""


@weil_ai.agent(identity=HOTEL_AGENT_API_KEY, verify=False)
def create_hotel_agent():
    return Agent(
        name="hotel_agent",
        model=LiteLlm(model="openai/gpt-4o"),
        description="Finds hotels and accommodation.",
        instruction="""
You are a hotel search specialist.

When the user asks about accommodation, use the search_hotels tool.
Return useful hotel options with prices and ratings.
Do not answer questions outside your expertise.
""",
        tools=[search_hotels],
    )


weil_hotel_agent = create_hotel_agent()
weil_hotel_agent.setup_secure_a2a(service=HOTEL_SERVICE)

root_agent = weil_hotel_agent.inner_agent

# Run with: uvicorn hotels.agent:a2a_app --port 8002
a2a_app = weil_hotel_agent.to_secure_a2a(host="127.0.0.1", port=8002)
