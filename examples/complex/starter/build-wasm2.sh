#!/bin/sh

(cd ../consumer; cargo build --target wasm32-wasip1 --no-default-features -F canonical,combined)
(cd ../consumer ; wasm-tools component new --adapt ../../../wasi_snapshot_preview1.reactor.wasm target/wasm32-wasip1/debug/consumer.wasm -o component.wasm )
cargo build --target wasm32-wasip1 --no-default-features -F canonical
wasm-tools component new --adapt ../../../wasi_snapshot_preview1.command.wasm target/wasm32-wasip1/debug/starter.wasm -o component.wasm 
#wasm-tools compose  -o combined1.wasm component.wasm -d publisher.wasm  --skip-validation 
wasm-tools compose  -o combined2.wasm component.wasm -d consumer.wasm  --skip-validation 
wasm-tools compose  -o combined.wasm combined2.wasm -d ../../../symmetric/component.wasm  --skip-validation 
