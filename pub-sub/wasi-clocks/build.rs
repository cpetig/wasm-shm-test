// native build only: dependents find the wasi-clocks-symmetric shared library through this
fn main() {
    #[cfg(feature = "symmetric")]
    symmetric_link_search::emit();
}
