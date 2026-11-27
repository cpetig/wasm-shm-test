# wasm (component model, canonical ABI) is the default build,
# `make native` builds the optional symmetric ABI variant

RUNTIME := runtime/target/debug/runtime
EX := examples/target/wasm32-wasip3/debug
NATIVE := --no-default-features -F symmetric

.PHONY: all runtime minimal pub-sub complex partition run-minimal run-pub-sub run-complex run-partition native run-native clean

all: runtime minimal pub-sub complex partition

runtime:
	cd runtime && cargo build

minimal:
	cd examples && cargo build --target wasm32-wasip3 -p main -p implementation
	wasm-tools compose -o $(EX)/minimal.wasm $(EX)/main.wasm -d $(EX)/implementation.wasm

pub-sub:
	pub-sub/build-wasm.sh

complex: pub-sub
	examples/complex/build-wasm.sh

# the unused pub-sub import of wasm-shm is dropped by the linker, no composition needed
partition:
	cd examples && cargo build --target wasm32-wasip3 -p partition

run-minimal: runtime minimal
	$(RUNTIME) $(EX)/minimal.wasm

run-pub-sub: runtime pub-sub
	$(RUNTIME) pub-sub/target/wasm32-wasip3/debug/combined.wasm

run-complex: runtime complex
	$(RUNTIME) $(EX)/complex.wasm

run-partition: runtime partition
	$(RUNTIME) $(EX)/partition.wasm

# native build option: shared libraries using the symmetric ABI, no wasm runtime involved
native:
	cd pub-sub && cargo build --target-dir target/native --workspace $(NATIVE)
	cd examples && cargo build --target-dir target/native -p starter -p partition $(NATIVE)

run-native:
	cd pub-sub && cargo run --target-dir target/native -p subscriber $(NATIVE)
	cd examples && cargo run --target-dir target/native -p starter $(NATIVE)
	cd examples && cargo run --target-dir target/native -p partition $(NATIVE)

clean:
	cd runtime && cargo clean
	cd pub-sub && cargo clean
	cd examples && cargo clean
