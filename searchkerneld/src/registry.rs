use flatbuffers::root;
use searchkernel::{Error as IndexError, ErrorCode as IndexErrorCode, Index};
use searchkernel_dispatch::{DispatchError, Dispatcher};
use searchkernel_ir::{ErrorCode, PortableError};
use searchkernel_protocol::{
    protocol::v2 as wire, request_index_target, v2 as api, RequestTargetError, PROTOCOL_VERSION,
};
use std::{
    collections::BTreeMap,
    fmt,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

const REQUEST_IDENTIFIER: &[u8; 4] = b"SKRQ";
const RESPONSE_IDENTIFIER: &str = "SKRS";

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
    MissingTarget,
    InvalidTarget(String),
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
            Self::MissingTarget => write!(
                formatter,
                "Search Kernel request is missing an index target"
            ),
            Self::InvalidTarget(message) => {
                write!(formatter, "invalid Search Kernel index target: {message}")
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
}

pub struct IndexRegistry {
    data_root: PathBuf,
    entries: Mutex<BTreeMap<String, Arc<RegistryEntry>>>,
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
        })
    }

    pub fn data_root(&self) -> &Path {
        &self.data_root
    }

    pub fn dispatch(&self, request: &[u8]) -> Result<Vec<u8>, RegistryDispatchError> {
        let envelope = parse_request(request)?;
        if envelope.protocol_version() != PROTOCOL_VERSION {
            return Err(RegistryDispatchError::UnsupportedProtocol(
                envelope.protocol_version(),
            ));
        }
        let target = request_index_target(envelope).map_err(|error| match error {
            RequestTargetError::Missing => RegistryDispatchError::MissingTarget,
            RequestTargetError::Invalid(error) => {
                RegistryDispatchError::InvalidTarget(error.to_string())
            }
        })?;

        match envelope.payload_type() {
            wire::RequestPayload::SearchKernel_V2_CreateIndexRequest => {
                let payload = envelope
                    .payload_as_search_kernel_v2_create_index_request()
                    .ok_or(RegistryDispatchError::InvalidRequest)?;
                let definition = payload
                    .definition()
                    .ok_or(RegistryDispatchError::InvalidRequest)?;
                self.create(target, definition)
            }
            wire::RequestPayload::SearchKernel_V2_OpenIndexRequest => {
                envelope
                    .payload_as_search_kernel_v2_open_index_request()
                    .ok_or(RegistryDispatchError::InvalidRequest)?;
                self.open(target)
            }
            wire::RequestPayload::NONE => Err(RegistryDispatchError::InvalidRequest),
            _ => self.dispatch_loaded(target, request),
        }
    }

    pub fn close_all(&self) -> Result<(), IndexError> {
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
            if let Err(error) = entry.index.close() {
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
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
    ) -> Result<Vec<u8>, RegistryDispatchError> {
        let mut entries = self
            .entries
            .lock()
            .map_err(|_| RegistryDispatchError::Poisoned)?;
        if entries.contains_key(target) {
            return Ok(encode_error(PortableError::new(
                ErrorCode::AlreadyExists,
                format!("index already exists: {target}"),
            )));
        }

        let definition_ir = match searchkernel_ir::index_definition_to_ir(definition) {
            Ok(definition) => definition,
            Err(error) => return Ok(encode_error(error)),
        };
        let definition_bytes = searchkernel_ir::index_definition_ir_to_api(&definition_ir);
        let path = match self.checked_index_path(target) {
            Ok(path) => path,
            Err(error) => return Ok(encode_error(error)),
        };
        let index = match Index::create_from_definition_bytes(&path, &definition_bytes) {
            Ok(index) => index,
            Err(error) => return Ok(encode_error(index_error_to_portable(error))),
        };
        let (context, session) = index.clone().into_session_parts();
        let entry = Arc::new(RegistryEntry {
            index,
            dispatcher: Arc::new(Dispatcher::new(context.clone(), session)),
        });
        entries.insert(target.to_owned(), entry);
        Ok(encode_lifecycle_result(
            context.definition(),
            wire::ResponsePayload::SearchKernel_V2_CreateIndexResult,
        ))
    }

    fn open(&self, target: &str) -> Result<Vec<u8>, RegistryDispatchError> {
        let mut entries = self
            .entries
            .lock()
            .map_err(|_| RegistryDispatchError::Poisoned)?;
        if let Some(entry) = entries.get(target) {
            let (context, _) = entry.index.clone().into_session_parts();
            return Ok(encode_lifecycle_result(
                context.definition(),
                wire::ResponsePayload::SearchKernel_V2_OpenIndexResult,
            ));
        }

        let path = match self.checked_index_path(target) {
            Ok(path) => path,
            Err(error) => return Ok(encode_error(error)),
        };
        let index = match Index::open(&path) {
            Ok(index) => index,
            Err(error) => return Ok(encode_error(index_error_to_portable(error))),
        };
        let (context, session) = index.clone().into_session_parts();
        let entry = Arc::new(RegistryEntry {
            index,
            dispatcher: Arc::new(Dispatcher::new(context.clone(), session)),
        });
        entries.insert(target.to_owned(), entry);
        Ok(encode_lifecycle_result(
            context.definition(),
            wire::ResponsePayload::SearchKernel_V2_OpenIndexResult,
        ))
    }

    fn dispatch_loaded(
        &self,
        target: &str,
        request: &[u8],
    ) -> Result<Vec<u8>, RegistryDispatchError> {
        let dispatcher = {
            let entries = self
                .entries
                .lock()
                .map_err(|_| RegistryDispatchError::Poisoned)?;
            entries.get(target).map(|entry| entry.dispatcher.clone())
        };
        let Some(dispatcher) = dispatcher else {
            return Ok(encode_error(PortableError::new(
                ErrorCode::NotFound,
                format!("index is not open: {target}"),
            )));
        };
        dispatcher.dispatch(request).map_err(map_dispatch_error)
    }

    fn checked_index_path(&self, target: &str) -> Result<PathBuf, PortableError> {
        let path = self.data_root.join(target);
        match std::fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                Err(PortableError::failed_precondition(
                    "unsafe_index_path",
                    Some("target"),
                    format!("index target {target:?} resolves through a symbolic link"),
                ))
            }
            Ok(metadata) if !metadata.is_dir() => Err(PortableError::failed_precondition(
                "unsafe_index_path",
                Some("target"),
                format!("index target {target:?} is not a directory"),
            )),
            Ok(_) => Ok(path),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(path),
            Err(error) => Err(PortableError::new(
                ErrorCode::Unavailable,
                format!("inspect index target {target:?}: {error}"),
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

fn index_error_to_portable(error: IndexError) -> PortableError {
    let code = match error.code() {
        IndexErrorCode::InvalidArgument => ErrorCode::InvalidArgument,
        IndexErrorCode::NotFound => ErrorCode::NotFound,
        IndexErrorCode::AlreadyExists => ErrorCode::AlreadyExists,
        IndexErrorCode::FailedPrecondition
        | IndexErrorCode::CorruptIndex
        | IndexErrorCode::IncompatibleIndex
        | IndexErrorCode::IndexClosed => ErrorCode::FailedPrecondition,
        IndexErrorCode::Conflict => ErrorCode::Conflict,
        IndexErrorCode::Unsupported => ErrorCode::Unsupported,
        IndexErrorCode::ResourceExhausted => ErrorCode::ResourceExhausted,
        IndexErrorCode::Internal => ErrorCode::Internal,
        IndexErrorCode::Unavailable => ErrorCode::Unavailable,
        IndexErrorCode::DeadlineExceeded => ErrorCode::DeadlineExceeded,
        _ => ErrorCode::Internal,
    };
    let mut portable = PortableError::new(code, error.to_string());
    portable.retryable = error.retryable();
    portable.details = error
        .details()
        .iter()
        .map(|detail| searchkernel_ir::ErrorDetail {
            code: detail.code.clone(),
            path: detail.path.clone(),
            message: detail.message.clone(),
            metadata: detail.metadata.clone(),
        })
        .collect();
    portable
}

fn encode_lifecycle_result(
    definition: &searchkernel_ir::IndexDefinitionIr,
    payload_type: wire::ResponsePayload,
) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let index = searchkernel_ir::index_definition_offset(&mut builder, definition);
    let payload = match payload_type {
        wire::ResponsePayload::SearchKernel_V2_CreateIndexResult => api::CreateIndexResult::create(
            &mut builder,
            &api::CreateIndexResultArgs { index: Some(index) },
        )
        .as_union_value(),
        wire::ResponsePayload::SearchKernel_V2_OpenIndexResult => api::OpenIndexResult::create(
            &mut builder,
            &api::OpenIndexResultArgs { index: Some(index) },
        )
        .as_union_value(),
        _ => unreachable!("lifecycle result kind is fixed by caller"),
    };
    encode_response(&mut builder, payload_type, payload)
}

fn encode_error(error: PortableError) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let payload = searchkernel_ir::error_offset(&mut builder, &error).as_union_value();
    encode_response(
        &mut builder,
        wire::ResponsePayload::SearchKernel_V2_Error,
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
