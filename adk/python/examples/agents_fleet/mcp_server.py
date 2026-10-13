#!/usr/bin/env python3

import os
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import uvicorn
from fastmcp import FastMCP
from weil_ai.mcp import secured, weil_middleware

from agents_fleet import tools

mcp = FastMCP("agents-fleet")


@mcp.tool(name="calculator")
@secured("finance.acme")
async def secured_calculator(expression: str) -> str:
    return tools.calculator.invoke({"expression": expression})


@mcp.tool(name="web_fetch")
@secured("infrastructure.weil")
async def secured_web_fetch(url: str) -> str:
    return tools.web_fetch.invoke({"url": url})


@mcp.tool(name="file_search")
@secured("infrastructure.weil")
async def secured_file_search(
    directory: str,
    query: str,
    extension: str | None = None,
) -> str:
    return tools.file_search.invoke(
        {"directory": directory, "query": query, "extension": extension}
    )


@mcp.tool(name="system_info")
@secured("finance.acme")
async def secured_system_info() -> str:
    return tools.system_info.invoke({})


@mcp.tool(name="process_manager")
@secured("infrastructure.weil")
async def secured_process_manager(search: str | None = None) -> str:
    return tools.process_manager.invoke({"search": search})


@mcp.tool(name="disk_analyzer")
@secured("infrastructure.weil")
async def secured_disk_analyzer(path: str = ".", top_n: int = 10) -> str:
    return tools.disk_analyzer.invoke({"path": path, "top_n": top_n})


@mcp.tool()
@secured("infrastructure.weil")
async def network_diagnostics(hostname: str) -> str:
    return tools.network_diagnostics.invoke({"hostname": hostname})


@mcp.tool()
@secured("finance.acme")
async def environment_variables(prefix: str | None = None) -> str:
    return tools.environment_variables.invoke({"prefix": prefix})


@mcp.tool()
async def port_checker(host: str, port: int, timeout: float = 2.0) -> str:
    return tools.port_checker.invoke(
        {"host": host, "port": port, "timeout": timeout}
    )


@mcp.tool()
@secured("infrastructure.weil")
async def safe_command_execution(command: str) -> str:
    return tools.safe_command_execution.invoke({"command": command})


app = mcp.http_app(transport="streamable-http")
app.add_middleware(weil_middleware())


if __name__ == "__main__":
    uvicorn.run(
        app,
        host=os.environ.get("MCP_HOST", "127.0.0.1"),
        port=int(os.environ.get("MCP_PORT", "8001")),
    )
