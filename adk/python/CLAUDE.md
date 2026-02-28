# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
# Install all dependencies (includes agent/MCP extras)
pip install -r requirements.txt

# Install core SDK as editable package
pip install -e .

# Install with LangChain agent support
pip install -e ".[agents]"

# Run examples
python examples/example.py
python examples/audit_example.py
python examples/example_derived.py
python examples/langchain.py
python examples/claude_mcp.py
```

There are **no automated tests** and **no linting configuration** in this SDK.

## Architecture

### Transaction Flow

```
PrivateKey(.wc) / MnemonicWallet
    → Wallet (secp256k1 ECDSA via coincurve, SHA-256 hash, 64-byte compact sig)
    → WeilContractClient._sign_and_construct_txn()
        - nonce = current time in ms
        - canonical JSON: recursive key sort via value_to_sorted_dict()
        - sign({from_addr, nonce, to_addr, user_txn{type,address,method,args,hide_args}})
    → PlatformApi.submit_transaction()
        - GZIP-compress payload dict
        - POST /contracts/execute_smartcontract (multipart form-data)
        - x-non-blocking header for fire-and-forget
    → Sentinel node (https://sentinel.unweil.me)
    → TransactionResult
```

### Package Layout (`weil_wallet/`)

- **`client.py`** — `WeilClient` (async context manager, semaphore-limited to 64 concurrent requests, `audit()` decorator/method, `wallet_addr()`); `WeilContractClient` (per-contract binding, constructs and signs txns)
- **`wallet.py`** — `PrivateKey` (hex-encoded, loads from `.wc` files); `Wallet` (signs, returns 64-byte r‖s compact signature matching Rust libsecp256k1)
- **`derived_wallet.py`** — `MnemonicWallet` (BIP39/BIP32, derivation path `m/44'/9345'/0'/0/{index}`); `create_wallet()` / `load_wallet()`; `pubkey_to_derived_address()` (Keccak-256 of uncompressed pubkey, last 20 bytes, 0x-prefixed — different from `get_address_from_public_key()` in `utils.py` which uses SHA-256)
- **`contract.py`** — `ContractId` (base32 RFC 4648 lowercase, 36 decoded bytes; `pod_counter()` reads first 4 bytes as big-endian i32 for shard routing)
- **`api/platform_api.py`** — `PlatformApi` static methods for blocking, non-blocking, and streaming HTTP submission
- **`api/request.py`** — `SubmitTxnRequest`, `UserTransaction`, `Transaction`, `Verifier` dataclasses
- **`transaction.py`** — `TransactionResult`, `TransactionHeader`, `BaseTransaction`, `TransactionStatus` enum
- **`utils.py`** — `value_to_sorted_dict()` (canonical JSON), `hash_sha256()`, `get_address_from_public_key()`, `compress()` (GZIP)
- **`constants.py`** — `SENTINEL_HOST = "https://sentinel.unweil.me"`, `DEFAULT_CONCURRENCY = 64`

### Agent Integration (`agents/weil.py`)

- **`WeilAgent`**: proxy wrapper around any agent; delegates all attribute access to the wrapped agent; adds `audit(log: str) -> TransactionResult` which calls the on-chain "auditor" applet synchronously via `asyncio.run()`
- **`@weil_agent`**: decorates agent factory functions — the factory returns a `WeilAgent`-wrapped instance; accepts `private_key_path=`, `wallet=`, `sentinel_host=`
- **`@weilagent`**: decorates agent classes — the constructor returns a `WeilAgent`-wrapped instance
- **`weil_audit(client_factory, agent_name)`**: decorator for MCP `call_tool` handlers; intercepts `"audit_log"` tool calls and submits a structured on-chain entry before calling through

The "auditor" applet is resolved by name (`"auditor"`) at runtime and its `ContractId` is cached on `WeilClient`.

### Key Design Notes

- **Two address schemes:** `get_address_from_public_key()` (SHA-256 of uncompressed key → hex) is used for standard wallets; `pubkey_to_derived_address()` (Keccak-256 last-20-bytes → 0x-prefixed) is used for BIP32-derived accounts. These are incompatible — don't mix them.
- **Canonical signing:** Fields must be sorted recursively before JSON serialization. The canonical order for the top-level signing object is `from_addr → nonce → to_addr → user_txn`, and within `user_txn`: `type → contract_address → contract_method → contract_input_bytes → should_hide_args`.
- **`should_hide_args`:** When `True`, the transaction is submitted as non-blocking (fire-and-forget); when `False`, it blocks for a `TransactionResult`.
- **Private key auto-discovery:** `agents/weil.py` searches for `private_key.wc` in CWD, `python/`, then `examples/` directory.
- **Build system:** `weil_wallet/pyproject.toml` defines the package; the top-level `requirements.txt` pins all transitive dependencies for reproducible installs.
