#!/bin/sh
# regenerate the checked in bindings (needs a built wit-bindgen CLI from the submodule)
BINDGEN=../../../wit-bindgen/target/debug/wit-bindgen
(cd src; $BINDGEN rust ../wit -w imports --format && mv imports.rs imports_wasm.rs)
(cd src; $BINDGEN rust ../wit -w imports --symmetric --link-name wasi_clocks_symmetric --format && mv imports.rs imports_symmetric.rs)
(cd native/src; ../$BINDGEN rust ../../wit -w exports --symmetric --format)
