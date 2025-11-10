contract_name=$(basename "$PWD")
git_root=$(git rev-parse --show-toplevel)
examples_path=$PLATFORM_SCRIPTS_ROOT/contracts/examples/assemblyscript

echo "$contract_name smart contract"
#echo "Installing dependencies…"
npm i

#echo "Building smart contract…"
npm run asbuild:release

widl=${PWD}/${contract_name}.widl
wasm=${PWD}/build/release.wasm

#echo "Deploy command:"
echo "deploy --widl-file $widl --file-path $wasm"

cp $widl ${examples_path}
cp $wasm "${examples_path}/${contract_name}.wasm"
