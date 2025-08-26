use wasm_shm::AttachOptions;

wit_bindgen::generate!({
    path: "../wit/",
    world: "rec-world",
    debug: true,
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

impl exports::test::complex::receiver::Guest for MyWorld {
    fn start(src: wasm_shm::Subscriber, block: bool) {
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
            wit_bindgen::rt::async_support::block_on(future);
        } else {
            wit_bindgen::rt::async_support::spawn(future);
        }
    }
}

export!(MyWorld);

// just to force linking
#[cfg(feature = "combined")]
pub fn dummy() {
    publisher_link::link_to_publisher();
}
