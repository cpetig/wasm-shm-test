use std::sync::atomic::AtomicPtr;

use exports::test::shm::image::Guest;
use test::shm::exchange::{AttachOptions, Memory, MemoryArea};

wit_bindgen::generate!({
    path: "../wit/shm.wit",
    world: "plugin"
});

struct MyGuest;

static BUFFER: AtomicPtr<u8> = AtomicPtr::new(std::ptr::null_mut());

impl Guest for MyGuest {
    fn increment(buffer: &Memory, where_: u32) {
        let mut addr = BUFFER.load(std::sync::atomic::Ordering::Acquire);
        if addr.is_null() {
            let layout =
                std::alloc::Layout::from_size_align(buffer.minimum_size() as usize, 1).unwrap();
            if layout.size() > 0 {
                addr = unsafe { std::alloc::alloc(layout) };
                BUFFER.store(addr, std::sync::atomic::Ordering::Release);
            }
            buffer
                .add_storage(MemoryArea {
                    addr: addr as usize as u32,
                    size: layout.size() as u32,
                })
                .unwrap();
        }
        let data = buffer.attach(AttachOptions::WRITE).unwrap();
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
