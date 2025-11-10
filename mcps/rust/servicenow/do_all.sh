#/bin/bash

echo Build
cargo build --manifest-path ./Cargo.toml --target wasm32-unknown-unknown --release

echo '-e --widl-file '  `pwd`/servicenow.widl ' --file-path '`pwd`/target/wasm32-unknown-unknown/release/servicenow.wasm ' -c ' `pwd`/config.yaml
