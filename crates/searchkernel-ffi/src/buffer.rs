use std::collections::BTreeMap;
use std::ptr;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock};

use crate::SkBuffer;

static LIVE_BUFFERS: AtomicUsize = AtomicUsize::new(0);
static NEXT_ALLOCATION_ID: AtomicU64 = AtomicU64::new(1);

struct Allocation {
    data: usize,
    _bytes: Box<[u8]>,
}

static BUFFERS: OnceLock<Mutex<BTreeMap<u64, Allocation>>> = OnceLock::new();

fn buffers() -> &'static Mutex<BTreeMap<u64, Allocation>> {
    BUFFERS.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn lock_buffers() -> MutexGuard<'static, BTreeMap<u64, Allocation>> {
    match buffers().lock() {
        Ok(guard) => guard,
        Err(error) => error.into_inner(),
    }
}

#[cfg(feature = "ffi-test-support")]
pub(crate) fn live_count() -> usize {
    LIVE_BUFFERS.load(Ordering::Acquire)
}

pub(crate) unsafe fn write(out: *mut SkBuffer, bytes: Vec<u8>) {
    // SAFETY: callers validate this output pointer and its empty state before
    // reaching this commit function.
    unsafe {
        debug_assert!(!out.is_null());
        debug_assert!((*out).data.is_null());
        debug_assert_eq!((*out).len, 0);
        debug_assert_eq!((*out).allocation_id, 0);
    }
    let boxed = bytes.into_boxed_slice();
    let len = boxed.len();
    debug_assert!(len > 0);
    let data = boxed.as_ptr().cast_mut();
    // A token, rather than the address, identifies ownership. This prevents a
    // stale copied descriptor from freeing a later allocation after the
    // allocator reuses the same address.
    let allocation_id = NEXT_ALLOCATION_ID
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
            current.checked_add(1)
        })
        .expect("response allocation ID exhausted");
    let previous = lock_buffers().insert(
        allocation_id,
        Allocation {
            data: data as usize,
            _bytes: boxed,
        },
    );
    debug_assert!(previous.is_none());
    // SAFETY: `out` was validated by the ABI entrypoint and the allocation is
    // retained in `BUFFERS` until `free` removes it.
    unsafe {
        (*out).data = data;
        (*out).len = len;
        (*out).allocation_id = allocation_id;
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
    let allocation_id = unsafe { (*buffer).allocation_id };
    // Clear first so a second call is harmless even if deallocation changes
    // process state unexpectedly.
    unsafe {
        (*buffer).data = ptr::null_mut();
        (*buffer).len = 0;
        (*buffer).allocation_id = 0;
    }

    if data.is_null() {
        return;
    }

    let mut allocations = lock_buffers();
    let owned = if allocations
        .get(&allocation_id)
        .is_some_and(|allocation| allocation.data == data as usize)
    {
        allocations.remove(&allocation_id)
    } else {
        None
    };
    drop(allocations);
    if let Some(owned) = owned {
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
            allocation_id: 0,
        };
        // SAFETY: this test provides a valid output structure.
        unsafe { write(&mut buffer, vec![1, 2, 3]) };
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

        // A copied descriptor with an old token cannot release a later
        // allocation, even if its address happens to be reused.
        unsafe { write(&mut buffer, vec![7, 8, 9]) };
        let stale = buffer;
        unsafe { free(&mut buffer) };
        unsafe { write(&mut buffer, vec![10, 11, 12]) };
        let live_id = buffer.allocation_id;
        let mut stale_copy = stale;
        unsafe { free(&mut stale_copy) };
        assert_eq!(buffer.allocation_id, live_id);
        assert_eq!(live_count(), 1);

        // Also verify the token independently of the address check: a
        // descriptor with the live pointer but an old token must not release
        // the live allocation.
        let mut wrong_token = buffer;
        wrong_token.allocation_id = stale.allocation_id;
        unsafe { free(&mut wrong_token) };
        assert_eq!(buffer.allocation_id, live_id);
        assert_eq!(live_count(), 1);
        unsafe { free(&mut buffer) };
        assert_eq!(live_count(), 0);

        // SAFETY: this test provides a valid output structure.
        unsafe { write(&mut buffer, vec![4, 5, 6]) };
        buffer.len = usize::MAX;
        // SAFETY: the pointer is still the buffer returned by `write`; the
        // deliberately corrupted public length must not affect deallocation.
        unsafe { free(&mut buffer) };
        assert_eq!(live_count(), 0);
    }
}
