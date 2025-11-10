#/bin/bash
#widl generate aurora.widl server rust

echo Build
cargo build --manifest-path ./Cargo.toml --target wasm32-unknown-unknown --release

echo '--widl-file'  `pwd`/aurora.widl '--file-path '`pwd`/target/wasm32-unknown-unknown/release/aurora.wasm
