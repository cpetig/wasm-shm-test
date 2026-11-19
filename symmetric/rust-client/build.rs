// dependents of wasm-shm find the symmetric-sharedmem library through this script
fn main() {
    if std::env::var_os("CARGO_FEATURE_SYMMETRIC").is_some() {
        symmetric_link_search::emit();
    }
}
