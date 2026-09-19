use std::{env, error::Error, net::SocketAddr, path::PathBuf, sync::Arc};

use searchkernel::Index;
use searchkernel_dispatch::Dispatcher;
use searchkerneld::{start_with_dispatcher, ServerConfig};

fn required_path(name: &str) -> Result<PathBuf, Box<dyn Error>> {
    env::var_os(name)
        .map(PathBuf::from)
        .ok_or_else(|| format!("{name} is required").into())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let index_path = required_path("SEARCHKERNEL_E2E_INDEX_PATH")?;
    let index = if index_path.is_dir() {
        Index::open(index_path)?
    } else {
        let definition_path = required_path("SEARCHKERNEL_E2E_INDEX_DEFINITION")?;
        let definition = std::fs::read(definition_path)?;
        Index::create_from_definition_bytes(index_path, &definition)?
    };
    let (context, session) = index.clone().into_session_parts();
    let dispatcher = Arc::new(Dispatcher::new(context, session));
    let address: SocketAddr = env::var("SEARCHKERNEL_E2E_GRPC_ADDR")?.parse()?;
    let mut server = start_with_dispatcher(ServerConfig::new(address), dispatcher).await?;

    tokio::select! {
        result = server.wait() => {
            result?;
            return Err("gRPC server exited unexpectedly".into());
        }
        signal = tokio::signal::ctrl_c() => {
            signal?;
            server.shutdown().await?;
        }
    }
    index.close()?;
    Ok(())
}
