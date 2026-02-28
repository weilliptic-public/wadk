import json
import mcp.server.stdio
import mcp.types as types
from mcp.server.lowlevel import NotificationOptions, Server
from mcp.server.models import InitializationOptions
import asyncio
import sys
from pathlib import Path
import os

# Allow importing weil_wallet when run as script (e.g. python examples/example.py)
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from weil_wallet import PrivateKey, Wallet, WeilClient

server = Server("weilchain-audit")


def _make_client() -> WeilClient:
    script_dir = os.path.dirname(os.path.abspath(__file__))
    for candidate in (
        os.path.join(script_dir, "private_key.wc"),
        "private_key.wc",
        os.path.join(os.path.dirname(script_dir), "private_key.wc"),
    ):
        if os.path.isfile(candidate):
            pk = PrivateKey.from_file(candidate)
            wallet = Wallet(pk)
            return WeilClient(wallet, sentinel_host=os.environ.get("SENTINEL_HOST"))
    raise FileNotFoundError(
        "private_key.wc not found. Place it in examples/, python/, or cwd."
    )


client = _make_client()


@server.list_tools()
async def list_tools():
    return [
        types.Tool(
            name="log_io",
            description="MUST be called before any file write, edit, delete, shell command, or external API call.",
            inputSchema={
                "type": "object",
                "properties": {
                    "action_type": {
                        "type": "string",
                        "enum": [
                            "file_write",
                            "file_edit",
                            "file_delete",
                            "shell_exec",
                            "api_call",
                            "config_change",
                        ],
                    },
                    "target": {
                        "type": "string",
                        "description": "File path, command, or resource being mutated",
                    },
                    "description": {
                        "type": "string",
                        "description": "Human-readable summary of what is about to happen",
                    },
                    "payload_hash": {
                        "type": "string",
                        "description": "SHA256 hash of the content/command being applied",
                    },
                    "session_id": {"type": "string"},
                },
                "required": ["action_type", "target", "description"],
            },
        )
    ]


@server.call_tool()
@client.audit()
async def call_tool(name: str, arguments: dict):
    if name == "log_io":
        return [
            types.TextContent(
                type="text",
                text=json.dumps({"status": "logged", "proceed": True}),
            )
        ]


async def run():
    """Run the server with lifespan management."""
    async with mcp.server.stdio.stdio_server() as (read_stream, write_stream):
        await server.run(
            read_stream,
            write_stream,
            InitializationOptions(
                server_name="weilchain-auditor",
                server_version="0.1.0",
                capabilities=server.get_capabilities(
                    notification_options=NotificationOptions(),
                    experimental_capabilities={},
                ),
            ),
        )


if __name__ == "__main__":
    asyncio.run(run())
