"""Launch director: plans a product launch with two remote and two local agents.

Start both servers first (see the README), then from examples/strands/launch-a2a
either chat with it:

    python director/agent.py

or ask one question and exit:

    python director/agent.py "Plan the launch of Aurora with Rs 2,00,000 for ads"
"""

import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[4]))
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import settings
import weil_ai
import weil_ai.strands as weil_strands
from strands import Agent, tool


# ---- Local agent WITH its own wallet: compliance --------------------------

PRODUCT = (
    "Aurora, a sunrise alarm lamp: fades from red to warm white over 30 minutes "
    "before your alarm, 12 nature sounds, app and Alexa control, Rs 6,999 planned."
)

# Phrases advertising standards do not allow without evidence, and why.
RISKY_CLAIMS = {
    "clinically proven": "needs a published clinical study",
    "cure": "medical claim; a lamp cannot claim to cure anything",
    "insomnia": "medical condition; implies treatment",
    "guaranteed": "absolute promise; needs a refund policy behind it",
    "best": "superlative; needs comparative evidence",
    "#1": "ranking claim; needs a cited source",
    "100%": "absolute claim; needs evidence",
    "never": "absolute claim; needs evidence",
}


@tool
@weil_ai.secured_user(settings.WEIL_SERVICE)
def approve_headline(headline: str) -> str:
    """Approve an ad headline, or reject it if it makes a risky claim."""
    # Whole words only, so "cure" does not match "secure".
    found = [
        f"{phrase!r} ({why})" for phrase, why in RISKY_CLAIMS.items()
        if re.search(rf"(?<!\w){re.escape(phrase)}s?(?!\w)", headline, re.IGNORECASE)
    ]
    if found:
        return "Rejected: " + "; ".join(found)
    return "Approved."


@weil_ai.agent(
    identity=settings.required("WEIL_COMPLIANCE_IDENTITY"),
    sentinel_host=settings.SENTINEL_HOST,
    verify=False,
)
def create_compliance_agent():
    return Agent(
        name="compliance_agent",
        description="Approves or rejects ad headlines.",
        model=settings.openai_model(),
        tools=[approve_headline],
        callback_handler=None,
        system_prompt=(
            "You are the compliance officer. Call approve_headline for each "
            "headline. If one is rejected, say why and suggest a compliant "
            "rewrite, then check the rewrite too. Be brief."
        ),
    )


# ---- The director ---------------------------------------------------------


@weil_ai.agent(
    identity=settings.required("WEIL_DIRECTOR_IDENTITY"),
    sentinel_host=settings.SENTINEL_HOST,
    verify=False,
)
def create_director_agent():
    # Remote agents. Made inside this factory, so the director's wallet signs
    # every request to them.
    research_agent = weil_strands.remote_agent_tool(
        settings.RESEARCH_URL, name="research_agent", description="Suggests a price from competitor prices."
    )
    media_agent = weil_strands.remote_agent_tool(
        settings.MEDIA_URL, name="media_agent", description="Suggests ad channels and books ads."
    )

    # Local agent with its own wallet: it runs as compliance, not as the director.
    compliance_agent = create_compliance_agent().inner_agent

    # Local agent with no wallet: it runs as the director.
    copywriter_agent = Agent(
        name="copywriter_agent",
        description="Writes ad headlines.",
        model=settings.openai_model(),
        callback_handler=None,
        system_prompt=(
            "You are an enthusiastic, bold copywriter. Write exactly three short "
            "ad headlines for the product and audience you are given, numbered. "
            "Go for impact; compliance will check them later."
        ),
    )

    return Agent(
        name="launch_director",
        description="Plans a product launch with research, copywriting, compliance and media buying.",
        model=settings.openai_model(),
        tools=[research_agent, media_agent, compliance_agent, copywriter_agent],
        system_prompt=(
            f"You are the launch director for {PRODUCT}\n"
            "Run the launch with your team, each at most once per step:\n"
            "1. research_agent: price band and the first segment to target.\n"
            "2. copywriter_agent: three headlines for that segment.\n"
            "3. compliance_agent: check every headline; use its compliant "
            "rewrites, and pick the best approved one.\n"
            "4. media_agent: a channel split for the ad budget.\n"
            "Only ask media_agent to book ads when the user explicitly asks; "
            "then pass the channel and amount. Never invent prices or channels. "
            "End with a short launch plan: price, segment, approved headline, "
            "channels.\n"
            "For a follow-up, do only what it asks, using the plan above: for "
            "example, to book ads call only media_agent, with the channel and "
            "amount from the plan or the user."
        ),
    )


weil_director = create_director_agent()
weil_director.setup_secure_strands()  # also secures all four sub-agents
director = weil_director.inner_agent  # the plain Strands agent


def ask(request: str) -> None:
    # Strands prints the answer, and each tool it calls, as it streams.
    director(request)
    print()


def chat() -> None:
    """Ask questions one after another; the director remembers the conversation."""
    print("Launch director for Aurora. Ask a question; type 'exit' or press Ctrl+C to quit.")
    while True:
        try:
            request = input("\nYou: ").strip()
        except (EOFError, KeyboardInterrupt):
            print()
            return
        if request.lower() in ("exit", "quit"):
            return
        if request:
            print("Director: ", end="", flush=True)
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
