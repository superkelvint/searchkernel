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

pub struct IndexRegistry {
    data_root: PathBuf,
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
            ControlPlaneRequest::Create { target, definition } => {
                self.create(&target, *definition, version)
            }
            ControlPlaneRequest::Open { target } => self.open(&target, version),
            ControlPlaneRequest::Delete { target } => self.delete_index(&target, version),
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

        let entry = self
            .entries
            .lock()
            .map_err(|_| RegistryDispatchError::Poisoned)?
            .remove(target);
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
