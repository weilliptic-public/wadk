#/bin/bash

echo Build
cargo build --target wasm32-unknown-unknown --release

echo '--widl-file'  `pwd`/counter.widl '--file-path '`pwd`/target/wasm32-unknown-unknown/release/counter.wasm
