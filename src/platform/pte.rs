#![allow(non_snake_case)]

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use core::sync::atomic::{AtomicUsize, Ordering, ATOMIC_USIZE_INIT};
use core::{intrinsics, ptr};

use header::time::timespec;
use mutex::{FUTEX_WAIT, FUTEX_WAKE};
use platform::types::{c_int, c_uint, c_void, pid_t};
use platform::{Pal, Sys};

pub struct Semaphore {
    lock: i32,
    count: i32,
}

type pte_osThreadHandle = pid_t;
type pte_osMutexHandle = *mut i32;
type pte_osSemaphoreHandle = *mut Semaphore;
type pte_osThreadEntryPoint = unsafe extern "C" fn(params: *mut c_void) -> c_int;

#[repr(C)]
#[derive(Eq, PartialEq)]
pub enum pte_osResult {
    PTE_OS_OK = 0,
    PTE_OS_NO_RESOURCES,
    PTE_OS_GENERAL_FAILURE,
    PTE_OS_TIMEOUT,
    PTE_OS_INTERRUPTED,
    PTE_OS_INVALID_PARAM,
}

use self::pte_osResult::*;

static mut pid_mutexes: Option<BTreeMap<pte_osThreadHandle, pte_osMutexHandle>> = None;
static mut pid_mutexes_lock: i32 = 0;

#[thread_local]
static mut LOCALS: *mut BTreeMap<c_uint, *mut c_void> = ptr::null_mut();

static NEXT_KEY: AtomicUsize = ATOMIC_USIZE_INIT;

unsafe fn locals() -> &'static mut BTreeMap<c_uint, *mut c_void> {
    if LOCALS == ptr::null_mut() {
        LOCALS = Box::into_raw(Box::new(BTreeMap::new()));
    }
    &mut *LOCALS
}

// pte_osResult pte_osInit(void)
#[no_mangle]
pub unsafe extern "C" fn pte_osInit() -> pte_osResult {
    PTE_OS_OK
}

#[no_mangle]
pub unsafe extern "C" fn pte_osThreadCreate(
    entryPoint: pte_osThreadEntryPoint,
    _stackSize: c_int,
    _initialPriority: c_int,
    argv: *mut c_void,
    ppte_osThreadHandle: *mut pte_osThreadHandle,
) -> pte_osResult {
    // XXX error handling
    let id = Sys::pte_clone();
    if id < 0 {
        return PTE_OS_GENERAL_FAILURE;
    }

    let mutex = Box::into_raw(Box::new(0));

    if id == 0 {
        // Wait until pte_osThreadStart
        pte_osMutexLock(mutex);
        entryPoint(argv);
        pte_osThreadExit();
    } else {
        pte_osMutexLock(&mut pid_mutexes_lock);
        if pid_mutexes.is_none() {
            pid_mutexes = Some(BTreeMap::new());
        }
        pid_mutexes.as_mut().unwrap().insert(id, mutex);
        pte_osMutexUnlock(&mut pid_mutexes_lock);
        *ppte_osThreadHandle = id;
    }
    PTE_OS_OK
}

#[no_mangle]
pub unsafe extern "C" fn pte_osThreadStart(handle: pte_osThreadHandle) -> pte_osResult {
    let mut ret = PTE_OS_GENERAL_FAILURE;
    pte_osMutexLock(&mut pid_mutexes_lock);
    if let Some(ref mutexes) = pid_mutexes {
        if let Some(mutex) = mutexes.get(&handle) {
            pte_osMutexUnlock(*mutex);
            ret = PTE_OS_OK;
        }
    }
    pte_osMutexUnlock(&mut pid_mutexes_lock);
    ret
}

#[no_mangle]
pub unsafe extern "C" fn pte_osThreadExit() {
    Sys::exit(0);
}

#[no_mangle]
pub unsafe extern "C" fn pte_osThreadExitAndDelete(handle: pte_osThreadHandle) -> pte_osResult {
    let res = pte_osThreadDelete(handle);
    if res != PTE_OS_OK {
        return res;
    }
    pte_osThreadExit();
    PTE_OS_OK
}

#[no_mangle]
pub unsafe extern "C" fn pte_osThreadDelete(handle: pte_osThreadHandle) -> pte_osResult {
    pte_osMutexLock(&mut pid_mutexes_lock);
    if let Some(ref mut mutexes) = pid_mutexes {
        if let Some(mutex) = mutexes.remove(&handle) {
            Box::from_raw(mutex);
        }
    }
    pte_osMutexUnlock(&mut pid_mutexes_lock);
    PTE_OS_OK
}

#[no_mangle]
pub unsafe extern "C" fn pte_osThreadWaitForEnd(handle: pte_osThreadHandle) -> pte_osResult {
    let mut status = 0;
    Sys::waitpid(handle, &mut status, 0);
    PTE_OS_OK
}

#[no_mangle]
pub unsafe extern "C" fn pte_osThreadCancel(handle: pte_osThreadHandle) -> pte_osResult {
    //TODO: allow cancel of thread
    PTE_OS_OK
}

#[no_mangle]
pub unsafe extern "C" fn pte_osThreadCheckCancel(handle: pte_osThreadHandle) -> pte_osResult {
    PTE_OS_OK
}

#[no_mangle]
pub unsafe extern "C" fn pte_osThreadSleep(msecs: c_uint) {
    let tm = timespec {
        tv_sec: msecs as i64 / 1000,
        tv_nsec: (msecs % 1000) as i64 * 1000000,
    };
    Sys::nanosleep(&tm, ptr::null_mut());
}

#[no_mangle]
pub unsafe extern "C" fn pte_osThreadGetHandle() -> pte_osThreadHandle {
    Sys::gettid()
}

#[no_mangle]
pub unsafe extern "C" fn pte_osThreadGetPriority(threadHandle: pte_osThreadHandle) -> c_int {
    // XXX Shouldn't Redox support priorities?
    1
}

#[no_mangle]
pub unsafe extern "C" fn pte_osThreadSetPriority(
    threadHandle: pte_osThreadHandle,
    newPriority: c_int,
) -> pte_osResult {
    PTE_OS_OK
}

#[no_mangle]
pub unsafe extern "C" fn pte_osThreadGetMinPriority() -> c_int {
    1
}

#[no_mangle]
pub unsafe extern "C" fn pte_osThreadGetMaxPriority() -> c_int {
    1
}

#[no_mangle]
pub unsafe extern "C" fn pte_osThreadGetDefaultPriority() -> c_int {
    1
}

#[no_mangle]
pub unsafe extern "C" fn pte_osMutexCreate(pHandle: *mut pte_osMutexHandle) -> pte_osResult {
    *pHandle = Box::into_raw(Box::new(0));
    PTE_OS_OK
}

#[no_mangle]
pub unsafe extern "C" fn pte_osMutexDelete(handle: pte_osMutexHandle) -> pte_osResult {
    Box::from_raw(handle);
    PTE_OS_OK
}

#[no_mangle]
pub unsafe extern "C" fn pte_osMutexLock(handle: pte_osMutexHandle) -> pte_osResult {
    let mut c = 0;
    for _i in 0..100 {
        c = intrinsics::atomic_cxchg(handle, 0, 1).0;
        if c == 0 {
            break;
        }
    }
    if c == 1 {
        c = intrinsics::atomic_xchg(handle, 2);
    }
    while c != 0 {
        Sys::futex(handle, FUTEX_WAIT, 2);
        c = intrinsics::atomic_xchg(handle, 2);
    }

    PTE_OS_OK
}

#[no_mangle]
pub unsafe extern "C" fn pte_osMutexUnlock(handle: pte_osMutexHandle) -> pte_osResult {
    if *handle == 2 {
        *handle = 0;
    } else if intrinsics::atomic_xchg(handle, 0) == 1 {
        return PTE_OS_OK;
    }
    for _i in 0..100 {
        if *handle != 0 {
            if intrinsics::atomic_cxchg(handle, 1, 2).0 != 0 {
                return PTE_OS_OK;
            }
        }
    }
    Sys::futex(handle, FUTEX_WAKE, 1);

    PTE_OS_OK
}

#[no_mangle]
pub unsafe extern "C" fn pte_osSemaphoreCreate(
    initialValue: c_int,
    pHandle: *mut pte_osSemaphoreHandle,
) -> pte_osResult {
    *pHandle = Box::into_raw(Box::new(Semaphore {
        lock: 0,
        count: initialValue,
    }));
    PTE_OS_OK
}

#[no_mangle]
pub unsafe extern "C" fn pte_osSemaphoreDelete(handle: pte_osSemaphoreHandle) -> pte_osResult {
    Box::from_raw(handle);
    PTE_OS_OK
}

#[no_mangle]
pub unsafe extern "C" fn pte_osSemaphorePost(
    handle: pte_osSemaphoreHandle,
    count: c_int,
) -> pte_osResult {
    let semaphore = &mut *handle;
    pte_osMutexLock(&mut semaphore.lock);
    intrinsics::atomic_xadd(&mut semaphore.count, 1);
    pte_osMutexUnlock(&mut semaphore.lock);
    PTE_OS_OK
}

#[no_mangle]
pub unsafe extern "C" fn pte_osSemaphorePend(
    handle: pte_osSemaphoreHandle,
    pTimeout: *mut c_uint,
) -> pte_osResult {
    //TODO: pTimeout
    let semaphore = &mut *handle;
    let mut acquired = false;
    while !acquired {
        pte_osMutexLock(&mut semaphore.lock);
        if intrinsics::atomic_load(&mut semaphore.count) > 0 {
            intrinsics::atomic_xsub(&mut semaphore.count, 1);
            acquired = true;
        }
        pte_osMutexUnlock(&mut semaphore.lock);
        Sys::sched_yield();
    }
    PTE_OS_OK
}

#[no_mangle]
pub unsafe extern "C" fn pte_osSemaphoreCancellablePend(
    handle: pte_osSemaphoreHandle,
    pTimeout: *mut c_uint,
) -> pte_osResult {
    //TODO
    pte_osSemaphorePend(handle, pTimeout)
}

#[no_mangle]
pub unsafe extern "C" fn pte_osAtomicExchange(ptarg: *mut c_int, val: c_int) -> c_int {
    intrinsics::atomic_xchg(ptarg, val)
}

#[no_mangle]
pub unsafe extern "C" fn pte_osAtomicCompareExchange(
    pdest: *mut c_int,
    exchange: c_int,
    comp: c_int,
) -> c_int {
    intrinsics::atomic_cxchg(pdest, comp, exchange).0
}

#[no_mangle]
pub unsafe extern "C" fn pte_osAtomicExchangeAdd(pAppend: *mut c_int, value: c_int) -> c_int {
    intrinsics::atomic_xadd(pAppend, value)
}

#[no_mangle]
pub unsafe extern "C" fn pte_osAtomicDecrement(pdest: *mut c_int) -> c_int {
    intrinsics::atomic_xadd(pdest, -1) - 1
}

#[no_mangle]
pub unsafe extern "C" fn pte_osAtomicIncrement(pdest: *mut c_int) -> c_int {
    intrinsics::atomic_xadd(pdest, 1) + 1
}

#[no_mangle]
pub unsafe extern "C" fn pte_osTlsSetValue(index: c_uint, value: *mut c_void) -> pte_osResult {
    locals().insert(index, value);
    PTE_OS_OK
}

#[no_mangle]
pub unsafe extern "C" fn pte_osTlsGetValue(index: c_uint) -> *mut c_void {
    locals()
        .get_mut(&index)
        .map(|x| *x)
        .unwrap_or(ptr::null_mut())
}

#[no_mangle]
pub unsafe extern "C" fn pte_osTlsAlloc(pKey: *mut c_uint) -> pte_osResult {
    NEXT_KEY.fetch_add(1, Ordering::SeqCst);
    PTE_OS_OK
}

#[no_mangle]
pub unsafe extern "C" fn pte_osTlsFree(index: c_uint) -> pte_osResult {
    // XXX free keys
    PTE_OS_OK
}
