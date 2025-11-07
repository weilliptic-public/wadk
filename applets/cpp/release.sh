#!/bin/bash
set -euo pipefail

if [[ ! -v WASM_ROOT || ! -d "$WASM_ROOT" ]]; then
    echo "WASM_ROOT is not set or is not a directory"
    exit 1
fi


echo First you should do
echo cd ../../adk/cpp/weilsdk
echo compile.sh


root_folder=`pwd`

for applet in counter fungible_token/yutaka non_fungible_token/asciiart xpod/A xpod/B xpod_2/first xpod_2/second ; do
	echo $applet
        cd $applet
        mkdir -p build
        cd build
        emcmake cmake ..
        make

       echo Copying .wasm and .widl to $WASM_ROOT/rust
       cp *.wasm $WASM_ROOT/cpp
       cd ..
       cp *.widl $WASM_ROOT/cpp

       cd $root_folder
done

