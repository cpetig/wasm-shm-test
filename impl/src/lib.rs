use exports::test::shm::image::Guest;
use test::shm::exchange::{Memory, Options};

pub mod host_shm;

wit_bindgen::generate!({
    path: "../wit/shm.wit",
    world: "plugin"
});

struct MyGuest;

impl Guest for MyGuest {
    fn increment(buffer: Memory, where_: u32) {
        let data = buffer.attach(Options::WRITE);
        assert!(where_ < data.size);
        let addr = data.addr as *mut u8;
        let mem = unsafe { addr.byte_add(where_ as usize) };
        unsafe {
            mem.write(mem.read() + 1);
        }
        buffer.detach(0);
    }
}

export!(MyGuest);
