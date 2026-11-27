#![cfg_attr(feature = "canonical", no_main)]
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

// symmetric: plain main, spawning needs to happen inside of block_on
#[cfg(feature = "symmetric")]
pub fn main() {
    wit_bindgen::block_on(run_subscribers());
}

// canonical: export the asynchronous wasi:cli/run (WASI 0.3) so that the host runs us as an async task
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
            super::run_subscribers().await;
            Ok(())
        }
    }
}
