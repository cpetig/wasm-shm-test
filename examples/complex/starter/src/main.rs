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

// just to force linking
#[cfg(feature = "symmetric")]
pub mod link {
    pub fn dummy() {
        #[link(name = "consumer")]
        unsafe extern "C" {
            fn testX3AcomplexX2FreceiverX00start(_: *mut u8, _: i32);
        }
        unsafe { testX3AcomplexX2FreceiverX00start(std::ptr::null_mut(), 0) };
        #[link(name = "publisher")]
        unsafe extern "C" {
            fn testX3AcomplexX2FsenderX00start() -> *mut u8;
        }
        unsafe { testX3AcomplexX2FsenderX00start() };
    }
}
