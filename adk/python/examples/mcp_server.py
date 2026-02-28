import uvicorn
from fastmcp import FastMCP
from weil_ai.mcp import secured, weil_middleware
import json

mcp = FastMCP("my-server")


@mcp.tool()
@secured("engg::weil")
async def search(query: str) -> str:
    resp = {"query": query, "result": "Bhavya Bhatt"}
    return json.dumps(resp)


app = mcp.http_app(transport="streamable-http")
app.add_middleware(weil_middleware())

if __name__ == "__main__":
    uvicorn.run(app, host="0.0.0.0", port=8001)
