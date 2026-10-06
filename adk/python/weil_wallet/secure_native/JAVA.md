# Java integration

`libweil_secure_wallet.so` has a CPython-independent C ABI. Java can bind it
with JNI, JNA, or the Foreign Function and Memory API (Panama). The Python
extension uses the same ABI through `python_binding.c`.

Build the standalone library on Linux:

```bash
sudo apt-get install build-essential cmake libssl-dev libcurl4-openssl-dev
cmake -S weil_wallet/secure_native -B build/secure-wallet
cmake --build build/secure-wallet
```

The output is `build/secure-wallet/libweil_secure_wallet.so`. Install it and its
header when required:

```bash
sudo cmake --install build/secure-wallet
```

The ABI uses an opaque `weil_secure_wallet *`. A binding should:

1. Call `weil_secure_harden_process` before loading secrets.
2. Construct a handle with `weil_secure_wallet_from_wallet_fd`,
   `weil_secure_wallet_from_key_fd`, or `weil_secure_wallet_from_api`.
3. Use `weil_secure_wallet_sign_digest` and
   `weil_secure_wallet_public_key` without exposing the handle address as an
   application-level value.
4. Copy only `weil_secure_metadata`, which contains public information.
5. Always call `weil_secure_wallet_free`, preferably from both explicit
   `AutoCloseable.close()` and a `Cleaner` fallback.

Call `weil_secure_wallet_abi_version()` before using the library. Version `1`
is the ABI defined in `weil_secure_wallet.h`.

For Panama or JNA, model the wallet handle as an opaque pointer and the error
argument as a caller-owned 256-byte buffer. JNI wrappers should store the handle
in a private `long` field and validate that it has not already been closed.
