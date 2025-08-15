#[cfg(feature = "symmetric")]
wit_bindgen::generate!({
    path: "../wit/shm.wit",
    world: "impl",
    debug: true,
    symmetric: true,
});
#[cfg(feature = "canonical")]
wit_bindgen::generate!({
    path: "../wit/shm.wit",
    world: "impl2",
    debug: true,
});

struct SharedImpl;

#[cfg(feature = "symmetric")]
struct MyMemory {
    address: *mut u8,
    capacity: u32,
    written: AtomicU32,
    read_consumed: AtomicU32,
    attach_count: AtomicU32,
    write: AtomicBool,
    shared: AtomicBool,
}

struct MyPublisher {
    elements: u32,
    element_size: u32,
    subscribers: Mutex<Vec<StreamWriter<Memory>>>,
    pool: Mutex<VecDeque<Memory>>,
}

struct Dummy;

export!(SharedImpl);

#[cfg(feature = "symmetric")]
use std::{
    alloc::Layout,
    collections::VecDeque,
    future::IntoFuture,
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        Arc, Mutex,
    },
};

use exchange::{Address, AttachOptions, Bytes, Error, Memory, MemoryArea};
#[cfg(feature = "symmetric")]
use exports::test::shm::exchange;
use exports::test::shm::pub_sub;
#[cfg(feature = "canonical")]
use test::shm::exchange;
use wit_bindgen::StreamWriter;

#[cfg(feature = "symmetric")]
impl exchange::Guest for SharedImpl {
    type Memory = Arc<MyMemory>;
    type Address = Dummy;
}

impl pub_sub::Guest for SharedImpl {
    type Publisher = Arc<MyPublisher>;
    type Subscriber = Arc<MyPublisher>;
}

#[cfg(feature = "symmetric")]
impl exchange::GuestAddress for Dummy {}

#[cfg(feature = "symmetric")]
impl exchange::GuestMemory for Arc<MyMemory> {
    fn new(size: Bytes) -> Self {
        Self::new(MyMemory {
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
            written: AtomicU32::new(0),
            attach_count: AtomicU32::new(0),
            write: AtomicBool::new(false),
            read_consumed: AtomicU32::new(0),
            shared: AtomicBool::new(false),
        })
    }
    fn attach(&self, opt: AttachOptions) -> Result<MemoryArea, Error> {
        if opt & AttachOptions::WRITE == AttachOptions::WRITE {
            if self
                .attach_count
                .compare_exchange(0, 1, Ordering::Acquire, Ordering::Relaxed)
                .is_err()
            {
                return Err(Error::Busy);
            }
            self.write.store(true, Ordering::Relaxed);
            self.shared.store(
                opt & AttachOptions::SHARED == AttachOptions::SHARED,
                Ordering::Relaxed,
            );
            self.written.store(0, Ordering::Release);
            Ok(MemoryArea {
                addr: unsafe { Address::from_handle(self.address as usize) },
                size: self.capacity,
            })
        } else {
            if self.write.load(Ordering::Acquire) == true
                && self.attach_count.load(Ordering::Acquire) != 0
            {
                return Err(Error::Busy);
            }
            let old_write = self.write.swap(false, Ordering::Relaxed);
            let old_count = self.attach_count.fetch_add(1, Ordering::Release);
            if old_write == true && old_count != 0 {
                return Err(Error::Busy);
            }
            let shared = self.shared.load(Ordering::Relaxed);
            let consumed = if shared {
                self.read_consumed.load(Ordering::Relaxed)
            } else {
                0
            };
            Ok(MemoryArea {
                addr: unsafe { Address::from_handle(self.address as usize + consumed as usize) },
                size: self.written.load(Ordering::Relaxed) - consumed,
            })
        }
    }
    fn detach(&self, consumed: Bytes) {
        let write = self.write.load(Ordering::Acquire);
        let count = self.attach_count.fetch_sub(1, Ordering::Relaxed);
        if write {
            self.written.store(consumed, Ordering::Release);
        } else {
            let shared = self.shared.load(Ordering::Relaxed);
            if !shared {
                assert!(count == 1);
                self.read_consumed.fetch_add(consumed, Ordering::Release);
            }
        }
    }
    fn minimum_size(&self) -> Bytes {
        0
    }
    fn optimum_size(_count: u32, _size: Bytes) -> Bytes {
        0
    }
    fn add_storage(_buffer: MemoryArea) -> Result<(), Error> {
        todo!()
    }
    fn create_local(buffer: MemoryArea) -> Memory {
        Memory::new(Arc::new(MyMemory {
            address: buffer.addr.take_handle() as *mut u8,
            capacity: buffer.size,
            written: AtomicU32::new(0),
            attach_count: AtomicU32::new(0),
            write: AtomicBool::new(false),
            read_consumed: AtomicU32::new(0),
            shared: AtomicBool::new(false),
        }))
    }
    fn clone(&self) -> Memory {
        Memory::new(Clone::clone(self))
    }
}

impl pub_sub::GuestSubscriber for Arc<MyPublisher> {
    fn get_stream(&self) -> wit_bindgen::rt::async_support::StreamReader<Memory> {
        let s = wit_stream::new::<Memory>();
        self.subscribers.lock().unwrap().push(s.0);
        s.1
    }

    fn clone(original: pub_sub::SubscriberBorrow<'_>) -> pub_sub::Subscriber {
        pub_sub::Subscriber::new(Clone::clone(original.get::<Arc<MyPublisher>>()))
    }
}

#[cfg(feature = "symmetric")]
type MemoryType = Arc<MyMemory>;
#[cfg(feature = "symmetric")]
type HandleType = usize;

#[cfg(feature = "canonical")]
type MemoryType = exchange::Memory;
#[cfg(feature = "canonical")]
type HandleType = u32;

fn mem_clone(obj: &Memory) -> Memory {
    #[cfg(feature = "symmetric")]
    let res = Memory::new(MemoryType::clone(obj.get::<Arc<MyMemory>>()));
    #[cfg(feature = "canonical")]
    let res = MemoryType::clone(obj);
    res
}

impl pub_sub::GuestPublisher for Arc<MyPublisher> {
    fn new(elements: u32, element_size: u32) -> Self {
        #[cfg(feature = "symmetric")]
        use exchange::GuestMemory;
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
        for _ in 0..elements {
            let area = unsafe {
                std::alloc::alloc(
                    Layout::from_size_align(element_size as usize, alignment).unwrap(),
                )
            };
            mem.push(MemoryType::create_local(MemoryArea {
                addr: unsafe { Address::from_handle(area as HandleType) },
                size: element_size,
            }));
        }
        Arc::new(MyPublisher {
            elements: elements,
            element_size: element_size,
            subscribers: Mutex::new(Vec::new()),
            pool: Mutex::new(VecDeque::from(mem)),
        })
    }
    fn allocate(&self) -> (Memory, u32) {
        let buf = self.pool.lock().unwrap().pop_front().unwrap();
        self.pool.lock().unwrap().push_back(mem_clone(&buf));
        (buf, 0)
    }
    fn publish(&self, value: Memory) {
        use futures::Future;
        for i in self.subscribers.lock().unwrap().iter_mut() {
            let new_buffer = mem_clone(&value);
            let fut = i.write(vec![new_buffer]).into_future();
            let waker = futures::task::Waker::noop();
            let mut ctx = futures::task::Context::from_waker(&waker);
            let mut pinned = std::pin::pin!(fut);
            match pinned.as_mut().poll(&mut ctx) {
                std::task::Poll::Ready(_) => {}
                std::task::Poll::Pending => (),
            }

            // fut.poll
        }
    }
    fn subscribers(&self) -> pub_sub::Subscriber {
        pub_sub::Subscriber::new(Clone::clone(self))
    }

    // fn clone(original: pub_sub::PublisherBorrow<'_>) -> pub_sub::Publisher {
    //     pub_sub::Publisher::new(Clone::clone(original.get::<Arc<MyPublisher>>()))
    // }
}
