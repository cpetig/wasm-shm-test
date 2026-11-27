# wasm (component model, canonical ABI) is the default build,
# `make native` builds the optional symmetric ABI variant

RUNTIME := runtime/target/debug/runtime
EX := examples/target/wasm32-wasip3/debug
NATIVE := --no-default-features -F symmetric

.PHONY: all runtime minimal pub-sub complex run-minimal run-pub-sub run-complex native run-native clean

all: runtime minimal pub-sub complex

runtime:
	cd runtime && cargo build

minimal:
	cd examples && cargo build --target wasm32-wasip3 -p main -p implementation
	wasm-tools compose -o $(EX)/minimal.wasm $(EX)/main.wasm -d $(EX)/implementation.wasm

pub-sub:
	pub-sub/build-wasm.sh

complex: pub-sub
	examples/complex/build-wasm.sh

run-minimal: runtime minimal
	$(RUNTIME) $(EX)/minimal.wasm

run-pub-sub: runtime pub-sub
	$(RUNTIME) pub-sub/target/wasm32-wasip3/debug/combined.wasm

run-complex: runtime complex
	$(RUNTIME) $(EX)/complex.wasm

# native build option: shared libraries using the symmetric ABI, no wasm runtime involved
native:
	cd pub-sub && cargo build --target-dir target/native --workspace $(NATIVE)
	cd examples && cargo build --target-dir target/native -p starter $(NATIVE)

run-native:
	cd pub-sub && cargo run --target-dir target/native -p subscriber $(NATIVE)
	cd examples && cargo run --target-dir target/native -p starter $(NATIVE)

clean:
	cd runtime && cargo clean
	cd pub-sub && cargo clean
	cd examples && cargo clean
