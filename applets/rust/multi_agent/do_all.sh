cargo b --target wasm32-unknown-unknown --release

echo '--widl-file'  `pwd`/*.widl '--file-path' `pwd`/target/wasm32-unknown-unknown/release/multi_agent.wasm