use crate::ALLOCATOR;
use core::{
    alloc::{GlobalAlloc, Layout},
    sync::atomic::{AtomicUsize, Ordering},
};

use super::types::*;

extern "C" {
    fn create_mspace(capacity: size_t, locked: c_int) -> usize;
    fn create_mspace_with_base(base: *mut c_void, capacity: size_t, locked: c_int) -> usize;
    fn mspace_malloc(msp: usize, bytes: size_t) -> *mut c_void;
    fn mspace_memalign(msp: usize, alignment: size_t, bytes: size_t) -> *mut c_void;
    fn mspace_realloc(msp: usize, oldmem: *mut c_void, bytes: size_t) -> *mut c_void;
    fn mspace_free(msp: usize, mem: *mut c_void);
    //fn dlmalloc(bytes: size_t) -> *mut c_void;
    //fn dlmemalign(alignment: size_t, bytes: size_t) -> *mut c_void;
    //fn dlrealloc(oldmem: *mut c_void, bytes: size_t) -> *mut c_void;
    //fn dlfree(mem: *mut c_void);
}

pub struct Allocator {
    mstate: AtomicUsize,
}

pub const NEWALLOCATOR: Allocator = Allocator {
    mstate: AtomicUsize::new(0),
};

impl Allocator {
    pub fn set_book_keeper(&self, mstate: usize) {
        self.mstate.store(mstate, Ordering::Relaxed);
    }

    pub fn get_book_keeper(&self) -> usize {
        self.mstate.load(Ordering::Relaxed)
    }
}

unsafe impl<'a> GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        alloc_align(layout.size(), layout.align()) as *mut u8
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        free(ptr as *mut c_void)
    }
}

pub unsafe fn alloc(size: usize) -> *mut c_void {
    mspace_malloc(ALLOCATOR.get_book_keeper(), size)
}

pub unsafe fn alloc_align(size: usize, alignment: usize) -> *mut c_void {
    mspace_memalign(ALLOCATOR.get_book_keeper(), alignment, size)
}

pub unsafe fn realloc(ptr: *mut c_void, size: size_t) -> *mut c_void {
    mspace_realloc(ALLOCATOR.get_book_keeper(), ptr, size)
}

pub unsafe fn free(ptr: *mut c_void) {
    mspace_free(ALLOCATOR.get_book_keeper(), ptr)
}

#[cfg(not(target_os = "dragonos"))]
pub fn new_mspace() -> usize {
    unsafe { create_mspace(0, 0) }
}

#[cfg(target_os = "dragonos")]
pub fn new_mspace() -> usize {
    use core::sync::atomic::AtomicU8;

    use crate::header::stdlib::malloc;

    static mut space: [[u8; 128 * 16]; 2] = [[0; 128 * 16]; 2];
    static cnt: AtomicU8 = AtomicU8::new(0);
    let x = cnt.fetch_add(1, Ordering::Relaxed);
    if x > 2 {
        panic!("new_mspace: too many mspace");
    }
    //println!("I am here");
    //println!("{:#?}",unsafe{space[x as usize].as_mut_ptr()});
    let r = unsafe {
        create_mspace_with_base(space[x as usize].as_mut_ptr() as *mut c_void, 128 * 16, 0)
    };
    println!("new_mspace: {:#018x}", r);
    return r;
}
