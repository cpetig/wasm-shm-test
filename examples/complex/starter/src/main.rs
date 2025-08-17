#[cfg(feature = "symmetric")]
wit_bindgen::generate!({
    path: "../wit/complex.wit",
    world: "starter",
    debug: true,
    symmetric: true,
    with: {
        "test:complex/external/subscriber": wasm_shm::Subscriber,
    }
});
#[cfg(feature = "canonical")]
wit_bindgen::generate!({
    path: "../wit/complex.wit",
    world: "starter",
    debug: true,
    with: {
        "test:complex/external/subscriber": wasm_shm::Subscriber,
    }
});

fn main() {
    let publ = test::complex::sender::start();
    let publ2 = wasm_shm::Subscriber::clone(&publ);
    test::complex::receiver::start(publ, false);
    test::complex::receiver::start(publ2, true);
}
