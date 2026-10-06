#/bin/bash

echo Building...
cargo build --target wasm32-unknown-unknown --release

echo '--widl-file'  `pwd`/publisher/publisher.widl '--file-path' /root/git/weil-applets/weilstream/target/wasm32-unknown-unknown/release/publisher.wasm
echo '--widl-file'  `pwd`/consumer/consumer.widl '--file-path' /root/git/weil-applets/weilstream/target/wasm32-unknown-unknown/release/consumer.wasm