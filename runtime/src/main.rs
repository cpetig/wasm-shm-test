use std::ffi::c_void;

use wasmtime::{
    component::{Component, Linker},
    Config, Engine, Store,
};
use wasmtime_wasi::{
    self,
    p2::{bindings::sync::Command, IoView, WasiCtx, WasiCtxBuilder, WasiView},
    ResourceTable,
};

wasmtime::component::bindgen!({
    path: "../wit/shm.wit",
    world: "main",
    include_generated_code_from_file: true,
    with: {
        "test:shm/exchange/memory": MyMemory,
    }
});

#[derive(Clone)]
pub struct MyMemory {
    size: u32,
    file: i32,
    buffer_addr: u32,
    buffer_size: u32,
    attached_addr: *const c_void,
}

unsafe impl Send for MyMemory {}

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
}

impl IoView for HostState {
    fn table(&mut self) -> &mut ResourceTable {
        &mut self.table
    }
}

struct SendMemory<T>(*mut T);

unsafe impl<T> Send for SendMemory<T> {}

mod myshm {
    use std::ffi::{c_char, c_void};

    use super::test::shm::exchange::{AttachOptions, Error, MemoryArea};
    use super::MyMemory;
    use super::SendMemory;
    use wasmtime::{
        component::{
            ComponentType, Lift, Resource, ResourceType,
            __internal::{CanonicalAbiInfo, InstanceType, InterfaceType, LiftContext},
        },
        StoreContextMut,
    };
    use wasmtime_wasi::p2::WasiView;

    // Hack: We wrap this type to remember the pointer to the linear memory from the lifting
    struct WrappedMemory {
        inner: Resource<MyMemory>,
        linear: SendMemory<u8>,
    }

    unsafe impl ComponentType for WrappedMemory {
        type Lower = <Resource<MyMemory> as ComponentType>::Lower;
        const ABI: CanonicalAbiInfo = <Resource<MyMemory> as ComponentType>::ABI;
        fn typecheck(ty: &InterfaceType, types: &InstanceType<'_>) -> anyhow::Result<()> {
            <Resource<MyMemory> as ComponentType>::typecheck(ty, types)
        }
    }

    unsafe impl Lift for WrappedMemory {
        fn linear_lift_from_flat(
            cx: &mut LiftContext<'_>,
            ty: InterfaceType,
            src: &Self::Lower,
        ) -> anyhow::Result<Self> {
            let linear = cx.memory().as_ptr().cast_mut();
            <Resource<MyMemory> as Lift>::linear_lift_from_flat(cx, ty, src).map(|a| {
                WrappedMemory {
                    inner: a,
                    linear: SendMemory(linear),
                }
            })
        }

        fn linear_lift_from_memory(
            cx: &mut LiftContext<'_>,
            ty: InterfaceType,
            bytes: &[u8],
        ) -> anyhow::Result<Self> {
            let linear = cx.memory().as_ptr().cast_mut();
            <Resource<MyMemory> as Lift>::linear_lift_from_memory(cx, ty, bytes).map(|a| {
                WrappedMemory {
                    inner: a,
                    linear: SendMemory(linear),
                }
            })
        }
    }

    unsafe impl Sync for WrappedMemory {}

    fn new<T: WasiView>(
        mut ctx: StoreContextMut<'_, T>,
        (size,): (u32,),
    ) -> wasmtime::Result<(Resource<MyMemory>,)> {
        let mut chars = c"shm_XXXXXX".to_bytes_with_nul().iter().map(|c| *c as i8);
        let mut name: [i8; 11] = std::array::from_fn(|_n| chars.next().unwrap());
        let file = unsafe { libc::mkstemp(&mut name as *mut i8) };
        unsafe { libc::unlink(&name as *const c_char) };
        unsafe { libc::lseek64(file, (size as i64) - 1, libc::SEEK_SET) };
        unsafe { libc::write(file, (&0u8 as *const u8).cast(), 1) };
        let view = ctx.data_mut();
        Ok((view.table().push(MyMemory {
            file,
            size,
            buffer_addr: 0,
            buffer_size: 0,
            attached_addr: std::ptr::null(),
        })?,))
    }

    fn dtor<T: WasiView>(mut ctx: StoreContextMut<'_, T>, objid: u32) -> wasmtime::Result<()> {
        let view = ctx.data_mut();
        let objid = Resource::new_own(objid);
        let obj: MyMemory = view.table().delete(objid).unwrap();
        unsafe { libc::close(obj.file) };
        Ok(())
    }

    fn attach<T: WasiView>(
        mut ctx: StoreContextMut<'_, T>,
        (
            WrappedMemory {
                inner: objid,
                linear,
            },
            flags,
        ): (WrappedMemory, AttachOptions),
    ) -> wasmtime::Result<(Result<MemoryArea, Error>,)> {
        let view = ctx.data_mut();
        let obj = view.table().get(&objid).unwrap().clone();
        let start = unsafe { linear.0.byte_add(obj.buffer_addr as usize) };
        let pagesize = unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as usize;
        let offset = start.align_offset(pagesize);
        let start = unsafe { start.add(offset) };
        dbg!((linear.0, obj.buffer_addr, start, offset, pagesize));
        let prot = if flags.contains(AttachOptions::WRITE) {
            libc::PROT_READ | libc::PROT_WRITE
        } else {
            libc::PROT_READ
        };
        let rounded = (obj.size + pagesize as u32 - 1) & (!(pagesize as u32 - 1));
        let addr = unsafe {
            libc::mmap(
                start.cast(),
                rounded as usize,
                prot,
                libc::MAP_SHARED | libc::MAP_FIXED,
                obj.file,
                0,
            )
        };
        if addr >= start.cast()
            && unsafe { addr.byte_add(obj.size as usize) }
                <= unsafe { start.byte_add(obj.buffer_size as usize) }.cast()
        {
            let obj = view.table().get_mut(&objid).unwrap();
            obj.attached_addr = addr;
            let linear_addr = unsafe { addr.byte_offset_from(linear.0.cast::<c_void>()) } as u32;
            Ok((Ok(MemoryArea {
                addr: linear_addr,
                size: obj.size,
            }),))
        } else {
            Ok((Err(Error::Internal),))
        }
    }

    fn detach<T: WasiView>(
        mut ctx: StoreContextMut<'_, T>,
        (objid, _consumed): (Resource<MyMemory>, u32),
    ) -> wasmtime::Result<()> {
        let view = ctx.data_mut();
        let obj = view.table().get_mut(&objid).unwrap();
        let pagesize = unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as u32;
        let rounded = (obj.size + pagesize - 1) & (!(pagesize - 1));
        let base = obj.attached_addr;
        unsafe { libc::munmap(base.cast_mut(), rounded as usize) };
        obj.attached_addr = std::ptr::null();
        Ok(())
    }

    fn minimum_size<T: WasiView>(
        mut ctx: StoreContextMut<'_, T>,
        (objid,): (Resource<MyMemory>,),
    ) -> wasmtime::Result<(u32,)> {
        let view = ctx.data_mut();
        let obj = view.table().get(&objid).unwrap();
        let pagesize = unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as u32;
        Ok((obj.size + 2 * pagesize,))
    }

    fn add_storage<T: WasiView>(
        mut ctx: StoreContextMut<'_, T>,
        (objid, area): (Resource<MyMemory>, MemoryArea),
    ) -> wasmtime::Result<(Result<(), Error>,)> {
        let view = ctx.data_mut();
        let obj = view.table().get_mut(&objid).unwrap();
        let pagesize = unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as u32;
        if area.size < obj.size + 2 * pagesize {
            return Ok((Err(Error::WrongSize),));
        }
        obj.buffer_addr = area.addr;
        obj.buffer_size = area.size;
        Ok((Ok(()),))
    }

    fn create_local<T: WasiView>(
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
        // shm.insert("[method]memory.attach", Definition::Func(HostFunc::new(attach::<T>)))?;
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
    wasmtime_wasi::p2::add_to_linker_sync(&mut linker)?;

    let command = Command::instantiate(&mut store, &component, &linker)?;

    command.wasi_cli_run().call_run(&mut store)?.ok();

    Ok(())
}
