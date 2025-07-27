use wasm_shm::{Address, AttachOptions, DataStream};

// this is just a placeholder, imagine it becoming more complex with in buffer string and list storage
fn lower(src: u32, dest: Address) {
    *unsafe { &mut *((dest.take_handle() as *mut u8).cast::<u32>()) } = src;
}

pub fn start() -> wasm_shm::DataStream {
    let publisher = wasm_shm::DataStream::new(5, 256);
    let writer = DataStream::clone(&publisher);
    wit_bindgen::rt::async_support::spawn(async move {
        for i in 1..21 {
            wasi_clocks::monotonic_clock::wait_for(1_000_000_000).await;
            let (buffer, _is_init) = writer.allocate();
            if let Ok(wasm_shm::MemoryArea { addr, size }) =
                buffer.attach(AttachOptions::WRITE | AttachOptions::SHARED)
            {
                assert!(size as usize >= std::mem::size_of::<u32>());
                lower(i, addr);
                buffer.detach(std::mem::size_of::<u32>() as u32);
                writer.publish(buffer);
            }
        }
    });
    publisher
}
