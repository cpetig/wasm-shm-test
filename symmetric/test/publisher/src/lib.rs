pub fn start() -> wasm_shm::DataStream {
    let publisher = wasm_shm::DataStream::new(5, 256);
    let writer = publisher.clone();
    wit_bindgen::rt::async_support::spawn(async move {
        for i in 1..21 {
            wasi::monotonic_clock::wait_for(1_000_000_000).await;
            let (buffer, is_init) = writer.allocate();
            if let Ok(wasm_shm::MemoryArea { addr, size }) =
                buffer.attach(AttachOptions::WRITE | AttachOptions::SHARED)
            {
                assert!(size >= std::mem::size_of::<u32>());
                *unsafe { &mut *(addr as *mut u32) } = i;
                buffer.detach(std::mem::size_of::<u32>());
                writer.publish(buffer);
            }
        }
    });
    publisher
}
