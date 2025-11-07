#/bin/bash

echo Build
cargo build --target wasm32-unknown-unknown --release

echo '--widl-file'  `pwd`/cross_counter.widl '--file-path '`pwd`/target/wasm32-unknown-unknown/release/cross_counter.wasm
