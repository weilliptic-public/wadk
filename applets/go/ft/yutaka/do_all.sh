#/bin/bash

#widl generate yutaka.widl server golang

echo Build
mkdir target

tinygo build -target wasm-unknown -o target/yutaka.wasm -gc=precise

echo '--widl-file'  `pwd`/yutaka.widl '--file-path '`pwd`/target/yutaka.wasm 

cp `pwd`/yutaka.widl /root/code/platform-scripts/contracts/examples/go
cp `pwd`/target/yutaka.wasm /root/code/platform-scripts/contracts/examples/go
