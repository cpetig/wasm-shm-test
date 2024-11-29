# Wasm shared memory proof of concept

This shows that six shared memory primitiv functions which are
compatible with plain wasm can also apply for the component model
(up to WASI 0.3).

The functions are defined by WIT but don't form a composable interface, 
because they can only be implemented by the host - as they require
access to the guest memory.

Module "test:shm/exchange" (wat style) in order of typical invocation

 - "[constructor]memory" (func (param size:i32) (result id:i32))
 - "[method]memory.minimum-size" (func (param id:i32) (result alloc_size:i32))
 - "[method]memory.add-storage" (func (param id:i32 addr:i32 size:i32 resultptr:i32))
 - "[method]memory.attach" (func (param id:i32 flags:i32 resultptr:i32))
 - "[method]memory.detach" (func (param id:i32 consumed:i32))
 - "[resource-drop]memory" (func (param id:i32))

or more readable

| name | param | result |
| ---- | ----- | ------ |
| constructor | size: i32 | id: i32 |
| method minimum-size| id: i32 | alloc_size: i32|
|method add-storage| id: i32 addr: i32 size: i32 *resultptr*: i32|👈|
|method attach| id: i32 flags: i32 *resultptr*: i32|👈|
|method detach| id: i32 consumed: i32|-|
|drop| id: i32|-|

See shm.wit for more in depth explanations of the semantics.

Please note that the guest is providing storage inside its linear memory 
but can never control the address used, e.g. due to page alignment 
or other MMU/MPU restrictions.

## Inside this example

The main component allocates a shared buffer, passes it to a reactor (impl) 
which attaches the buffer, increments the specified byte and returns. Then
the main component attaches the buffer and verifies the requested increments.

These components are compiled as wasm components and the provided runtime is 
based on wasmtime, and provides the shared memory primitives.

Caveat: This implementation isn't checking the semantic behavior 
(exclusive attach) and abuses wasmtime internals. 
**Don't plan to use it in production!**