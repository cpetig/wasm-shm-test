# Wasm shared memory proof of concept

This shows that a handful of shared memory primitives, compatible with plain
wasm, also work for the component model (WASI 0.2 and 0.3). Large buffers
(images, tensors, …) can then be passed between components without copying.

The primitives are defined in WIT (`wit/shm.wit`, interface `test:shm/exchange`),
but they don't form a composable interface: only the host can implement them,
because they need access to the guest's linear memory.

See [DESIGN.md](DESIGN.md) for the ideas behind it, and how this could map to
a canonical ABI option.

## The `exchange` interface

Core functions of `test:shm/exchange`, in the order of a typical invocation:

| function                            | params                 | result    |
| ----------------------------------- | ---------------------- | --------- |
| `[constructor]memory-block`         | size                   | id        |
| `[method]memory-block.minimum-size` | id                     | alloc size |
| `[static]memory-block.add-storage`  | addr, size, *resultptr* | via resultptr |
| `[method]memory-block.attach`       | id, flags, *resultptr* | via resultptr |
| `[method]memory-block.detach`       | id, consumed           | –         |
| `[resource-drop]memory-block`       | id                     | –         |

The guest provides storage inside its linear memory (`add-storage`), but it
never controls the address used, e.g. because of page alignment or other
MMU/MPU restrictions. Attaching maps the block into that storage: exclusive
for writing, shared for reading.

Convenience additions:

- `clone`: broadcast a read-only buffer to multiple consumers
- `optimum-size`: buffer-independent allocation size for several simultaneous
  attachments with minimal overhead
- `create-partition`: hand out sub-regions (for allocator components)
- `create-local`: wrap local memory in a memory block, bound to one linear
  memory (likely superseded by `create-partition`)

## Layout

| directory | contents |
| --------- | -------- |
| `wit/` | `shm.wit`: the `exchange` and `pub-sub` interfaces, and the worlds of the examples |
| `runtime/` | wasmtime based host implementing `test:shm/exchange` via `mmap`, runs a composed component |
| `examples/minimal/` | `main` allocates a buffer, `impl` increments a byte in it, `main` checks the result |
| `pub-sub/` | publisher/subscriber component that passes memory blocks over streams (zero copy) |
| `pub-sub/rust-client/` | crate `wasm-shm`: Rust bindings for using `exchange` and `pub-sub` |
| `pub-sub/wasi-clocks/` | Rust bindings for importing `wasi:clocks` 0.3 |
| `pub-sub/test/` | `publisher` publishes 20 integers, `subscriber` reads them through two subscriptions |
| `examples/complex/` | publishes a `list<string>` to two subscribers using [flat data types](https://github.com/cpetig/flat-types-rust), never copying the contents after the initial (mostly in-place) creation |
| `wit-bindgen/` | submodule: fork of wit-bindgen (WASI 0.3 bindings, plus the native build option) |
| `flat-types-rust/` | submodule: the flat data types used by the complex example |

## Building and running

Prerequisites: a nightly Rust toolchain with the `wasm32-wasip2` target,
[`wasm-tools`](https://github.com/bytecodealliance/wasm-tools), and the
submodules (`git submodule update --init --recursive`).

```sh
make run-minimal     # prints "... = 3"
make run-pub-sub     # two subscribers receive values, one per second
make run-complex
```

`make` builds everything. Each step on its own:

- runtime: `cd runtime && cargo build`; run a component with
  `runtime/target/debug/runtime <component.wasm>`
- minimal example: `cd examples && cargo build --target wasm32-wasip2 -p main -p implementation`,
  then compose `main.wasm` with `implementation.wasm` (see `Makefile`)
- pub-sub: `pub-sub/build-wasm.sh` builds and composes `pub-sub/target/wasm32-wasip2/debug/combined.wasm`
- complex example: `examples/complex/build-wasm.sh` (also builds the pub-sub component)

Components that wait on streams can't block inside the synchronous
`wasi:cli/run` 0.2 that Rust's `main` becomes. The pub-sub subscriber and the
complex starter therefore export the asynchronous WASI 0.3 `wasi:cli/run`
instead (`no_main` plus `generate!`), and the runtime prefers that export.

Caveat: the runtime doesn't check the semantic rules (e.g. exclusive attach)
and relies on wasmtime internals. **Don't use it in production!**

## Native build option (symmetric ABI)

All crates can also be built as native shared libraries that call each other
directly, using the
[symmetric ABI](https://github.com/WebAssembly/component-model/issues/386)
of the wit-bindgen fork (see `wit-bindgen/crates/cpp/DESIGN.md`) instead of
the component model. The same source code
compiles for both. In this mode `pub-sub` also implements `test:shm/exchange`
(in-process, no `mmap`), and `pub-sub/wasi-clocks/native` provides `wasi:clocks`.
No wasm runtime is involved.

The features `canonical` (default) and `symmetric` are mutually exclusive:

```sh
cd pub-sub && cargo run -p subscriber --no-default-features -F symmetric
cd examples && cargo run -p starter --no-default-features -F symmetric
```

or `make native` / `make run-native`. Use `cargo run`: it sets the library path
for the shared libraries, while running a binary directly needs
`LD_LIBRARY_PATH`.

## Limitations and ideas

- `publish` doesn't wait for slow subscribers: a write that can't complete
  immediately is dropped. Blocking attachments are a planned extension.
- The complex example can be a foundation for
  [caller provided buffers](https://github.com/WebAssembly/component-model/issues/369),
  see also [this issue](https://github.com/WebAssembly/component-model/issues/398).
- The generated bindings checked in under `pub-sub/rust-client/src` and
  `pub-sub/wasi-clocks/src` only ease debugging, `generate!` works equally
  well. Regenerate them with the `generate.sh` scripts.
