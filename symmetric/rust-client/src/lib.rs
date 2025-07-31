mod client;

pub use client::test::shm::exchange::{Address, Error, Memory, MemoryArea, AttachOptions};
pub use client::test::shm::pub_sub::{Publisher, Subscriber};
