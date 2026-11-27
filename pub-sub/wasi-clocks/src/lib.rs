#[cfg(feature = "symmetric")]
pub mod imports_symmetric;
#[cfg(feature = "canonical")]
pub mod imports_wasm;
#[cfg(feature = "symmetric")]
pub use imports_symmetric as imports;
#[cfg(feature = "canonical")]
pub use imports_wasm as imports;

pub use imports::wasi::clocks::{monotonic_clock, system_clock};
