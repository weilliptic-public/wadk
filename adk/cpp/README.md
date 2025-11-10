## Compiling C++ code to WASM module

```
clang++ --target=wasm32 -emit-llvm -c -S <FILENAME>.cpp
llc -march=wasm32 -filetype=obj <FILENAME>.ll
wasm-ld --no-entry --export-all -o <FILENAME>.wasm <FILENAME>.o
```

```
clang++ --target=wasm32-unknown-unknown -Wl,--no-entry -Wl,--export-all -o output.wasm add.cpp -Wl,--allow-undefined -nostdlib
```
