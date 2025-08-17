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

impl exports::test::complex::receiver::Guest for MyWorld {
    fn start(src: wasm_shm::Subscriber, block: bool) {
        todo!()
    }
}

export!(MyWorld);
