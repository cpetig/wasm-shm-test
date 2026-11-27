#!/bin/sh
# build the pub-sub test for wasm32-wasip3 (canonical ABI) and compose subscriber + pub-sub
set -e
cd "$(dirname "$0")"
# compose is chatty (deprecation notice, "will be imported" warnings),
# only show its output when it fails
compose() {
    log=$(wasm-tools compose --skip-validation "$@" 2>&1) || { echo "$log" >&2; exit 1; }
}
cargo build --target wasm32-wasip3
T=target/wasm32-wasip3/debug
cp $T/pub_sub.wasm $T/pub-sub.wasm
# validation is skipped: the subscriber exports async wasi:cli/run
compose -o $T/combined.wasm $T/subscriber.wasm -d $T/pub-sub.wasm
echo "run with: $(pwd)/../runtime/target/debug/runtime $(pwd)/$T/combined.wasm"
