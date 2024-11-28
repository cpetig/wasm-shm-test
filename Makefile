all:
	(cd impl; cargo build --target wasm32-wasip2)
	(cd main; cargo build --target wasm32-wasip2)
	(cd runtime; wasm-tools compose main.wasm -d implementation.wasm -o combined.wasm)
	(cd runtime; cargo build)
