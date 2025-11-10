cargo b --target wasm32-unknown-unknown --release

echo '--widl-file'  `pwd`/sap_hana.widl '--file-path '`pwd`/target/wasm32-unknown-unknown/release/sap_hana.wasm
