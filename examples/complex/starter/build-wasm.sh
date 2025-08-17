#!/bin/sh

(cd ../publisher; cargo build --target wasm32-wasip1 --no-default-features -F canonical)
(cd ../consumer; cargo build --target wasm32-wasip1 --no-default-features -F canonical)
cargo build --target wasm32-wasip1 --no-default-features -F canonical
