"""Example MCP server secured with WeilChain access control.

This module demonstrates how to build a FastMCP server that uses the
``secured`` decorator and ``weil_middleware`` from ``weil_ai.mcp`` to
enforce on-chain access control on every tool call.

Running the server
------------------
    python mcp_server.py

The server listens on ``http://0.0.0.0:8001`` using the streamable-HTTP
MCP transport. Clients must include the four authentication headers
(``X-Wallet-Address``, ``X-Signature``, ``X-Message``, ``X-Timestamp``)
on every POST request; requests that fail signature verification are
rejected with HTTP 401 before they reach any tool handler.
"""

import uvicorn
from fastmcp import FastMCP
from weil_ai.mcp import secured, weil_middleware
import json

# Create the FastMCP server instance. The name "my-server" is advertised
# to MCP clients during the initialize handshake.
mcp = FastMCP("my-server")


@mcp.tool()
@secured("engg.weil")
async def search(query: str) -> str:
    """Search tool guarded by the ``engg.weil`` on-chain access policy.

    The ``@secured("engg.weil")`` decorator resolves the service name to a
    WeilChain ContractId and calls ``key_has_purpose`` before executing the
    handler body. If the caller's wallet address is not permitted, an MCP
    error response is returned without reaching this function.

    Args:
        query: Free-text search string supplied by the MCP client.

    Returns:
        JSON-encoded object with the original ``query`` and a ``result`` field.
    """
    resp = {"query": query, "result": "Bhavya Bhatt"}
    return json.dumps(resp)


# Build the ASGI application using the streamable-HTTP MCP transport and
# attach the WeilChain authentication middleware.
app = mcp.http_app(transport="streamable-http")
app.add_middleware(weil_middleware())

if __name__ == "__main__":
    uvicorn.run(app, host="0.0.0.0", port=8001)
