# Linux secure wallet backend

This directory contains the native implementation used by
`Wallet.from_wallet_file(..., secure=True)` and the lower-level raw-key
constructor.

The implementation is split into two layers:

- `weil_secure_wallet.c/.h` provide a CPython-independent opaque C ABI and can
  be built as `libweil_secure_wallet.so`.
- `python_binding.c` is the thin CPython adapter used by `weil_wallet`.

See `JAVA.md` for standalone CMake and Java binding guidance.

Build requirements are a C11 compiler, Python development headers, OpenSSL
development headers (`libssl-dev`), and libcurl development headers
(`libcurl4-openssl-dev` on Debian/Ubuntu). The extension is built only on Linux.

Secure Agent Registry loading performs the `/get_agent_wallet` HTTPS request
through libcurl in the native extension. The response is written directly into
a protected mapping and never enters HTTPX or Python's JSON parser. Configure
external wallet storage with `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`,
`AWS_REGION`, and `AWS_BUCKET_NAME` environment variables.

```python
@weil_ai.agent(
    api_key=os.environ["WEIL_API_KEY"],
    secure_wallet=True,
    require_lock=True,
    harden_process=True,
)
def create_agent():
    return MyAgent()
```

Install `libcurl4-openssl-dev` before rebuilding the editable package so the
native extension is relinked with libcurl.

The full wallet document is read and parsed in a temporary protected mapping.
Base58Check decoding and BIP32 derivation happen in native code; only public
account and organization metadata crosses into Python. The persistent key page
is surrounded by `PROT_NONE` guard pages, locked with
`mlock`, excluded from core dumps with `MADV_DONTDUMP`, and marked
`MADV_WIPEONFORK` when the running kernel supports it. Key input is read from a
file descriptor directly into that page and temporary native values are cleared
before release. `close()` explicitly wipes and unmaps the page.

OpenSSL necessarily creates short-lived internal scalar values while performing
secp256k1 operations. Production callers should therefore use
`harden_process=True`, which disables process core dumps in addition to securing
the persistent key allocation.

Secure full-wallet loading intentionally creates a snapshot of the wallet's
selected account. Its signer, address, public key, and active organization are
available, but Python does not receive the xprv, external private keys, or the
complete account list. Load a new secure snapshot to change accounts.

Build the standalone library with:

```bash
cmake -S weil_wallet/secure_native -B build/secure-wallet
cmake --build build/secure-wallet
```
