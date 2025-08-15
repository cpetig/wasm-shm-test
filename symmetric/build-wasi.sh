#!/bin/sh
(cd test/subscriber ; cargo build --target wasm32-wasip1 --no-default-features -F canonical )
(cd test/subscriber ; wasm-tools component new --adapt ../../../wasi_snapshot_preview1.command.wasm target/wasm32-wasip1/debug/subscriber.wasm -o component.wasm )
cargo build --target wasm32-wasip1 --no-default-features -F canonical
wasm-tools component new --adapt ../wasi_snapshot_preview1.reactor.wasm target/wasm32-wasip1/debug/symmetric_sharedmem.wasm -o component.wasm
# this needs async, so skip validation for now
wasm-tools  compose  -o combined.wasm test/subscriber/component.wasm -d component.wasm --skip-validation 
