// native build only: dependents of wasm-shm find the pub-sub shared library through this
fn main() {
    #[cfg(feature = "symmetric")]
    symmetric_link_search::emit();
}
