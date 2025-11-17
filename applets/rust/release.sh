#!/bin/bash
set -euo pipefail

if [[ ! -v WASM_ROOT || ! -d "$WASM_ROOT" ]]; then
    echo "WASM_ROOT is not set or is not a directory"
    exit 1
fi

cargo build --target wasm32-unknown-unknown --release

echo Copying .wasm and .widl to $WASM_ROOT/rust
cp target/wasm32-unknown-unknown/release/*.wasm $WASM_ROOT/rust
find . |grep -E '\.widl' | xargs -I{}   cp {} $WASM_ROOT/rust

