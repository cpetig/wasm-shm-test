#!/bin/sh
# regenerate the checked in bindings (needs a built wit-bindgen CLI from the submodule),
# the bindgen! macro works equally well, these files just ease debugging
BINDGEN=../../../wit-bindgen/target/debug/wit-bindgen
(cd src; $BINDGEN rust ../../../wit -w client --format && mv client.rs client_wasm.rs)
(cd src; $BINDGEN rust ../../../wit -w client --symmetric --link-name pub_sub --format && mv client.rs client_symmetric.rs)
