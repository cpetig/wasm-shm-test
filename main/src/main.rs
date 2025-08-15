use test::shm::exchange::{Address, AttachOptions, Memory, MemoryArea};

wit_bindgen::generate!({
    path: "../wit/shm.wit",
    world: "main"
});

fn main() {
    let layout =
        std::alloc::Layout::from_size_align(Memory::optimum_size(1, 1024) as usize, 1).unwrap();
    if layout.size() > 0 {
        let addr = unsafe { std::alloc::alloc(layout) };
        Memory::add_storage(MemoryArea {
            addr: unsafe { Address::from_handle((addr as usize).try_into().unwrap()) },
            size: layout.size() as u32,
        })
        .unwrap();
    }

    let mem = Memory::new(1024);

    test::shm::image::increment(&mem, 512);
    test::shm::image::increment(&mem, 512);
    test::shm::image::increment(&mem, 512);

    let addr = mem.attach(AttachOptions::empty()).unwrap();
    let addr2 = addr.addr.take_handle() as *const u8;
    dbg!(unsafe { addr2.byte_add(512).read() });
    mem.detach(0);
}
