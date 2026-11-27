# Design: shared memory blocks for the component model

This document collects the ideas behind the prototype: the semantics of memory
blocks, how they could map to WIT and the canonical ABI, and how they relate to
the other proposals for avoiding copies. For an introduction and the build
instructions see the [README](README.md).

## More high-level description 

- Shared memory objects are managed by the host and map to a resource handle.
- The guest can pass a region of its *linear* *memory* to the host for 
  mapping shared memory objects into its address space.
  - Due to page size limitations the needed address space might be larger,
    e.g. rounded up to a *page* *boundary* at both ends, so the guest should ask
    the necessary allocation amount for a specific mapped size
  - The host is free to answer zero to the allocation request and use a 
    fixed valid address, e.g. because an embedded CPU might only feature a
    memory protection unit (*MPU*), not a memory management unit (MMU)
- A write attachment receives the amount of unconsumed bytes from the last detach
  (publish) and communicates the amount of *valid* bytes at detach.
  - A read attachment receives the amount of valid bytes, the detach communicates
    the amount of *consumed* bytes (always zero for shared attach).
  - This enables defined *ownership* transfers via shared memory
- As exclusive attachment for write and shared attachment for read are
  managed by the host, the host can *emulate* missing mmap capabilities by 
  copying into and out of the linear memory at attach/detach time.
- For *performance* reasons the host can decide to map a larger portion of
  shared memory simultaneously into multiple guests and then only communicate
  address sections inside that region without changing the MMU. To well behaved
  guests the difference is not noticeable, ill-behaved guests aren't fully
  sandboxed and thus don't trap when accessing outside of the negotiated region.
- Another component can pass subregions into shared memory to other components.
  Thus, a guest component managing the allocation of shared memory is possible;
  the host is only providing the infrastructure not the logic. 
  - This includes directing DMA into these buffers using host primitives. Thus
    guest components can act as device drivers.

## Mapping to WIT (proposal)

- This is meant to enable transferring large objects across components with
  minimized copying overhead, e.g. images or tensors for WASI-NN.
- This should become another canonical option
- Short rule: Whenever linear memory is referenced, a shared memory handle 
  should be passed instead.
- Memory pointers in the canonical representation become shared memory handles.
  Size annotations are no longer necessary (list, string) because the buffer
  already has an associated size communicated at attach/detach time.
- Large input objects (beyond the flattened capacity) are passed in a shared
  memory buffer
- For output objects the caller provides a pre-allocated shared memory buffer
  as the last argument. The callee is free to return a different buffer, even
  returning the buffer at a later time, e.g. to enable zero-copy double buffering
  for write. If the buffer is too small the callee can either return a newly
  allocated buffer or an invalid handle.
- For returning shared read-only memory a `borrow<T>` annotation of the 
  function result makes most sense.
  - Similarly `borrow<T>` arguments indicate that the ownership of a buffer is not
    transferred. This feeds the need for a standardized way to duplicate memory handles.

### Argument flattening semantics (still sketchy idea)

- Each parameter should become its own memory block handle, if a bundle is
  intended simply place the parameters in a struct. This is undistinguishable for
  the normal canonical case, with shared memory it makes a difference.

## Ownership transfer in detail

The byte counts passed at `detach` turn a memory block into a channel with
defined ownership, not just a shared scratch area:

1. A producer attaches with `write`. This is exclusive: while it is attached,
   no other attachment succeeds (`busy`).
2. The producer fills the block and calls `detach(valid)`. The block now holds
   `valid` bytes of data, and the producer gives up access.
3. A consumer attaches for reading and sees exactly the valid region.
   - Without `shared`, the data is *moved*: `detach(consumed)` marks bytes as
     taken, and the next reader sees only the rest. A stream of messages can
     be drained piecewise this way.
   - With `shared` set by the producer, the data can be read any number of
     times, by several consumers at once (broadcast, see `clone`).
4. A later write attachment learns how much was left unconsumed, so a
   producer can append to, or recycle, a partially drained block.

Because the host sees every attach and detach, it always knows who may access
the contents. That is what makes copy-based emulation possible, and what
would let a runtime revoke mappings eagerly or lazily without the guests
noticing.

The in-process implementation of the native build (`pub-sub/src/lib.rs`)
follows steps 1 to 3; a write attachment there starts from an empty block
instead of reporting the unconsumed rest. The wasm runtime doesn't check the
protocol yet.

## Relation to lazy lowering and caller-provided buffers

[Lazy value lowering](https://github.com/WebAssembly/component-model/issues/383)
and [caller-provided buffers](https://github.com/WebAssembly/component-model/issues/369)
are the component model's planned replacement for `cabi_realloc`. They attack
the allocation half of the cost of passing data:

- With lazy lowering the callee receives a handle to a pending value instead of
  a pointer. It calls `canon value.lower` once it knows where the data should
  go: into an existing buffer, a ring, or nowhere if it turns out to be
  unneeded. The value is still copied, but exactly once and to a destination
  of the receiver's choice.
- With caller-provided buffers the caller decides where results land, which
  avoids allocations in the callee and suits I/O style APIs.

Memory blocks attack the copy itself. The data lives outside every linear
memory, and attaching only changes what is visible where. For large payloads
that travel through several components, the costs differ:

| | copies per hop | allocations per hop | receiver picks destination |
| - | - | - | - |
| today (`realloc`) | 1 | 1 | no |
| lazy lowering | 1 | 0 (receiver may reuse storage) | yes |
| caller-provided buffer | 1 | 0 | caller does |
| memory block handle | 0 | 0 (blocks are pooled) | not needed, data stays put |

The mechanisms compose:

- A memory block attached for writing is a natural *destination* for
  `canon value.lower`. A producer that can't build its data in place pays one
  copy into shared memory, and every later hop is free.
- The "output buffer as last argument" rule in
  [Mapping to WIT](#mapping-to-wit-proposal) is a caller-provided buffer that
  the callee may keep, forward or replace. With lazy lowering's terminology,
  the result is lowered into a buffer supplied by the caller, but ownership of
  that buffer can move with the result.
- Lazy lowering removes the reason to nest lists through `realloc`. A memory
  block plus a [flat representation](https://github.com/WebAssembly/component-model/issues/398)
  removes the reason to *lower* nested lists at all: the consumer reads them
  in place (see `examples/complex`).

So memory blocks wouldn't replace lazy lowering. They would cover the large,
long-lived payloads (frames, tensors, DMA buffers) where even a single copy
per hop dominates the cost.

## Implementation notes

- **Runtime (wasmtime, Linux).** Each block is an unlinked temporary file of
  the block's size. `add-storage` only records the donated region, per linear
  memory. `attach` maps the file with `mmap(MAP_FIXED | MAP_SHARED)` at the
  first page boundary inside that region, so a region needs the block size
  plus up to two pages (`minimum-size`). `detach` unmaps it. The runtime finds
  the guest's memory base by wrapping the lifted resource and area types,
  which uses wasmtime internals.
- **Without an MMU.** `minimum-size` and `optimum-size` may return 0. The
  guest then skips `add-storage`, and `attach` returns addresses the guest
  can already reach, e.g. a region the MPU opens for the duration of the
  attachment.
- **Native build.** In the symmetric ABI all components share one address
  space, so blocks are plain heap allocations and `attach` returns their
  address directly; storage sizes are 0.
- **The memory-control proposal.** Core wasm
  [memory-control](https://github.com/WebAssembly/memory-control) would give
  engines a supported way to map into linear memory, replacing the `mmap`
  behind the engine's back. [#568](https://github.com/WebAssembly/component-model/issues/568)
  discusses how the component model could expose such mappings.

## Outlook: ABI v2, lazy lowering and the GC option

Luke Wagner's component model update at the
[Bytecode Alliance Plumbers Summit, October 2026](https://www.youtube.com/watch?v=HBq9DgmCfPk)
places the related work on the roadmap. Timestamps are approximate.

- **ABI v2 is next** ([42:30](https://www.youtube.com/watch?v=HBq9DgmCfPk&t=2550s)).
  It will be an opt-in ABI option, rolled out incrementally and made the only
  option at 1.0. Lazy lowering is its core
  ([43:00](https://www.youtube.com/watch?v=HBq9DgmCfPk&t=2580s)): no more
  `cabi_realloc`, lazy values forwarded to imports without copying through
  the forwarder's memory, the size known before lowering (so a receiver can
  refuse oversized values), recursive values, and ABI options per value.
  Multi-value returns, error context and the stackful async ABI would come
  with it.
- **Caller-supplied return buffers**
  ([45:50](https://www.youtube.com/watch?v=HBq9DgmCfPk&t=2750s)). For
  dynamically sized results that aren't lists, the caller pre-fills the
  return area with a buffer pointer and length. The callee writes the result
  there if it fits, and falls back to a lazy value otherwise.
- **The Wasm GC ABI is on the list for 1.0**, after the next items
  ([47:00](https://www.youtube.com/watch?v=HBq9DgmCfPk&t=2820s)). It's
  motivated by Kotlin, Scala, Dart, Java and OCaml, which currently have to
  copy through linear memory. Weak handles, so that guest GCs can avoid
  ownership cycles, are discussed together with callbacks
  ([49:00](https://www.youtube.com/watch?v=HBq9DgmCfPk&t=2940s)).
- **`stream.forward`** splices a readable end into a writable end so the
  forwarding instance can be torn down early
  ([24:40](https://www.youtube.com/watch?v=HBq9DgmCfPk&t=1480s)).

What this means for memory blocks:

- **ABI v2 is the natural vehicle.** It is being designed now, as an opt-in
  ABI option, with feedback from the new W3C component model subgroup. A
  shared memory option fits the same opt-in, incremental model.
- **Memory blocks can make lazy values zero copy.** A receiver of a lazy
  value already chooses the destination and learns the size first. If the
  bytes behind the value are already in a host memory block (because the
  producer wrote them into an attached block), and the receiver lowers them
  into page-aligned storage, the host can *map* instead of copy, using a
  private copy-on-write mapping. Otherwise it copies. Guests can't tell the
  difference, so the read-only case needs no new WIT type. The explicit
  `exchange` API stays for what lazy values can't express: exclusive write
  access, ownership transfer of a buffer, and device or DMA buffers.
- **Caller-supplied return buffers resemble the "output buffer as last
  argument" rule** in [Mapping to WIT](#mapping-to-wit-proposal). The
  difference: there, ownership of the buffer can move. The callee may keep
  it, forward it, or return a different one (double buffering). That is a
  point worth raising in the ABI v2 discussion.
- **The GC option is a sibling, not a replacement.** It changes where values
  land (GC objects instead of linear memory), not whether they are copied
  between components. GC arrays are accessed through `array.get`/`array.set`,
  so they can't replace mappings into linear memory for C, C++ and Rust code,
  DMA, or targets without a GC. The useful alignment is at the WIT level: one
  buffer type, like the mappable type in
  [#568](https://github.com/WebAssembly/component-model/issues/568), should
  lower sensibly under the linear memory, GC and shared memory options alike.
  GC languages would use memory blocks as resources, wrapped with
  finalizers; weak handles help there too.

## Open questions

- **Drop while attached.** Should dropping the last handle of an attached
  block trap, defer until detach, or unmap immediately?
- **Enforcement cost.** Exclusive write attachment is cheap to check in the
  host. Revoking a mapping when a block is handed on (instead of trusting the
  guest to detach first) costs an MMU operation per hop.
- **Several memories.** With multi-memory a component has more than one linear
  memory. `add-storage` and `attach` would need to name the memory, which the
  canonical ABI's `memory` option could provide.
- **memory64.** `bytes` is `u32`; blocks larger than 4 GiB or 64-bit memories
  need a wider type. The component model spec now has rules for memory64
  (not implemented yet), which `add-storage` and `attach` should follow.
- **Alignment of contents.** Data written at one mapping address must be
  readable at another. Plain bytes and flat (relative pointer) encodings work;
  absolute pointers inside a block don't.
- **Threads.** With shared-everything threads several threads of one
  component could attach the same block. Read attachments generalize; write
  attachments need a per-thread or per-task owner.
- **Side channels.** Mutually distrusting components that read the same
  physical pages share caches. Copy-based emulation is the fallback where
  that matters.
