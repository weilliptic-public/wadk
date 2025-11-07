echo Build
cargo build --target wasm32-unknown-unknown --release

echo '--widl-file'  `pwd`/B.widl '--file-path '`pwd`/target/wasm32-unknown-unknown/release/B.wasm
