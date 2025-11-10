#/bin/bash

#widl generate yutaka.widl server rust

echo Build
cargo build --target wasm32-unknown-unknown --release

echo '--widl-file'  `pwd`/confluence.widl '--file-path '`pwd`/target/wasm32-unknown-unknown/release/confluence.wasm '--config-file' `pwd`/confluence.yaml 
