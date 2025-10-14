#!/bin/sh
(cd ../examples/minimal/impl; cargo build --target wasm32-wasip2)
(cd ../examples/minimal/main; cargo build --target wasm32-wasip2)
wasm-tools compose -o combined.wasm main.wasm -d implementation.wasm
