use test::shm::exchange::{Area, Options};
use wasmtime::{
    component::{Component, Linker, Resource},
    Config, Engine, Store,
};
use wasmtime_wasi::{
    self, bindings::sync::Command, ResourceTable, WasiCtx, WasiCtxBuilder, WasiView,
};

wasmtime::component::bindgen!({
    path: "../wit/shm.wit",
    world: "main",
    with: {
        "test:shm/exchange/memory": MyMemory,
    }
});

pub struct MyMemory(i32);

impl test::shm::exchange::HostMemory for HostState {
    fn new(&mut self, size: u32) -> Resource<MyMemory> {
        let mut chars = c"shm_XXXXXX".to_bytes_with_nul().iter().map(|c| *c as i8);
        let mut name: [i8; 11] = std::array::from_fn(|_n| chars.next().unwrap());
        let file = unsafe { libc::mkstemp(&mut name as *mut i8) };
        unsafe { libc::lseek64(file, (size as i64) - 1, libc::SEEK_SET) };
        unsafe { libc::write(file, (&0u8 as *const u8).cast(), 1) };
        self.table.push(MyMemory(file)).unwrap()
    }

    fn attach(&mut self, _self_: Resource<MyMemory>, _opt: Options) -> Area {
        // I think I will need to drill a hole into the component abstraction macros
        // to get a pointer to allocated memory
        //self.ctx().
        todo!()
    }

    fn detach(&mut self, _self_: Resource<MyMemory>, _consumed: u32) {
        todo!()
    }

    fn drop(&mut self, _rep: Resource<MyMemory>) -> wasmtime::Result<()> {
        todo!()
    }
}

struct HostState {
    ctx: WasiCtx,
    table: ResourceTable,
}

impl Default for HostState {
    fn default() -> Self {
        let mut builder = WasiCtxBuilder::new();
        builder.inherit_stdio();
        Self {
            ctx: builder.build(),
            table: Default::default(),
        }
    }
}

impl WasiView for HostState {
    fn ctx(&mut self) -> &mut WasiCtx {
        &mut self.ctx
    }
    fn table(&mut self) -> &mut ResourceTable {
        &mut self.table
    }
}

impl test::shm::exchange::Host for HostState {}

fn main() -> anyhow::Result<()> {
    let mut config = Config::new();
    config.wasm_component_model(true);

    let engine = Engine::new(&config)?;
    let mut store = Store::new(&engine, HostState::default());

    let wasm_module_path = "combined.wasm";
    let component = Component::from_file(&engine, wasm_module_path)?;

    let mut linker = Linker::new(&engine);
    test::shm::exchange::add_to_linker(&mut linker, |s| s)?;
    wasmtime_wasi::add_to_linker_sync(&mut linker)?;

    let command = Command::instantiate(&mut store, &component, &linker)?;

    command.wasi_cli_run().call_run(&mut store)?.ok();

    Ok(())
}
