use wasm_shm::AttachOptions;

fn lift(src: wasm_shm::Address) -> u32 {
    *unsafe { &*((src.take_handle() as *const u8).cast::<u32>()) }
}

fn read_value(buf: wasm_shm::MemoryBlock) -> u32 {
    if let Ok(wasm_shm::MemoryArea { addr, size }) = buf.attach(AttachOptions::SHARED) {
        assert!(size as usize >= std::mem::size_of::<u32>());
        let value = lift(addr);
        buf.detach(std::mem::size_of::<u32>() as u32);
        value
    } else {
        todo!()
    }
}

pub fn main() {
    let publisher = publisher::start();
    let mut stream = publisher.get_stream();
    let future1 = async move {
        loop {
            if let Some(buf) = stream.next().await {
                let value = read_value(buf);
                println!("Received   {value}");
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
                let value = read_value(buf);
                println!("Received_2 {value}");
            } else {
                break;
            }
        }
    };
    let combination = async move { futures::join!(future1, future2) };
    wit_bindgen::rt::async_support::block_on(combination);
}
