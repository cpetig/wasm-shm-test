#[cfg(feature = "symmetric")]
wit_bindgen::generate!({
    path: "../wit/complex.wit",
    world: "send-world",
    debug: true,
    symmetric: true,
    with: {
        "test:complex/external/subscriber": wasm_shm::Subscriber,
    }
});
#[cfg(feature = "canonical")]
wit_bindgen::generate!({
    path: "../wit/complex.wit",
    world: "send-world",
    debug: true,
    with: {
        "test:complex/external/subscriber": wasm_shm::Subscriber,
    }
});

struct MyWorld;

impl exports::test::complex::sender::Guest for MyWorld {
    fn start() -> wasm_shm::Subscriber {
        todo!()
    }
}

export!(MyWorld);
