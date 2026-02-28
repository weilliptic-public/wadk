# Python SDK

Python packages for building on WeilChain. The SDK is split into two installable packages:

| Package                       | Description                                      |
| ----------------------------- | ------------------------------------------------ |
| [`weil_wallet`](#weil_wallet) | Core wallet, signing, and transaction primitives |
| [`weil_ai`](#weil_ai)         | AI agent integration and MCP server utilities    |

---

## Installation

Each package has its own `pyproject.toml` and is installed independently. Install from the `python/` directory.

### Install `weil_wallet`

```bash
cd python/weil_wallet
pip3 install .
```

### Install `weil_ai`

`weil_ai` depends on `weil_wallet`, so install `weil_wallet` first.

```bash
cd python/weil_wallet
pip3 install .

cd ../weil_ai
pip3 install .
```

### Virtual environment setup (recommended)

````bash
python3 -m venv .venv
source .venv/bin/activate

pip3 install ./weil_wallet
pip3 install ./weil_ai

---

## `weil_wallet`

Core SDK for signing and submitting transactions to WeilChain smart contracts.

### Private key wallets

A private key is a hex-encoded 32-byte secp256k1 key stored in a `.wc` file.

```python
from weil_wallet import PrivateKey, Wallet

# Load from a .wc file
pk = PrivateKey.from_file("private_key.wc")
wallet = Wallet(pk)

# Or from a hex string directly
pk = PrivateKey.from_hex("a1b2c3...")
wallet = Wallet(pk)
````

### Mnemonic wallets (BIP39 / BIP32)

`MnemonicWallet` derives multiple accounts from a 24-word mnemonic using the path `m/44'/9345'/0'/0/{index}`.

```python
from weil_wallet import MnemonicWallet, create_wallet, load_wallet

# Generate a new wallet (random 24-word mnemonic)
mnemonic_wallet = create_wallet()
print(mnemonic_wallet.mnemonic)

# Restore from an existing mnemonic
mnemonic_wallet = create_wallet(mnemonic="word1 word2 ... word24")

# Derive an account at a given index
account = mnemonic_wallet.derive_account(0)
print(account.address)   # 0x-prefixed Keccak-256 address

# Use the account with WeilClient
wallet = account.to_weil_wallet()

# Save and reload
mnemonic_wallet.store_wallet("my_wallet.wc")
mnemonic_wallet = load_wallet("my_wallet.wc")
```

### Executing contract methods

`WeilClient` is an async context manager that signs and submits transactions to a smart contract applet.

```python
import asyncio
from weil_wallet import PrivateKey, Wallet, WeilClient, ContractId

async def main():
    pk = PrivateKey.from_file("private_key.wc")
    wallet = Wallet(pk)
    contract_id = ContractId("aaaaaayvitmkip5jdz524cnavebftb5prmgjv32eq5ppvpaxdwgu2knxmu")

    async with WeilClient(wallet) as client:
        result = await client.execute(
            contract_id,
            method_name="audit",
            method_args='{"log": "hello from python"}',
            should_hide_args=True,
        )
        print(result.status)
        print(result.txn_result)

asyncio.run(main())
```

**`execute` parameters:**

| Parameter          | Type         | Description                                   |
| ------------------ | ------------ | --------------------------------------------- |
| `contract_id`      | `ContractId` | Target applet address                         |
| `method_name`      | `str`        | Exported method to call                       |
| `method_args`      | `str`        | JSON-encoded arguments                        |
| `should_hide_args` | `bool`       | Hide args on-chain                            |
| `is_non_blocking`  | `bool`       | Return immediately without waiting for result |

### Streaming responses

```python
async with WeilClient(wallet) as client:
    stream = await client.execute_with_streaming(contract_id, "my_method", '{}')
    async for chunk in stream:
        print(chunk)
```

### On-chain audit logging

```python
async with WeilClient(wallet) as client:
    # Direct call
    result = await client.audit('{"action": "user_login", "user": "alice"}')

    # Decorator — logs all arguments before the function runs
    @client.audit()
    async def handle_request(user: str, action: str):
        ...
```

### Binding a client to a single contract

```python
async with WeilClient(wallet) as client:
    contract_client = client.to_contract_client(contract_id)
    result = await contract_client.execute("my_method", '{"key": "value"}')
```

### API reference

| Symbol               | Description                                                         |
| -------------------- | ------------------------------------------------------------------- |
| `PrivateKey`         | Load a hex-encoded private key from a file or string                |
| `Wallet`             | secp256k1 signing wallet; produces 64-byte compact signatures       |
| `WeilClient`         | Async client; manages HTTP connection and concurrency (default: 64) |
| `WeilContractClient` | Per-contract client returned by `client.to_contract_client()`       |
| `ContractId`         | Base32 RFC 4648 contract address with shard-routing support         |
| `MnemonicWallet`     | BIP39/BIP32 hierarchical wallet                                     |
| `WalletAccount`      | Single derived account (private key, public key, address)           |
| `create_wallet`      | Generate or restore a `MnemonicWallet`                              |
| `load_wallet`        | Reload a wallet saved with `store_wallet()`                         |
| `TransactionResult`  | Result returned after a blocking `execute()` call                   |
| `TransactionStatus`  | Enum of transaction status codes                                    |
| `ByteStream`         | Async iterator over streamed response chunks                        |

---

## `weil_ai`

AI agent and MCP server integration for WeilChain.

### Wrapping an agent with a Weil identity

`WeilAgent` proxies any agent object (LangChain, CrewAI, OpenAI Agents SDK, AutoGen, LlamaIndex, …) and adds two methods without changing the original interface:

- `agent.get_auth_headers()` — signed HTTP headers for MCP transports
- `agent.audit(log)` — write a log entry to the on-chain auditor applet

```python
from weil_ai import WeilAgent

agent = WeilAgent(my_agent, private_key_path="private_key.wc")

# All original methods pass through unchanged
result = agent.run("What is 2 + 2?")

# Get signed headers for an MCP transport
headers = agent.get_auth_headers()

# Write an audit entry on-chain
agent.audit('{"method": "run", "query": "What is 2 + 2?"}')
```

### `@weil_agent` decorator

Decorate an agent factory function to automatically wrap its return value:

```python
import weil_ai

@weil_ai.agent("private_key.wc")
def create_agent():
    return MyAgent()

agent = create_agent()
agent.get_auth_headers()
agent.run("hello")
agent.audit("ran query")
```

Or pass a pre-built `Wallet`:

```python
from weil_wallet import PrivateKey, Wallet
import weil_ai

wallet = Wallet(PrivateKey.from_file("private_key.wc"))

@weil_ai.agent(wallet)
def create_agent():
    return MyAgent()
```

### HTTP request authentication

`build_auth_headers` / `verify_weil_signature` handle the four-header signing scheme used by MCP transports.

**Client side** — sign a request:

```python
from weil_wallet import PrivateKey, Wallet
from weil_ai import build_auth_headers

wallet = Wallet(PrivateKey.from_file("private_key.wc"))
headers = build_auth_headers(wallet)
# headers = {
#   "X-Wallet-Address": "...",
#   "X-Signature":      "...",
#   "X-Message":        '{"timestamp":"..."}',
#   "X-Timestamp":      "...",
# }
```

**Server side** — verify a request:

```python
from weil_ai import verify_weil_signature

ok = verify_weil_signature(
    wallet_address=request.headers["X-Wallet-Address"],
    signature_hex=request.headers["X-Signature"],
    message=request.headers["X-Message"],
    timestamp=request.headers["X-Timestamp"],
)
```

### MCP server with `weil_middleware` and `@secured`

`weil_middleware()` is a Starlette middleware that verifies the four auth headers on every `POST` request and stores the verified wallet address in a `ContextVar`. `@secured` enforces on-chain access control for individual tool handlers.

```python
from fastmcp import FastMCP
from weil_ai import secured, weil_middleware, current_wallet_addr

mcp = FastMCP("my-server")

@mcp.tool()
@secured("my_service::weil")   # checks key_has_purpose on-chain
async def protected_tool(query: str) -> str:
    addr = current_wallet_addr()   # wallet address of the caller
    return f"Hello {addr}"

# Attach middleware to the ASGI app
app = mcp.http_app(transport="streamable-http")
app.add_middleware(weil_middleware())
```

Run the server:

```bash
uvicorn my_server:app --host 0.0.0.0 --port 8000
```

### Stdio MCP server with audit logging

Use `@client.audit()` from `weil_wallet` to log every tool call on-chain before it executes:

```python
import mcp.server.stdio
import mcp.types as types
from mcp.server.lowlevel import Server
from weil_wallet import PrivateKey, Wallet, WeilClient

server = Server("my-server")
client = WeilClient(Wallet(PrivateKey.from_file("private_key.wc")))

@server.call_tool()
@client.audit()
async def call_tool(name: str, arguments: dict):
    ...
```

### API reference

| Symbol                  | Description                                                                     |
| ----------------------- | ------------------------------------------------------------------------------- |
| `WeilAgent`             | Proxy wrapper that adds `get_auth_headers()` and `audit()` to any agent         |
| `weil_agent` / `agent`  | Decorator factory for agent factory functions                                   |
| `build_auth_headers`    | Build the four signed HTTP auth headers from a `Wallet`                         |
| `verify_weil_signature` | Verify the four auth headers (server-side)                                      |
| `weil_middleware`       | Starlette middleware class that verifies headers and sets the wallet ContextVar |
| `current_wallet_addr`   | Read the verified wallet address for the current request                        |
| `secured`               | Decorator factory that enforces on-chain access control for MCP tools           |

---

## Examples

| File                          | Description                                           |
| ----------------------------- | ----------------------------------------------------- |
| `examples/example.py`         | Basic wallet setup and contract execution             |
| `examples/example_derived.py` | Mnemonic wallet derivation and account usage          |
| `examples/langchain.py`       | LangChain agent wrapped with `@weil_ai.agent`         |
| `examples/claude_mcp.py`      | Stdio MCP server with on-chain audit logging          |
| `examples/mcp_server.py`      | HTTP MCP server with `weil_middleware` and `@secured` |
| `examples/mcp_client.py`      | MCP client using signed auth headers                  |
| `examples/audit_example.py`   | Direct audit log submission                           |
| `examples/agent_trace.py`     | Agent tracing example                                 |
| `examples/verify_me.py`       | Standalone signature verification                     |
