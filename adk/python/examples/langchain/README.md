# LangChain Secure Hooks

This example shows the LangChain equivalent of the Google ADK secure setup.

Use normal LangChain tools and agents, but:

```python
@tool
@weil_ai.secured_user("engg.weil")
def add(a: int, b: int) -> int:
    return a + b
```

and create the agent through Weil's LangChain helper inside a Weil identity:

```python
@weil_ai.agent(api_key="weil_langchain_agent_key_here")
def create_math_agent():
    return weil_ai.langchain.create_agent(
        model=model,
        tools=[add],
        system_prompt="You are a math agent.",
    )
```

`weil_ai.langchain.create_agent(...)` calls LangChain's own `create_agent(...)`
and injects Weil middleware. The middleware:

- audits `before_model` and `after_model`
- wraps LangChain tool calls
- sets the active Weil wallet context so `@secured_user("engg.weil")` checks the
  wrapped agent wallet

Run:

```bash
python examples/langchain/secure_hooks.py
```
