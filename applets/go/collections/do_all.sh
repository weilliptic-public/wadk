#/bin/bash

#widl generate counter.widl server golang

echo Build
mkdir target
mkdir target/wasi
tinygo build -target wasi -o target/wasi/collections.wasm

echo '--widl-file'  `pwd`/`ls *.widl` '--file-path '`pwd`/`ls target/wasi/*.wasm`


