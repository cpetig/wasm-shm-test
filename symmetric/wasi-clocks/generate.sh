#!/bin/sh
(cd src; ../../../wit-bindgen/target/debug/wit-bindgen rust ../wit -w exports --symmetric --format)
(cd import/src; ../../../../wit-bindgen/target/debug/wit-bindgen rust ../../wit -w imports --symmetric --link-name wasi-clocks-symmetric --format && mv imports.rs imports_symmetric.rs)
(cd import/src; ../../../../wit-bindgen/target/debug/wit-bindgen rust ../../wit -w imports --format && mv imports.rs imports_wasm.rs)
