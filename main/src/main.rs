use test::shm::exchange::{AttachOptions, MemoryArea};

wit_bindgen::generate!({
    path: "../wit/shm.wit",
    world: "main"
});

fn main() {
    let mem = test::shm::exchange::Memory::new(1024);

    test::shm::image::increment(&mem, 512);
    test::shm::image::increment(&mem, 512);
    test::shm::image::increment(&mem, 512);

    let layout = std::alloc::Layout::from_size_align(mem.minimum_size() as usize, 1).unwrap();
    if layout.size() > 0 {
        let addr = unsafe { std::alloc::alloc(layout) };
        mem.add_storage(MemoryArea {
            addr: addr as usize as u32,
            size: layout.size() as u32,
        })
        .unwrap();
    }
    let addr = mem.attach(AttachOptions::empty()).unwrap();
    let addr2 = addr.addr as *const u8;
    dbg!(unsafe { addr2.byte_add(512).read() });
    mem.detach(0);
}
