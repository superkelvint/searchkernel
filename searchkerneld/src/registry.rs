use flatbuffers::root;
use searchkernel::{Error as IndexError, ErrorCode as IndexErrorCode, Index};
use searchkernel_dispatch::{DispatchError, Dispatcher};
use searchkernel_ir::{wire::v1::{self as wire_ir, ControlPlaneRequest}, PortableError};
use searchkernel_protocol::{
    protocol::v1::{self as wire, PROTOCOL_VERSION},
    wire::v1 as api,
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

const REQUEST_IDENTIFIER: &[u8; 4] = b"SKRQ";
const RESPONSE_IDENTIFIER: &str = "SKRS";

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
    pub(crate) generation: Option<String>,
}

impl RegistryDispatchResult {
    fn payload(payload: Vec<u8>) -> Self {
        Self {
            payload,
            generation: None,
        }
    }

    fn with_generation(payload: Vec<u8>, generation: String) -> Self {
        Self {
            payload,
            generation: Some(generation),
        }
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
        Ok(self.dispatch_with_generation(request, None)?.payload)
    }

    pub(crate) fn dispatch_grpc(
        &self,
        request: &[u8],
        expected_generation: Option<&str>,
    ) -> Result<RegistryDispatchResult, RegistryDispatchError> {
        self.dispatch_with_generation(request, expected_generation)
    }

    fn dispatch_with_generation(
        &self,
        request: &[u8],
        expected_generation: Option<&str>,
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

        let envelope = parse_request(request)?;
        if envelope.protocol_version() != PROTOCOL_VERSION {
            return Err(RegistryDispatchError::UnsupportedProtocol(
                envelope.protocol_version(),
            ));
        }
        let control = match wire_ir::resolve_control_plane_request(envelope) {
            Ok(control) => control,
            Err(wire_ir::ControlPlaneResolutionError::InvalidEnvelope) => {
                return Err(RegistryDispatchError::InvalidRequest)
            }
            Err(wire_ir::ControlPlaneResolutionError::Semantic(error)) => {
                return Ok(RegistryDispatchResult::payload(encode_error(error)))
            }
        };

        match control {
            ControlPlaneRequest::Create { target, definition } => self.create(target, definition),
            ControlPlaneRequest::Open { target } => self.open(target),
            ControlPlaneRequest::Delete { target } => self.delete_index(target),
            ControlPlaneRequest::Route { target } => {
                self.dispatch_loaded(target, request, expected_generation)
            }
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

    fn create(
        &self,
        target: &str,
        definition: api::IndexDefinition<'_>,
    ) -> Result<RegistryDispatchResult, RegistryDispatchError> {
        let definition_ir = match wire_ir::index_definition_to_ir(definition) {
            Ok(definition) => definition,
            Err(error) => return Ok(RegistryDispatchResult::payload(encode_error(error))),
        };
        let definition_bytes = wire_ir::index_definition_ir_to_api(&definition_ir);

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
                return Ok(RegistryDispatchResult::payload(encode_error(
                    searchkernel_ir::already_exists_error(),
                )));
            }
        }

        let path = match self.checked_index_path(target) {
            Ok(path) => path,
            Err(error) => return Ok(RegistryDispatchResult::payload(encode_error(error))),
        };
        let index = match Index::create_from_definition_bytes(&path, &definition_bytes) {
            Ok(index) => index,
            Err(error) => {
                return Ok(RegistryDispatchResult::payload(encode_error(
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
        Ok(RegistryDispatchResult::with_generation(
            encode_lifecycle_result(
                context.definition(),
                wire::ResponsePayload::SearchKernel_Wire_CreateIndexResult,
            ),
            generation,
        ))
    }

    fn open(&self, target: &str) -> Result<RegistryDispatchResult, RegistryDispatchError> {
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
                return Ok(RegistryDispatchResult::with_generation(
                    encode_lifecycle_result(
                        context.definition(),
                        wire::ResponsePayload::SearchKernel_Wire_OpenIndexResult,
                    ),
                    entry.generation.clone(),
                ));
            }
        }

        let path = match self.checked_index_path(target) {
            Ok(path) => path,
            Err(error) => return Ok(RegistryDispatchResult::payload(encode_error(error))),
        };
        let index = match Index::open(&path) {
            Ok(index) => index,
            Err(error) => {
                return Ok(RegistryDispatchResult::payload(encode_error(
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
        Ok(RegistryDispatchResult::with_generation(
            encode_lifecycle_result(
                context.definition(),
                wire::ResponsePayload::SearchKernel_Wire_OpenIndexResult,
            ),
            generation,
        ))
    }

    fn delete_index(&self, target: &str) -> Result<RegistryDispatchResult, RegistryDispatchError> {
        let transition = self.transition_lock(target)?;
        let _transition = transition
            .lock()
            .map_err(|_| RegistryDispatchError::Poisoned)?;
        let path = match self.checked_index_path(target) {
            Ok(path) => path,
            Err(error) => return Ok(RegistryDispatchResult::payload(encode_error(error))),
        };

        let entry = self
            .entries
            .lock()
            .map_err(|_| RegistryDispatchError::Poisoned)?
            .remove(target);
        if entry.is_none() && !path.exists() {
            return Ok(RegistryDispatchResult::payload(encode_error(
                searchkernel_ir::not_found_error(),
            )));
        }

        if let Some(entry) = entry {
            let _lifecycle = entry
                .lifecycle
                .write()
                .map_err(|_| RegistryDispatchError::Poisoned)?;
            if let Err(error) = entry.index.close() {
                return Ok(RegistryDispatchResult::payload(encode_error(
                    lifecycle_index_error_to_portable(
                        searchkernel_ir::LifecycleOperation::Delete,
                        error,
                    ),
                )));
            }
        }

        match Index::delete_storage(&path) {
            Ok(()) => Ok(RegistryDispatchResult::payload(encode_delete_index_result())),
            Err(error) => Ok(RegistryDispatchResult::payload(encode_error(
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
            return Ok(RegistryDispatchResult::payload(encode_error(
                searchkernel_ir::not_found_error(),
            )));
        };
        if expected_generation.is_some_and(|generation| generation != entry.generation.as_str()) {
            return Ok(RegistryDispatchResult::payload(encode_error(
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

fn parse_request(bytes: &[u8]) -> Result<wire::RequestEnvelope<'_>, RegistryDispatchError> {
    if bytes.len() < 8 || bytes[4..8] != *REQUEST_IDENTIFIER {
        return Err(RegistryDispatchError::InvalidRequest);
    }
    root::<wire::RequestEnvelope<'_>>(bytes).map_err(|_| RegistryDispatchError::InvalidRequest)
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

fn encode_lifecycle_result(
    definition: &searchkernel_ir::IndexDefinitionIr,
    payload_type: wire::ResponsePayload,
) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let index = wire_ir::index_definition_offset(&mut builder, definition);
    let payload = match payload_type {
        wire::ResponsePayload::SearchKernel_Wire_CreateIndexResult => {
            api::CreateIndexResult::create(
                &mut builder,
                &api::CreateIndexResultArgs { index: Some(index) },
            )
            .as_union_value()
        }
        wire::ResponsePayload::SearchKernel_Wire_OpenIndexResult => api::OpenIndexResult::create(
            &mut builder,
            &api::OpenIndexResultArgs { index: Some(index) },
        )
        .as_union_value(),
        _ => unreachable!("lifecycle result kind is fixed by caller"),
    };
    encode_response(&mut builder, payload_type, payload)
}

fn encode_delete_index_result() -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let payload =
        api::DeleteIndexResult::create(&mut builder, &api::DeleteIndexResultArgs::default())
            .as_union_value();
    encode_response(
        &mut builder,
        wire::ResponsePayload::SearchKernel_Wire_DeleteIndexResult,
        payload,
    )
}

fn encode_error(error: PortableError) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let payload = wire_ir::error_offset(&mut builder, &error).as_union_value();
    encode_response(
        &mut builder,
        wire::ResponsePayload::SearchKernel_Wire_Error,
        payload,
    )
}

fn encode_response(
    builder: &mut flatbuffers::FlatBufferBuilder<'_>,
    payload_type: wire::ResponsePayload,
    payload: flatbuffers::WIPOffset<flatbuffers::UnionWIPOffset>,
) -> Vec<u8> {
    let envelope = wire::ResponseEnvelope::create(
        builder,
        &wire::ResponseEnvelopeArgs {
            protocol_version: PROTOCOL_VERSION,
            payload_type,
            payload: Some(payload),
        },
    );
    builder.finish(envelope, Some(RESPONSE_IDENTIFIER));
    builder.finished_data().to_vec()
}
