#!/bin/sh
(cd ../examples/minimal/impl; cargo build --target wasm32-wasip3)
(cd ../examples/minimal/main; cargo build --target wasm32-wasip3)
wasm-tools compose -o combined.wasm main.wasm -d implementation.wasm
