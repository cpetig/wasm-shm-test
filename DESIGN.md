## More high-level description 

- Shared memory objects are managed by the host and map to a resource handle.
- The guest can map pass a region of its *linear* *memory* to the host for 
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
