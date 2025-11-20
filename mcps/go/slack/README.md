# Slack MCP
The Slack MCP exposes a simple surface for listing users and channels, fetching conversation history, and sending messages via Slack's Web API.

## Configuration
- `token`: Slack bot/user token with scopes to read channels/DMs and post messages (e.g., `channels:read`, `groups:read`, `im:history`, `chat:write`).
- Place the token in `slack.yaml` alongside the build artifact. The contract reads this secret at runtime; no other settings are required.

## Core Tools
1. `list_users` – List Slack users visible to the token.
2. `list_public_channels` – List public channels the token can access.
3. `list_private_channels` – List private channels the token is a member of.
4. `get_conversations_from_channel` – Fetch message history for a specific channel name.
5. `get_conversations_with_user` – Fetch direct message history with a user (by real name).
6. `send_message_to_channel` – Send a text message to a channel (by name).
7. `send_message_to_user` – Send a direct message to a user (by real name).

## Deployment
Build the contract and deploy it with your token:
```
Weilliptic$$$> deploy --widl-file <path>/slack.widl --file-path <path>/slack.wasm --config-file <path>/slack.yaml -w <Pod Id to deploy> 
```

## Prompt Examples
- "List all private channels I belong to."
- "Send 'Daily standup in 5 minutes' to #engineering."
- "Show the last 20 messages with Jane Doe."
- "List every public channel and member counts."
- "Post 'Deployment completed' to #release-updates."
