mod exports;

exports::export!(WasiClocks with_types_in exports);

struct WasiClocks;

use exports::exports::wasi::clocks::{monotonic_clock, system_clock};

impl system_clock::Guest for WasiClocks {
    fn now() -> system_clock::Instant {
        todo!()
    }
    fn get_resolution() -> system_clock::Duration {
        todo!()
    }
}

impl monotonic_clock::Guest for WasiClocks {
    fn now() -> monotonic_clock::Mark {
        todo!()
    }
    fn get_resolution() -> monotonic_clock::Duration {
        todo!()
    }
    async fn wait_until(_when: monotonic_clock::Mark) {
        todo!()
    }
    async fn wait_for(how_long: monotonic_clock::Duration) {
        wit_bindgen::rt::async_support::wait_on(wit_bindgen::rt::EventSubscription::from_timeout(how_long)).await;
    }
}
