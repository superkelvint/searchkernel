//! Transport-only gRPC host for Search Kernel.
//!
//! The generated RPC accepts and returns opaque FlatBuffer frames. Semantic
//! dispatch is intentionally not wired into this crate yet.

use std::{fmt, net::SocketAddr};

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

/// The temporary transport implementation of the generated RPC service.
#[derive(Clone, Copy, Debug, Default)]
pub struct SearchKernelTransport;

#[tonic::async_trait]
impl SearchKernel for SearchKernelTransport {
    async fn execute(
        &self,
        _request: Request<FlatBufferFrame>,
    ) -> Result<Response<FlatBufferFrame>, Status> {
        Err(Status::unimplemented(
            "Search Kernel Execute is not wired to the dispatcher yet",
        ))
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
    let listener = TcpListener::bind(config.listen_addr)
        .await
        .map_err(ServerError::Bind)?;
    let local_addr = listener.local_addr().map_err(ServerError::Bind)?;
    let incoming = TcpListenerStream::new(listener);
    let (shutdown, shutdown_requested) = oneshot::channel();

    let task = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(SearchKernelServer::new(SearchKernelTransport))
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
