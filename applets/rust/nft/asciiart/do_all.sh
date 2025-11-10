#/bin/bash

echo Build
cargo build --target wasm32-unknown-unknown --release

echo '--widl-file'  `pwd`/asciiart.widl '--file-path '`pwd`/target/wasm32-unknown-unknown/release/asciiart.wasm
