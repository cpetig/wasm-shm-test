use std::{collections::HashMap, ffi::c_void};

use wasmtime::{
    component::{Component, Linker},
    Config, Engine, Store,
};
use wasmtime_wasi::{
    self, p2::bindings::Command, ResourceTable, WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView,
};
use wasmtime_wasi_io::IoView;

wasmtime::component::bindgen!({
    path: "../wit/shm.wit",
    world: "main",
    include_generated_code_from_file: true,
    debug: true,
    with: {
        "test:shm/exchange/memory-block": MyMemory,
        "test:shm/exchange/address": MyAddress,
    }
});

pub struct MyMemoryInner {
    size: u32,
    file: i32,
}

impl MyMemoryInner {
    fn new(size: u32) -> Self {
        use std::ffi::c_char;

        let mut chars = c"shm_XXXXXX".to_bytes_with_nul().iter().map(|c| *c as i8);
        let mut name: [i8; 11] = std::array::from_fn(|_n| chars.next().unwrap());
        let file = unsafe { libc::mkstemp(&mut name as *mut i8) };
        unsafe { libc::unlink(&name as *const c_char) };
        unsafe { libc::lseek64(file, (size as i64) - 1, libc::SEEK_SET) };
        unsafe { libc::write(file, (&0u8 as *const u8).cast(), 1) };
        MyMemoryInner { size, file }
    }
}

impl Drop for MyMemoryInner {
    fn drop(&mut self) {
        unsafe { libc::close(self.file) };
    }
}

#[derive(Clone)]
pub struct MyMemory {
    inner: std::sync::Arc<MyMemoryInner>,
    attached_addr: *const c_void,
}

unsafe impl Send for MyMemory {}

pub struct MyAddress;

struct Mapping {
    addr: u32,
    size: u32,
}

#[derive(Copy, Clone, Eq, Hash, PartialEq)]
struct MemoryId(*const ());

unsafe impl Send for MemoryId {}

trait GetBuffers {
    fn get_buffers(&mut self) -> &mut HashMap<MemoryId, Vec<Mapping>>;
}

struct HostState {
    ctx: WasiCtx,
    table: ResourceTable,
    buffers: HashMap<MemoryId, Vec<Mapping>>,
}

impl Default for HostState {
    fn default() -> Self {
        let mut builder = WasiCtxBuilder::new();
        builder.inherit_stdio();
        Self {
            ctx: builder.build(),
            table: Default::default(),
            buffers: HashMap::new(),
        }
    }
}

impl WasiView for HostState {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.ctx,
            table: &mut self.table,
        }
    }
}

impl IoView for HostState {
    fn table(&mut self) -> &mut ResourceTable {
        &mut self.table
    }
}

impl GetBuffers for HostState {
    fn get_buffers(&mut self) -> &mut HashMap<MemoryId, Vec<Mapping>> {
        &mut self.buffers
    }
}

struct SendMemory<T>(*mut T);

unsafe impl<T> Send for SendMemory<T> {}

mod myshm {
    use std::ffi::c_void;

    use crate::{GetBuffers, MemoryId, MyMemoryInner};

    use super::test::shm::exchange::{AttachOptions, Bytes, Error, MemoryArea};
    use super::{Mapping, MyAddress, MyMemory, SendMemory};
    use wasmtime::component::Lower;
    use wasmtime::{
        component::{
            ComponentType, Lift, Resource, ResourceType,
            __internal::{
                CanonicalAbiInfo, InstanceType, InterfaceType, LiftContext, LowerContext,
            },
        },
        StoreContextMut,
    };
    use wasmtime_wasi::WasiView;
    use wasmtime_wasi_io::IoView;

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

    struct WrappedArea {
        addr: u32,
        size: Bytes,
        linear: SendMemory<u8>,
    }

    #[derive(Copy, Clone)]
    struct AreaLower {
        addr: wasmtime::ValRaw,
        size: wasmtime::ValRaw,
    }

    unsafe impl ComponentType for WrappedArea {
        type Lower = AreaLower;
        const ABI: CanonicalAbiInfo = <MemoryArea as ComponentType>::ABI;
        fn typecheck(ty: &InterfaceType, types: &InstanceType<'_>) -> anyhow::Result<()> {
            <MemoryArea as ComponentType>::typecheck(ty, types)
        }
    }

    unsafe impl Lift for WrappedArea {
        fn linear_lift_from_flat(
            cx: &mut LiftContext<'_>,
            _ty: InterfaceType,
            src: &Self::Lower,
        ) -> anyhow::Result<Self> {
            // if !cx.options.has_memory() {
            //     dbg!(cx.instance_mut().component().get_export(None, "memory"));
            //     anyhow::bail!("WrappedArea without memory")
            // }
            let linear = cx.memory().as_ptr().cast_mut();
            let addr = u32::linear_lift_from_flat(cx, InterfaceType::U32, &src.addr)?;
            let size = u32::linear_lift_from_flat(cx, InterfaceType::U32, &src.size)?;
            Ok(WrappedArea {
                addr,
                size,
                linear: SendMemory(linear),
            })
        }

        fn linear_lift_from_memory(
            cx: &mut LiftContext<'_>,
            _ty: InterfaceType,
            bytes: &[u8],
        ) -> anyhow::Result<Self> {
            let linear = cx.memory().as_ptr().cast_mut();
            let addr = u32::linear_lift_from_memory(cx, InterfaceType::U32, &bytes[0..4])?;
            let size = u32::linear_lift_from_memory(cx, InterfaceType::U32, &bytes[4..8])?;
            Ok(WrappedArea {
                addr,
                size,
                linear: SendMemory(linear),
            })
        }
    }

    unsafe impl Lower for WrappedArea {
        fn linear_lower_to_flat<T>(
            &self,
            _cx: &mut LowerContext<'_, T>,
            _ty: InterfaceType,
            _dst: &mut std::mem::MaybeUninit<Self::Lower>,
        ) -> anyhow::Result<()> {
            todo!()
        }

        fn linear_lower_to_memory<T>(
            &self,
            cx: &mut LowerContext<'_, T>,
            _ty: InterfaceType,
            offset: usize,
        ) -> anyhow::Result<()> {
            u32::linear_lower_to_memory(&self.addr, cx, InterfaceType::U32, offset)?;
            u32::linear_lower_to_memory(&self.size, cx, InterfaceType::U32, offset + 4)?;
            Ok(())
        }
    }

    unsafe impl Sync for WrappedArea {}

    fn new<T: WasiView + IoView>(
        mut ctx: StoreContextMut<'_, T>,
        (size,): (u32,),
    ) -> wasmtime::Result<(Resource<MyMemory>,)> {
        let view = ctx.data_mut();
        Ok((view.table().push(MyMemory {
            inner: std::sync::Arc::new(MyMemoryInner::new(size)),
            attached_addr: std::ptr::null(),
        })?,))
    }

    fn dtor<T: WasiView + IoView>(
        mut ctx: StoreContextMut<'_, T>,
        objid: u32,
    ) -> wasmtime::Result<()> {
        let view = ctx.data_mut();
        let objid = Resource::new_own(objid);
        let _obj: MyMemory = view.table().delete(objid).unwrap();
        Ok(())
    }

    fn attach<T: WasiView + IoView + GetBuffers>(
        mut ctx: StoreContextMut<'_, T>,
        (
            WrappedMemory {
                inner: objid,
                linear,
            },
            flags,
        ): (WrappedMemory, AttachOptions),
    ) -> wasmtime::Result<(Result<WrappedArea, Error>,)> {
        let view = ctx.data_mut();
        let obj = view.table().get(&objid).unwrap().clone();
        let linear_list = view.get_buffers().get(&MemoryId(linear.0.cast()));
        let mapping = linear_list.and_then(|l| l.get(0));
        if let Some(Mapping {
            addr: buffer_addr,
            size: buffer_size,
        }) = mapping
        {
            let start = unsafe { linear.0.byte_add(*buffer_addr as usize) };
            let pagesize = unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as usize;
            let offset = start.align_offset(pagesize);
            let start = unsafe { start.add(offset) };
            // dbg!((linear.0, buffer_addr, start, offset, pagesize));
            let prot = if flags.contains(AttachOptions::WRITE) {
                libc::PROT_READ | libc::PROT_WRITE
            } else {
                libc::PROT_READ
            };
            let rounded = (obj.inner.size + pagesize as u32 - 1) & (!(pagesize as u32 - 1));
            let addr = unsafe {
                libc::mmap(
                    start.cast(),
                    rounded as usize,
                    prot,
                    libc::MAP_SHARED | libc::MAP_FIXED,
                    obj.inner.file,
                    0,
                )
            };
            if addr.addr() == 0xffff_ffff_ffff_ffff {
                return Ok((Err(Error::Internal),));
            }
            // dbg!((rounded, addr));
            if addr >= start.cast()
                && unsafe { addr.byte_add(obj.inner.size as usize) }
                    <= unsafe { start.byte_add(*buffer_size as usize) }.cast()
            {
                let obj = view.table().get_mut(&objid).unwrap();
                obj.attached_addr = addr;
                let linear_addr =
                    unsafe { addr.byte_offset_from(linear.0.cast::<c_void>()) } as u32;
                Ok((Ok(WrappedArea {
                    addr: linear_addr,
                    size: obj.inner.size,
                    linear,
                }),))
            } else {
                Ok((Err(Error::Internal),))
            }
        } else {
            dbg!(linear.0);
            Ok((Err(Error::Internal),))
        }
    }

    fn detach<T: WasiView + IoView>(
        mut ctx: StoreContextMut<'_, T>,
        (objid, _consumed): (Resource<MyMemory>, u32),
    ) -> wasmtime::Result<()> {
        let view = ctx.data_mut();
        let obj = view.table().get_mut(&objid).unwrap();
        let pagesize = unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as u32;
        let rounded = (obj.inner.size + pagesize - 1) & (!(pagesize - 1));
        let base = obj.attached_addr;
        let res = unsafe { libc::munmap(base.cast_mut(), rounded as usize) };
        assert!(res == 0);
        obj.attached_addr = std::ptr::null();
        Ok(())
    }

    fn minimum_size<T: WasiView + IoView>(
        mut ctx: StoreContextMut<'_, T>,
        (objid,): (Resource<MyMemory>,),
    ) -> wasmtime::Result<(u32,)> {
        let view = ctx.data_mut();
        let obj = view.table().get(&objid).unwrap();
        let pagesize = unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as u32;
        Ok((obj.inner.size + 2 * pagesize,))
    }

    fn optimum_size<T: WasiView>(
        _ctx: StoreContextMut<'_, T>,
        (count, size): (u32, u32),
    ) -> wasmtime::Result<(u32,)> {
        let pagesize = unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as u32;
        Ok((count * size + 2 * pagesize,))
    }

    fn add_storage<T: WasiView + IoView + GetBuffers>(
        mut ctx: StoreContextMut<'_, T>,
        (area,): (WrappedArea,),
    ) -> wasmtime::Result<(Result<(), Error>,)> {
        let view = ctx.data_mut();
        let buffers = view.get_buffers();
        // dbg!((&area.size, &area.addr, &area.linear.0));
        let pagesize = unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as u32;
        if area.size < 2 * pagesize {
            return Ok((Err(Error::WrongSize),));
        }
        let mapping = super::Mapping {
            addr: area.addr,
            size: area.size,
        };
        buffers
            .entry(MemoryId(area.linear.0.cast()))
            .or_default()
            .push(mapping);
        Ok((Ok(()),))
    }

    fn create_local<T: WasiView>(
        _ctx: StoreContextMut<'_, T>,
        (_area,): (WrappedArea,),
    ) -> wasmtime::Result<(Resource<MyMemory>,)> {
        todo!()
    }

    fn mem_clone<T: WasiView + IoView>(
        mut ctx: StoreContextMut<'_, T>,
        (objid,): (Resource<MyMemory>,),
    ) -> wasmtime::Result<(Resource<MyMemory>,)> {
        let view = ctx.data_mut();
        let obj = view.table().get(&objid).unwrap();
        let obj2 = MyMemory {
            attached_addr: std::ptr::null(),
            inner: std::sync::Arc::clone(&obj.inner),
        };
        let res = view.table().push(obj2)?;
        Ok((res,))
    }

    fn ignore<T: WasiView>(_ctx: StoreContextMut<'_, T>, _obj: u32) -> wasmtime::Result<()> {
        todo!()
    }

    pub(crate) fn add_to_linker<T: WasiView + IoView + GetBuffers + 'static>(
        l: &mut wasmtime::component::Linker<T>,
    ) -> wasmtime::Result<()> {
        let mut root = l.root();
        let mut shm = root.instance("test:shm/exchange")?;
        shm.resource("address", ResourceType::host::<MyAddress>(), ignore::<T>)?;
        shm.resource("memory-block", ResourceType::host::<MyMemory>(), dtor)?;
        shm.func_wrap("[constructor]memory-block", new::<T>)?;
        shm.func_wrap("[method]memory-block.attach", attach::<T>)?;
        shm.func_wrap("[method]memory-block.detach", detach::<T>)?;
        shm.func_wrap("[method]memory-block.minimum-size", minimum_size::<T>)?;
        shm.func_wrap("[method]memory-block.clone", mem_clone::<T>)?;
        shm.func_wrap("[static]memory-block.optimum-size", optimum_size::<T>)?;
        shm.func_wrap("[static]memory-block.add-storage", add_storage::<T>)?;
        shm.func_wrap("[static]memory-block.create-local", create_local::<T>)?;
        Ok(())
    }
}

fn main() -> anyhow::Result<()> {
    let future = async move {
        let mut config = Config::new();
        config
            .async_support(true)
            .wasm_component_model(true)
            .wasm_component_model_async(true);

        let engine = Engine::new(&config)?;
        let mut store = Store::new(&engine, HostState::default());

        let wasm_module_path = "combined.wasm";
        let component = Component::from_file(&engine, wasm_module_path)?;

        let mut linker = Linker::new(&engine);
        myshm::add_to_linker(&mut linker)?;
        wasmtime_wasi::p2::add_to_linker_async(&mut linker)?;
        wasmtime_wasi::p3::add_to_linker(&mut linker)?;

        let instance = linker.instantiate_async(&mut store, &component).await?;
        let command = Command::new(&mut store, &instance)?;
        // instance
        //     .run_concurrent(&mut store, async move |store| {
        //         command.wasi_cli_run().call_run(store).await
        //     })
        command
            .wasi_cli_run()
            .call_run(&mut store)
            .await?
            .map_err(|_e| anyhow::Error::msg("fail?"))?;
        Ok::<(), anyhow::Error>(())
    };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()?;
    let _res = runtime.block_on(future)?;

    Ok(())
}
