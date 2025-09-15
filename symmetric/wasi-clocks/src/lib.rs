mod exports;

exports::export!(WasiClocks with_types_in exports);

struct WasiClocks;

use exports::exports::wasi::clocks::{monotonic_clock, wall_clock};

impl wall_clock::Guest for WasiClocks {
    fn now() -> wall_clock::Datetime {
        todo!()
    }
    fn get_resolution() -> wall_clock::Datetime {
        todo!()
    }
}

impl monotonic_clock::Guest for WasiClocks {
    fn now() -> monotonic_clock::Instant {
        todo!()
    }
    fn get_resolution() -> monotonic_clock::Duration {
        todo!()
    }
    async fn wait_until(_when: monotonic_clock::Instant) {
        todo!()
    }
    async fn wait_for(how_long: monotonic_clock::Duration) {
        wit_bindgen::rt::async_support::wait_on(wit_bindgen::rt::EventSubscription::from_timeout(how_long)).await;
    }
}
