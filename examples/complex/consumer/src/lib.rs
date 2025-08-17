use wasm_shm::AttachOptions;

#[cfg(feature = "symmetric")]
wit_bindgen::generate!({
    path: "../wit/complex.wit",
    world: "rec-world",
    debug: true,
    symmetric: true,
    with: {
        "test:complex/external/subscriber": wasm_shm::Subscriber,
    }
});
#[cfg(feature = "canonical")]
wit_bindgen::generate!({
    path: "../wit/complex.wit",
    world: "rec-world",
    debug: true,
    with: {
        "test:complex/external/subscriber": wasm_shm::Subscriber,
    }
});

struct MyWorld;

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
