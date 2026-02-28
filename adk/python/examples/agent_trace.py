# trace_mcp_server.py
import asyncio
import json
import uuid
import httpx
import os
import subprocess
from datetime import datetime, timezone
from pathlib import Path
from mcp.server import Server
from mcp.server.stdio import stdio_server
from weil_wallet import PrivateKey, Wallet, WeilClient

server = Server("agent-trace")


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
            return WeilClient(wallet)
    raise FileNotFoundError(
        "private_key.wc not found. Place it in examples/, python/, or cwd."
    )


client = _make_client()


def get_git_head() -> str:
    result = subprocess.run(
        ["git", "rev-parse", "HEAD"], capture_output=True, text=True
    )
    return result.stdout.strip()


def get_git_root() -> Path:
    result = subprocess.run(
        ["git", "rev-parse", "--show-toplevel"], capture_output=True, text=True
    )
    return Path(result.stdout.strip())


def compute_ranges(old_content: str, new_content: str) -> list[dict]:
    """Diff old vs new and return added line ranges."""
    old_lines = old_content.splitlines(keepends=True)
    new_lines = new_content.splitlines(keepends=True)

    ranges = []
    current_line = 1
    i_old = 0
    i_new = 0

    import difflib

    matcher = difflib.SequenceMatcher(None, old_lines, new_lines)

    for op, old_start, old_end, new_start, new_end in matcher.get_opcodes():
        added_count = new_end - new_start
        removed_count = old_end - old_start

        if op == "equal":
            current_line += added_count
        elif op == "insert":
            ranges.append(
                {"start_line": current_line, "end_line": current_line + added_count - 1}
            )
            current_line += added_count
        elif op == "replace":
            ranges.append(
                {"start_line": current_line, "end_line": current_line + added_count - 1}
            )
            current_line += added_count
        elif op == "delete":
            pass  # removed lines don't affect new file line numbers

    return ranges


async def post_trace(trace_record: dict):
    await client.audit(json.dumps(trace_record))


def build_trace_record(file_path: str, ranges: list[dict]) -> dict:
    git_root = get_git_root()
    relative_path = str(Path(file_path).relative_to(git_root))

    return {
        "version": "0.1.0",
        "id": str(uuid.uuid4()),
        "timestamp": datetime.now(timezone.utc).isoformat(),
        "vcs": {"type": "git", "revision": get_git_head()},
        "tool": {"name": "claude-code", "version": "1.0"},
        "files": [
            {
                "path": relative_path,
                "conversations": [
                    {
                        "contributor": {
                            "type": "ai",
                            "model_id": "anthropic/claude-sonnet-4-6",
                        },
                        "ranges": ranges,
                    }
                ],
            }
        ],
    }


# --- MCP Tool: intercept Write ---


@server.call_tool()
async def handle_tool(name: str, arguments: dict) -> list:
    if name == "Write":
        file_path = arguments["file_path"]
        new_content = arguments["content"]

        # Snapshot old content
        try:
            old_content = Path(file_path).read_text(encoding="utf-8")
        except FileNotFoundError:
            old_content = ""

        # Write the file
        Path(file_path).parent.mkdir(parents=True, exist_ok=True)
        Path(file_path).write_text(new_content, encoding="utf-8")

        # Compute ranges and post trace
        ranges = compute_ranges(old_content, new_content)

        if ranges:
            trace = build_trace_record(file_path, ranges)
            await post_trace(trace)
            print(f"[agent-trace] Recorded {len(ranges)} range(s) for {file_path}")

        return [{"type": "text", "text": f"Written: {file_path}"}]

    # Pass through any other tools unmodified
    return [{"type": "text", "text": f"Tool {name} not handled by agent-trace"}]


# --- Entry point ---


async def main():
    async with stdio_server() as (read_stream, write_stream):
        await server.run(
            read_stream, write_stream, server.create_initialization_options()
        )


if __name__ == "__main__":
    asyncio.run(main())
