use wasmtime::{
    component::{Component, Linker},
    Config, Engine, Store,
};
use wasmtime_wasi::{
    self, bindings::sync::Command, ResourceTable, WasiCtx, WasiCtxBuilder, WasiView,
};

wasmtime::component::bindgen!({
    path: "../wit/shm.wit",
    world: "main",
    include_generated_code_from_file: true,
    with: {
        "test:shm/exchange/memory": MyMemory,
    }
});

pub struct MyMemory {
    size: u32,
    file: i32,
    buffer_addr: u32,
    buffer_size: u32,
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

mod myshm {
    use super::test::shm::exchange::{AttachOptions, Error, MemoryArea};
    use wasmtime::{
        component::{Resource, ResourceType, Val},
        StoreContextMut,
    };
    use wasmtime_wasi::WasiView;

    const PAGESIZE: u32 = 4096;

    use super::MyMemory;

    fn new<T: WasiView>(
        mut ctx: StoreContextMut<'_, T>,
        (size,): (u32,),
    ) -> wasmtime::Result<(Resource<MyMemory>,)> {
        let mut chars = c"shm_XXXXXX".to_bytes_with_nul().iter().map(|c| *c as i8);
        let mut name: [i8; 11] = std::array::from_fn(|_n| chars.next().unwrap());
        let file = unsafe { libc::mkstemp(&mut name as *mut i8) };
        unsafe { libc::lseek64(file, (size as i64) - 1, libc::SEEK_SET) };
        unsafe { libc::write(file, (&0u8 as *const u8).cast(), 1) };
        let view = ctx.data_mut();
        Ok((view.table().push(MyMemory {
            file,
            size,
            buffer_addr: 0,
            buffer_size: 0,
        })?,))
    }

    fn dtor<T>(_ctx: StoreContextMut<'_, T>, _obj: u32) -> wasmtime::Result<()> {
        todo!()
    }

    fn attach<T>(
        _ctx: StoreContextMut<'_, T>,
        (objid, flags): (Resource<MyMemory>, AttachOptions),
    ) -> wasmtime::Result<(Result<MemoryArea, Error>,)> {
        todo!()
    }

    fn detach<T>(
        _ctx: StoreContextMut<'_, T>,
        (objid, consumed): (Resource<MyMemory>, u32),
    ) -> wasmtime::Result<()> {
        todo!()
    }

    fn minimum_size<T: WasiView>(
        mut ctx: StoreContextMut<'_, T>,
        (objid,): (Resource<MyMemory>,),
    ) -> wasmtime::Result<(u32,)> {
        let view = ctx.data_mut();
        let obj = view.table().get(&objid).unwrap();
        Ok((obj.size + 2 * PAGESIZE,))
    }

    fn add_storage<T: WasiView>(
        mut ctx: StoreContextMut<'_, T>,
        (objid, area): (Resource<MyMemory>, MemoryArea),
    ) -> wasmtime::Result<(Result<(), Error>,)> {
        let view = ctx.data_mut();
        let obj = view.table().get_mut(&objid).unwrap();
        if area.size < obj.size + 2 * PAGESIZE {
            return Ok((Err(Error::WrongSize),));
        }
        obj.buffer_addr = area.addr;
        obj.buffer_size = area.size;
        Ok((Ok(()),))
    }

    fn create_local<T>(
        _ctx: StoreContextMut<'_, T>,
        (_area,): (MemoryArea,),
    ) -> wasmtime::Result<(Resource<MyMemory>,)> {
        todo!()
    }

    pub(crate) fn add_to_linker<T: WasiView + 'static>(
        l: &mut wasmtime::component::Linker<T>,
    ) -> wasmtime::Result<()> {
        let mut root = l.root();
        let mut shm = root.instance("test:shm/exchange")?;
        shm.resource("memory", ResourceType::host::<MyMemory>(), dtor)?;
        shm.func_wrap("[constructor]memory", new::<T>)?;
        shm.func_wrap("[method]memory.attach", attach::<T>)?;
        shm.func_wrap("[method]memory.detach", detach::<T>)?;
        shm.func_wrap("[method]memory.minimum-size", minimum_size::<T>)?;
        shm.func_wrap("[method]memory.add-storage", add_storage::<T>)?;
        shm.func_wrap("[static]memory.create-local", create_local::<T>)?;
        Ok(())
    }
}

fn main() -> anyhow::Result<()> {
    let mut config = Config::new();
    config.wasm_component_model(true);

    let engine = Engine::new(&config)?;
    let mut store = Store::new(&engine, HostState::default());

    let wasm_module_path = "combined.wasm";
    let component = Component::from_file(&engine, wasm_module_path)?;

    let mut linker = Linker::new(&engine);
    myshm::add_to_linker(&mut linker)?;
    wasmtime_wasi::add_to_linker_sync(&mut linker)?;

    let command = Command::instantiate(&mut store, &component, &linker)?;

    command.wasi_cli_run().call_run(&mut store)?.ok();

    Ok(())
}
