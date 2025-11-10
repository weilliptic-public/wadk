cargo b --target wasm32-unknown-unknown --release

echo  '-e -c' `pwd`/config.yaml'--widl-file'  `pwd`/s3.widl '--file-path '`pwd`/target/wasm32-unknown-unknown/release/s3.wasm
