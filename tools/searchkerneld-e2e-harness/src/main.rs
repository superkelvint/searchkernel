use std::{env, error::Error, net::SocketAddr, path::PathBuf, sync::Arc};

use searchkernel::Index;
use searchkernel_dispatch::Dispatcher;
use searchkerneld::{start_with_dispatcher, ServerConfig};

#[derive(Debug)]
struct Config {
    index_path: PathBuf,
    definition_path: Option<PathBuf>,
    grpc_addr: SocketAddr,
}

fn config() -> Result<Config, Box<dyn Error>> {
    let mut args = env::args_os().skip(1);
    let mut index_path = None;
    let mut definition_path = None;
    let mut grpc_addr = None;

    while let Some(argument) = args.next() {
        match argument.to_string_lossy().as_ref() {
            "--index-path" => index_path = args.next().map(PathBuf::from),
            "--definition" => definition_path = args.next().map(PathBuf::from),
            "--grpc-addr" => {
                grpc_addr = Some(
                    args.next()
                        .ok_or("--grpc-addr requires an address")?
                        .to_string_lossy()
                        .parse()?,
                )
            }
            value => return Err(format!("unknown argument {value}").into()),
        }
    }

    let index_path = index_path
        .or_else(|| env::var_os("SEARCHKERNEL_E2E_INDEX_PATH").map(PathBuf::from))
        .ok_or("--index-path or SEARCHKERNEL_E2E_INDEX_PATH is required")?;
    let definition_path = definition_path
        .or_else(|| env::var_os("SEARCHKERNEL_E2E_INDEX_DEFINITION").map(PathBuf::from));
    let grpc_addr = match grpc_addr {
        Some(value) => value,
        None => env::var_os("SEARCHKERNEL_E2E_GRPC_ADDR")
            .ok_or("--grpc-addr or SEARCHKERNEL_E2E_GRPC_ADDR is required")?
            .to_string_lossy()
            .parse()?,
    };

    Ok(Config {
        index_path,
        definition_path,
        grpc_addr,
    })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let config = config()?;
    let index = if config.index_path.is_dir() {
        Index::open(&config.index_path)?
    } else {
        let definition_path = config
            .definition_path
            .ok_or("a definition is required when the index path does not exist")?;
        let definition = std::fs::read(definition_path)?;
        Index::create_from_definition_bytes(&config.index_path, &definition)?
    };
    let (context, session) = index.clone().into_session_parts();
    let dispatcher = Arc::new(Dispatcher::new(context, session));
    let mut server = start_with_dispatcher(ServerConfig::new(config.grpc_addr), dispatcher).await?;

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
