#/bin/bash

#widl generate counter.widl server golang

echo Build
mkdir target
mkdir target/wasi
tinygo build -target wasi -o target/wasi/counter.wasm

echo '--widl-file'  `pwd`/counter.widl '--file-path '`pwd`/target/wasi/counter.wasm 

cp `pwd`/counter.widl /root/code/platform-scripts/contracts/examples/go
cp `pwd`/target/wasi/counter.wasm /root/code/platform-scripts/contracts/examples/go

