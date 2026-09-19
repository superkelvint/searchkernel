//! gRPC host for Search Kernel.
//!
//! The generated RPC accepts and returns opaque FlatBuffer frames. All
//! semantic work is delegated to the canonical transport-independent
//! dispatcher.

use std::{fmt, net::SocketAddr, sync::Arc};

use searchkernel_dispatch::{DispatchError, Dispatcher};
use tokio::{net::TcpListener, sync::oneshot, task::JoinHandle};
use tokio_stream::wrappers::TcpListenerStream;
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

/// Configuration for a gRPC transport server.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServerConfig {
    /// Address on which the gRPC listener is bound. Port zero requests an
    /// ephemeral port and is useful for tests.
    pub listen_addr: SocketAddr,
}

impl ServerConfig {
    /// Creates a configuration for the supplied listener address.
    pub const fn new(listen_addr: SocketAddr) -> Self {
        Self { listen_addr }
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self::new(SocketAddr::from(([127, 0, 0, 1], 8080)))
    }
}

/// The gRPC service for one optional process-local execution target.
#[derive(Clone, Default)]
pub struct SearchKernelTransport {
    dispatcher: Option<Arc<Dispatcher>>,
}

impl SearchKernelTransport {
    /// Creates a service with the supplied canonical dispatcher.
    pub fn with_dispatcher(dispatcher: Arc<Dispatcher>) -> Self {
        Self {
            dispatcher: Some(dispatcher),
        }
    }

    fn dispatch(&self, payload: &[u8]) -> Result<Vec<u8>, Box<Status>> {
        let dispatcher = self.dispatcher.as_ref().ok_or_else(|| {
            Box::new(Status::failed_precondition(
                "no Search Kernel target is configured",
            ))
        })?;
        dispatcher
            .dispatch(payload)
            .map_err(|error| Box::new(dispatch_error_status(error)))
    }
}

#[tonic::async_trait]
impl SearchKernel for SearchKernelTransport {
    async fn execute(
        &self,
        request: Request<FlatBufferFrame>,
    ) -> Result<Response<FlatBufferFrame>, Status> {
        let payload = self
            .dispatch(&request.into_inner().payload)
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
    /// The serving task exited without a shutdown request.
    UnexpectedExit,
    /// The process signal handler could not be installed or awaited.
    Signal(std::io::Error),
    /// The configured listen address was invalid.
    InvalidListenAddress(std::net::AddrParseError),
    /// Reading the daemon environment failed.
    Environment(std::env::VarError),
}

impl fmt::Display for ServerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bind(error) => write!(formatter, "bind gRPC listener: {error}"),
            Self::Serve(error) => write!(formatter, "serve gRPC listener: {error}"),
            Self::Join(error) => write!(formatter, "join gRPC serving task: {error}"),
            Self::UnexpectedExit => write!(formatter, "gRPC serving task exited unexpectedly"),
            Self::Signal(error) => write!(formatter, "wait for shutdown signal: {error}"),
            Self::InvalidListenAddress(error) => {
                write!(formatter, "parse gRPC listen address: {error}")
            }
            Self::Environment(error) => write!(formatter, "read daemon environment: {error}"),
        }
    }
}

impl std::error::Error for ServerError {}

/// A running gRPC server and its controlled shutdown handle.
pub struct RunningServer {
    local_addr: SocketAddr,
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
    /// Returns the address selected by the operating system.
    pub const fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// Waits for the serving task to finish without requesting shutdown.
    ///
    /// This is useful to let a daemon react immediately if its serving task
    /// exits before an external shutdown signal arrives.
    pub async fn wait(&mut self) -> Result<(), ServerError> {
        (&mut self.task)
            .await
            .map_err(ServerError::Join)?
            .map_err(ServerError::Serve)
    }

    /// Requests graceful shutdown and waits for the serving task to finish.
    pub async fn shutdown(mut self) -> Result<(), ServerError> {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }

        self.task
            .await
            .map_err(ServerError::Join)?
            .map_err(ServerError::Serve)
    }
}

/// Binds and starts the transport-only gRPC server.
pub async fn start(config: ServerConfig) -> Result<RunningServer, ServerError> {
    start_with_service(config, SearchKernelTransport::default()).await
}

/// Binds and starts a gRPC server backed by one canonical dispatcher.
pub async fn start_with_dispatcher(
    config: ServerConfig,
    dispatcher: Arc<Dispatcher>,
) -> Result<RunningServer, ServerError> {
    start_with_service(config, SearchKernelTransport::with_dispatcher(dispatcher)).await
}

async fn start_with_service(
    config: ServerConfig,
    service: SearchKernelTransport,
) -> Result<RunningServer, ServerError> {
    let listener = TcpListener::bind(config.listen_addr)
        .await
        .map_err(ServerError::Bind)?;
    let local_addr = listener.local_addr().map_err(ServerError::Bind)?;
    let incoming = TcpListenerStream::new(listener);
    let (shutdown, shutdown_requested) = oneshot::channel();

    let task = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(SearchKernelServer::new(service))
            .serve_with_incoming_shutdown(incoming, async move {
                let _ = shutdown_requested.await;
            })
            .await
    });

    Ok(RunningServer {
        local_addr,
        shutdown: Some(shutdown),
        task,
    })
}
