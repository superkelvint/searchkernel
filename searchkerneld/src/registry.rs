use searchkernel::{Error as IndexError, ErrorCode as IndexErrorCode, Index};
use searchkernel_dispatch::{DispatchError, Dispatcher};
use searchkernel_ir::{
    wire::{self, ControlPlaneDecodeError, WireVersion},
    ControlPlaneRequest, IndexDefinitionIr, PortableError,
};
use std::{
    collections::BTreeMap,
    fmt,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex, RwLock, Weak,
    },
    time::{SystemTime, UNIX_EPOCH},
};

static NEXT_GENERATION_SEQUENCE: AtomicU64 = AtomicU64::new(1);

fn new_generation() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let process = std::process::id();
    let sequence = NEXT_GENERATION_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!("{nanos:032x}-{process:08x}-{sequence:016x}")
}

pub(crate) struct RegistryDispatchResult {
    pub(crate) payload: Vec<u8>,
}

impl RegistryDispatchResult {
    fn payload(payload: Vec<u8>) -> Self {
        Self { payload }
    }
}

#[derive(Debug)]
pub enum RegistryInitError {
    Io(std::io::Error),
    NotDirectory(PathBuf),
}

impl fmt::Display for RegistryInitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "prepare Search Kernel data root: {error}"),
            Self::NotDirectory(path) => {
                write!(
                    formatter,
                    "Search Kernel data root is not a directory: {}",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for RegistryInitError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RegistryDispatchError {
    InvalidRequest,
    UnsupportedProtocol(u32),
    UnsupportedOperation(u8),
    Closed,
    Poisoned,
}

impl fmt::Display for RegistryDispatchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRequest => write!(formatter, "invalid Search Kernel request FlatBuffer"),
            Self::UnsupportedProtocol(version) => {
                write!(
                    formatter,
                    "unsupported Search Kernel protocol version {version}"
                )
            }
            Self::UnsupportedOperation(value) => {
                write!(
                    formatter,
                    "unsupported Search Kernel operation discriminator {value}"
                )
            }
            Self::Closed => write!(formatter, "Search Kernel target is closed"),
            Self::Poisoned => write!(formatter, "Search Kernel index registry lock is poisoned"),
        }
    }
}

impl std::error::Error for RegistryDispatchError {}

struct RegistryEntry {
    index: Index,
    dispatcher: Arc<Dispatcher>,
    lifecycle: RwLock<()>,
    generation: String,
}

struct ConfigurationPlan {
    target: String,
    generation: Option<String>,
    definition: IndexDefinitionIr,
}

pub struct IndexRegistry {
    data_root: PathBuf,
    plans: Mutex<BTreeMap<String, ConfigurationPlan>>,
    entries: Mutex<BTreeMap<String, Arc<RegistryEntry>>>,
    transition_locks: Mutex<BTreeMap<String, Weak<Mutex<()>>>>,
    shutdown: RwLock<bool>,
}

impl IndexRegistry {
    pub fn new(data_root: impl Into<PathBuf>) -> Result<Self, RegistryInitError> {
        let data_root = data_root.into();
        std::fs::create_dir_all(&data_root).map_err(RegistryInitError::Io)?;
        if !data_root.is_dir() {
            return Err(RegistryInitError::NotDirectory(data_root));
        }
        Ok(Self {
            data_root,
            plans: Mutex::new(BTreeMap::new()),
            entries: Mutex::new(BTreeMap::new()),
            transition_locks: Mutex::new(BTreeMap::new()),
            shutdown: RwLock::new(false),
        })
    }

    fn transition_lock(&self, target: &str) -> Result<Arc<Mutex<()>>, RegistryDispatchError> {
        let mut locks = self
            .transition_locks
            .lock()
            .map_err(|_| RegistryDispatchError::Poisoned)?;
        if let Some(lock) = locks.get(target).and_then(Weak::upgrade) {
            return Ok(lock);
        }

        // Keep the coordination table bounded even when callers probe many
        // distinct valid-but-missing targets.
        locks.retain(|_, lock| lock.strong_count() != 0);
        let lock = Arc::new(Mutex::new(()));
        locks.insert(target.to_owned(), Arc::downgrade(&lock));
        Ok(lock)
    }

    pub fn data_root(&self) -> &Path {
        &self.data_root
    }

    pub fn dispatch(&self, request: &[u8]) -> Result<Vec<u8>, RegistryDispatchError> {
        Ok(self.dispatch_routed(request)?.payload)
    }

    pub(crate) fn dispatch_grpc(
        &self,
        request: &[u8],
    ) -> Result<RegistryDispatchResult, RegistryDispatchError> {
        self.dispatch_routed(request)
    }

    fn dispatch_routed(
        &self,
        request: &[u8],
    ) -> Result<RegistryDispatchResult, RegistryDispatchError> {
        // Keep shutdown linearizable with every request, including lifecycle
        // transitions that temporarily operate outside the registry map.
        let shutdown = self
            .shutdown
            .read()
            .map_err(|_| RegistryDispatchError::Poisoned)?;
        if *shutdown {
            return Err(RegistryDispatchError::Closed);
        }

        let decoded = match wire::decode_control_plane_request(request) {
            Ok(decoded) => decoded,
            Err(ControlPlaneDecodeError::InvalidRequest) => {
                return Err(RegistryDispatchError::InvalidRequest)
            }
            Err(ControlPlaneDecodeError::UnsupportedProtocol(version)) => {
                return Err(RegistryDispatchError::UnsupportedProtocol(version))
            }
            Err(ControlPlaneDecodeError::Semantic { version, error }) => {
                return Ok(RegistryDispatchResult::payload(wire::encode_error(
                    version, &error,
                )))
            }
        };
        let version = decoded.version;

        match decoded.request {
            ControlPlaneRequest::ListIndexes => self.list_indexes(version),
            ControlPlaneRequest::PlanChange {
                target,
                expected_generation,
                definition,
            } => self.plan_change(
                &target,
                expected_generation.as_deref(),
                *definition,
                version,
            ),
            ControlPlaneRequest::ApplyChange {
                target,
                expected_generation,
                plan_id,
            } => self.apply_change(&target, expected_generation.as_deref(), &plan_id, version),
            ControlPlaneRequest::Create { target, definition } => {
                self.create(&target, *definition, version)
            }
            ControlPlaneRequest::Open { target } => self.open(&target, version),
            ControlPlaneRequest::Delete {
                target,
                expected_generation,
            } => self.delete_index(&target, expected_generation.as_deref(), version),
            ControlPlaneRequest::Route {
                target,
                expected_generation,
            } => self.dispatch_loaded(&target, request, expected_generation.as_deref(), version),
        }
    }

    pub fn close_all(&self) -> Result<(), IndexError> {
        // Taking the exclusive shutdown gate waits for every in-flight request
        // to finish. Marking the registry closed before draining the map also
        // prevents a create/open transition from publishing a fresh entry
        // after shutdown has begun.
        let mut shutdown = self.shutdown.write().map_err(|_| {
            IndexError::search(
                IndexErrorCode::Internal,
                "Search Kernel registry shutdown lock is poisoned",
            )
        })?;
        if *shutdown {
            return Ok(());
        }
        *shutdown = true;

        let entries = {
            let mut entries = self.entries.lock().map_err(|_| {
                IndexError::search(
                    IndexErrorCode::Internal,
                    "Search Kernel index registry lock is poisoned",
                )
            })?;
            std::mem::take(&mut *entries)
        };

        let mut first_error = None;
        for entry in entries.into_values() {
            let lifecycle = match entry.lifecycle.write() {
                Ok(lifecycle) => lifecycle,
                Err(_) => {
                    if first_error.is_none() {
                        first_error = Some(IndexError::search(
                            IndexErrorCode::Internal,
                            "Search Kernel index lifecycle lock is poisoned",
                        ));
                    }
                    continue;
                }
            };
            if let Err(error) = entry.index.close() {
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
            drop(lifecycle);
        }
        match first_error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    pub fn loaded_targets(&self) -> Result<Vec<String>, RegistryDispatchError> {
        let entries = self
            .entries
            .lock()
            .map_err(|_| RegistryDispatchError::Poisoned)?;
        Ok(entries.keys().cloned().collect())
    }

    fn list_indexes(
        &self,
        version: WireVersion,
    ) -> Result<RegistryDispatchResult, RegistryDispatchError> {
        let listing = match std::fs::read_dir(&self.data_root) {
            Ok(listing) => listing,
            Err(_) => {
                return Ok(RegistryDispatchResult::payload(wire::encode_error(
                    version,
                    &searchkernel_ir::index_discovery_failure(),
                )))
            }
        };
        let mut candidates = Vec::new();
        for entry in listing {
            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => {
                    return Ok(RegistryDispatchResult::payload(wire::encode_error(
                        version,
                        &searchkernel_ir::index_discovery_failure(),
                    )))
                }
            };
            let Some(name) = entry.file_name().to_str().map(ToOwned::to_owned) else {
                continue;
            };
            if searchkernel_ir::validate_index_target(&name).is_ok() {
                candidates.push(name);
            }
        }
        candidates.sort();
        candidates.dedup();
        let mut targets = Vec::new();
        for target in candidates {
            // The directory iteration is a candidate snapshot, not a global
            // transaction. Inspect each candidate under the SAME transition
            // lock as create/open/delete. Siblings remain independently usable.
            let transition = self.transition_lock(&target)?;
            let _transition = transition
                .lock()
                .map_err(|_| RegistryDispatchError::Poisoned)?;
            if let Ok(path) = self.checked_index_path(&target) {
                if Index::inspect_storage(path).is_ok() {
                    targets.push(target);
                }
            }
        }
        Ok(RegistryDispatchResult::payload(
            wire::encode_list_indexes_result(version, &targets),
        ))
    }

    fn plan_change(
        &self,
        target: &str,
        expected_generation: Option<&str>,
        definition: IndexDefinitionIr,
        version: wire::WireVersion,
    ) -> Result<RegistryDispatchResult, RegistryDispatchError> {
        let transition = self.transition_lock(target)?;
        let _transition = transition
            .lock()
            .map_err(|_| RegistryDispatchError::Poisoned)?;
        let path = match self.checked_index_path(target) {
            Ok(path) => path,
            Err(error) => {
                return Ok(RegistryDispatchResult::payload(encode_registry_error(
                    version, error,
                )))
            }
        };
        let current = self
            .entries
            .lock()
            .map_err(|_| RegistryDispatchError::Poisoned)?
            .get(target)
            .cloned();
        let generation = current.as_ref().map(|entry| entry.generation.as_str());
        if generation != expected_generation || (path.exists() && current.is_none()) {
            return Ok(RegistryDispatchResult::payload(encode_registry_error(
                version,
                searchkernel_ir::stale_index_handle_error(),
            )));
        }
        if let Err(error) = searchkernel_ir_adapter::validate_index_definition(&definition) {
            return Ok(RegistryDispatchResult::payload(encode_registry_error(
                version, error,
            )));
        }
        let supported = current.as_ref().is_none_or(|entry| {
            searchkernel_ir_adapter::rank_profile_change_supported(
                entry.index.clone().into_session_parts().0.definition(),
                &definition,
            )
        });
        let reasons = if supported {
            vec![if current.is_some() {
                "Rank profiles will be applied with an exclusive close and reopen; document data is retained.".to_owned()
            } else {
                "A new index will be created; an existing target is never replaced.".to_owned()
            }]
        } else {
            vec!["Field, schema identity, summaries, constants and model changes require reindexing, which is not supported by this apply operation.".to_owned()]
        };
        let plan_id = new_generation();
        if supported {
            // Exercise native configuration/ranking compilation without touching
            // the selected index. Validation never publishes the target.
            let probe_root = self
                .data_root
                .join(format!(".configuration-probe-{plan_id}"));
            if let Err(error) = std::fs::create_dir(&probe_root) {
                return Ok(RegistryDispatchResult::payload(encode_registry_error(
                    version,
                    PortableError::new(searchkernel_ir::ErrorCode::Unavailable, error.to_string()),
                )));
            }
            let probe_path = probe_root.join("index");
            let bytes = wire::v1::index_definition_ir_to_api(&definition);
            let probe = Index::create_from_definition_bytes(&probe_path, &bytes);
            let validated = match probe {
                Ok(probe) => probe.close(),
                Err(error) => Err(error),
            };
            let cleanup = if probe_path.exists() {
                Index::delete_storage(&probe_path)
            } else {
                Ok(())
            };
            let cleanup_root = std::fs::remove_dir_all(&probe_root)
                .map_err(|error| IndexError::Infrastructure(error.to_string()));
            if let Err(error) = validated.and(cleanup).and(cleanup_root) {
                return Ok(RegistryDispatchResult::payload(encode_registry_error(
                    version,
                    configuration_error_to_portable(error),
                )));
            }
            let mut plans = self
                .plans
                .lock()
                .map_err(|_| RegistryDispatchError::Poisoned)?;
            if plans.len() >= 128 {
                if let Some(oldest) = plans.keys().next().cloned() {
                    plans.remove(&oldest);
                }
            }
            plans.insert(
                plan_id.clone(),
                ConfigurationPlan {
                    target: target.to_owned(),
                    generation: generation.map(str::to_owned),
                    definition: definition.clone(),
                },
            );
        }
        Ok(RegistryDispatchResult::payload(
            wire::encode_plan_index_change_result(
                version,
                &definition,
                generation,
                if supported { &plan_id } else { "" },
                supported,
                current.is_some(),
                &reasons,
            ),
        ))
    }

    fn apply_change(
        &self,
        target: &str,
        expected_generation: Option<&str>,
        plan_id: &str,
        version: wire::WireVersion,
    ) -> Result<RegistryDispatchResult, RegistryDispatchError> {
        let transition = self.transition_lock(target)?;
        let _transition = transition
            .lock()
            .map_err(|_| RegistryDispatchError::Poisoned)?;
        let invalid_plan = searchkernel_ir::invalid_configuration_plan_error;
        let mut plans = self
            .plans
            .lock()
            .map_err(|_| RegistryDispatchError::Poisoned)?;
        let Some(plan) = plans.get(plan_id) else {
            return Ok(RegistryDispatchResult::payload(encode_registry_error(
                version,
                invalid_plan(),
            )));
        };
        if plan.target != target || plan.generation.as_deref() != expected_generation {
            return Ok(RegistryDispatchResult::payload(encode_registry_error(
                version,
                invalid_plan(),
            )));
        }
        let plan = plans
            .remove(plan_id)
            .ok_or(RegistryDispatchError::Poisoned)?;
        drop(plans);

        let path = match self.checked_index_path(target) {
            Ok(path) => path,
            Err(error) => {
                return Ok(RegistryDispatchResult::payload(encode_registry_error(
                    version, error,
                )))
            }
        };

        // Retire the currently reachable incarnation under the registry mutex
        // before waiting for its exclusive lifecycle guard. This preserves the
        // canonical map -> lifecycle lock order used by dispatch_loaded() and
        // delete_index(): requests admitted before retirement drain, while new
        // requests cannot select the retiring entry and deadlock the transition.
        let entry = {
            let mut entries = self
                .entries
                .lock()
                .map_err(|_| RegistryDispatchError::Poisoned)?;
            let entry = entries.get(target).cloned();
            if entry.as_ref().map(|entry| entry.generation.as_str()) != expected_generation
                || (entry.is_none() && path.exists())
            {
                return Ok(RegistryDispatchResult::payload(encode_registry_error(
                    version,
                    searchkernel_ir::stale_index_handle_error(),
                )));
            }
            if entry.is_some() {
                entries.remove(target)
            } else {
                None
            }
        };

        let bytes = wire::v1::index_definition_ir_to_api(&plan.definition);
        let result = if let Some(entry) = entry.as_ref() {
            let _lifecycle = match entry.lifecycle.write() {
                Ok(lifecycle) => lifecycle,
                Err(_) => {
                    self.entries
                        .lock()
                        .map_err(|_| RegistryDispatchError::Poisoned)?
                        .insert(target.to_owned(), Arc::clone(entry));
                    return Err(RegistryDispatchError::Poisoned);
                }
            };

            match entry.index.apply_rank_definition_bytes_recovering(&bytes) {
                Ok(index) => Ok(index),
                Err((error, recovered)) => {
                    if let Some(index) = recovered {
                        // The old native owner was closed, rollback completed,
                        // and the restored owner retained the filesystem
                        // lifecycle lock throughout. Publish it as a fresh
                        // incarnation so every stale client generation fails.
                        let generation = new_generation();
                        let (context, session) = index.clone().into_session_parts();
                        let dispatcher = Arc::new(Dispatcher::new(context.clone(), session));
                        self.entries
                            .lock()
                            .map_err(|_| RegistryDispatchError::Poisoned)?
                            .insert(
                                target.to_owned(),
                                Arc::new(RegistryEntry {
                                    index,
                                    dispatcher,
                                    lifecycle: RwLock::new(()),
                                    generation,
                                }),
                            );
                    } else if entry.index.describe().is_ok() {
                        // Preparation failed before shutdown. Restore the same
                        // entry and generation because its incarnation never
                        // changed.
                        self.entries
                            .lock()
                            .map_err(|_| RegistryDispatchError::Poisoned)?
                            .insert(target.to_owned(), Arc::clone(entry));
                    }
                    return Ok(RegistryDispatchResult::payload(encode_registry_error(
                        version,
                        configuration_error_to_portable(error),
                    )));
                }
            }
        } else {
            Index::create_from_definition_bytes(&path, &bytes)
        };

        match result {
            Ok(index) => {
                let generation = new_generation();
                let (context, session) = index.clone().into_session_parts();
                let dispatcher = Arc::new(Dispatcher::new(context.clone(), session));
                let payload = wire::encode_apply_index_change_result(
                    version,
                    context.definition(),
                    &generation,
                );
                self.entries
                    .lock()
                    .map_err(|_| RegistryDispatchError::Poisoned)?
                    .insert(
                        target.to_owned(),
                        Arc::new(RegistryEntry {
                            index,
                            dispatcher,
                            lifecycle: RwLock::new(()),
                            generation,
                        }),
                    );
                Ok(RegistryDispatchResult::payload(payload))
            }
            Err(error) => Ok(RegistryDispatchResult::payload(encode_registry_error(
                version,
                configuration_error_to_portable(error),
            ))),
        }
    }

    fn create(
        &self,
        target: &str,
        definition: IndexDefinitionIr,
        version: WireVersion,
    ) -> Result<RegistryDispatchResult, RegistryDispatchError> {
        let definition_bytes = wire::encode_persisted_index_definition(&definition);

        let transition = self.transition_lock(target)?;
        let _transition = transition
            .lock()
            .map_err(|_| RegistryDispatchError::Poisoned)?;
        {
            let entries = self
                .entries
                .lock()
                .map_err(|_| RegistryDispatchError::Poisoned)?;
            if entries.contains_key(target) {
                return Ok(RegistryDispatchResult::payload(encode_registry_error(
                    version,
                    searchkernel_ir::already_exists_error(),
                )));
            }
        }

        let path = match self.checked_index_path(target) {
            Ok(path) => path,
            Err(error) => {
                return Ok(RegistryDispatchResult::payload(encode_registry_error(
                    version, error,
                )))
            }
        };
        let index = match Index::create_from_definition_bytes(&path, &definition_bytes) {
            Ok(index) => index,
            Err(error) => {
                return Ok(RegistryDispatchResult::payload(encode_registry_error(
                    version,
                    lifecycle_index_error_to_portable(
                        searchkernel_ir::LifecycleOperation::Create,
                        error,
                    ),
                )))
            }
        };
        let (context, session) = index.clone().into_session_parts();
        let generation = new_generation();
        let entry = Arc::new(RegistryEntry {
            index,
            dispatcher: Arc::new(Dispatcher::new(context.clone(), session)),
            lifecycle: RwLock::new(()),
            generation: generation.clone(),
        });
        self.entries
            .lock()
            .map_err(|_| RegistryDispatchError::Poisoned)?
            .insert(target.to_owned(), entry);
        Ok(RegistryDispatchResult::payload(
            wire::encode_create_index_result(version, context.definition(), &generation),
        ))
    }

    fn open(
        &self,
        target: &str,
        version: WireVersion,
    ) -> Result<RegistryDispatchResult, RegistryDispatchError> {
        let transition = self.transition_lock(target)?;
        let _transition = transition
            .lock()
            .map_err(|_| RegistryDispatchError::Poisoned)?;
        {
            let entries = self
                .entries
                .lock()
                .map_err(|_| RegistryDispatchError::Poisoned)?;
            if let Some(entry) = entries.get(target) {
                let (context, _) = entry.index.clone().into_session_parts();
                return Ok(RegistryDispatchResult::payload(
                    wire::encode_open_index_result(
                        version,
                        context.definition(),
                        &entry.generation,
                    ),
                ));
            }
        }

        let path = match self.checked_index_path(target) {
            Ok(path) => path,
            Err(error) => {
                return Ok(RegistryDispatchResult::payload(encode_registry_error(
                    version, error,
                )))
            }
        };
        let index = match Index::open(&path) {
            Ok(index) => index,
            Err(error) => {
                return Ok(RegistryDispatchResult::payload(encode_registry_error(
                    version,
                    lifecycle_index_error_to_portable(
                        searchkernel_ir::LifecycleOperation::Open,
                        error,
                    ),
                )))
            }
        };
        let (context, session) = index.clone().into_session_parts();
        let generation = new_generation();
        let entry = Arc::new(RegistryEntry {
            index,
            dispatcher: Arc::new(Dispatcher::new(context.clone(), session)),
            lifecycle: RwLock::new(()),
            generation: generation.clone(),
        });
        self.entries
            .lock()
            .map_err(|_| RegistryDispatchError::Poisoned)?
            .insert(target.to_owned(), entry);
        Ok(RegistryDispatchResult::payload(
            wire::encode_open_index_result(version, context.definition(), &generation),
        ))
    }

    fn delete_index(
        &self,
        target: &str,
        expected_generation: Option<&str>,
        version: WireVersion,
    ) -> Result<RegistryDispatchResult, RegistryDispatchError> {
        let transition = self.transition_lock(target)?;
        let _transition = transition
            .lock()
            .map_err(|_| RegistryDispatchError::Poisoned)?;
        let path = match self.checked_index_path(target) {
            Ok(path) => path,
            Err(error) => {
                return Ok(RegistryDispatchResult::payload(encode_registry_error(
                    version, error,
                )))
            }
        };

        let mut entries = self
            .entries
            .lock()
            .map_err(|_| RegistryDispatchError::Poisoned)?;
        // A generation belongs to a loaded incarnation of this process.
        // Check before removing/closing storage, under the transition lock.
        if let Some(expected) = expected_generation {
            if entries.get(target).map(|entry| entry.generation.as_str()) != Some(expected) {
                return Ok(RegistryDispatchResult::payload(encode_registry_error(
                    version,
                    searchkernel_ir::stale_index_handle_error(),
                )));
            }
        }
        let entry = entries.remove(target);
        drop(entries);
        if entry.is_none() && !path.exists() {
            return Ok(RegistryDispatchResult::payload(encode_registry_error(
                version,
                searchkernel_ir::not_found_error(),
            )));
        }

        if let Some(entry) = entry {
            let _lifecycle = entry
                .lifecycle
                .write()
                .map_err(|_| RegistryDispatchError::Poisoned)?;
            if let Err(error) = entry.index.close() {
                return Ok(RegistryDispatchResult::payload(encode_registry_error(
                    version,
                    lifecycle_index_error_to_portable(
                        searchkernel_ir::LifecycleOperation::Delete,
                        error,
                    ),
                )));
            }
        }

        match Index::delete_storage(&path) {
            Ok(()) => Ok(RegistryDispatchResult::payload(
                wire::encode_delete_index_result(version),
            )),
            Err(error) => Ok(RegistryDispatchResult::payload(encode_registry_error(
                version,
                lifecycle_index_error_to_portable(
                    searchkernel_ir::LifecycleOperation::Delete,
                    error,
                ),
            ))),
        }
    }

    fn dispatch_loaded(
        &self,
        target: &str,
        request: &[u8],
        expected_generation: Option<&str>,
        version: WireVersion,
    ) -> Result<RegistryDispatchResult, RegistryDispatchError> {
        // Acquire the per-entry admission guard while the entry is still
        // reachable from the registry. Delete removes the entry under the same
        // registry lock before taking its exclusive guard, so every request
        // that successfully selects the target is guaranteed to drain first.
        let entries = self
            .entries
            .lock()
            .map_err(|_| RegistryDispatchError::Poisoned)?;
        let Some(entry) = entries.get(target).cloned() else {
            return Ok(RegistryDispatchResult::payload(encode_registry_error(
                version,
                searchkernel_ir::not_found_error(),
            )));
        };
        if expected_generation.is_some_and(|generation| generation != entry.generation.as_str()) {
            return Ok(RegistryDispatchResult::payload(encode_registry_error(
                version,
                searchkernel_ir::stale_index_handle_error(),
            )));
        }
        let lifecycle = entry
            .lifecycle
            .read()
            .map_err(|_| RegistryDispatchError::Poisoned)?;
        drop(entries);

        let result = entry
            .dispatcher
            .dispatch(request)
            .map_err(map_dispatch_error)?;
        drop(lifecycle);
        Ok(RegistryDispatchResult::payload(result))
    }

    fn checked_index_path(&self, target: &str) -> Result<PathBuf, PortableError> {
        let path = self.data_root.join(target);
        match std::fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                Err(searchkernel_ir::path_failure(
                    target,
                    searchkernel_ir::PathFailure::SymbolicLink,
                    None,
                ))
            }
            Ok(metadata) if !metadata.is_dir() => Err(searchkernel_ir::path_failure(
                target,
                searchkernel_ir::PathFailure::NotDirectory,
                None,
            )),
            Ok(_) => Ok(path),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(path),
            Err(error) => Err(searchkernel_ir::path_failure(
                target,
                searchkernel_ir::PathFailure::InspectFailed,
                Some(&error.to_string()),
            )),
        }
    }
}

fn encode_registry_error(version: WireVersion, error: PortableError) -> Vec<u8> {
    wire::encode_error(version, &error)
}

fn map_dispatch_error(error: DispatchError) -> RegistryDispatchError {
    match error {
        DispatchError::InvalidRequest => RegistryDispatchError::InvalidRequest,
        DispatchError::UnsupportedProtocol(version) => {
            RegistryDispatchError::UnsupportedProtocol(version)
        }
        DispatchError::UnsupportedOperation(value) => {
            RegistryDispatchError::UnsupportedOperation(value)
        }
        DispatchError::Closed => RegistryDispatchError::Closed,
    }
}

fn lifecycle_index_error_to_portable(
    operation: searchkernel_ir::LifecycleOperation,
    error: IndexError,
) -> PortableError {
    searchkernel_ir::lifecycle_failure(operation, error.to_lifecycle_failure())
}

fn configuration_error_to_portable(error: IndexError) -> PortableError {
    let code = match error.code() {
        searchkernel::ErrorCode::InvalidArgument => searchkernel_ir::ErrorCode::InvalidArgument,
        searchkernel::ErrorCode::Unsupported => searchkernel_ir::ErrorCode::Unsupported,
        searchkernel::ErrorCode::FailedPrecondition
        | searchkernel::ErrorCode::CorruptIndex
        | searchkernel::ErrorCode::IncompatibleIndex
        | searchkernel::ErrorCode::IndexClosed => searchkernel_ir::ErrorCode::FailedPrecondition,
        searchkernel::ErrorCode::AlreadyExists => searchkernel_ir::ErrorCode::AlreadyExists,
        _ => searchkernel_ir::ErrorCode::Unavailable,
    };
    let mut result = PortableError::new(code, error.to_string());
    for detail in error.details() {
        result = result.with_detail(&detail.code, detail.path.as_deref(), &detail.message);
    }
    result
}
