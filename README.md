# Wasm shared memory proof of concept

This shows that six shared memory primitiv functions which are
compatible with plain wasm can also apply for the component model
(up to WASI 0.3).

The functions are defined by WIT but don't form a composable interface, 
because they can only be implemented by the host - as they require
access to the guest memory.

Module "test:shm/exchange" (wat style) in order of typical invocation

 - "[constructor]memory-block" (func (param size:i32) (result id:i32))
 - "[method]memory-block.minimum-size" (func (param id:i32) (result alloc_size:i32))
 - "[static]memory-block.add-storage" (func (param id:i32 addr:i32 size:i32 resultptr:i32))
 - "[method]memory-block.attach" (func (param id:i32 flags:i32 resultptr:i32))
 - "[method]memory-block.detach" (func (param id:i32 consumed:i32))
 - "[resource-drop]memory-block" (func (param id:i32))

or more readable

| name | param | result |
| ---- | ----- | ------ |
| constructor | size | id |
| method minimum-size| id | alloc_size|
|method add-storage| addr size *resultptr*|👈|
|method attach| id flags *resultptr*|👈|
|method detach| id consumed|-|
|drop| id|-|

See shm.wit for more in depth explanations of the semantics.

Please note that the guest is providing storage inside its linear memory 
but can never control the address used, e.g. due to page alignment 
or other MMU/MPU restrictions.

In addition there are some convenience additions:

 - clone: for broadcasting a read-only buffer to multiple consumers
 - optimum-size: calculate buffer independent recommended allocation size,
    supporting multiple attachments simultaneously with minimal overhead
 - create-local: wrap local memory in a memory-block,
    bound to one linear address space

## Minimal example

The `main` component allocates a shared buffer, passes it to a reactor (`impl`) 
which attaches the buffer, increments the specified byte and returns. Then
the main component attaches the buffer and verifies the requested increments.

These components are compiled as wasm components and the provided runtime is 
based on wasmtime, and provides the shared memory primitives.

Caveat: This implementation isn't checking the semantic behavior 
(exclusive attach) and abuses wasmtime internals. 
**Don't plan to use it in production!**

## Publisher subscriber

The `pub-sub` folder contains a publisher-subscriber setup which uses
memory blocks to achieve zero copy.

In there:

 - `src`: Publisher subscriber component using memory blocks provided by the host;
     also contains a memory block implementation for symmetric mode (native compilation)
 - `rust-client`: Import crate for the `pub-sub` component
 - `test/publisher`: Publishes 20 integers (linked into subscriber module),
    this uses WASI 0.3 clocks and streams
 - `test/subscriber`: Two subscription to published values

Full source code compatibility between native (symmetric ABI) and 
wasm (canonical ABI) requires a fork of wit-bindgen (included).

The generated code checked in at rust-client is just for debugging convenience,
the `bindgen!` macro works equally well.

## Complex example

`examples/complex` contains a complex example which uses 
[flat data types](https://github.com/cpetig/flat-types-rust) to publish a
`list<string>` to two subscribers - without ever copying the contents after
the initial (mostly in-place) buffer creation.

See [this issue](https://github.com/WebAssembly/component-model/issues/398)
for further discussion.

## Building

 - runtime: Simply `cargo build` in `runtime`
 - simple example: `cd runtime; ./build.sh; cargo run`
 - pub-sub native: `cd pub-sub/test/subscriber; cargo run`
 - pub-sub wasm: `cd pub-sub; ./build-wasi.sh; ../runtime/target/debug/runtime`
 - complex example: `cd pub-sub; ./build-wasi.sh; cd ../example/complex/starter ; ./build-wasm2.sh ; ../../../runtime/target/debug/runtime `
 - complex native: `cd example/complex/starter ; cargo run`

## TODO

 - On wasmtime the complex example built by `build-wasm.sh` ends in a deadlock, 
   while fusing consumer and publisher into one module fixes this somehow.
 - The wasi-clocks emulation for symmetric works around that async functions are
   not yet supported by symmetric (streams and futures work).
 - The complex example can create a foundation for 
   [caller provided buffers](https://github.com/WebAssembly/component-model/issues/369).
