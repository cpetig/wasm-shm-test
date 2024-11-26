use test::shm::exchange::Options;

wit_bindgen::generate!({
    path: "../wit/shm.wit",
    world: "main"
});

fn main() {
    let mem = test::shm::exchange::Memory::new(1024);
    test::shm::image::increment(&mem, 512);
    test::shm::image::increment(&mem, 512);
    test::shm::image::increment(&mem, 512);
    let addr = mem.attach(Options::empty());
    let addr2 = addr.addr as *const u8;
    dbg!(unsafe { addr2.byte_add(512).read() });
    mem.detach(0);
}
