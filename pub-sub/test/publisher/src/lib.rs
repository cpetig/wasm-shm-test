use wasm_shm::{Address, AttachOptions};

// this is just a placeholder, imagine it becoming more complex with in buffer string and list storage
fn lower(src: u32, dest: Address) {
    *unsafe { &mut *((dest.take_handle() as *mut u8).cast::<u32>()) } = src;
}

// attach buffer and write value to it
// this could be hidden in bindgen code in some future
fn write_to_buffer(value: u32, buffer: &mut wasm_shm::MemoryBlock) -> Result<(), wasm_shm::Error> {
    let wasm_shm::MemoryArea { addr, size } =
        buffer.attach(AttachOptions::WRITE | AttachOptions::SHARED)?;
    assert!(size as usize >= std::mem::size_of::<u32>());
    lower(value, addr);
    buffer.detach(std::mem::size_of::<u32>() as u32);
    Ok(())
}

use wasi_clocks::monotonic_clock::wait_for;

pub fn start() -> wasm_shm::Subscriber {
    let memsize = wasm_shm::MemoryBlock::optimum_size(5, 256);
    let _alloc = if memsize > 0 {
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
    let publisher = wasm_shm::Publisher::new(5, 256);
    let subscriber = publisher.subscribers();
    wit_bindgen::spawn(async move {
        for i in 1..21 {
            wait_for(1_000_000_000).await;
            let (mut buffer, _initialized) = publisher.allocate();
            if write_to_buffer(i, &mut buffer).is_ok() {
                publisher.publish(buffer);
            }
        }
    });
    subscriber
}
