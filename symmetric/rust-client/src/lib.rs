#[cfg(feature = "symmetric")]
mod client_symmetric;
#[cfg(feature = "canonical")]
mod client_wasm;
#[cfg(feature = "symmetric")]
use client_symmetric as client;
#[cfg(feature = "canonical")]
use client_wasm as client;

pub use client::test::shm::exchange::{Address, AttachOptions, Error, Memory, MemoryArea};
pub use client::test::shm::pub_sub::{Publisher, Subscriber};
