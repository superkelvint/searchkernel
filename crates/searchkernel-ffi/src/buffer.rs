use std::collections::BTreeMap;
use std::ptr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock};

use crate::SkBuffer;

static LIVE_BUFFERS: AtomicUsize = AtomicUsize::new(0);
static BUFFERS: OnceLock<Mutex<BTreeMap<usize, Box<[u8]>>>> = OnceLock::new();

fn buffers() -> &'static Mutex<BTreeMap<usize, Box<[u8]>>> {
    BUFFERS.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn lock_buffers() -> MutexGuard<'static, BTreeMap<usize, Box<[u8]>>> {
    match buffers().lock() {
        Ok(guard) => guard,
        Err(error) => error.into_inner(),
    }
}

#[cfg(feature = "ffi-test-support")]
pub(crate) fn live_count() -> usize {
    LIVE_BUFFERS.load(Ordering::Acquire)
}

pub(crate) fn write(out: *mut SkBuffer, bytes: Vec<u8>) {
    // SAFETY: callers validate this output pointer and its empty state before
    // reaching this commit function.
    unsafe {
        debug_assert!(!out.is_null());
        debug_assert!((*out).data.is_null());
        debug_assert_eq!((*out).len, 0);
    }
    let boxed = bytes.into_boxed_slice();
    let len = boxed.len();
    debug_assert!(len > 0);
    let data = boxed.as_ptr().cast_mut();
    // Keep the owning allocation and its true length private. The caller may
    // inspect or accidentally alter `SkBuffer::len`, but that metadata is not
    // used to reconstruct or release the allocation.
    lock_buffers().insert(data as usize, boxed);
    // SAFETY: `out` was validated by the ABI entrypoint and the allocation is
    // retained in `BUFFERS` until `free` removes it.
    unsafe {
        (*out).data = data;
        (*out).len = len;
    }
    LIVE_BUFFERS.fetch_add(1, Ordering::AcqRel);
}

/// Release a buffer created by `write` and clear its public state.
pub(crate) unsafe fn free(buffer: *mut SkBuffer) {
    if buffer.is_null() {
        return;
    }

    // SAFETY: callers of this internal function provide a valid `SkBuffer`;
    // the public contract only permits Search Kernel-owned non-empty buffers.
    let data = unsafe { (*buffer).data };
    // Clear first so a second call is harmless even if deallocation changes
    // process state unexpectedly.
    unsafe {
        (*buffer).data = ptr::null_mut();
        (*buffer).len = 0;
    }

    if data.is_null() {
        return;
    }

    let owned = lock_buffers().remove(&(data as usize));
    if owned.is_some() {
        drop(owned);
        LIVE_BUFFERS.fetch_sub(1, Ordering::AcqRel);
    }
}

#[cfg(all(test, feature = "ffi-test-support"))]
mod tests {
    use super::*;

    #[test]
    fn owned_buffers_are_cleared_and_counted() {
        let mut buffer = SkBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        write(&mut buffer, vec![1, 2, 3]);
        assert!(!buffer.data.is_null());
        assert_eq!(buffer.len, 3);
        assert_eq!(live_count(), 1);

        // SAFETY: this is the buffer returned by the write function.
        unsafe { free(&mut buffer) };
        assert!(buffer.data.is_null());
        assert_eq!(buffer.len, 0);
        assert_eq!(live_count(), 0);

        // SAFETY: the first free cleared the structure.
        unsafe { free(&mut buffer) };
        assert_eq!(live_count(), 0);

        write(&mut buffer, vec![4, 5, 6]);
        buffer.len = usize::MAX;
        // SAFETY: the pointer is still the buffer returned by `write`; the
        // deliberately corrupted public length must not affect deallocation.
        unsafe { free(&mut buffer) };
        assert_eq!(live_count(), 0);
    }
}
