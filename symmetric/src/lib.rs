wit_bindgen::generate!({
    path: "../wit/shm.wit",
    world: "impl",
    debug: true,
    symmetric: true,
});

struct SharedImpl;

struct MyMemory {
    address: *mut u8,
    capacity: u32,
    used: AtomicU32,
    count: AtomicU32,
    write: AtomicBool,
}

struct MyDataStream {
    elements: u32,
    element_size: u32,
    subscribers: Mutex<Vec<StreamWriter<Memory>>>,
    pool: Mutex<VecDeque<Memory>>,
}

struct Dummy;

export!(SharedImpl);

use std::{
    alloc::Layout,
    collections::VecDeque,
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        Mutex,
    },
};

use exports::test::shm::exchange::{Address, AttachOptions, Error, Memory, MemoryArea};
use wit_bindgen::StreamWriter;

impl exports::test::shm::exchange::Guest for SharedImpl {
    type Memory = MyMemory;
    type Address = Dummy;
}

impl exports::test::shm::publisher::Guest for SharedImpl {
    type DataStream = MyDataStream;
}

impl exports::test::shm::exchange::GuestAddress for Dummy {}

impl exports::test::shm::exchange::GuestMemory for MyMemory {
    fn new(size: u32) -> Self {
        Self {
            address: unsafe {
                std::alloc::alloc(
                    std::alloc::Layout::from_size_align(
                        size as usize,
                        if size < 2 || size & 1 != 0 {
                            1
                        } else if size < 4 || size & 2 != 0 {
                            2
                        } else if size < 8 || size & 4 != 0 {
                            4
                        } else {
                            8
                        },
                    )
                    .unwrap(),
                )
            },
            capacity: size,
            used: AtomicU32::new(0),
            count: AtomicU32::new(0),
            write: AtomicBool::new(false),
        }
    }
    fn attach(&self, opt: AttachOptions) -> Result<MemoryArea, Error> {
        if opt & AttachOptions::WRITE == AttachOptions::WRITE {
            if self
                .count
                .compare_exchange(0, 1, Ordering::Acquire, Ordering::Relaxed)
                .is_err()
            {
                return Err(Error::Busy);
            }
            self.write.store(true, Ordering::Relaxed);
            self.used.store(0, Ordering::Release);
            Ok(MemoryArea {
                addr: unsafe { Address::from_handle(self.address as usize) },
                size: self.capacity,
            })
        } else {
            if self.write.load(Ordering::Acquire) == true && self.count.load(Ordering::Acquire) != 0
            {
                return Err(Error::Busy);
            }
            let old_write = self.write.swap(false, Ordering::Relaxed);
            let old_count = self.count.fetch_add(1, Ordering::Release);
            if old_write == true && old_count != 0 {
                return Err(Error::Busy);
            }
            Ok(MemoryArea {
                addr: unsafe { Address::from_handle(self.address as usize) },
                size: self.used.load(Ordering::Relaxed),
            })
        }
    }
    fn detach(&self, consumed: u32) {
        let write = self.write.load(Ordering::Acquire);
        let count = self.count.fetch_sub(1, Ordering::Relaxed);
        if write {
            self.used.store(consumed, Ordering::Release);
        } else {
            if count == 1 {
                self.used.fetch_sub(consumed, Ordering::Release);
            }
        }
    }
    fn minimum_size(&self) -> u32 {
        0
    }
    fn add_storage(&self, _buffer: MemoryArea) -> Result<(), Error> {
        todo!()
    }
    fn create_local(buffer: MemoryArea) -> Memory {
        Memory::new(MyMemory {
            address: buffer.addr.take_handle() as *mut u8,
            capacity: buffer.size,
            used: AtomicU32::new(0),
            count: AtomicU32::new(0),
            write: AtomicBool::new(false),
        })
    }
}

impl exports::test::shm::publisher::GuestDataStream for MyDataStream {
    fn new(elements: u32, element_size: u32) -> Self {
        use crate::exports::test::shm::exchange::GuestMemory;
        let mut mem = Vec::new();
        let alignment = if element_size < 2 || element_size & 1 != 0 {
            1
        } else if element_size < 4 || element_size & 2 != 0 {
            2
        } else if element_size < 8 || element_size & 4 != 0 {
            4
        } else {
            8
        };
        for i in 0..elements {
            let area = unsafe {
                std::alloc::alloc(
                    Layout::from_size_align(element_size as usize, alignment).unwrap(),
                )
            };
            mem.push(MyMemory::create_local(MemoryArea {
                addr: unsafe { Address::from_handle(area as usize) },
                size: element_size,
            }));
        }
        MyDataStream {
            elements: elements,
            element_size: element_size,
            subscribers: Mutex::new(Vec::new()),
            pool: Mutex::new(VecDeque::from(mem)),
        }
    }
    fn subscribe(&self) -> wit_bindgen::rt::async_support::StreamReader<Memory> {
        let s = wit_stream::new::<Memory>();
        self.subscribers.lock().unwrap().push(s.0);
        s.1
        // //exports::test::shm::
        // todo!()
    }
    fn allocate(&self) -> (Memory, bool) {
        todo!()
    }
    fn publish(&self, _value: Memory) {
        todo!()
    }
    fn clone(
        _original: exports::test::shm::publisher::DataStreamBorrow<'_>,
    ) -> exports::test::shm::publisher::DataStream {
        todo!()
    }
}
