use test::shm::exchange::{Address, AttachOptions, Memory, MemoryArea};

wit_bindgen::generate!({
    path: "../wit/shm.wit",
    world: "main"
});

fn main() {
    const MEMSIZE: u32 = 1024;
    const WRITEPOS: u32 = 512;
    let layout =
        std::alloc::Layout::from_size_align(Memory::optimum_size(1, MEMSIZE) as usize, 1).unwrap();
    if layout.size() > 0 {
        let addr = unsafe { std::alloc::alloc(layout) };
        Memory::add_storage(MemoryArea {
            addr: unsafe { Address::from_handle((addr as usize).try_into().unwrap()) },
            size: layout.size() as u32,
        })
        .unwrap();
    }

    let mem = Memory::new(MEMSIZE);

    test::shm::image::increment(&mem, WRITEPOS);
    test::shm::image::increment(&mem, WRITEPOS);
    test::shm::image::increment(&mem, WRITEPOS);

    let addr = mem.attach(AttachOptions::empty()).unwrap();
    let addr2 = addr.addr.take_handle() as *const u8;
    dbg!(unsafe { addr2.byte_add(WRITEPOS as usize).read() });
    mem.detach(0);
}
