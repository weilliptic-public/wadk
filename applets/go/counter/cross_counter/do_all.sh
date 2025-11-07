#/bin/bash

#widl generate counter.widl server golang

echo Build
mkdir target
mkdir target/wasi
tinygo build -target wasi -o target/wasi/cross_counter.wasm

echo '--widl-file'  `pwd`/cross_counter.widl '--file-path '`pwd`/target/wasi/cross_counter.wasm 

cp `pwd`/cross_counter.widl /root/code/platform-scripts/contracts/examples/go
cp `pwd`/target/wasi/cross_counter.wasm /root/code/platform-scripts/contracts/examples/go

