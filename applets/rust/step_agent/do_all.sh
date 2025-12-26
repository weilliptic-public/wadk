cargo b --release --target wasm32-unknown-unknown

echo '--widl-file'  `pwd`/*.widl '--file-path' `pwd`/target/wasm32-unknown-unknown/release/*.wasm