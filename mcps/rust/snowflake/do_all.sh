#/bin/bash

#widl generate snowflake.widl server rust

echo Build
cargo build --target wasm32-unknown-unknown --release

echo '--widl-file'  `pwd`/snowflake.widl '--file-path '`pwd`/target/wasm32-unknown-unknown/release/snowflake.wasm '--config-file' `pwd`/snowflake.yaml '--context-file' `pwd`/tpcds_sf10tcl_v2.txt
