use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock};

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

pub(crate) unsafe fn write(
    out_data: *mut *mut u8,
    out_len: *mut usize,
    out_allocation_id: *mut u64,
    bytes: Vec<u8>,
) -> Result<(), ()> {
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
        .map_err(|_| ())?;
    let previous = lock_buffers().insert(
        allocation_id,
        Allocation {
            data: data as usize,
            _bytes: boxed,
        },
    );
    debug_assert!(previous.is_none());
    // SAFETY: all outputs were validated by the ABI entrypoint and the
    // allocation is retained in `BUFFERS` until `free` removes it.
    unsafe {
        *out_data = data;
        *out_len = len;
        *out_allocation_id = allocation_id;
    }
    LIVE_BUFFERS.fetch_add(1, Ordering::AcqRel);
    Ok(())
}

/// Release a pointer/token pair created by `write`.
pub(crate) unsafe fn free(data: *mut u8, allocation_id: u64) {
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
    use std::ptr;

    #[test]
    fn owned_buffers_are_cleared_and_counted() {
        let mut data = ptr::null_mut();
        let mut len = 0;
        let mut allocation_id = 0;
        // SAFETY: this test provides valid output pointers.
        unsafe { write(&mut data, &mut len, &mut allocation_id, vec![1, 2, 3]) }
            .expect("test allocation");
        assert!(!data.is_null());
        assert_eq!(len, 3);
        assert_eq!(live_count(), 1);

        // SAFETY: this is the pointer/token pair returned by write.
        unsafe { free(data, allocation_id) };
        assert_eq!(live_count(), 0);

        // SAFETY: releasing the same pointer/token pair again is harmless.
        unsafe { free(data, allocation_id) };
        assert_eq!(live_count(), 0);

        // A copied pointer/token pair cannot release a later allocation,
        // even if the allocator happens to reuse the same address.
        let mut stale_data = ptr::null_mut();
        let mut stale_len = 0;
        let mut stale_id = 0;
        unsafe {
            write(
                &mut stale_data,
                &mut stale_len,
                &mut stale_id,
                vec![7, 8, 9],
            )
            .expect("test allocation")
        };
        unsafe { free(stale_data, stale_id) };
        let mut live_data = ptr::null_mut();
        let mut live_len = 0;
        let mut live_id = 0;
        unsafe {
            write(
                &mut live_data,
                &mut live_len,
                &mut live_id,
                vec![10, 11, 12],
            )
            .expect("test allocation")
        };
        unsafe { free(stale_data, stale_id) };
        assert_eq!(live_count(), 1);

        // A live pointer with an old token must not release the live
        // allocation either.
        unsafe { free(live_data, stale_id) };
        assert_eq!(live_count(), 1);
        unsafe { free(live_data, live_id) };
        assert_eq!(live_count(), 0);

        // Length is not part of the deallocation API and cannot affect it.
        unsafe { write(&mut data, &mut len, &mut allocation_id, vec![4, 5, 6]) }
            .expect("test allocation");
        len = usize::MAX;
        assert_eq!(len, usize::MAX);
        unsafe { free(data, allocation_id) };
        assert_eq!(live_count(), 0);
    }
}
