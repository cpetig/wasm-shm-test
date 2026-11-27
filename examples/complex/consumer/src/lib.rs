use wasm_shm::AttachOptions;

wit_bindgen::generate!({
    path: "../wit/",
    world: "rec-world",
    with: {
        "test:shm/exchange": pub_sub,
        "test:shm/pub-sub": pub_sub,
    }
});

struct MyWorld;
mod pub_sub {
    pub use wasm_shm::Subscriber;
}

type Index = u8;

fn read_value(buf: wasm_shm::MemoryBlock) {
    use flat::Visit;
    if let Ok(wasm_shm::MemoryArea { addr, size }) = buf.attach(AttachOptions::SHARED) {
        let slice =
            unsafe { std::slice::from_raw_parts(addr.take_handle() as *const u8, size as usize) };
        let view = flat::View::<flat::Vec<flat::String<Index>, Index>>::new(&slice);
        let mut res = String::new();
        view.visit(|v| v.visit(|s| { res += s; res += " "; }));
        dbg!(res);
        buf.detach(view.len() as u32);
    } else {
        todo!()
    }
}

// every component has its own linear memory, so the consumer needs storage of its own
// to attach the shared memory blocks it receives (once per component)
fn ensure_storage() {
    static DONE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if DONE.swap(true, std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    let memsize = wasm_shm::MemoryBlock::optimum_size(1, 1024);
    // the host may not need storage at all
    if memsize == 0 {
        return;
    }
    let area = unsafe {
        std::alloc::alloc(std::alloc::Layout::from_size_align(memsize as usize, 8).unwrap())
    };
    wasm_shm::MemoryBlock::add_storage(wasm_shm::MemoryArea {
        addr: unsafe { wasm_shm::Address::from_handle((area as usize).try_into().unwrap()) },
        size: memsize,
    })
    .unwrap();
}

impl exports::test::complex::receiver::Guest for MyWorld {
    async fn start(src: wasm_shm::Subscriber, block: bool) {
        ensure_storage();
        let mut stream = src.get_stream();
        drop(src); // this closes the publisher when the writer drops
        let future = async move {
            loop {
                if let Some(buf) = stream.next().await {
                    read_value(buf);
                } else {
                    break;
                }
            }
        };
        if block {
            future.await;
        } else {
            wit_bindgen::spawn(future);
        }
    }
}

export!(MyWorld);
