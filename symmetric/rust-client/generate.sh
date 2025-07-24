#!/bin/sh
(cd src; ../../../../wit-bindgen/target/debug/wit-bindgen rust ../../../wit -w client --symmetric --link-name symmetric_sharedmem --format)
