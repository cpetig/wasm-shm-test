#[repr(C)]
pub struct Area {
    pub addr: *mut (),
    pub size: usize,
}

unsafe extern "C" {
    pub fn shm_create(size: usize) -> usize;
    pub fn shm_attach(handle: usize, flags: u32) -> Area;
    pub fn shm_detach(handle: usize, consumed: usize);
    pub fn shm_drop(handle: usize);
}
