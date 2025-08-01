#!/bin/sh
(cd src; ../../../../wit-bindgen/target/debug/wit-bindgen rust ../../../wit -w client --symmetric --link-name symmetric_sharedmem --format && mv client.rs client_symmetric.rs)
(cd src; ../../../../wit-bindgen/target/debug/wit-bindgen rust ../../../wit -w client --format && mv client.rs client_wasm.rs)
