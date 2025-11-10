#/bin/bash

#widl generate asciiart.widl server go

echo Build
mkdir target

tinygo build -target wasm-unknown -o target/asciiart.wasm -gc=precise

echo '--widl-file'  `pwd`/asciiart.widl '--file-path '`pwd`/target/asciiart.wasm 

cp `pwd`/asciiart.widl /root/code/platform-scripts/contracts/examples/go
cp `pwd`/target/asciiart.wasm /root/code/platform-scripts/contracts/examples/go
