// native (symmetric ABI) build only: find the shared libraries to link against
fn main() {
    #[cfg(feature = "symmetric")]
    symmetric_link_search::emit();
}
