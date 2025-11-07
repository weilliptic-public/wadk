
#!/bin/bash
set -euo pipefail

if [[ ! -v WASM_ROOT || ! -d "$WASM_ROOT" ]]; then
    echo "WASM_ROOT is not set or is not a directory"
    exit 1
fi


mkdir -p $WASM_ROOT/go

root_folder=`pwd`

for applet in slack ; do
	echo $applet
        cd $applet
        applet_name=$(basename "$applet")
        mkdir -p build
        tinygo build -target wasm-unknown -o build/$applet_name.wasm -gc=precise

	echo Copying .wasm and .widl to $WASM_ROOT/go/
	cp build/*.wasm $WASM_ROOT/go/
        cp *.widl $WASM_ROOT/go/

        cd $root_folder
done

