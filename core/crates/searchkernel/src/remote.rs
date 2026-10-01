//! Optional synchronous Rust client for the multi-index `searchkerneld` daemon.
//!
//! The public Rust API remains synchronous and identical after connection.
//! A dedicated Tokio runtime thread owns the HTTP/2 channel, so the synchronous
//! Rust API never needs to create or nest a runtime on the caller's thread.
//! Operations are still synchronous and therefore block the calling thread;
//! async applications should invoke them from their normal blocking-work path.

use crate::{Index, IndexDefinition, Result};
use searchkernel_ir::{
    self as ir, Backend, BackendSession, ErrorCode, IndexContext, PortableError,
};
use searchkernel_protocol::protocol::v1::{self as wire, PROTOCOL_VERSION};
use searchkernel_rpc::{
    rpc::{search_kernel_client::SearchKernelClient, FlatBufferFrame},
    INDEX_GENERATION_METADATA_KEY, MAX_GRPC_MESSAGE_SIZE,
};
#[cfg(unix)]
use std::path::PathBuf;
use std::{
    collections::HashMap,
    sync::{mpsc as std_mpsc, Arc, Mutex, Weak},
    thread,
    time::Duration,
};
use tokio::sync::mpsc;
#[cfg(unix)]
use tonic::transport::Uri;
use tonic::transport::{Channel, Endpoint};
#[cfg(unix)]
use tower::service_fn;

/// Connection and request limits for a remote Rust `RemoteClient`.
#[derive(Clone, Debug)]
pub struct RemoteOptions {
    connect_timeout: Duration,
    request_timeout: Duration,
    max_message_size: usize,
}

impl Default for RemoteOptions {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(10),
            request_timeout: Duration::from_secs(60),
            max_message_size: MAX_GRPC_MESSAGE_SIZE,
        }
    }
}

impl RemoteOptions {
    /// Set the maximum time allowed to establish the gRPC channel.
    #[must_use]
    pub fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    /// Set the maximum time allowed for each Search Kernel operation.
    #[must_use]
    pub fn request_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = timeout;
        self
    }

    /// Set the maximum encoded and decoded gRPC message size.
    #[must_use]
    pub fn max_message_size(mut self, bytes: usize) -> Self {
        self.max_message_size = bytes;
        self
    }

    fn validate(&self) -> ir::Result<()> {
        if self.connect_timeout.is_zero() {
            return Err(PortableError::invalid(
                "invalid_remote_connect_timeout",
                None::<String>,
                "remote connect timeout must be greater than zero",
            ));
        }
        if self.request_timeout.is_zero() {
            return Err(PortableError::invalid(
                "invalid_remote_request_timeout",
                None::<String>,
                "remote request timeout must be greater than zero",
            ));
        }
        if self.max_message_size == 0 {
            return Err(PortableError::invalid(
                "invalid_remote_message_size",
                None::<String>,
                "remote message size must be greater than zero",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
enum RemoteAddress {
    Tcp(String),
    #[cfg(unix)]
    Unix(PathBuf),
}

impl RemoteAddress {
    fn parse(value: &str) -> ir::Result<Self> {
        let value = value.trim();
        if value.is_empty() {
            return Err(PortableError::invalid(
                "invalid_remote_endpoint",
                None::<String>,
                "remote endpoint must not be empty",
            ));
        }
        #[cfg(unix)]
        if let Some(path) = value
            .strip_prefix("unix://")
            .or_else(|| value.strip_prefix("unix:"))
        {
            if path.is_empty() {
                return Err(PortableError::invalid(
                    "invalid_remote_endpoint",
                    None::<String>,
                    "Unix-socket endpoint must include a path",
                ));
            }
            return Ok(Self::Unix(PathBuf::from(path)));
        }
        #[cfg(not(unix))]
        if value.starts_with("unix:") {
            return Err(PortableError::unsupported(
                "unix_remote_not_supported",
                None::<String>,
                "Unix-domain remote endpoints are not supported on this platform",
            ));
        }
        if value.starts_with("https://") {
            return Err(PortableError::unsupported(
                "remote_tls_not_supported",
                None::<String>,
                "the Rust remote client does not yet configure TLS; use a trusted private network, tunnel, or service-mesh endpoint",
            ));
        }
        let endpoint = if value.starts_with("http://") {
            value.to_owned()
        } else if value.contains("://") {
            return Err(PortableError::invalid(
                "invalid_remote_endpoint",
                None::<String>,
                "remote endpoint must be host:port, http://host:port, or unix:///path",
            ));
        } else {
            format!("http://{value}")
        };
        Ok(Self::Tcp(endpoint))
    }
}

struct RemoteReply {
    payload: Vec<u8>,
    generation: Option<String>,
}

enum Command {
    Execute {
        payload: Vec<u8>,
        retry_safe: bool,
        generation: Option<String>,
        reply: std_mpsc::SyncSender<ir::Result<RemoteReply>>,
    },
    Shutdown,
}

struct RemoteTransport {
    commands: mpsc::UnboundedSender<Command>,
    worker: Mutex<Option<thread::JoinHandle<()>>>,
}

impl RemoteTransport {
    fn connect(address: RemoteAddress, options: RemoteOptions) -> ir::Result<Self> {
        options.validate()?;
        let (commands, receiver) = mpsc::unbounded_channel();
        let (ready_tx, ready_rx) = std_mpsc::sync_channel(1);
        let worker_options = options.clone();
        let worker = thread::Builder::new()
            .name("searchkernel-remote".to_owned())
            .spawn(move || {
                let runtime = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => runtime,
                    Err(error) => {
                        let _ = ready_tx.send(Err(transport_error(
                            ErrorCode::Internal,
                            false,
                            format!("create remote runtime: {error}"),
                        )));
                        return;
                    }
                };
                runtime.block_on(run_worker(address, worker_options, receiver, ready_tx));
            })
            .map_err(|error| {
                transport_error(
                    ErrorCode::Internal,
                    false,
                    format!("start remote runtime thread: {error}"),
                )
            })?;

        match ready_rx.recv() {
            Ok(Ok(())) => Ok(Self {
                commands,
                worker: Mutex::new(Some(worker)),
            }),
            Ok(Err(error)) => {
                let _ = worker.join();
                Err(error)
            }
            Err(_) => {
                let _ = worker.join();
                Err(transport_error(
                    ErrorCode::Unavailable,
                    true,
                    "remote runtime exited before connection completed",
                ))
            }
        }
    }

    fn call(
        &self,
        payload: Vec<u8>,
        retry_safe: bool,
        generation: Option<&str>,
    ) -> ir::Result<RemoteReply> {
        let (reply_tx, reply_rx) = std_mpsc::sync_channel(1);
        self.commands
            .send(Command::Execute {
                payload,
                retry_safe,
                generation: generation.map(str::to_owned),
                reply: reply_tx,
            })
            .map_err(|_| {
                transport_error(
                    ErrorCode::Unavailable,
                    true,
                    "remote transport is not running",
                )
            })?;
        reply_rx.recv().map_err(|_| {
            request_transport_error(
                ErrorCode::Unavailable,
                retry_safe,
                !retry_safe,
                "remote transport stopped before returning a response",
            )
        })?
    }

    fn shutdown(&self) -> ir::Result<()> {
        let _ = self.commands.send(Command::Shutdown);
        if let Some(worker) = self
            .worker
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take()
        {
            worker.join().map_err(|_| {
                transport_error(
                    ErrorCode::Internal,
                    false,
                    "remote transport runtime thread panicked during shutdown",
                )
            })?;
        }
        Ok(())
    }
}

impl Drop for RemoteTransport {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

async fn connect_channel(address: RemoteAddress, options: &RemoteOptions) -> ir::Result<Channel> {
    match address {
        RemoteAddress::Tcp(endpoint) => {
            let endpoint = Endpoint::from_shared(endpoint)
                .map_err(|error| {
                    PortableError::invalid(
                        "invalid_remote_endpoint",
                        None::<String>,
                        format!("invalid remote endpoint: {error}"),
                    )
                })?
                .connect_timeout(options.connect_timeout);
            endpoint.connect().await.map_err(|error| {
                transport_error(
                    ErrorCode::Unavailable,
                    true,
                    format!("connect to remote Search Kernel: {error}"),
                )
            })
        }
        #[cfg(unix)]
        RemoteAddress::Unix(path) => {
            let endpoint =
                Endpoint::from_static("http://[::]:50051").connect_timeout(options.connect_timeout);
            endpoint
                .connect_with_connector(service_fn(move |_: Uri| {
                    let path = path.clone();
                    async move {
                        let stream = tokio::net::UnixStream::connect(path).await?;
                        Ok::<_, std::io::Error>(hyper_util::rt::TokioIo::new(stream))
                    }
                }))
                .await
                .map_err(|error| {
                    transport_error(
                        ErrorCode::Unavailable,
                        true,
                        format!("connect to remote Search Kernel Unix socket: {error}"),
                    )
                })
        }
    }
}

async fn run_worker(
    address: RemoteAddress,
    options: RemoteOptions,
    mut receiver: mpsc::UnboundedReceiver<Command>,
    ready: std_mpsc::SyncSender<ir::Result<()>>,
) {
    let channel = match connect_channel(address, &options).await {
        Ok(channel) => channel,
        Err(error) => {
            let _ = ready.send(Err(error));
            return;
        }
    };
    let client = SearchKernelClient::new(channel)
        .max_decoding_message_size(options.max_message_size)
        .max_encoding_message_size(options.max_message_size);
    if ready.send(Ok(())).is_err() {
        return;
    }

    while let Some(command) = receiver.recv().await {
        match command {
            Command::Execute {
                payload,
                retry_safe,
                generation,
                reply,
            } => {
                let mut client = client.clone();
                let request_timeout = options.request_timeout;
                tokio::spawn(async move {
                    let mut request = tonic::Request::new(FlatBufferFrame { payload });
                    let result = if let Some(generation) = generation {
                        match generation.parse() {
                            Ok(value) => {
                                request
                                    .metadata_mut()
                                    .insert(INDEX_GENERATION_METADATA_KEY, value);
                                None
                            }
                            Err(_) => Some(Err(invalid_remote_response(
                                "remote index generation is not valid gRPC metadata",
                            ))),
                        }
                    } else {
                        None
                    };
                    let result = match result {
                        Some(result) => result,
                        None => {
                            match tokio::time::timeout(request_timeout, client.execute(request))
                                .await
                            {
                                Ok(Ok(response)) => {
                                    let generation = response
                                    .metadata()
                                    .get(INDEX_GENERATION_METADATA_KEY)
                                    .map(|value| {
                                        value.to_str().map(str::to_owned).map_err(|_| {
                                            invalid_remote_response(
                                                "remote index generation response metadata is invalid",
                                            )
                                        })
                                    })
                                    .transpose();
                                    match generation {
                                        Ok(generation) => Ok(RemoteReply {
                                            payload: response.into_inner().payload,
                                            generation,
                                        }),
                                        Err(error) => Err(error),
                                    }
                                }
                                Ok(Err(status)) => Err(status_error(status, retry_safe)),
                                Err(_) => Err(request_transport_error(
                                    ErrorCode::DeadlineExceeded,
                                    retry_safe,
                                    !retry_safe,
                                    "remote Search Kernel request timed out",
                                )),
                            }
                        }
                    };
                    let _ = reply.send(result);
                });
            }
            Command::Shutdown => break,
        }
    }
}

fn transport_error(code: ErrorCode, retryable: bool, message: impl Into<String>) -> PortableError {
    let message = message.into();
    let mut error = PortableError::new(code, message.clone()).with_detail(
        "grpc_transport",
        None::<String>,
        message,
    );
    error.retryable = retryable;
    error
}

fn request_transport_error(
    code: ErrorCode,
    retryable: bool,
    outcome_unknown: bool,
    message: impl Into<String>,
) -> PortableError {
    let message = message.into();
    let mut error = transport_error(code, retryable, message);
    if outcome_unknown {
        error = error.with_detail(
            "remote_outcome_unknown",
            None::<String>,
            "the remote mutation may have executed before the transport failure; do not retry blindly",
        );
    }
    error
}

fn status_error(status: tonic::Status, retry_safe: bool) -> PortableError {
    let (code, transient, delivery_ambiguous) = match status.code() {
        // searchkerneld reserves these statuses for failures before portable
        // backend execution. Portable semantic errors travel inside the current
        // response envelope instead.
        tonic::Code::InvalidArgument => (ErrorCode::Internal, false, false),
        tonic::Code::FailedPrecondition => (ErrorCode::FailedPrecondition, false, false),
        tonic::Code::Unimplemented => (ErrorCode::FailedPrecondition, false, false),
        // This can be daemon admission rejection (pre-execution) or a client
        // decode-size failure after execution, so mutation completion is
        // conservatively unknown.
        tonic::Code::ResourceExhausted => (ErrorCode::ResourceExhausted, true, true),

        // A transport can fail after the daemon has accepted or even completed
        // a request. Reads may be retried; writes must expose that their remote
        // completion is unknown even when the status itself is not retryable.
        tonic::Code::Unavailable => (ErrorCode::Unavailable, true, true),
        tonic::Code::DeadlineExceeded => (ErrorCode::DeadlineExceeded, true, true),
        tonic::Code::Cancelled => (ErrorCode::Unavailable, true, true),
        tonic::Code::Aborted => (ErrorCode::Conflict, true, true),
        tonic::Code::Internal | tonic::Code::Unknown | tonic::Code::DataLoss => {
            (ErrorCode::Internal, false, true)
        }

        // searchkerneld does not currently use these transport statuses. Keep
        // their public code useful while conservatively treating a mutation's
        // delivery as ambiguous.
        tonic::Code::NotFound => (ErrorCode::NotFound, false, true),
        tonic::Code::AlreadyExists => (ErrorCode::AlreadyExists, false, true),
        tonic::Code::Unauthenticated | tonic::Code::PermissionDenied => {
            (ErrorCode::FailedPrecondition, false, false)
        }
        tonic::Code::OutOfRange => (ErrorCode::Internal, false, true),
        tonic::Code::Ok => (ErrorCode::Internal, false, true),
    };
    let retryable = transient && retry_safe;
    request_transport_error(
        code,
        retryable,
        delivery_ambiguous && !retry_safe,
        format!(
            "remote Search Kernel gRPC {}: {}",
            status.code(),
            status.message()
        ),
    )
}

struct RemoteBackend {
    transport: Arc<RemoteTransport>,
    target: Option<String>,
    generation: Option<String>,
}

impl RemoteBackend {
    fn new(
        transport: Arc<RemoteTransport>,
        target: Option<String>,
        generation: Option<String>,
    ) -> Self {
        Self {
            transport,
            target,
            generation,
        }
    }

    fn connect_legacy(endpoint: &str, options: RemoteOptions) -> ir::Result<(Self, IndexContext)> {
        let address = RemoteAddress::parse(endpoint)?;
        let transport = Arc::new(RemoteTransport::connect(address, options)?);
        let backend = Self::new(transport, None, None);
        let response = backend.call(ir::encode_describe_request(), true)?;
        let envelope = response_envelope(&response)?;
        if let Some(error) = response_error(envelope)? {
            return Err(error);
        }
        if envelope.payload_type() != wire::ResponsePayload::SearchKernel_Wire_DescribeResult {
            return Err(invalid_remote_response(
                "describe response has the wrong payload kind",
            ));
        }
        let describe = envelope
            .payload_as_search_kernel_wire_describe_result()
            .ok_or_else(|| invalid_remote_response("describe response payload is missing"))?;
        let definition = describe.index().ok_or_else(|| {
            invalid_remote_response("describe response index definition is missing")
        })?;
        let definition = ir::index_definition_to_ir(definition).map_err(|error| {
            invalid_remote_response(format!(
                "describe response contains an invalid index definition: {}",
                error.message
            ))
        })?;
        Ok((backend, IndexContext::from_definition_ir(definition)))
    }

    fn target(&self) -> Option<&str> {
        self.target.as_deref()
    }

    fn call(&self, payload: Vec<u8>, retry_safe: bool) -> ir::Result<Vec<u8>> {
        self.transport
            .call(payload, retry_safe, self.generation.as_deref())
            .map(|response| response.payload)
    }
}

fn expected_response<'a>(
    payload: &'a [u8],
    expected: wire::ResponsePayload,
) -> ir::Result<wire::ResponseEnvelope<'a>> {
    let envelope = response_envelope(payload)?;
    if let Some(error) = response_error(envelope)? {
        return Err(error);
    }
    if envelope.payload_type() != expected {
        return Err(invalid_remote_response(format!(
            "remote response payload mismatch: expected {:?}, received {:?}",
            expected,
            envelope.payload_type()
        )));
    }
    Ok(envelope)
}

fn invalid_remote_response(message: impl Into<String>) -> PortableError {
    let message = message.into();
    PortableError::new(ErrorCode::Internal, message.clone()).with_detail(
        "invalid_remote_response",
        None::<String>,
        message,
    )
}

fn validate_response_id(operation: &str, expected: &str, actual: &str) -> ir::Result<()> {
    if actual != expected {
        return Err(invalid_remote_response(format!(
            "{operation} response identity mismatch: requested {expected:?}, received {actual:?}"
        )));
    }
    Ok(())
}

fn response_envelope(payload: &[u8]) -> ir::Result<wire::ResponseEnvelope<'_>> {
    if !wire::response_envelope_buffer_has_identifier(payload) {
        return Err(invalid_remote_response(
            "remote response is missing the Search Kernel response identifier",
        ));
    }
    let envelope = wire::root_as_response_envelope(payload).map_err(|error| {
        invalid_remote_response(format!(
            "invalid Search Kernel response FlatBuffer: {error}"
        ))
    })?;
    if envelope.protocol_version() != PROTOCOL_VERSION {
        return Err(PortableError::new(
            ErrorCode::FailedPrecondition,
            format!(
                "remote response protocol version {} does not match client version {}",
                envelope.protocol_version(),
                PROTOCOL_VERSION
            ),
        )
        .with_detail(
            "unsupported_protocol",
            None::<String>,
            "client and daemon protocol versions differ",
        ));
    }
    Ok(envelope)
}

fn response_error(envelope: wire::ResponseEnvelope<'_>) -> ir::Result<Option<PortableError>> {
    if envelope.payload_type() != wire::ResponsePayload::SearchKernel_Wire_Error {
        return Ok(None);
    }
    let error = envelope
        .payload_as_search_kernel_wire_error()
        .ok_or_else(|| invalid_remote_response("portable error payload is missing"))?;
    ir::portable_error_from_api(error).map(Some)
}

impl Backend for RemoteBackend {
    fn insert(&self, request: ir::InsertRequestIr) -> ir::Result<ir::InsertResult> {
        let payload = self.call(
            ir::encode_insert_request_for_target(&request, self.target())?,
            false,
        )?;
        let envelope = expected_response(
            &payload,
            wire::ResponsePayload::SearchKernel_Wire_InsertResult,
        )?;
        let result = ir::insert_result_from_api(
            envelope
                .payload_as_search_kernel_wire_insert_result()
                .ok_or_else(|| invalid_remote_response("insert result payload is missing"))?,
        )?;
        validate_response_id("insert", request.document().id().as_str(), &result.id)?;
        Ok(result)
    }

    fn upsert(&self, request: ir::UpsertRequestIr) -> ir::Result<ir::UpsertResult> {
        let payload = self.call(
            ir::encode_upsert_request_for_target(&request, self.target())?,
            false,
        )?;
        let envelope = expected_response(
            &payload,
            wire::ResponsePayload::SearchKernel_Wire_UpsertResult,
        )?;
        let result = ir::upsert_result_from_api(
            envelope
                .payload_as_search_kernel_wire_upsert_result()
                .ok_or_else(|| invalid_remote_response("upsert result payload is missing"))?,
        )?;
        validate_response_id("upsert", request.document().id().as_str(), &result.id)?;
        Ok(result)
    }

    fn patch(&self, request: ir::PatchRequestIr) -> ir::Result<ir::PatchResult> {
        let payload = self.call(
            ir::encode_patch_request_for_target(&request, self.target())?,
            false,
        )?;
        let envelope = expected_response(
            &payload,
            wire::ResponsePayload::SearchKernel_Wire_PatchResult,
        )?;
        let result = ir::patch_result_from_api(
            envelope
                .payload_as_search_kernel_wire_patch_result()
                .ok_or_else(|| invalid_remote_response("patch result payload is missing"))?,
        )?;
        validate_response_id("patch", request.id().as_str(), &result.id)?;
        Ok(result)
    }

    fn update(&self, request: ir::UpdateRequestIr) -> ir::Result<ir::UpdateResult> {
        let payload = self.call(
            ir::encode_update_request_for_target(&request, self.target())?,
            false,
        )?;
        let envelope = expected_response(
            &payload,
            wire::ResponsePayload::SearchKernel_Wire_UpdateResult,
        )?;
        let result = ir::update_result_from_api(
            envelope
                .payload_as_search_kernel_wire_update_result()
                .ok_or_else(|| invalid_remote_response("update result payload is missing"))?,
        )?;
        validate_response_id("update", request.id().as_str(), &result.id)?;
        Ok(result)
    }

    fn delete(&self, request: ir::DeleteRequestIr) -> ir::Result<ir::DeleteResult> {
        let payload = self.call(
            ir::encode_delete_request_for_target(&request, self.target())?,
            false,
        )?;
        let envelope = expected_response(
            &payload,
            wire::ResponsePayload::SearchKernel_Wire_DeleteResult,
        )?;
        let result = ir::delete_result_from_api(
            envelope
                .payload_as_search_kernel_wire_delete_result()
                .ok_or_else(|| invalid_remote_response("delete result payload is missing"))?,
        )?;
        validate_response_id("delete", request.id().as_str(), &result.id)?;
        Ok(result)
    }

    fn get(&self, request: ir::GetRequestIr) -> ir::Result<ir::GetResult> {
        let payload = self.call(
            ir::encode_get_request_for_target(&request, self.target())?,
            true,
        )?;
        let envelope =
            expected_response(&payload, wire::ResponsePayload::SearchKernel_Wire_GetResult)?;
        let result = ir::get_result_from_api(
            envelope
                .payload_as_search_kernel_wire_get_result()
                .ok_or_else(|| invalid_remote_response("get result payload is missing"))?,
        )?;
        if let Some(document) = &result.document {
            validate_response_id("get", request.id().as_str(), &document.id)?;
        }
        Ok(result)
    }

    fn search(&self, request: ir::SearchRequestIr) -> ir::Result<ir::SearchResult> {
        let payload = self.call(
            ir::encode_search_request_for_target(&request, self.target())?,
            true,
        )?;
        let envelope = expected_response(
            &payload,
            wire::ResponsePayload::SearchKernel_Wire_SearchResult,
        )?;
        ir::search_result_from_api(
            envelope
                .payload_as_search_kernel_wire_search_result()
                .ok_or_else(|| invalid_remote_response("search result payload is missing"))?,
        )
    }

    fn analyze(&self, request: ir::AnalyzeRequestIr) -> ir::Result<ir::AnalyzeResult> {
        let payload = self.call(
            ir::encode_analyze_request_for_target(&request, self.target())?,
            true,
        )?;
        let envelope = expected_response(
            &payload,
            wire::ResponsePayload::SearchKernel_Wire_AnalyzeResult,
        )?;
        ir::analyze_result_from_api(
            envelope
                .payload_as_search_kernel_wire_analyze_result()
                .ok_or_else(|| invalid_remote_response("analyze result payload is missing"))?,
        )
    }

    fn close(self: Box<Self>) -> ir::Result<()> {
        if self.target.is_none() {
            self.transport.shutdown()
        } else {
            Ok(())
        }
    }
}

/// A connection to a `searchkerneld` daemon that may host many logical indexes.
///
/// Each returned [`Index`] keeps its own target identity while sharing this
/// client's gRPC transport.
#[derive(Clone)]
pub struct RemoteClient {
    transport: Arc<RemoteTransport>,
    sessions: Arc<Mutex<HashMap<String, Vec<Weak<BackendSession>>>>>,
    lifecycle_lock: Arc<Mutex<()>>,
}

impl RemoteClient {
    /// Connect to `searchkerneld` without selecting an index.
    ///
    /// Plain `host:port` values use insecure gRPC over TCP. `http://`
    /// endpoints are also accepted, and Unix platforms additionally accept
    /// `unix:///path/to/searchkerneld.sock`.
    pub fn connect(endpoint: impl AsRef<str>) -> Result<Self> {
        Self::connect_with(endpoint, RemoteOptions::default())
    }

    /// Connect with explicit transport timeouts and message-size limits.
    pub fn connect_with(endpoint: impl AsRef<str>, options: RemoteOptions) -> Result<Self> {
        let address = RemoteAddress::parse(endpoint.as_ref()).map_err(crate::Error::from_ir)?;
        let transport =
            RemoteTransport::connect(address, options).map_err(crate::Error::from_ir)?;
        Ok(Self {
            transport: Arc::new(transport),
            sessions: Arc::new(Mutex::new(HashMap::new())),
            lifecycle_lock: Arc::new(Mutex::new(())),
        })
    }

    /// Create a logical target and return an index handle bound to it.
    pub fn create_index(
        &self,
        target: impl AsRef<str>,
        definition: IndexDefinition,
    ) -> Result<Index> {
        let _lifecycle = self.lifecycle_guard()?;
        let target = target.as_ref();
        let input = crate::schema::index_definition_to_input(&definition)?;
        let request = ir::encode_create_index_request(target, &input);
        let response = self
            .transport
            .call(request, false, None)
            .map_err(crate::Error::from_ir)?;
        let envelope = self.response(
            &response.payload,
            wire::ResponsePayload::SearchKernel_Wire_CreateIndexResult,
        )?;
        let generation = response.generation.clone().ok_or_else(|| {
            crate::Error::from_ir(invalid_remote_response(
                "create-index response is missing target generation metadata",
            ))
        })?;
        let result = envelope
            .payload_as_search_kernel_wire_create_index_result()
            .ok_or_else(|| {
                crate::Error::from_ir(invalid_remote_response("create result payload is missing"))
            })?;
        let definition = result.index().ok_or_else(|| {
            crate::Error::from_ir(invalid_remote_response(
                "create result index definition is missing",
            ))
        })?;
        self.index_from_api_definition(target, definition, generation)
    }

    /// Open an existing logical target and return an index handle bound to it.
    pub fn open_index(&self, target: impl AsRef<str>) -> Result<Index> {
        let _lifecycle = self.lifecycle_guard()?;
        let target = target.as_ref();
        let request = ir::encode_open_index_request(target);
        let response = self
            .transport
            .call(request, true, None)
            .map_err(crate::Error::from_ir)?;
        let envelope = self.response(
            &response.payload,
            wire::ResponsePayload::SearchKernel_Wire_OpenIndexResult,
        )?;
        let generation = response.generation.clone().ok_or_else(|| {
            crate::Error::from_ir(invalid_remote_response(
                "open-index response is missing target generation metadata",
            ))
        })?;
        let result = envelope
            .payload_as_search_kernel_wire_open_index_result()
            .ok_or_else(|| {
                crate::Error::from_ir(invalid_remote_response("open result payload is missing"))
            })?;
        let definition = result.index().ok_or_else(|| {
            crate::Error::from_ir(invalid_remote_response(
                "open result index definition is missing",
            ))
        })?;
        self.index_from_api_definition(target, definition, generation)
    }

    /// Delete one logical target from the daemon registry.
    pub fn delete_index(&self, target: impl AsRef<str>) -> Result<()> {
        let _lifecycle = self.lifecycle_guard()?;
        let target = target.as_ref();
        let request = ir::encode_delete_index_request(target);
        let response = self
            .transport
            .call(request, false, None)
            .map_err(crate::Error::from_ir)?;
        let envelope = self.response(
            &response.payload,
            wire::ResponsePayload::SearchKernel_Wire_DeleteIndexResult,
        )?;
        envelope
            .payload_as_search_kernel_wire_delete_index_result()
            .ok_or_else(|| {
                crate::Error::from_ir(invalid_remote_response(
                    "delete-index result payload is missing",
                ))
            })?;
        self.invalidate_target_sessions(target)?;
        Ok(())
    }

    fn lifecycle_guard(&self) -> Result<std::sync::MutexGuard<'_, ()>> {
        self.lifecycle_lock.lock().map_err(|_| {
            crate::Error::from_ir(invalid_remote_response(
                "remote client lifecycle lock is poisoned",
            ))
        })
    }

    fn register_target_session(&self, target: &str, session: &Arc<BackendSession>) -> Result<()> {
        let mut sessions = self.sessions.lock().map_err(|_| {
            crate::Error::from_ir(invalid_remote_response(
                "remote client target-session registry is poisoned",
            ))
        })?;
        let target_sessions = sessions.entry(target.to_owned()).or_default();
        target_sessions.retain(|session| session.strong_count() > 0);
        target_sessions.push(Arc::downgrade(session));
        Ok(())
    }

    fn invalidate_target_sessions(&self, target: &str) -> Result<()> {
        let sessions = self
            .sessions
            .lock()
            .map_err(|_| {
                crate::Error::from_ir(invalid_remote_response(
                    "remote client target-session registry is poisoned",
                ))
            })?
            .remove(target)
            .unwrap_or_default();

        let mut first_error = None;
        for session in sessions {
            if let Some(session) = session.upgrade() {
                if let Err(error) = session.close() {
                    if first_error.is_none() {
                        first_error = Some(error);
                    }
                }
            }
        }
        match first_error {
            Some(error) => Err(crate::Error::from_ir(error)),
            None => Ok(()),
        }
    }

    fn response<'a>(
        &self,
        payload: &'a [u8],
        expected: wire::ResponsePayload,
    ) -> Result<wire::ResponseEnvelope<'a>> {
        expected_response(payload, expected).map_err(crate::Error::from_ir)
    }

    fn index_from_api_definition(
        &self,
        target: &str,
        definition: wire::IndexDefinition<'_>,
        generation: String,
    ) -> Result<Index> {
        let definition = ir::index_definition_to_ir(definition).map_err(|error| {
            crate::Error::from_ir(invalid_remote_response(format!(
                "lifecycle result contains an invalid index definition: {}",
                error.message
            )))
        })?;
        let context = IndexContext::from_definition_ir(definition);
        let backend = RemoteBackend::new(
            Arc::clone(&self.transport),
            Some(target.to_owned()),
            Some(generation),
        );
        let session = BackendSession::new(Box::new(backend));
        self.register_target_session(target, &session)?;
        Index::from_remote_session(context, session)
    }
}

impl Index {
    /// Connect to a legacy single-index `searchkerneld` using the ergonomic Rust API.
    ///
    /// Multi-index daemons require [`RemoteClient::connect`] followed by
    /// `create_index(target, definition)` or `open_index(target)`.
    pub fn connect(endpoint: impl AsRef<str>) -> Result<Self> {
        Self::connect_with(endpoint, RemoteOptions::default())
    }

    /// Connect to a legacy single-index daemon with explicit transport options.
    pub fn connect_with(endpoint: impl AsRef<str>, options: RemoteOptions) -> Result<Self> {
        let (backend, context) = RemoteBackend::connect_legacy(endpoint.as_ref(), options)
            .map_err(crate::Error::from_ir)?;
        Self::from_remote_session(context, BackendSession::new(Box::new(backend)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancelled_status_is_not_misreported_as_deadline() {
        let safe = status_error(tonic::Status::cancelled("server cancelled request"), true);
        assert_eq!(safe.code, ErrorCode::Unavailable);
        assert!(safe.retryable);
        assert_eq!(safe.details[0].code, "grpc_transport");

        let mutation = status_error(tonic::Status::cancelled("server cancelled request"), false);
        assert_eq!(mutation.code, ErrorCode::Unavailable);
        assert!(!mutation.retryable);
        assert_eq!(mutation.details[0].code, "grpc_transport");
        assert!(mutation
            .details
            .iter()
            .any(|detail| detail.code == "remote_outcome_unknown"));
    }

    #[test]
    fn disconnected_mutation_reply_does_not_claim_retryable() {
        let (commands, mut receiver) = mpsc::unbounded_channel();
        let transport = RemoteTransport {
            commands,
            worker: Mutex::new(None),
        };

        let caller = thread::spawn(move || transport.call(vec![0], false, None));
        match receiver
            .blocking_recv()
            .expect("transport call must enqueue an execute command")
        {
            Command::Execute {
                retry_safe, reply, ..
            } => {
                assert!(!retry_safe);
                drop(reply);
            }
            Command::Shutdown => panic!("unexpected shutdown before execute"),
        }

        let error = match caller.join().expect("transport caller thread") {
            Ok(_) => panic!("dropped reply must fail the transport call"),
            Err(error) => error,
        };
        assert_eq!(error.code, ErrorCode::Unavailable);
        assert!(
            !error.retryable,
            "an interrupted mutation may have executed remotely"
        );
        assert!(error
            .details
            .iter()
            .any(|detail| detail.code == "remote_outcome_unknown"));
    }

    #[test]
    fn mismatched_response_identity_is_rejected() {
        let error = validate_response_id("get", "requested", "other")
            .expect_err("mismatched response identity must be rejected");
        assert_eq!(error.code, ErrorCode::Internal);
        assert_eq!(error.details[0].code, "invalid_remote_response");
        validate_response_id("get", "requested", "requested").expect("matching response identity");
    }

    #[test]
    fn grpc_protocol_failures_do_not_impersonate_portable_errors() {
        let malformed = status_error(
            tonic::Status::invalid_argument("malformed Search Kernel frame"),
            true,
        );
        assert_eq!(malformed.code, ErrorCode::Internal);
        assert_eq!(malformed.details[0].code, "grpc_transport");

        let mismatch = status_error(
            tonic::Status::unimplemented("unknown operation discriminator"),
            true,
        );
        assert_eq!(mismatch.code, ErrorCode::FailedPrecondition);
        assert_eq!(mismatch.details[0].code, "grpc_transport");
    }

    #[test]
    fn terminal_transport_failure_marks_mutation_outcome_unknown() {
        let mutation = status_error(tonic::Status::internal("connection failed late"), false);
        assert_eq!(mutation.code, ErrorCode::Internal);
        assert!(!mutation.retryable);
        assert!(mutation
            .details
            .iter()
            .any(|detail| detail.code == "remote_outcome_unknown"));

        let exhausted = status_error(
            tonic::Status::resource_exhausted("response too large"),
            false,
        );
        assert_eq!(exhausted.code, ErrorCode::ResourceExhausted);
        assert!(!exhausted.retryable);
        assert!(exhausted
            .details
            .iter()
            .any(|detail| detail.code == "remote_outcome_unknown"));

        let read = status_error(tonic::Status::internal("connection failed late"), true);
        assert_eq!(read.code, ErrorCode::Internal);
        assert!(!read.retryable);
        assert!(read
            .details
            .iter()
            .all(|detail| detail.code != "remote_outcome_unknown"));
    }

    #[test]
    fn invalid_remote_options_fail_before_transport_startup() {
        let error = RemoteOptions::default()
            .request_timeout(Duration::ZERO)
            .validate()
            .expect_err("zero request timeout must be rejected");
        assert_eq!(error.code, ErrorCode::InvalidArgument);

        let error = RemoteOptions::default()
            .connect_timeout(Duration::ZERO)
            .validate()
            .expect_err("zero connect timeout must be rejected");
        assert_eq!(error.code, ErrorCode::InvalidArgument);

        let error = RemoteOptions::default()
            .max_message_size(0)
            .validate()
            .expect_err("zero message size must be rejected");
        assert_eq!(error.code, ErrorCode::InvalidArgument);
    }

    #[test]
    fn endpoint_parser_rejects_implicit_tls_and_unknown_schemes() {
        let tls = RemoteAddress::parse("https://example.test:50051")
            .expect_err("TLS must not be silently downgraded");
        assert_eq!(tls.code, ErrorCode::Unsupported);

        let unknown = RemoteAddress::parse("grpc://example.test:50051")
            .expect_err("unknown endpoint schemes must be rejected");
        assert_eq!(unknown.code, ErrorCode::InvalidArgument);
    }
}
