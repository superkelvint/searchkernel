//! gRPC host for Search Kernel.
//!
//! The generated RPC accepts and returns opaque FlatBuffer frames. All
//! semantic work is delegated to the canonical transport-independent
//! dispatcher.

#[cfg(feature = "native")]
mod registry;
mod search_endpoints;

#[cfg(feature = "native")]
pub use registry::{IndexRegistry, RegistryDispatchError, RegistryInitError};

#[cfg(feature = "test-support")]
#[doc(hidden)]
pub mod search_endpoint_test_support {
    pub use crate::search_endpoints::{
        await_blocking_with_timeout, SearchEndpointAdmissionPermit, SearchEndpointBindError,
        SearchEndpointConcurrency, SearchEndpointConcurrencyError, SearchEndpointConfig,
        SearchEndpointExecutionWaitError, SearchEndpointPolicy, SearchEndpointRouteError,
        SearchEndpointRouter,
    };
}

#[cfg(unix)]
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
#[cfg(unix)]
use std::os::unix::fs::{FileTypeExt, MetadataExt};
#[cfg(unix)]
use std::path::{Path, PathBuf};
use std::{
    fmt,
    future::Future,
    net::SocketAddr,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use base64::Engine as _;
use bytes::Bytes;
use flatbuffers::root;
use http::{
    header::{ALLOW, CONTENT_TYPE},
    Method, Request as HttpRequest, Response as HttpResponse, StatusCode,
};
use http_body_util::{BodyExt, Full};
use searchkernel_dispatch::{DispatchError, Dispatcher};
use searchkernel_protocol::{
    json::{request_json_to_flatbuffer, response_flatbuffer_to_json},
    protocol::v1 as wire,
};
#[cfg(unix)]
use tokio::net::{UnixListener, UnixStream};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpListener,
    sync::{watch, Semaphore},
    task::JoinHandle,
};
#[cfg(unix)]
use tokio_stream::wrappers::UnixListenerStream;
use tokio_stream::{wrappers::TcpListenerStream, Stream};
use tonic::{body::BoxBody, Request, Response, Status};
use tonic_web::GrpcWebLayer;
use tower::{Layer, Service};

/// Shared Rust bindings generated from the public Search Kernel gRPC protocol.
pub use searchkernel_rpc::rpc;

use rpc::{
    search_kernel_server::{SearchKernel, SearchKernelServer},
    FlatBufferFrame,
};

use searchkernel_rpc::INDEX_GENERATION_METADATA_KEY;
pub use searchkernel_rpc::MAX_GRPC_MESSAGE_SIZE;

/// HTTP/JSON edge endpoint carrying standard FlatBuffers JSON envelopes.
pub const JSON_EXECUTE_PATH: &str = "/v1/execute";

/// Default maximum number of native dispatches that may execute concurrently.
pub const DEFAULT_EXECUTION_LIMIT: usize = 16;

/// Default maximum number of requests admitted across all transport listeners.
pub const DEFAULT_ADMISSION_LIMIT: usize = 32;

/// The address type used by the gRPC transport server.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ListenAddress {
    /// A TCP socket address.
    Tcp(SocketAddr),
    /// A Unix-domain socket path.
    #[cfg(unix)]
    Unix(PathBuf),
}

impl From<SocketAddr> for ListenAddress {
    fn from(address: SocketAddr) -> Self {
        Self::Tcp(address)
    }
}

/// Configuration for a gRPC transport server.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServerConfig {
    /// Address on which the gRPC listener is bound. A TCP port of zero requests
    /// an ephemeral port and is useful for tests.
    pub listen_addr: ListenAddress,
    /// Optional TCP address on which the HTTP/1 gRPC-Web listener is bound.
    ///
    /// The browser listener is separate from [`Self::listen_addr`] so local
    /// SDKs can continue to use the Unix-domain socket without changing
    /// transports. Both listeners share the same dispatcher and execution
    /// limit when enabled. The admission limit is shared as well.
    pub web_listen_addr: Option<SocketAddr>,
    /// Maximum number of blocking dispatcher calls that may execute at once.
    ///
    /// Requests above this limit wait asynchronously for admission instead of
    /// creating additional blocking-pool jobs.
    pub execution_limit: usize,
    /// Maximum number of requests admitted into the RPC service at once,
    /// including requests waiting for a native dispatch permit.
    pub admission_limit: usize,
}

impl ServerConfig {
    /// Creates a configuration for the supplied listener address.
    pub const fn new(listen_addr: SocketAddr) -> Self {
        Self {
            listen_addr: ListenAddress::Tcp(listen_addr),
            web_listen_addr: None,
            execution_limit: DEFAULT_EXECUTION_LIMIT,
            admission_limit: DEFAULT_ADMISSION_LIMIT,
        }
    }

    /// Creates a configuration for a Unix-domain socket.
    #[cfg(unix)]
    pub fn unix(path: impl Into<PathBuf>) -> Self {
        Self {
            listen_addr: ListenAddress::Unix(path.into()),
            web_listen_addr: None,
            execution_limit: DEFAULT_EXECUTION_LIMIT,
            admission_limit: DEFAULT_ADMISSION_LIMIT,
        }
    }

    /// Alias for [`Self::unix`] that makes the transport explicit at call sites.
    #[cfg(unix)]
    pub fn new_unix(path: impl Into<PathBuf>) -> Self {
        Self::unix(path)
    }

    /// Sets the maximum number of native dispatcher calls that may execute
    /// concurrently. A value of zero is rejected by [`start`].
    pub const fn with_execution_limit(mut self, execution_limit: usize) -> Self {
        self.execution_limit = execution_limit;
        self
    }

    /// Sets the maximum number of requests admitted across all listeners.
    /// A value of zero is rejected by [`start`].
    pub const fn with_admission_limit(mut self, admission_limit: usize) -> Self {
        self.admission_limit = admission_limit;
        self
    }

    /// Enables the HTTP/1 gRPC-Web listener at the supplied TCP address.
    pub const fn with_web_listen_addr(mut self, web_listen_addr: SocketAddr) -> Self {
        self.web_listen_addr = Some(web_listen_addr);
        self
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self::new(SocketAddr::from(([127, 0, 0, 1], 8080)))
    }
}

#[derive(Clone)]
enum DispatchTarget {
    Single(Arc<Dispatcher>),
    #[cfg(feature = "native")]
    Registry(Arc<IndexRegistry>),
}

/// The gRPC service for one process-local execution target or daemon registry.
#[derive(Clone)]
pub struct SearchKernelTransport {
    target: DispatchTarget,
    execution_limit: Arc<Semaphore>,
}

struct GrpcDispatchResult {
    payload: Vec<u8>,
    generation: Option<String>,
}

impl SearchKernelTransport {
    fn single(dispatcher: Arc<Dispatcher>, execution_limit: Arc<Semaphore>) -> Self {
        Self {
            target: DispatchTarget::Single(dispatcher),
            execution_limit,
        }
    }

    #[cfg(feature = "native")]
    fn registry(registry: Arc<IndexRegistry>, execution_limit: Arc<Semaphore>) -> Self {
        Self {
            target: DispatchTarget::Registry(registry),
            execution_limit,
        }
    }

    fn dispatch(&self, payload: &[u8]) -> Result<Vec<u8>, Box<Status>> {
        match &self.target {
            DispatchTarget::Single(dispatcher) => {
                reject_targeted_single_request(payload)
                    .map_err(|message| Box::new(Status::failed_precondition(message)))?;
                dispatcher
                    .dispatch(payload)
                    .map_err(|error| Box::new(dispatch_error_status(error)))
            }
            #[cfg(feature = "native")]
            DispatchTarget::Registry(registry) => registry
                .dispatch(payload)
                .map_err(|error| Box::new(registry_error_status(error))),
        }
    }

    fn dispatch_grpc(
        &self,
        payload: &[u8],
        expected_generation: Option<&str>,
    ) -> Result<GrpcDispatchResult, Box<Status>> {
        match &self.target {
            DispatchTarget::Single(dispatcher) => {
                reject_targeted_single_request(payload)
                    .map_err(|message| Box::new(Status::failed_precondition(message)))?;
                let payload = dispatcher
                    .dispatch(payload)
                    .map_err(|error| Box::new(dispatch_error_status(error)))?;
                Ok(GrpcDispatchResult {
                    payload,
                    generation: None,
                })
            }
            #[cfg(feature = "native")]
            DispatchTarget::Registry(registry) => {
                let result = registry
                    .dispatch_grpc(payload, expected_generation)
                    .map_err(|error| Box::new(registry_error_status(error)))?;
                Ok(GrpcDispatchResult {
                    payload: result.payload,
                    generation: result.generation,
                })
            }
        }
    }

    fn dispatch_json(&self, json: &[u8]) -> Result<Vec<u8>, (StatusCode, String)> {
        let request = request_json_to_flatbuffer(json).map_err(|error| {
            (
                StatusCode::BAD_REQUEST,
                format!("invalid Search Kernel FlatBuffers JSON request: {error}"),
            )
        })?;
        let response = match &self.target {
            DispatchTarget::Single(dispatcher) => {
                reject_targeted_single_request(&request)
                    .map_err(|message| (StatusCode::PRECONDITION_FAILED, message.to_owned()))?;
                dispatcher.dispatch(&request).map_err(dispatch_error_http)?
            }
            #[cfg(feature = "native")]
            DispatchTarget::Registry(registry) => {
                registry.dispatch(&request).map_err(registry_error_http)?
            }
        };
        response_flatbuffer_to_json(&response).map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("encode Search Kernel FlatBuffers JSON response: {error}"),
            )
        })
    }
}

async fn run_blocking<T, F>(execution_limit: Arc<Semaphore>, operation: F) -> Result<T, Box<Status>>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    let permit = execution_limit.acquire_owned().await.map_err(|error| {
        Box::new(Status::internal(format!(
            "execution limit is unavailable: {error}"
        )))
    })?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        operation()
    })
    .await
    .map_err(|error| {
        Box::new(Status::internal(format!(
            "blocking dispatch task failed: {error}"
        )))
    })
}

#[tonic::async_trait]
impl SearchKernel for SearchKernelTransport {
    async fn execute(
        &self,
        request: Request<FlatBufferFrame>,
    ) -> Result<Response<FlatBufferFrame>, Status> {
        let generation = request
            .metadata()
            .get(INDEX_GENERATION_METADATA_KEY)
            .map(|value| {
                value.to_str().map(str::to_owned).map_err(|_| {
                    Status::invalid_argument("invalid Search Kernel index generation metadata")
                })
            })
            .transpose()?;
        let payload = request.into_inner().payload;
        let transport = self.clone();
        let result = run_blocking(self.execution_limit.clone(), move || {
            transport.dispatch_grpc(&payload, generation.as_deref())
        })
        .await
        .map_err(|status| *status)?
        .map_err(|status| *status)?;

        let mut response = Response::new(FlatBufferFrame {
            payload: result.payload,
        });
        if let Some(generation) = result.generation {
            let value = generation
                .parse()
                .map_err(|_| Status::internal("invalid Search Kernel index generation"))?;
            response
                .metadata_mut()
                .insert(INDEX_GENERATION_METADATA_KEY, value);
        }
        Ok(response)
    }
}

fn reject_targeted_single_request(payload: &[u8]) -> Result<(), &'static str> {
    const REQUEST_IDENTIFIER: &[u8; 4] = b"SKRQ";
    if payload.len() < 8 || payload[4..8] != *REQUEST_IDENTIFIER {
        return Ok(());
    }
    let Ok(envelope) = root::<wire::RequestEnvelope<'_>>(payload) else {
        return Ok(());
    };
    if envelope.target().is_some() {
        Err("targeted Search Kernel requests require the multi-index daemon")
    } else {
        Ok(())
    }
}

fn dispatch_error_status(error: DispatchError) -> Status {
    match error {
        DispatchError::InvalidRequest => {
            Status::invalid_argument("invalid Search Kernel request FlatBuffer")
        }
        DispatchError::UnsupportedProtocol(version) => Status::failed_precondition(format!(
            "unsupported Search Kernel protocol version {version}"
        )),
        DispatchError::UnsupportedOperation(operation) => Status::unimplemented(format!(
            "unsupported Search Kernel operation discriminator {operation}"
        )),
        DispatchError::Closed => Status::failed_precondition("Search Kernel target is closed"),
    }
}

fn dispatch_error_http(error: DispatchError) -> (StatusCode, String) {
    match error {
        DispatchError::InvalidRequest => (
            StatusCode::BAD_REQUEST,
            "invalid Search Kernel request FlatBuffer".to_owned(),
        ),
        DispatchError::UnsupportedProtocol(version) => (
            StatusCode::PRECONDITION_FAILED,
            format!("unsupported Search Kernel protocol version {version}"),
        ),
        DispatchError::UnsupportedOperation(operation) => (
            StatusCode::NOT_IMPLEMENTED,
            format!("unsupported Search Kernel operation discriminator {operation}"),
        ),
        DispatchError::Closed => (
            StatusCode::CONFLICT,
            "Search Kernel target is closed".to_owned(),
        ),
    }
}

#[cfg(feature = "native")]
fn registry_error_status(error: RegistryDispatchError) -> Status {
    match error {
        RegistryDispatchError::InvalidRequest => {
            Status::invalid_argument("invalid Search Kernel request FlatBuffer")
        }
        RegistryDispatchError::UnsupportedProtocol(version) => Status::failed_precondition(
            format!("unsupported Search Kernel protocol version {version}"),
        ),
        RegistryDispatchError::UnsupportedOperation(operation) => Status::unimplemented(format!(
            "unsupported Search Kernel operation discriminator {operation}"
        )),
        RegistryDispatchError::Closed => {
            Status::failed_precondition("Search Kernel target is closed")
        }
        RegistryDispatchError::Poisoned => {
            Status::internal("Search Kernel index registry lock is poisoned")
        }
    }
}

#[cfg(feature = "native")]
fn registry_error_http(error: RegistryDispatchError) -> (StatusCode, String) {
    match error {
        RegistryDispatchError::InvalidRequest => (
            StatusCode::BAD_REQUEST,
            "invalid Search Kernel request FlatBuffer".to_owned(),
        ),
        RegistryDispatchError::UnsupportedProtocol(version) => (
            StatusCode::PRECONDITION_FAILED,
            format!("unsupported Search Kernel protocol version {version}"),
        ),
        RegistryDispatchError::UnsupportedOperation(operation) => (
            StatusCode::NOT_IMPLEMENTED,
            format!("unsupported Search Kernel operation discriminator {operation}"),
        ),
        RegistryDispatchError::Closed => (
            StatusCode::CONFLICT,
            "Search Kernel target is closed".to_owned(),
        ),
        RegistryDispatchError::Poisoned => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Search Kernel index registry lock is poisoned".to_owned(),
        ),
    }
}

fn http_body_response(
    status: StatusCode,
    content_type: &'static str,
    body: impl Into<Bytes>,
) -> HttpResponse<BoxBody> {
    HttpResponse::builder()
        .status(status)
        .header(CONTENT_TYPE, content_type)
        .body(tonic::body::boxed(Full::new(body.into())))
        .expect("static HTTP response must be valid")
}

async fn json_execute_response(
    request: HttpRequest<BoxBody>,
    transport: SearchKernelTransport,
) -> HttpResponse<BoxBody> {
    if request.method() != Method::POST {
        return HttpResponse::builder()
            .status(StatusCode::METHOD_NOT_ALLOWED)
            .header(ALLOW, "POST")
            .header(CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(tonic::body::boxed(Full::new(Bytes::from_static(
                b"POST required",
            ))))
            .expect("static method response must be valid");
    }

    let is_json = request
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("application/json"));
    if !is_json {
        return http_body_response(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "text/plain; charset=utf-8",
            Bytes::from_static(b"content-type must be application/json"),
        );
    }

    let mut body = request.into_body();
    let mut json = Vec::new();
    while let Some(frame) = body.frame().await {
        let frame = match frame {
            Ok(frame) => frame,
            Err(error) => {
                return http_body_response(
                    StatusCode::BAD_REQUEST,
                    "text/plain; charset=utf-8",
                    format!("read HTTP request body: {error}"),
                )
            }
        };
        if let Some(data) = frame.data_ref() {
            let Some(next_len) = json.len().checked_add(data.len()) else {
                return http_body_response(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "text/plain; charset=utf-8",
                    Bytes::from_static(b"Search Kernel JSON request is too large"),
                );
            };
            if next_len > MAX_GRPC_MESSAGE_SIZE {
                return http_body_response(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "text/plain; charset=utf-8",
                    Bytes::from_static(b"Search Kernel JSON request is too large"),
                );
            }
            json.extend_from_slice(data);
        }
    }

    let execution_limit = transport.execution_limit.clone();
    let result = run_blocking(execution_limit, move || transport.dispatch_json(&json)).await;
    match result {
        Err(status) => http_body_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "text/plain; charset=utf-8",
            status.message().to_owned(),
        ),
        Ok(Err((status, message))) => {
            http_body_response(status, "text/plain; charset=utf-8", message)
        }
        Ok(Ok(response)) if response.len() > MAX_GRPC_MESSAGE_SIZE => http_body_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "text/plain; charset=utf-8",
            Bytes::from_static(b"Search Kernel JSON response exceeds transport limit"),
        ),
        Ok(Ok(response)) => {
            http_body_response(StatusCode::OK, "application/json", Bytes::from(response))
        }
    }
}

#[derive(Clone)]
struct AdmissionLayer {
    semaphore: Arc<Semaphore>,
    json_transport: Option<SearchKernelTransport>,
}

impl AdmissionLayer {
    fn new(semaphore: Arc<Semaphore>, json_transport: Option<SearchKernelTransport>) -> Self {
        Self {
            semaphore,
            json_transport,
        }
    }
}

impl<S> Layer<S> for AdmissionLayer {
    type Service = AdmissionService<S>;

    fn layer(&self, service: S) -> Self::Service {
        AdmissionService {
            inner: service,
            semaphore: self.semaphore.clone(),
            json_transport: self.json_transport.clone(),
        }
    }
}

#[derive(Clone)]
struct AdmissionService<S> {
    inner: S,
    semaphore: Arc<Semaphore>,
    json_transport: Option<SearchKernelTransport>,
}

impl<S> Service<HttpRequest<BoxBody>> for AdmissionService<S>
where
    S: Service<HttpRequest<BoxBody>, Response = HttpResponse<BoxBody>> + Send + 'static,
    S::Future: Send + 'static,
    S::Error: Send + 'static,
{
    type Response = HttpResponse<BoxBody>;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, context: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(context)
    }

    fn call(&mut self, request: HttpRequest<BoxBody>) -> Self::Future {
        let Some(permit) = self.semaphore.clone().try_acquire_owned().ok() else {
            let response = admission_response(&request);
            return Box::pin(async move { Ok(response) });
        };

        if request.uri().path() == JSON_EXECUTE_PATH {
            if let Some(transport) = self.json_transport.clone() {
                return Box::pin(async move {
                    let response = json_execute_response(request, transport).await;
                    drop(permit);
                    Ok(response)
                });
            }
        }

        let future = self.inner.call(request);
        Box::pin(async move {
            let result = future.await;
            drop(permit);
            result
        })
    }
}

fn admission_response(request: &HttpRequest<BoxBody>) -> HttpResponse<BoxBody> {
    if request.uri().path() == JSON_EXECUTE_PATH {
        return http_body_response(
            StatusCode::TOO_MANY_REQUESTS,
            "text/plain; charset=utf-8",
            Bytes::from_static(b"Search Kernel request admission limit reached"),
        );
    }

    let status = Status::resource_exhausted("Search Kernel request admission limit reached");
    if request
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.starts_with("application/grpc-web"))
    {
        let text_response = request
            .headers()
            .get("accept")
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| {
                matches!(
                    value,
                    "application/grpc-web-text" | "application/grpc-web-text+proto"
                )
            });
        let mut headers = http::HeaderMap::new();
        let _ = status.add_header(&mut headers);
        let mut trailers = Vec::new();
        for (name, value) in &headers {
            trailers.extend_from_slice(name.as_str().as_bytes());
            trailers.extend_from_slice(b": ");
            trailers.extend_from_slice(value.as_bytes());
            trailers.extend_from_slice(b"\r\n");
        }
        let length = u32::try_from(trailers.len()).unwrap_or(u32::MAX);
        let mut frame = Vec::with_capacity(5 + trailers.len());
        frame.push(0x80);
        frame.extend_from_slice(&length.to_be_bytes());
        frame.extend_from_slice(&trailers);
        let frame = if text_response {
            base64::engine::general_purpose::STANDARD
                .encode(frame)
                .into_bytes()
        } else {
            frame
        };
        let content_type = if text_response {
            "application/grpc-web-text+proto"
        } else {
            "application/grpc-web+proto"
        };
        return HttpResponse::builder()
            .status(StatusCode::OK)
            .header(CONTENT_TYPE, content_type)
            .body(tonic::body::boxed(Full::new(Bytes::from(frame))))
            .expect("static gRPC-Web admission response must be valid");
    }

    let mut response = HttpResponse::new(tonic::body::empty_body());
    let _ = status.add_header(response.headers_mut());
    response
}

/// Errors reported while starting, serving, or stopping the transport host.
#[derive(Debug)]
pub enum ServerError {
    /// The configured listener could not be bound.
    Bind(std::io::Error),
    /// The tonic serving task returned a transport error.
    Serve(tonic::transport::Error),
    /// The serving task could not be joined.
    Join(tokio::task::JoinError),
    /// The Unix-domain socket could not be removed after serving stopped.
    Cleanup(std::io::Error),
    /// The serving task exited without a shutdown request.
    UnexpectedExit,
    /// The process signal handler could not be installed or awaited.
    Signal(std::io::Error),
    /// The configured listen address was invalid.
    InvalidListenAddress(std::net::AddrParseError),
    /// The configured Unix-domain socket path was empty.
    InvalidUnixSocketPath,
    /// Reading the daemon environment failed.
    Environment(std::env::VarError),
    /// The configured execution limit must admit at least one request.
    InvalidExecutionLimit,
    /// The configured admission limit must admit at least one request.
    InvalidAdmissionLimit,
}

impl fmt::Display for ServerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bind(error) => write!(formatter, "bind gRPC listener: {error}"),
            Self::Serve(error) => write!(formatter, "serve gRPC listener: {error}"),
            Self::Join(error) => write!(formatter, "join gRPC serving task: {error}"),
            Self::Cleanup(error) => write!(formatter, "remove gRPC Unix socket: {error}"),
            Self::UnexpectedExit => write!(formatter, "gRPC serving task exited unexpectedly"),
            Self::Signal(error) => write!(formatter, "wait for shutdown signal: {error}"),
            Self::InvalidListenAddress(error) => {
                write!(formatter, "parse gRPC listen address: {error}")
            }
            Self::InvalidUnixSocketPath => write!(formatter, "gRPC Unix socket path is empty"),
            Self::Environment(error) => write!(formatter, "read daemon environment: {error}"),
            Self::InvalidExecutionLimit => {
                write!(formatter, "execution limit must be greater than zero")
            }
            Self::InvalidAdmissionLimit => {
                write!(formatter, "admission limit must be greater than zero")
            }
        }
    }
}

impl std::error::Error for ServerError {}

/// A running gRPC server and its controlled shutdown handle.
pub struct RunningServer {
    local_addr: ListenAddress,
    web_local_addr: Option<SocketAddr>,
    shutdown: Option<watch::Sender<bool>>,
    task: JoinHandle<Result<(), tonic::transport::Error>>,
    web_task: Option<JoinHandle<Result<(), tonic::transport::Error>>>,
    #[cfg(unix)]
    unix_socket_ownership: Option<UnixSocketOwnership>,
}

#[cfg(unix)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct UnixSocketIdentity {
    device: u64,
    inode: u64,
}

#[cfg(unix)]
struct UnixSocketOwnership {
    identity: UnixSocketIdentity,
    lock: UnixSocketLock,
    // Keep the original socket inode alive until pathname cleanup. Otherwise
    // a replacement socket can immediately reuse the same (device, inode)
    // pair after the serving task drops its listener.
    _listener_fd: OwnedFd,
}

#[cfg(unix)]
impl UnixSocketIdentity {
    fn from_path(path: &Path) -> std::io::Result<Option<Self>> {
        let metadata = match std::fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        if !metadata.file_type().is_socket() {
            return Ok(None);
        }
        Ok(Some(Self {
            device: metadata.dev(),
            inode: metadata.ino(),
        }))
    }
}

#[cfg(unix)]
struct UnixSocketLock {
    _file: std::fs::File,
}

#[cfg(unix)]
impl UnixSocketLock {
    fn try_acquire(path: &Path) -> std::io::Result<Self> {
        let mut lock_path = path.as_os_str().to_os_string();
        lock_path.push(".lock");
        let file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(PathBuf::from(lock_path))?;
        loop {
            // All Search Kernel daemon instances serialize probe/remove/bind
            // and ownership-checked cleanup through this sidecar lock. The
            // lock file intentionally remains after shutdown: removing it
            // while another process holds it could let a later process lock a
            // new inode and reintroduce the pathname race.
            if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
                return Ok(Self { _file: file });
            }
            let error = std::io::Error::last_os_error();
            if error.kind() != std::io::ErrorKind::Interrupted {
                return Err(error);
            }
        }
    }
}

#[cfg(unix)]
fn duplicate_unix_listener(listener: &UnixListener) -> std::io::Result<OwnedFd> {
    let duplicated = unsafe { libc::dup(listener.as_raw_fd()) };
    if duplicated == -1 {
        Err(std::io::Error::last_os_error())
    } else {
        // SAFETY: dup returned a new, owned file descriptor on success.
        Ok(unsafe { OwnedFd::from_raw_fd(duplicated) })
    }
}

impl fmt::Debug for RunningServer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let task_finished =
            self.task.is_finished() || self.web_task.as_ref().is_some_and(JoinHandle::is_finished);
        formatter
            .debug_struct("RunningServer")
            .field("local_addr", &self.local_addr)
            .field("web_local_addr", &self.web_local_addr)
            .field("shutdown_requested", &self.shutdown.is_none())
            .field("task_finished", &task_finished)
            .finish()
    }
}

impl RunningServer {
    /// Returns the address selected by the operating system or configured for
    /// the Unix-domain socket.
    pub fn local_addr(&self) -> &ListenAddress {
        &self.local_addr
    }

    /// Returns the bound TCP address, if this server uses TCP.
    pub fn tcp_addr(&self) -> Option<SocketAddr> {
        match self.local_addr {
            ListenAddress::Tcp(address) => Some(address),
            #[cfg(unix)]
            ListenAddress::Unix(_) => None,
        }
    }

    /// Returns the bound browser TCP address, if the gRPC-Web listener is
    /// enabled.
    pub fn web_tcp_addr(&self) -> Option<SocketAddr> {
        self.web_local_addr
    }

    /// Returns the configured Unix-domain socket path, if this server uses UDS.
    #[cfg(unix)]
    pub fn unix_path(&self) -> Option<&Path> {
        match &self.local_addr {
            ListenAddress::Tcp(_) => None,
            ListenAddress::Unix(path) => Some(path),
        }
    }

    /// Waits for the serving task to finish without requesting shutdown.
    ///
    /// This is useful to let a daemon react immediately if its serving task
    /// exits before an external shutdown signal arrives.
    pub async fn wait(&mut self) -> Result<(), ServerError> {
        let result = if let Some(web_task) = self.web_task.as_mut() {
            tokio::select! {
                result = &mut self.task => result,
                result = web_task => result,
            }
        } else {
            (&mut self.task).await
        }
        .map_err(ServerError::Join)?
        .map_err(ServerError::Serve);
        self.abort_remaining_tasks();
        self.cleanup_socket()?;
        result
    }

    /// Requests graceful shutdown and waits for the serving task to finish.
    pub async fn shutdown(mut self) -> Result<(), ServerError> {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(true);
        }

        let result = (&mut self.task)
            .await
            .map_err(ServerError::Join)?
            .map_err(ServerError::Serve);
        let web_result = if let Some(web_task) = &mut self.web_task {
            Some(
                web_task
                    .await
                    .map_err(ServerError::Join)?
                    .map_err(ServerError::Serve),
            )
        } else {
            None
        };
        self.cleanup_socket()?;
        result.and(web_result.unwrap_or(Ok(())))
    }

    fn abort_remaining_tasks(&mut self) {
        if !self.task.is_finished() {
            self.task.abort();
        }
        if let Some(web_task) = self.web_task.as_mut() {
            if !web_task.is_finished() {
                web_task.abort();
            }
        }
    }

    fn cleanup_socket(&mut self) -> Result<(), ServerError> {
        #[cfg(unix)]
        if let ListenAddress::Unix(path) = &self.local_addr {
            let Some(ownership) = self.unix_socket_ownership.take() else {
                return Ok(());
            };
            let UnixSocketOwnership {
                identity,
                lock: _lock,
                _listener_fd,
            } = ownership;
            if UnixSocketIdentity::from_path(path)
                .map_err(ServerError::Cleanup)?
                .is_some_and(|current| current == identity)
            {
                match std::fs::remove_file(path) {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => return Err(ServerError::Cleanup(error)),
                }
            }
        }
        Ok(())
    }
}

impl Drop for RunningServer {
    fn drop(&mut self) {
        self.abort_remaining_tasks();
        let _ = self.cleanup_socket();
    }
}

/// Binds and starts a server backed by one canonical per-index dispatcher.
pub async fn start(
    config: ServerConfig,
    dispatcher: Arc<Dispatcher>,
) -> Result<RunningServer, ServerError> {
    let execution_limit = (config.execution_limit > 0)
        .then(|| Arc::new(Semaphore::new(config.execution_limit)))
        .ok_or(ServerError::InvalidExecutionLimit)?;
    let admission_limit = (config.admission_limit > 0)
        .then(|| Arc::new(Semaphore::new(config.admission_limit)))
        .ok_or(ServerError::InvalidAdmissionLimit)?;
    start_with_service(
        config,
        SearchKernelTransport::single(dispatcher, execution_limit),
        admission_limit,
    )
    .await
}

/// Binds and starts a daemon server backed by the shared multi-index registry.
#[cfg(feature = "native")]
pub async fn start_multi(
    config: ServerConfig,
    registry: Arc<IndexRegistry>,
) -> Result<RunningServer, ServerError> {
    let execution_limit = (config.execution_limit > 0)
        .then(|| Arc::new(Semaphore::new(config.execution_limit)))
        .ok_or(ServerError::InvalidExecutionLimit)?;
    let admission_limit = (config.admission_limit > 0)
        .then(|| Arc::new(Semaphore::new(config.admission_limit)))
        .ok_or(ServerError::InvalidAdmissionLimit)?;
    start_with_service(
        config,
        SearchKernelTransport::registry(registry, execution_limit),
        admission_limit,
    )
    .await
}

async fn start_with_service(
    config: ServerConfig,
    service: SearchKernelTransport,
    admission_limit: Arc<Semaphore>,
) -> Result<RunningServer, ServerError> {
    let (shutdown, _) = watch::channel(false);

    // Bind every requested listener before starting any serving task. If a
    // browser port is unavailable, this prevents the already-bound local
    // listener from being left running after `start` returns an error.
    let web_listener = match config.web_listen_addr {
        Some(web_listen_addr) => {
            let listener = TcpListener::bind(web_listen_addr)
                .await
                .map_err(ServerError::Bind)?;
            Some((listener.local_addr().map_err(ServerError::Bind)?, listener))
        }
        None => None,
    };

    let (local_addr, task, _unix_socket_ownership) = match config.listen_addr {
        ListenAddress::Tcp(listen_addr) => {
            let listener = TcpListener::bind(listen_addr)
                .await
                .map_err(ServerError::Bind)?;
            let local_addr = listener.local_addr().map_err(ServerError::Bind)?;
            let incoming = TcpListenerStream::new(listener);
            let task = serve(
                incoming,
                service.clone(),
                admission_limit.clone(),
                shutdown.subscribe(),
                false,
            );
            (ListenAddress::Tcp(local_addr), task, None)
        }
        #[cfg(unix)]
        ListenAddress::Unix(path) => {
            let (listener, ownership) = bind_unix_listener(&path).await?;
            let incoming = UnixListenerStream::new(listener);
            let task = serve(
                incoming,
                service.clone(),
                admission_limit.clone(),
                shutdown.subscribe(),
                false,
            );
            (ListenAddress::Unix(path), task, Some(ownership))
        }
    };

    let (web_local_addr, web_task) = match web_listener {
        Some((web_local_addr, listener)) => {
            let incoming = TcpListenerStream::new(listener);
            let web_task = serve(
                incoming,
                service,
                admission_limit,
                shutdown.subscribe(),
                true,
            );
            (Some(web_local_addr), Some(web_task))
        }
        None => (None, None),
    };

    Ok(RunningServer {
        local_addr,
        web_local_addr,
        shutdown: Some(shutdown),
        task,
        web_task,
        #[cfg(unix)]
        unix_socket_ownership: _unix_socket_ownership,
    })
}

fn serve<I, IO>(
    incoming: I,
    service: SearchKernelTransport,
    admission_limit: Arc<Semaphore>,
    mut shutdown_requested: watch::Receiver<bool>,
    grpc_web: bool,
) -> JoinHandle<Result<(), tonic::transport::Error>>
where
    I: Stream<Item = Result<IO, std::io::Error>> + Send + 'static,
    IO: AsyncRead + AsyncWrite + tonic::transport::server::Connected + Unpin + Send + 'static,
    IO::ConnectInfo: Clone + Send + Sync + 'static,
{
    tokio::spawn(async move {
        let shutdown = async move {
            let _ = shutdown_requested.wait_for(|shutdown| *shutdown).await;
        };
        if grpc_web {
            tonic::transport::Server::builder()
                .accept_http1(true)
                .layer(AdmissionLayer::new(
                    admission_limit.clone(),
                    Some(service.clone()),
                ))
                .layer(GrpcWebLayer::new())
                .add_service(
                    SearchKernelServer::new(service)
                        .max_decoding_message_size(MAX_GRPC_MESSAGE_SIZE)
                        .max_encoding_message_size(MAX_GRPC_MESSAGE_SIZE),
                )
                .serve_with_incoming_shutdown(incoming, shutdown)
                .await
        } else {
            tonic::transport::Server::builder()
                .layer(AdmissionLayer::new(admission_limit, None))
                .add_service(
                    SearchKernelServer::new(service)
                        .max_decoding_message_size(MAX_GRPC_MESSAGE_SIZE)
                        .max_encoding_message_size(MAX_GRPC_MESSAGE_SIZE),
                )
                .serve_with_incoming_shutdown(incoming, shutdown)
                .await
        }
    })
}

#[cfg(unix)]
async fn bind_unix_listener(
    path: &Path,
) -> Result<(UnixListener, UnixSocketOwnership), ServerError> {
    let lock = UnixSocketLock::try_acquire(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::WouldBlock {
            ServerError::Bind(std::io::Error::new(
                std::io::ErrorKind::AddrInUse,
                format!("Unix socket {path:?} is already in use by a live server"),
            ))
        } else {
            ServerError::Bind(error)
        }
    })?;
    // Probe an existing socket before removing it. Non-socket paths are left
    // untouched so a bind error cannot destroy an unrelated file.
    let stale_socket = match UnixSocketIdentity::from_path(path).map_err(ServerError::Bind)? {
        Some(identity) => match UnixStream::connect(path).await {
            Ok(_) => {
                return Err(ServerError::Bind(std::io::Error::new(
                    std::io::ErrorKind::AddrInUse,
                    format!("Unix socket {path:?} is already in use by a live server"),
                )))
            }
            Err(error) if error.kind() == std::io::ErrorKind::ConnectionRefused => {
                UnixSocketIdentity::from_path(path)
                    .map_err(ServerError::Bind)?
                    .is_some_and(|current| current == identity)
            }
            Err(error) => return Err(ServerError::Bind(error)),
        },
        None => false,
    };

    if stale_socket {
        match std::fs::remove_file(path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(ServerError::Bind(error)),
        }
    }

    let listener = UnixListener::bind(path).map_err(ServerError::Bind)?;
    let identity = UnixSocketIdentity::from_path(path)
        .map_err(ServerError::Bind)?
        .ok_or_else(|| {
            ServerError::Bind(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Unix socket {path:?} disappeared after binding"),
            ))
        })?;
    let listener_fd = duplicate_unix_listener(&listener).map_err(ServerError::Bind)?;
    Ok((
        listener,
        UnixSocketOwnership {
            identity,
            lock,
            _listener_fd: listener_fd,
        },
    ))
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use std::path::PathBuf;
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };

    #[cfg(unix)]
    use tokio::net::UnixListener;
    use tokio::sync::Semaphore;

    use super::{
        duplicate_unix_listener, run_blocking, ListenAddress, RunningServer, UnixSocketIdentity,
        UnixSocketLock, UnixSocketOwnership,
    };

    #[cfg(feature = "native")]
    #[test]
    fn native_storage_migration_api_is_available_to_daemon() {
        fn migrate(
            source: &std::path::Path,
            destination: &std::path::Path,
        ) -> searchkernel::Result<()> {
            searchkernel::Index::migrate_storage(source, destination)
        }

        let _callable: fn(&std::path::Path, &std::path::Path) -> searchkernel::Result<()> = migrate;
    }

    #[tokio::test]
    async fn blocking_work_waits_for_an_execution_permit() {
        let execution_limit = Arc::new(Semaphore::new(1));
        let held_permit = execution_limit
            .clone()
            .acquire_owned()
            .await
            .expect("test execution permit");
        let started = Arc::new(AtomicBool::new(false));
        let task_started = started.clone();

        let task = tokio::spawn(run_blocking(execution_limit, move || {
            task_started.store(true, Ordering::Release);
            42
        }));

        tokio::task::yield_now().await;
        assert!(!started.load(Ordering::Acquire));

        drop(held_permit);
        assert_eq!(
            task.await.expect("blocking task join").expect("dispatch"),
            42
        );
        assert!(started.load(Ordering::Acquire));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn drop_does_not_remove_a_replacement_unix_socket() {
        let path = std::env::temp_dir().join(format!(
            "searchkerneld-cleanup-race-{}.sock",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);

        let original_listener = UnixListener::bind(&path).expect("bind original UDS");
        let original_identity = UnixSocketIdentity::from_path(&path)
            .expect("read original UDS identity")
            .expect("original UDS identity");
        let original_fd = duplicate_unix_listener(&original_listener).expect("duplicate UDS");
        let original_lock = UnixSocketLock::try_acquire(&path).expect("lock original UDS");
        drop(original_listener);
        std::fs::remove_file(&path).expect("remove stale original UDS");

        let task = tokio::spawn(async { Ok::<(), tonic::transport::Error>(()) });
        let server = RunningServer {
            local_addr: ListenAddress::Unix(PathBuf::from(&path)),
            web_local_addr: None,
            shutdown: None,
            task,
            web_task: None,
            unix_socket_ownership: Some(UnixSocketOwnership {
                identity: original_identity,
                lock: original_lock,
                _listener_fd: original_fd,
            }),
        };

        let replacement_listener = UnixListener::bind(&path).expect("bind replacement UDS");
        drop(server);
        assert!(path.exists(), "cleanup removed the replacement socket");

        drop(replacement_listener);
        let _ = std::fs::remove_file(path);
    }
}
