use wasm_shm::AttachOptions;

fn lift(src: wasm_shm::Address) -> u32 {
    *unsafe { &*((src.take_handle() as *const u8).cast::<u32>()) }
}

pub fn main() {
    let publisher = publisher::start();
    let mut stream = publisher.get_stream();
    let future1 = async move {
        loop {
            if let Some(buf) = stream.next().await {
                if let Ok(wasm_shm::MemoryArea { addr, size }) = buf.attach(AttachOptions::SHARED) {
                    assert!(size as usize >= std::mem::size_of::<u32>());
                    let value = lift(addr);
                    println!("Received   {value}");
                    buf.detach(std::mem::size_of::<u32>() as u32);
                }
            } else {
                break;
            }
        }
    };
    let mut stream2 = publisher.get_stream();
    drop(publisher);
    let future2 = async move {
        loop {
            if let Some(buf) = stream2.next().await {
                if let Ok(wasm_shm::MemoryArea { addr, size }) = buf.attach(AttachOptions::SHARED) {
                    assert!(size as usize >= std::mem::size_of::<u32>());
                    let value = lift(addr);
                    println!("Received_2 {value}");
                    buf.detach(std::mem::size_of::<u32>() as u32);
                }
            } else {
                break;
            }
        }
    };
    let combination = async move { futures::join!(future1, future2) };
    // let combination = future1;
    wit_bindgen::rt::async_support::block_on(combination);
}
