use searchkernel_ir::{Backend, IndexContext};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, OnceLock};

pub(crate) struct HandleState {
    pub(crate) context: Arc<IndexContext>,
    pub(crate) backend: Arc<dyn Backend>,
}

struct Registry {
    #[cfg(any(feature = "ffi-test-support", feature = "production"))]
    next: u64,
    entries: BTreeMap<u64, Arc<HandleState>>,
}

static REGISTRY: OnceLock<Mutex<Registry>> = OnceLock::new();

fn registry() -> &'static Mutex<Registry> {
    REGISTRY.get_or_init(|| {
        Mutex::new(Registry {
            #[cfg(any(feature = "ffi-test-support", feature = "production"))]
            next: 1,
            entries: BTreeMap::new(),
        })
    })
}

fn lock_registry() -> std::sync::MutexGuard<'static, Registry> {
    match registry().lock() {
        Ok(guard) => guard,
        Err(error) => error.into_inner(),
    }
}

#[cfg(any(feature = "ffi-test-support", feature = "production"))]
pub(crate) fn register(context: Arc<IndexContext>, backend: Arc<dyn Backend>) -> u64 {
    let mut registry = lock_registry();
    let handle = registry.next;
    if handle == 0 {
        return 0;
    }
    let Some(next) = registry.next.checked_add(1) else {
        return 0;
    };
    registry.next = next;
    registry
        .entries
        .insert(handle, Arc::new(HandleState { context, backend }));
    handle
}

pub(crate) fn lookup(handle: u64) -> Option<Arc<HandleState>> {
    if handle == 0 {
        return None;
    }
    let registry = lock_registry();
    registry.entries.get(&handle).cloned()
}

pub(crate) fn close(handle: u64) -> bool {
    if handle == 0 {
        return false;
    }
    let removed = {
        let mut registry = lock_registry();
        registry.entries.remove(&handle)
    };
    let closed = removed.is_some();
    // Drop the backend/context state after releasing the global registry lock.
    drop(removed);
    closed
}

#[cfg(feature = "ffi-test-support")]
pub(crate) fn live_count() -> usize {
    lock_registry().entries.len()
}
