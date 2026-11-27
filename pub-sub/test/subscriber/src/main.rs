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

async fn run_subscribers() {
    println!("Publisher example");
    let publisher = publisher::start();
    let mut stream = publisher.get_stream();
    let mut stream2 = publisher.get_stream();
    drop(publisher);
    let future1 = async move {
        while let Some(buf) = stream.next().await {
            let value = read_value(buf);
            println!("Received   {value}");
        }
    };
    let future2 = async move {
        while let Some(buf) = stream2.next().await {
            let value = read_value(buf);
            println!("Received_2 {value}");
        }
    };
    futures::join!(future1, future2);
}

// std's main is exported as the async wasi:cli/run on wasm32-wasip3; spawning needs to happen inside of block_on
pub fn main() {
    wit_bindgen::block_on(run_subscribers());
}
