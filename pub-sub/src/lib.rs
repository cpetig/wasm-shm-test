#[cfg(feature = "symmetric")]
wit_bindgen::generate!({
    path: "../wit/shm.wit",
    world: "impl",
});
#[cfg(feature = "canonical")]
wit_bindgen::generate!({
    path: "../wit/shm.wit",
    world: "impl2",
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
    // rights of every handle (and clone) of this block: false for read-only partitions
    writable: bool,
    // partitions: keeps the parent's storage alive, and the range is registered
    // in the parent while this block lives (see Drop)
    parent: Option<Arc<MyMemory>>,
    start: u32,
    // ranges (start, size) of the live partitions handed out from this block
    partitions: Mutex<Vec<(u32, u32)>>,
}

struct MyPublisher {
    _elements: u32,
    _element_size: u32,
    subscribers: Mutex<Vec<StreamWriter<MemoryBlock>>>,
    pool: Mutex<VecDeque<MemoryBlock>>,
}

#[cfg(feature = "symmetric")]
struct Dummy;

export!(SharedImpl);

use std::{
    collections::VecDeque,
    future::IntoFuture,
    sync::{Arc, Mutex},
};

#[cfg(feature = "symmetric")]
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use exchange::MemoryBlock;
#[cfg(feature = "symmetric")]
use exchange::{Address, AttachOptions, Bytes, Error, MemoryArea};
#[cfg(feature = "symmetric")]
use exports::test::shm::exchange;
use exports::test::shm::pub_sub;
#[cfg(feature = "canonical")]
use test::shm::exchange;
use wit_bindgen::StreamWriter;

#[cfg(feature = "symmetric")]
impl exchange::Guest for SharedImpl {
    type MemoryBlock = Arc<MyMemory>;
    type Address = Dummy;
}

impl pub_sub::Guest for SharedImpl {
    type Publisher = Arc<MyPublisher>;
    type Subscriber = Arc<MyPublisher>;
}

#[cfg(feature = "symmetric")]
impl exchange::GuestAddress for Dummy {}

#[cfg(feature = "symmetric")]
impl MyMemory {
    fn with(
        address: *mut u8,
        capacity: u32,
        writable: bool,
        parent: Option<Arc<MyMemory>>,
        start: u32,
    ) -> Arc<Self> {
        Arc::new(MyMemory {
            address,
            capacity,
            written: AtomicU32::new(0),
            attach_count: AtomicU32::new(0),
            write: AtomicBool::new(false),
            read_consumed: AtomicU32::new(0),
            shared: AtomicBool::new(false),
            writable,
            parent,
            start,
            partitions: Mutex::new(Vec::new()),
        })
    }

    fn new(size: u32) -> Arc<Self> {
        let align = if size < 2 || size & 1 != 0 {
            1
        } else if size < 4 || size & 2 != 0 {
            2
        } else if size < 8 || size & 4 != 0 {
            4
        } else {
            8
        };
        let address = unsafe {
            std::alloc::alloc(std::alloc::Layout::from_size_align(size as usize, align).unwrap())
        };
        MyMemory::with(address, size, true, None, 0)
    }
}

#[cfg(feature = "symmetric")]
impl Drop for MyMemory {
    fn drop(&mut self) {
        // last handle of a partition is gone: its range is free again
        if let Some(parent) = &self.parent {
            let mut list = parent.partitions.lock().unwrap();
            if let Some(pos) = list.iter().position(|&(s, n)| s == self.start && n == self.capacity)
            {
                list.swap_remove(pos);
            }
        }
    }
}

fn mem_new(size: u32) -> MemoryBlock {
    #[cfg(feature = "symmetric")]
    let res: MemoryBlock = MemoryBlock::new(MyMemory::new(size));
    #[cfg(feature = "canonical")]
    let res = MemoryType::new(size);
    res
}

#[cfg(feature = "symmetric")]
impl exchange::GuestMemoryBlock for Arc<MyMemory> {
    fn new(size: Bytes) -> Self {
        MyMemory::new(size)
    }
    fn attach(&self, opt: AttachOptions) -> Result<MemoryArea, Error> {
        if opt & AttachOptions::WRITE == AttachOptions::WRITE {
            if !self.writable {
                return Err(Error::ReadOnly);
            }
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
                // several readers may be attached (read is shared), each consumes its part
                assert!(count >= 1);
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
    fn create_local(buffer: MemoryArea) -> MemoryBlock {
        MemoryBlock::new(MyMemory::with(
            buffer.addr.take_handle() as *mut u8,
            buffer.size,
            true,
            None,
            0,
        ))
    }
    fn clone(&self) -> MemoryBlock {
        // shares the Arc, so rights and attach state are shared with the clone
        MemoryBlock::new(Clone::clone(self))
    }
    fn create_partition(
        &self,
        start: Bytes,
        size: Bytes,
        mode: AttachOptions,
    ) -> Result<MemoryBlock, Error> {
        let write = mode & AttachOptions::WRITE == AttachOptions::WRITE;
        if write && !self.writable {
            return Err(Error::ReadOnly);
        }
        let end = start as u64 + size as u64;
        if size == 0 || end > self.capacity as u64 {
            return Err(Error::OutOfRange);
        }
        // check and register under one lock
        let mut list = self.partitions.lock().unwrap();
        if list.iter().any(|&(s, n)| start < s + n && s < end as u32) {
            return Err(Error::Overlap);
        }
        list.push((start, size));
        drop(list);
        // TODO: lock the range in the parent (see shm.wit), for now the parent can
        // still attach all of its bytes independent of the partitions
        let address = unsafe { self.address.add(start as usize) };
        Ok(MemoryBlock::new(MyMemory::with(
            address,
            size,
            write,
            Some(Clone::clone(self)),
            start,
        )))
    }
}

impl pub_sub::GuestSubscriber for Arc<MyPublisher> {
    fn get_stream(&self) -> wit_bindgen::StreamReader<MemoryBlock> {
        let s = wit_stream::new::<MemoryBlock>();
        self.subscribers.lock().unwrap().push(s.0);
        s.1
    }

    fn clone(original: pub_sub::SubscriberBorrow<'_>) -> pub_sub::Subscriber {
        pub_sub::Subscriber::new(Clone::clone(original.get::<Arc<MyPublisher>>()))
    }
}

#[cfg(feature = "symmetric")]
type MemoryType = Arc<MyMemory>;

#[cfg(feature = "canonical")]
type MemoryType = exchange::MemoryBlock;

fn mem_clone(obj: &MemoryBlock) -> MemoryBlock {
    #[cfg(feature = "symmetric")]
    let res = MemoryBlock::new(MemoryType::clone(obj.get::<Arc<MyMemory>>()));
    #[cfg(feature = "canonical")]
    let res = MemoryType::clone(obj);
    res
}

impl pub_sub::GuestPublisher for Arc<MyPublisher> {
    fn new(elements: u32, element_size: u32) -> Self {
        let mem: Vec<_> = (0..elements).map(|_| mem_new(element_size)).collect();
        Arc::new(MyPublisher {
            _elements: elements,
            _element_size: element_size,
            subscribers: Mutex::new(Vec::new()),
            pool: Mutex::new(VecDeque::from(mem)),
        })
    }
    fn allocate(&self) -> (MemoryBlock, u32) {
        let buf = self.pool.lock().unwrap().pop_front().unwrap();
        self.pool.lock().unwrap().push_back(mem_clone(&buf));
        (buf, 0)
    }
    fn publish(&self, value: MemoryBlock) {
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
            // a pending write is dropped here: slow subscribers miss values (see README)
        }
    }
    fn subscribers(&self) -> pub_sub::Subscriber {
        pub_sub::Subscriber::new(Clone::clone(self))
    }

    fn get_stream(&self) -> wit_bindgen::StreamReader<MemoryBlock> {
        let s = wit_stream::new::<MemoryBlock>();
        self.subscribers.lock().unwrap().push(s.0);
        s.1
    }
}
