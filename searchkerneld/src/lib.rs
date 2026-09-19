//! gRPC host for Search Kernel.
//!
//! The generated RPC accepts and returns opaque FlatBuffer frames. All
//! semantic work is delegated to the canonical transport-independent
//! dispatcher.

#[cfg(unix)]
use std::os::unix::fs::FileTypeExt;
#[cfg(unix)]
use std::path::{Path, PathBuf};
use std::{fmt, net::SocketAddr, sync::Arc};

use searchkernel_dispatch::{DispatchError, Dispatcher};
#[cfg(unix)]
use tokio::net::{UnixListener, UnixStream};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpListener,
    sync::{oneshot, Semaphore},
    task::JoinHandle,
};
#[cfg(unix)]
use tokio_stream::wrappers::UnixListenerStream;
use tokio_stream::{wrappers::TcpListenerStream, Stream};
use tonic::{Request, Response, Status};

/// Rust bindings generated from the public Search Kernel gRPC protocol.
#[allow(clippy::result_large_err)]
pub mod rpc {
    tonic::include_proto!("searchkernel.rpc.v1");
}

use rpc::{
    search_kernel_server::{SearchKernel, SearchKernelServer},
    FlatBufferFrame,
};

/// Explicit transport bound for canonical FlatBuffer request and response
/// frames. Tonic's default decode limit is 4 MiB, which is too small for
/// legitimate documents and tensor payloads.
pub const MAX_GRPC_MESSAGE_SIZE: usize = 256 * 1024 * 1024;

/// Default maximum number of native dispatches that may execute concurrently.
pub const DEFAULT_EXECUTION_LIMIT: usize = 16;

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
    /// Maximum number of blocking dispatcher calls that may execute at once.
    ///
    /// Requests above this limit wait asynchronously for admission instead of
    /// creating additional blocking-pool jobs.
    pub execution_limit: usize,
}

impl ServerConfig {
    /// Creates a configuration for the supplied listener address.
    pub const fn new(listen_addr: SocketAddr) -> Self {
        Self {
            listen_addr: ListenAddress::Tcp(listen_addr),
            execution_limit: DEFAULT_EXECUTION_LIMIT,
        }
    }

    /// Creates a configuration for a Unix-domain socket.
    #[cfg(unix)]
    pub fn unix(path: impl Into<PathBuf>) -> Self {
        Self {
            listen_addr: ListenAddress::Unix(path.into()),
            execution_limit: DEFAULT_EXECUTION_LIMIT,
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
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self::new(SocketAddr::from(([127, 0, 0, 1], 8080)))
    }
}

/// The gRPC service for one process-local execution target.
#[derive(Clone)]
pub struct SearchKernelTransport {
    dispatcher: Arc<Dispatcher>,
    execution_limit: Arc<Semaphore>,
}

impl SearchKernelTransport {
    /// Creates a service with the supplied canonical dispatcher.
    fn new(dispatcher: Arc<Dispatcher>, execution_limit: Arc<Semaphore>) -> Self {
        Self {
            dispatcher,
            execution_limit,
        }
    }

    fn dispatch(&self, payload: &[u8]) -> Result<Vec<u8>, Box<Status>> {
        self.dispatcher
            .dispatch(payload)
            .map_err(|error| Box::new(dispatch_error_status(error)))
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
        let payload = request.into_inner().payload;
        let transport = self.clone();
        let payload = run_blocking(self.execution_limit.clone(), move || {
            transport.dispatch(&payload)
        })
        .await
        .map_err(|status| *status)?
        .map_err(|status| *status)?;
        Ok(Response::new(FlatBufferFrame { payload }))
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
        }
    }
}

impl std::error::Error for ServerError {}

/// A running gRPC server and its controlled shutdown handle.
pub struct RunningServer {
    local_addr: ListenAddress,
    shutdown: Option<oneshot::Sender<()>>,
    task: JoinHandle<Result<(), tonic::transport::Error>>,
}

impl fmt::Debug for RunningServer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RunningServer")
            .field("local_addr", &self.local_addr)
            .field("shutdown_requested", &self.shutdown.is_none())
            .field("task_finished", &self.task.is_finished())
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
        let result = (&mut self.task)
            .await
            .map_err(ServerError::Join)?
            .map_err(ServerError::Serve);
        self.cleanup_socket()?;
        result
    }

    /// Requests graceful shutdown and waits for the serving task to finish.
    pub async fn shutdown(mut self) -> Result<(), ServerError> {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }

        let result = (&mut self.task)
            .await
            .map_err(ServerError::Join)?
            .map_err(ServerError::Serve);
        self.cleanup_socket()?;
        result
    }

    fn cleanup_socket(&self) -> Result<(), ServerError> {
        #[cfg(unix)]
        if let ListenAddress::Unix(path) = &self.local_addr {
            match std::fs::remove_file(path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(ServerError::Cleanup(error)),
            }
        }
        Ok(())
    }
}

impl Drop for RunningServer {
    fn drop(&mut self) {
        self.task.abort();
        #[cfg(unix)]
        if let ListenAddress::Unix(path) = &self.local_addr {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// Binds and starts a gRPC server backed by one canonical dispatcher.
pub async fn start(
    config: ServerConfig,
    dispatcher: Arc<Dispatcher>,
) -> Result<RunningServer, ServerError> {
    let execution_limit = (config.execution_limit > 0)
        .then(|| Arc::new(Semaphore::new(config.execution_limit)))
        .ok_or(ServerError::InvalidExecutionLimit)?;
    start_with_service(
        config,
        SearchKernelTransport::new(dispatcher, execution_limit),
    )
    .await
}

async fn start_with_service(
    config: ServerConfig,
    service: SearchKernelTransport,
) -> Result<RunningServer, ServerError> {
    let (shutdown, shutdown_requested) = oneshot::channel();

    let (local_addr, task) = match config.listen_addr {
        ListenAddress::Tcp(listen_addr) => {
            let listener = TcpListener::bind(listen_addr)
                .await
                .map_err(ServerError::Bind)?;
            let local_addr = listener.local_addr().map_err(ServerError::Bind)?;
            let incoming = TcpListenerStream::new(listener);
            let task = serve(incoming, service, shutdown_requested);
            (ListenAddress::Tcp(local_addr), task)
        }
        #[cfg(unix)]
        ListenAddress::Unix(path) => {
            let listener = bind_unix_listener(&path).await?;
            let incoming = UnixListenerStream::new(listener);
            let task = serve(incoming, service, shutdown_requested);
            (ListenAddress::Unix(path), task)
        }
    };

    Ok(RunningServer {
        local_addr,
        shutdown: Some(shutdown),
        task,
    })
}

fn serve<I, IO>(
    incoming: I,
    service: SearchKernelTransport,
    shutdown_requested: oneshot::Receiver<()>,
) -> JoinHandle<Result<(), tonic::transport::Error>>
where
    I: Stream<Item = Result<IO, std::io::Error>> + Send + 'static,
    IO: AsyncRead + AsyncWrite + tonic::transport::server::Connected + Unpin + Send + 'static,
    IO::ConnectInfo: Clone + Send + Sync + 'static,
{
    tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(
                SearchKernelServer::new(service)
                    .max_decoding_message_size(MAX_GRPC_MESSAGE_SIZE)
                    .max_encoding_message_size(MAX_GRPC_MESSAGE_SIZE),
            )
            .serve_with_incoming_shutdown(incoming, async move {
                let _ = shutdown_requested.await;
            })
            .await
    })
}

#[cfg(unix)]
async fn bind_unix_listener(path: &Path) -> Result<UnixListener, ServerError> {
    // Probe an existing socket before removing it. Non-socket paths are left
    // untouched so a bind error cannot destroy an unrelated file.
    let stale_socket = match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_socket() => match UnixStream::connect(path).await {
            Ok(_) => {
                return Err(ServerError::Bind(std::io::Error::new(
                    std::io::ErrorKind::AddrInUse,
                    format!("Unix socket {path:?} is already in use by a live server"),
                )))
            }
            Err(_) => true,
        },
        Ok(_) | Err(_) => false,
    };

    if stale_socket {
        match std::fs::remove_file(path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(ServerError::Bind(error)),
        }
    }

    UnixListener::bind(path).map_err(ServerError::Bind)
}

#[cfg(test)]
mod tests {
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };

    use tokio::sync::Semaphore;

    use super::{run_blocking, DEFAULT_EXECUTION_LIMIT};

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

    #[test]
    fn default_execution_limit_is_bounded() {
        assert!(DEFAULT_EXECUTION_LIMIT > 0);
    }
}
