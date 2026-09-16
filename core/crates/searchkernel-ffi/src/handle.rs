#[cfg(feature = "ffi-test-support")]
use searchkernel_ir::Backend;
use searchkernel_ir::{BackendSession, IndexContext};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, OnceLock};

pub(crate) struct HandleState {
    pub(crate) context: Arc<IndexContext>,
    pub(crate) session: Arc<BackendSession>,
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

#[cfg(feature = "ffi-test-support")]
pub(crate) fn register(context: Arc<IndexContext>, backend: Arc<dyn Backend>) -> u64 {
    register_session(context, BackendSession::from_backend(backend))
}

#[cfg(any(feature = "ffi-test-support", feature = "production"))]
pub(crate) fn register_session(context: Arc<IndexContext>, session: Arc<BackendSession>) -> u64 {
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
        .insert(handle, Arc::new(HandleState { context, session }));
    handle
}

pub(crate) fn lookup(handle: u64) -> Option<Arc<HandleState>> {
    if handle == 0 {
        return None;
    }
    let registry = lock_registry();
    registry.entries.get(&handle).cloned()
}

pub(crate) fn close(handle: u64) -> Option<searchkernel_ir::Result<()>> {
    if handle == 0 {
        return None;
    }
    let removed = {
        let mut registry = lock_registry();
        registry.entries.remove(&handle)
    };
    let removed = removed?;
    // Close the backend/context state after releasing the global registry lock.
    Some(removed.session.close())
}

#[cfg(feature = "ffi-test-support")]
pub(crate) fn live_count() -> usize {
    lock_registry().entries.len()
}

#[cfg(all(test, feature = "ffi-test-support"))]
mod tests {
    use super::*;
    use crate::test_support::RecordingBackend;
    use searchkernel_ir::api;

    fn test_context() -> Arc<IndexContext> {
        let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
        let name = builder.create_string("title");
        let stored = api::OptionalBool::new(true);
        let searchable = api::OptionalBool::new(true);
        let field = api::FieldDefinition::create(
            &mut builder,
            &api::FieldDefinitionArgs {
                name: Some(name),
                kind: api::FieldKind::Text,
                cardinality: api::FieldCardinality::Single,
                stored: Some(&stored),
                searchable: Some(&searchable),
                ..Default::default()
            },
        );
        let fields = builder.create_vector(&[field]);
        let definition = api::IndexDefinition::create(
            &mut builder,
            &api::IndexDefinitionArgs {
                fields: Some(fields),
                ..Default::default()
            },
        );
        builder.finish(definition, None);
        let bytes = builder.finished_data().to_vec();
        let root = searchkernel_ir::__flatbuffers::root::<api::IndexDefinition>(&bytes)
            .expect("test definition");
        Arc::new(IndexContext::from_api(root).expect("test context"))
    }

    #[test]
    fn handles_are_unique_and_close_is_idempotently_rejected() {
        let handle = register(test_context(), Arc::new(RecordingBackend::new()));
        assert_ne!(handle, 0);
        assert!(lookup(handle).is_some());
        assert!(close(handle).expect("registered handle").is_ok());
        assert!(lookup(handle).is_none());
        assert!(close(handle).is_none());
        assert!(close(0).is_none());
    }
}
