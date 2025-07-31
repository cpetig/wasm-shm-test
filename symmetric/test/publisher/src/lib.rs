use wasm_shm::{Address, AttachOptions, Publisher};

// this is just a placeholder, imagine it becoming more complex with in buffer string and list storage
fn lower(src: u32, dest: Address) {
    *unsafe { &mut *((dest.take_handle() as *mut u8).cast::<u32>()) } = src;
}

mod easy_way_out {
    use wit_bindgen::rt;

    // only works on symmetric (avoids async function)
    pub async fn wait_for(nanoseconds: u64) {
        rt::async_support::wait_on(rt::EventSubscription::from_timeout(nanoseconds)).await;
    }
}

use easy_way_out::wait_for;
use wit_bindgen::rt;

pub fn start() -> wasm_shm::Subscriber {
    let publisher = wasm_shm::Publisher::new(5, 256);
    let subscriber = publisher.subscribers();
    rt::async_support::spawn(async move {
        for i in 1..21 {
            wait_for(1_000_000_000).await;
            // this could be hidden in bindgen code in some future
            let (buffer, _is_init) = publisher.allocate();
            if let Ok(wasm_shm::MemoryArea { addr, size }) =
                buffer.attach(AttachOptions::WRITE | AttachOptions::SHARED)
            {
                assert!(size as usize >= std::mem::size_of::<u32>());
                lower(i, addr);
                buffer.detach(std::mem::size_of::<u32>() as u32);
                publisher.publish(buffer);
            }
        }
    });
    subscriber
}
