//! Sub-partitions of memory blocks: the semantics of `create-partition`,
//! as assertions. Runs on wasm (`make run-partition`) and natively (`make run-native`).
use wasm_shm::{Address, AttachOptions, Error, MemoryArea, MemoryBlock};

const PAGE: u32 = 4096;

fn write() -> AttachOptions {
    AttachOptions::WRITE
}
fn read() -> AttachOptions {
    AttachOptions::empty()
}

/// address of an attached area as a pointer into our linear memory
fn ptr(area: MemoryArea) -> *mut u8 {
    area.addr.take_handle() as *mut u8
}

fn main() {
    // On wasm the host maps blocks into memory we donate beforehand, this is not needed
    // natively (the size is 0 there). Room for up to 3 simultaneous attachments of the
    // whole block.
    let size = 4 * PAGE;
    let storage = MemoryBlock::optimum_size(3, size) as usize;
    if storage > 0 {
        let layout = std::alloc::Layout::from_size_align(storage, 1).unwrap();
        let addr = unsafe { std::alloc::alloc(layout) };
        MemoryBlock::add_storage(MemoryArea {
            addr: unsafe { Address::from_handle(addr as usize as _) },
            size: storage as u32,
        })
        .unwrap();
    }

    // the parent block of 4 pages, the constructor gives read-write rights
    let block = MemoryBlock::new(size);

    // --- creating partitions: rights, overlap, range
    let p1 = block.create_partition(0, PAGE, write()).ok().unwrap(); // read-write
    println!("ok: rw partition p1 = [0, {PAGE})");
    let p2 = block.create_partition(PAGE, PAGE, read()).ok().unwrap(); // read-only
    println!("ok: ro partition p2 = [{PAGE}, {})", 2 * PAGE);
    // partitions of one block must not overlap while they live
    assert!(matches!(block.create_partition(2048, PAGE, write()), Err(Error::Overlap)));
    println!("ok: [2048, 6144) overlaps p1 and p2 -> overlap");
    assert!(matches!(block.create_partition(16000, 1000, write()), Err(Error::OutOfRange)));
    assert!(matches!(block.create_partition(0, 0, write()), Err(Error::OutOfRange)));
    println!("ok: [16000, 1000) exceeds the block, and size 0 -> out-of-range");

    // --- read-only handles
    assert!(matches!(p2.attach(write()), Err(Error::ReadOnly)));
    println!("ok: attach(write) on read-only p2 -> read-only");
    assert!(matches!(p2.create_partition(0, 100, write()), Err(Error::ReadOnly)));
    println!("ok: write partition of read-only p2 -> read-only");
    // partitions of partitions are fine, the no-overlap rule is per parent
    let nested = p2.create_partition(0, 100, read()).ok().unwrap();
    println!("ok: ro partition [0, 100) of p2 (nested)");
    // a clone keeps the rights of its handle
    let p2_clone = p2.clone();
    assert!(matches!(p2_clone.attach(write()), Err(Error::ReadOnly)));
    println!("ok: clone of read-only p2 is read-only as well");

    // --- every partition is a block of its own: write exclusive, read shared
    let p1_clone = p1.clone();
    let area = p1.attach(write()).ok().unwrap();
    assert!(area.size >= PAGE);
    let p1_ptr = ptr(area);
    let pattern: Vec<u8> = (0..200).map(|i| (i * 7 + 1) as u8).collect();
    unsafe { std::ptr::copy_nonoverlapping(pattern.as_ptr(), p1_ptr, pattern.len()) };
    // the clone shares the attach state with p1
    assert!(matches!(p1_clone.attach(read()), Err(Error::Busy)));
    println!("ok: p1 attached for write, a clone of p1 cannot attach(read) -> busy");
    p1.detach(pattern.len() as u32);
    let area = p1_clone.attach(read()).ok().unwrap();
    assert!(area.size as usize >= pattern.len()); // the valid bytes (everything on wasm)
    let read_ptr = ptr(area);
    assert_eq!(unsafe { std::slice::from_raw_parts(read_ptr, pattern.len()) }, &pattern[..]);
    // readers share
    let area2 = p1.attach(read()).ok().unwrap();
    assert!(matches!(p1.attach(write()), Err(Error::Busy)));
    p1.detach(0);
    p1_clone.detach(0);
    let _ = ptr(area2); // the address must be taken, dropping it would drop a handle
    println!("ok: after detach(n) the clone reads the pattern, readers share, a writer is busy");

    // --- a partition at an unaligned offset, data is shared with the parent
    let off = 3 * PAGE + 100;
    let p4 = block.create_partition(off, 1000, write()).ok().unwrap();
    let area = p4.attach(write()).ok().unwrap();
    assert!(area.size >= 1000);
    let p4_ptr = ptr(area);
    unsafe { p4_ptr.write(0xA5) };
    unsafe { p4_ptr.add(999).write(0x5A) };
    p4.detach(1000);
    // the parent works while its partitions exist (TODO in shm.wit: lock the range)
    let area = block.attach(read()).ok().unwrap();
    let base = ptr(area);
    assert_eq!(unsafe { base.add(off as usize).read() }, 0xA5);
    assert_eq!(unsafe { base.add(off as usize + 999).read() }, 0x5A);
    assert_eq!(unsafe { std::slice::from_raw_parts(base, pattern.len()) }, &pattern[..]);
    block.detach(0);
    println!("ok: marker written through p4 is visible in the parent at {off}, as is p1's pattern at 0");

    // --- the parent stays usable, even while a partition is attached for writing
    let part = p1.attach(write()).ok().unwrap();
    let whole = block.attach(write()).ok().unwrap();
    unsafe { ptr(whole).write(0x42) };
    assert_eq!(unsafe { ptr(part).read() }, 0x42);
    p1.detach(1);
    block.detach(1);
    println!("ok: parent attached for write while p1 is, both see the same bytes");

    // --- a range is free again when the last handle of its partition is gone
    assert!(matches!(block.create_partition(0, PAGE, read()), Err(Error::Overlap)));
    drop(p1);
    // one handle (the clone) is left
    assert!(matches!(block.create_partition(0, PAGE, read()), Err(Error::Overlap)));
    drop(p1_clone);
    let again = block.create_partition(0, PAGE, write()).ok().unwrap();
    println!("ok: [0, {PAGE}) is free after the last handle of p1 was dropped");

    // --- the partition keeps the storage alive after the parent is gone
    drop(block);
    let area = again.attach(read()).ok().unwrap();
    assert_eq!(unsafe { ptr(area).read() }, 0x42);
    again.detach(0);
    drop((p2, p2_clone, nested, p4, again));
    println!("ok: partition outlives the parent handle");

    println!("partition example passed");
}
