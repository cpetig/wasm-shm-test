use wasm_shm::{Address, AttachOptions};
use wit_bindgen::rt;

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

// a simple replacement for wasi::clocks::monotonic_clock::wait_for (no async)
#[cfg(feature = "symmetric")]
mod easy_way_out {
    use wit_bindgen::rt;

    // only works on symmetric (avoids async function)
    pub async fn wait_for(nanoseconds: u64) {
        rt::async_support::wait_on(rt::EventSubscription::from_timeout(nanoseconds)).await;
    }
}

const NUMBERS: [&str; 10] = [
    "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
];
type Index = u8;

// attach buffer and write value to it
// this could be hidden in bindgen code in some future
fn write_to_buffer(value: u32, buffer: &mut wasm_shm::MemoryBlock) -> Result<(), wasm_shm::Error> {
    use flat::{Assign, Fill};
    let wasm_shm::MemoryArea { addr, size } =
        buffer.attach(AttachOptions::WRITE | AttachOptions::SHARED)?;
    let mut slice =
        unsafe { std::slice::from_raw_parts_mut(addr.take_handle() as *mut u8, size as usize) };
    let mut writer = flat::Creator::<flat::Vec<flat::String<Index>, Index>>::new(&mut slice);
    writer.allocate(2).expect("root alloc");
    writer.push(|w| w.set("hello")).expect("first element");
    writer
        .push(|w| w.set(NUMBERS[value as usize - 1]))
        .expect("second element");
    let view = writer.finish().expect("finish");
    dbg!(view.len());
    buffer.detach(view.len() as u32);
    Ok(())
}

#[cfg(feature = "symmetric")]
use easy_way_out::wait_for;
#[cfg(feature = "canonical")]
use wasi_clocks::monotonic_clock::wait_for;

impl exports::test::complex::sender::Guest for MyWorld {
    fn start() -> wasm_shm::Subscriber {
        let memsize = wasm_shm::MemoryBlock::optimum_size(1, 1024);
        let alloc = if memsize > 0 {
            let area = unsafe {
                std::alloc::alloc(std::alloc::Layout::from_size_align(memsize as usize, 8).unwrap())
            };
            wasm_shm::MemoryBlock::add_storage(wasm_shm::MemoryArea {
                addr: unsafe { Address::from_handle((area as usize).try_into().unwrap()) },
                size: memsize,
            })
            .unwrap();
            Some(area)
        } else {
            None
        };
        let publisher = wasm_shm::Publisher::new(1, 1024);
        let subscriber = publisher.subscribers();
        rt::async_support::spawn(async move {
            for i in 1..11 {
                wait_for(1_000_000_000).await;
                let (mut buffer, _initialized) = publisher.allocate();
                if write_to_buffer(i, &mut buffer).is_ok() {
                    publisher.publish(buffer);
                }
            }
        });
        subscriber
    }
}

export!(MyWorld);
