#![cfg_attr(feature = "canonical", no_main)]
wit_bindgen::generate!({
    path: "../wit/",
    world: "starter",
    with: {
        "test:shm/exchange": pub_sub,
        "test:shm/pub-sub": pub_sub,
    }
});

mod pub_sub {
    pub use wasm_shm::Subscriber;
}

async fn run_all() {
    let publ = test::complex::sender::start().await;
    let publ2 = wasm_shm::Subscriber::clone(&publ);
    // the first receiver spawns its work, the second one runs until the stream ends
    test::complex::receiver::start(publ, false).await;
    test::complex::receiver::start(publ2, true).await;
}

#[cfg(feature = "symmetric")]
fn main() {
    wit_bindgen::block_on(run_all());
}

// canonical: export the asynchronous wasi:cli/run (WASI 0.3)
#[cfg(feature = "canonical")]
mod cli {
    wit_bindgen::generate!({
        inline: r#"
            package wasi:cli@0.3.0-rc-2026-02-09;
            interface run { run: async func() -> result; }
            world app { export run; }
        "#,
    });

    struct App;
    export!(App);

    impl exports::wasi::cli::run::Guest for App {
        async fn run() -> Result<(), ()> {
            super::run_all().await;
            Ok(())
        }
    }
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
