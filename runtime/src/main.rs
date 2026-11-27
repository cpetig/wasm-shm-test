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
    with: {
        "test:shm/exchange.memory-block": MyMemory,
        "test:shm/exchange.address": MyAddress,
    }
});

fn page_size() -> u64 {
    unsafe { libc::sysconf(libc::_SC_PAGESIZE) as u64 }
}

/// the unlinked temp file backing a block and all of its partitions
pub struct Storage {
    file: i32,
}

impl Storage {
    fn new(size: u32) -> Self {
        use std::ffi::c_char;

        let mut chars = c"shm_XXXXXX".to_bytes_with_nul().iter().map(|c| *c as i8);
        let mut name: [i8; 11] = std::array::from_fn(|_n| chars.next().unwrap());
        let file = unsafe { libc::mkstemp(&mut name as *mut i8) };
        unsafe { libc::unlink(&name as *const c_char) };
        unsafe { libc::lseek64(file, (size as i64) - 1, libc::SEEK_SET) };
        unsafe { libc::write(file, (&0u8 as *const u8).cast(), 1) };
        Storage { file }
    }
}

impl Drop for Storage {
    fn drop(&mut self) {
        unsafe { libc::close(self.file) };
    }
}

/// a block or partition, shared by all clones (rights and attach state are per block)
pub struct MyMemoryInner {
    storage: std::sync::Arc<Storage>,
    /// position of this block inside of the file
    offset: u64,
    size: u32,
    /// false for read-only partitions (and clones of them)
    writable: bool,
    /// attach exclusion: -1 = write attached, n >= 0: number of read attachments
    busy: std::sync::Mutex<i32>,
    /// partitions: keeps the parent (and thus the registry) alive, remembers our range
    parent: Option<std::sync::Arc<MyMemoryInner>>,
    start: u32,
    /// ranges (start, size) of the live partitions of this block
    partitions: std::sync::Mutex<Vec<(u32, u32)>>,
}

impl MyMemoryInner {
    fn new(size: u32) -> Self {
        MyMemoryInner {
            storage: std::sync::Arc::new(Storage::new(size)),
            offset: 0,
            size,
            writable: true,
            busy: std::sync::Mutex::new(0),
            parent: None,
            start: 0,
            partitions: std::sync::Mutex::new(Vec::new()),
        }
    }

    /// bytes between the page boundary at or below our start and our start
    fn interior(&self) -> u64 {
        self.offset % page_size()
    }
}

impl Drop for MyMemoryInner {
    fn drop(&mut self) {
        // last handle of a partition is gone: its range is free again
        if let Some(parent) = &self.parent {
            let mut list = parent.partitions.lock().unwrap();
            if let Some(pos) = list
                .iter()
                .position(|&(s, n)| s == self.start && n == self.size)
            {
                list.swap_remove(pos);
            }
        }
    }
}

/// one attachment of a handle
struct Attached {
    /// where the file is mapped (page aligned) and how much
    map_addr: *mut c_void,
    map_len: usize,
    /// the donated storage it was taken from
    linear: MemoryId,
    region: usize,
    /// offset of the mapping inside of that region
    region_off: u32,
}

#[derive(Clone)]
pub struct MyMemory {
    inner: std::sync::Arc<MyMemoryInner>,
    // the attachments of this handle (not shared with clones), detach pops the last one
    attached: std::sync::Arc<std::sync::Mutex<Vec<Attached>>>,
}

unsafe impl Send for MyMemory {}
unsafe impl Sync for MyMemory {}

pub struct MyAddress;

/// storage donated by add-storage, handed out in page aligned pieces
struct Mapping {
    addr: u32,
    size: u32,
    /// pieces in use: (offset inside of the region, length)
    used: Vec<(u32, u32)>,
}

impl Mapping {
    /// find room for `len` bytes (a multiple of the page size) at a page boundary of the
    /// guest's address space, `linear` is the host address of the linear memory
    fn allocate(&mut self, linear: *mut u8, len: u32) -> Option<u32> {
        let pagesize = page_size();
        let base = linear as u64 + self.addr as u64;
        let first = (base.next_multiple_of(pagesize) - base) as u32;
        let mut candidate = first;
        let mut used = self.used.clone();
        used.sort();
        for (s, n) in used {
            if candidate as u64 + len as u64 <= s as u64 {
                break;
            }
            candidate = candidate.max(s + n);
        }
        if candidate as u64 + len as u64 <= self.size as u64 {
            self.used.push((candidate, len));
            Some(candidate)
        } else {
            None
        }
    }
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
    use std::collections::HashMap;

    use crate::{page_size, Attached, GetBuffers, MemoryId, MyMemoryInner};

    use super::test::shm::exchange::{AttachOptions, Bytes, Error, MemoryArea};
    use super::{Mapping, MyAddress, MyMemory, SendMemory};
    use wasmtime::component::Lower;
    use wasmtime::{
        component::{
            __internal::{
                CanonicalAbiInfo, InstanceType, InterfaceType, LiftContext, LowerContext,
            },
            ComponentType, Lift, Resource, ResourceType,
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
        fn typecheck(ty: &InterfaceType, types: &InstanceType<'_>) -> wasmtime::Result<()> {
            <Resource<MyMemory> as ComponentType>::typecheck(ty, types)
        }
    }

    unsafe impl Lift for WrappedMemory {
        fn linear_lift_from_flat(
            cx: &mut LiftContext<'_>,
            ty: InterfaceType,
            src: &Self::Lower,
        ) -> wasmtime::Result<Self> {
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
        ) -> wasmtime::Result<Self> {
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
        fn typecheck(ty: &InterfaceType, types: &InstanceType<'_>) -> wasmtime::Result<()> {
            <MemoryArea as ComponentType>::typecheck(ty, types)
        }
    }

    unsafe impl Lift for WrappedArea {
        fn linear_lift_from_flat(
            cx: &mut LiftContext<'_>,
            _ty: InterfaceType,
            src: &Self::Lower,
        ) -> wasmtime::Result<Self> {
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
        ) -> wasmtime::Result<Self> {
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
        ) -> wasmtime::Result<()> {
            todo!()
        }

        fn linear_lower_to_memory<T>(
            &self,
            cx: &mut LowerContext<'_, T>,
            _ty: InterfaceType,
            offset: usize,
        ) -> wasmtime::Result<()> {
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
            attached: Default::default(),
        })?,))
    }

    /// undo one attachment: restore anonymous memory, free the storage, release the exclusion
    fn release(buffers: &mut HashMap<MemoryId, Vec<Mapping>>, inner: &MyMemoryInner, att: Attached) {
        // restore a zero filled anonymous mapping instead of leaving a hole (munmap) in the
        // guest's linear memory
        let res = unsafe {
            libc::mmap(
                att.map_addr,
                att.map_len,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_FIXED | libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
                -1,
                0,
            )
        };
        assert!(res != libc::MAP_FAILED);
        if let Some(region) = buffers
            .get_mut(&att.linear)
            .and_then(|l| l.get_mut(att.region))
        {
            if let Some(pos) = region
                .used
                .iter()
                .position(|&u| u == (att.region_off, att.map_len as u32))
            {
                region.used.swap_remove(pos);
            }
        }
        let mut busy = inner.busy.lock().unwrap();
        if *busy < 0 {
            *busy = 0;
        } else {
            *busy -= 1;
        }
    }

    fn dtor<T: WasiView + IoView + GetBuffers>(
        mut ctx: StoreContextMut<'_, T>,
        objid: u32,
    ) -> wasmtime::Result<()> {
        let view = ctx.data_mut();
        let objid = Resource::new_own(objid);
        let obj: MyMemory = view.table().delete(objid).unwrap();
        // a dropped handle detaches what it still has attached
        let pending: Vec<Attached> = obj.attached.lock().unwrap().drain(..).collect();
        for att in pending {
            release(view.get_buffers(), &obj.inner, att);
        }
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
        let inner = &obj.inner;
        let write = flags.contains(AttachOptions::WRITE);
        if write && !inner.writable {
            return Ok((Err(Error::ReadOnly),));
        }
        // Map the file from the page boundary at or below our start and cover start..start+size.
        // The bytes around an unaligned partition in these pages become visible as well
        // ("leak neighbors"), the returned address points to our start.
        let pagesize = page_size();
        let interior = inner.interior();
        let map_len = (interior + inner.size as u64).next_multiple_of(pagesize);
        let file_off = inner.offset - interior;

        // rw is exclusive, ro is shared
        let mut busy = inner.busy.lock().unwrap();
        if *busy < 0 || (write && *busy != 0) {
            return Ok((Err(Error::Busy),));
        }
        let linear_id = MemoryId(linear.0.cast());
        // pick room in one of the donated regions of this memory
        let found = view.get_buffers().get_mut(&linear_id).and_then(|l| {
            l.iter_mut().enumerate().find_map(|(idx, region)| {
                let off = region.allocate(linear.0, map_len as u32)?;
                Some((idx, off, region.addr))
            })
        });
        let Some((region, region_off, region_addr)) = found else {
            return Ok((Err(Error::NoStorage),));
        };
        let target = unsafe { linear.0.byte_add(region_addr as usize + region_off as usize) };
        let prot = if write {
            libc::PROT_READ | libc::PROT_WRITE
        } else {
            libc::PROT_READ
        };
        let addr = unsafe {
            libc::mmap(
                target.cast(),
                map_len as usize,
                prot,
                libc::MAP_SHARED | libc::MAP_FIXED,
                inner.storage.file,
                file_off as i64,
            )
        };
        let att = Attached {
            map_addr: target.cast(),
            map_len: map_len as usize,
            linear: linear_id,
            region,
            region_off,
        };
        if addr == libc::MAP_FAILED || addr != target.cast() {
            // only give the room back, nothing got mapped (busy is untouched)
            let region = &mut view.get_buffers().get_mut(&linear_id).unwrap()[att.region];
            region.used.retain(|&u| u != (att.region_off, att.map_len as u32));
            return Ok((Err(Error::Internal),));
        }
        *busy = if write { -1 } else { *busy + 1 };
        drop(busy);
        let linear_addr = region_addr + region_off + interior as u32;
        obj.attached.lock().unwrap().push(att);
        Ok((Ok(WrappedArea {
            addr: linear_addr,
            size: inner.size,
            linear,
        }),))
    }

    fn detach<T: WasiView + IoView + GetBuffers>(
        mut ctx: StoreContextMut<'_, T>,
        (objid, _consumed): (Resource<MyMemory>, u32),
    ) -> wasmtime::Result<()> {
        let view = ctx.data_mut();
        let obj = view.table().get(&objid).unwrap().clone();
        let att = obj.attached.lock().unwrap().pop();
        if let Some(att) = att {
            release(view.get_buffers(), &obj.inner, att);
        }
        Ok(())
    }

    fn minimum_size<T: WasiView + IoView>(
        mut ctx: StoreContextMut<'_, T>,
        (objid,): (Resource<MyMemory>,),
    ) -> wasmtime::Result<(u32,)> {
        let view = ctx.data_mut();
        let obj = view.table().get(&objid).unwrap();
        // interior offset of a partition + alignment and rounding slack
        Ok((obj.inner.size + obj.inner.interior() as u32 + 2 * page_size() as u32,))
    }

    fn optimum_size<T: WasiView>(
        _ctx: StoreContextMut<'_, T>,
        (count, size): (u32, u32),
    ) -> wasmtime::Result<(u32,)> {
        let pagesize = page_size() as u32;
        // each attachment may start anywhere inside of a page and is rounded up to pages,
        // plus the alignment of the start of the region
        Ok((count * (size + 2 * pagesize) + pagesize,))
    }

    fn add_storage<T: WasiView + IoView + GetBuffers>(
        mut ctx: StoreContextMut<'_, T>,
        (area,): (WrappedArea,),
    ) -> wasmtime::Result<(Result<(), Error>,)> {
        let view = ctx.data_mut();
        let buffers = view.get_buffers();
        let pagesize = page_size() as u32;
        if area.size < 2 * pagesize {
            return Ok((Err(Error::WrongSize),));
        }
        let mapping = super::Mapping {
            addr: area.addr,
            size: area.size,
            used: Vec::new(),
        };
        buffers
            .entry(MemoryId(area.linear.0.cast()))
            .or_default()
            .push(mapping);
        Ok((Ok(()),))
    }

    fn create_partition<T: WasiView + IoView>(
        mut ctx: StoreContextMut<'_, T>,
        (objid, start, size, mode): (Resource<MyMemory>, u32, u32, AttachOptions),
    ) -> wasmtime::Result<(Result<Resource<MyMemory>, Error>,)> {
        let view = ctx.data_mut();
        let parent = std::sync::Arc::clone(&view.table().get(&objid).unwrap().inner);
        let write = mode.contains(AttachOptions::WRITE);
        if write && !parent.writable {
            return Ok((Err(Error::ReadOnly),));
        }
        let end = start as u64 + size as u64;
        if size == 0 || end > parent.size as u64 {
            return Ok((Err(Error::OutOfRange),));
        }
        // check and register under one lock
        let mut list = parent.partitions.lock().unwrap();
        if list.iter().any(|&(s, n)| start < s + n && s < end as u32) {
            return Ok((Err(Error::Overlap),));
        }
        list.push((start, size));
        drop(list);
        // TODO: lock the range in the parent (see shm.wit), for now the parent can
        // still attach all of its bytes independent of the partitions
        let inner = MyMemoryInner {
            storage: std::sync::Arc::clone(&parent.storage),
            offset: parent.offset + start as u64,
            size,
            writable: write,
            busy: std::sync::Mutex::new(0),
            start,
            parent: Some(parent),
            partitions: std::sync::Mutex::new(Vec::new()),
        };
        let res = view.table().push(MyMemory {
            inner: std::sync::Arc::new(inner),
            attached: Default::default(),
        })?;
        Ok((Ok(res),))
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
        // same block (rights, attach state), but its own attachments
        let obj2 = MyMemory {
            attached: Default::default(),
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
        shm.resource("memory-block", ResourceType::host::<MyMemory>(), dtor::<T>)?;
        shm.func_wrap("[constructor]memory-block", new::<T>)?;
        shm.func_wrap("[method]memory-block.attach", attach::<T>)?;
        shm.func_wrap("[method]memory-block.detach", detach::<T>)?;
        shm.func_wrap("[method]memory-block.minimum-size", minimum_size::<T>)?;
        shm.func_wrap("[method]memory-block.clone", mem_clone::<T>)?;
        shm.func_wrap("[method]memory-block.create-partition", create_partition::<T>)?;
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
            //.async_support(true)
            .wasm_component_model(true)
            .wasm_component_model_async(true);

        let engine = Engine::new(&config)?;
        let mut store = Store::new(&engine, HostState::default());

        // the composed component to run, see README
        let wasm_module_path = std::env::args()
            .nth(1)
            .unwrap_or_else(|| "combined.wasm".into());
        let component = Component::from_file(&engine, &wasm_module_path)
            .map_err(|e| e.context(format!("loading {wasm_module_path}")))?;

        let mut linker = Linker::new(&engine);
        myshm::add_to_linker(&mut linker)?;
        wasmtime_wasi::p2::add_to_linker_async(&mut linker)?;
        wasmtime_wasi::p3::add_to_linker(&mut linker)?;

        let instance = linker.instantiate_async(&mut store, &component).await?;
        // prefer an asynchronous WASI 0.3 wasi:cli/run export, fall back to the synchronous 0.2 one
        let is_p3 = component
            .component_type()
            .exports(&engine)
            .any(|(name, _)| name.starts_with("wasi:cli/run@0.3"));
        if is_p3 {
            let command = wasmtime_wasi::p3::bindings::Command::new(&mut store, &instance)?;
            store
                .run_concurrent(async move |accessor| {
                    command.wasi_cli_run().call_run(accessor).await
                })
                .await??
                .map_err(|_e| anyhow::Error::msg("fail?"))?;
        } else {
            let command = Command::new(&mut store, &instance)?;
            command
                .wasi_cli_run()
                .call_run(&mut store)
                .await?
                .map_err(|_e| anyhow::Error::msg("fail?"))?;
        }
        Ok::<(), anyhow::Error>(())
    };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()?;
    let _res = runtime.block_on(future)?;

    Ok(())
}
