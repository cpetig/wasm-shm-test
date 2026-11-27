#!/bin/sh
# build the complex example for wasm32-wasip2 (canonical ABI) and compose
# starter + publisher + consumer + pub-sub into one component
set -e
cd "$(dirname "$0")"
# compose is chatty (deprecation notice, "will be imported" warnings),
# only show its output when it fails
compose() {
    log=$(wasm-tools compose --skip-validation "$@" 2>&1) || { echo "$log" >&2; exit 1; }
}
ROOT=$(pwd)/../..
# the pub-sub component
"$ROOT/pub-sub/build-wasm.sh" >/dev/null
PUBSUB=$ROOT/pub-sub/target/wasm32-wasip2/debug/pub-sub.wasm
(cd .. && cargo build --target wasm32-wasip2 -p starter -p publisher -p consumer)
T=../target/wasm32-wasip2/debug
OUT=$T/complex.wasm
# validation is skipped: the starter exports async wasi:cli/run
compose -o $T/c1.wasm $T/starter.wasm -d $T/publisher.wasm
compose -o $T/c2.wasm $T/c1.wasm -d $T/consumer.wasm
compose -o $OUT $T/c2.wasm -d "$PUBSUB"
echo "run with: $ROOT/runtime/target/debug/runtime $(cd $T && pwd)/complex.wasm"
