use std::{env, error::Error, path::PathBuf, sync::Arc};

use searchkernel::Index;
use searchkernel_dispatch::Dispatcher;
use searchkerneld::{start, ServerConfig, ServerError};

const DEFAULT_UNIX_SOCKET: &str = "/tmp/searchkerneld.sock";

#[derive(Debug)]
struct Config {
    index_path: PathBuf,
    definition_path: Option<PathBuf>,
    server: ServerConfig,
}

fn config() -> Result<Config, Box<dyn Error>> {
    let mut args = env::args_os().skip(1);
    let mut index_path = None;
    let mut definition_path = None;
    let mut listen = None;

    while let Some(argument) = args.next() {
        match argument.to_string_lossy().as_ref() {
            "--index" => {
                index_path = Some(
                    args.next()
                        .ok_or("--index requires a path")
                        .map(PathBuf::from)?,
                )
            }
            "--definition" => {
                definition_path = Some(
                    args.next()
                        .ok_or("--definition requires a path")
                        .map(PathBuf::from)?,
                )
            }
            "--listen" => {
                listen = Some(args.next().ok_or("--listen requires an address")?);
            }
            value => return Err(format!("unknown argument {value}").into()),
        }
    }

    let index_path = index_path
        .or_else(|| env::var_os("SEARCHKERNEL_INDEX_PATH").map(PathBuf::from))
        .ok_or("--index or SEARCHKERNEL_INDEX_PATH is required")?;
    let definition_path =
        definition_path.or_else(|| env::var_os("SEARCHKERNEL_INDEX_DEFINITION").map(PathBuf::from));
    let server = match listen {
        Some(value) => config_from_address(Some(&value.to_string_lossy()))?,
        None => match env::var("SEARCHKERNEL_GRPC_ADDR") {
            Ok(value) => config_from_address(Some(&value))?,
            Err(env::VarError::NotPresent) => default_server_config(),
            Err(error) => return Err(ServerError::Environment(error).into()),
        },
    };

    Ok(Config {
        index_path,
        definition_path,
        server,
    })
}

fn default_server_config() -> ServerConfig {
    #[cfg(unix)]
    {
        ServerConfig::unix(DEFAULT_UNIX_SOCKET)
    }
    #[cfg(not(unix))]
    {
        ServerConfig::default()
    }
}

fn open_index(config: &Config) -> Result<Index, Box<dyn Error>> {
    if config.index_path.is_dir() {
        if config.definition_path.is_some() {
            return Err("--definition is only valid when creating an index".into());
        }
        return Ok(Index::open(&config.index_path)?);
    }

    let definition_path = config
        .definition_path
        .as_ref()
        .ok_or("--definition is required when creating an index")?;
    let definition = std::fs::read(definition_path)?;
    Ok(Index::create_from_definition_bytes(
        &config.index_path,
        &definition,
    )?)
}

fn config_from_address(value: Option<&str>) -> Result<ServerConfig, ServerError> {
    #[cfg(unix)]
    if let Some(path) = value.and_then(|value| {
        value
            .strip_prefix("unix://")
            .or_else(|| value.strip_prefix("unix:"))
    }) {
        return if path.is_empty() {
            Err(ServerError::InvalidUnixSocketPath)
        } else {
            Ok(ServerConfig::unix(path))
        };
    }

    value
        .map(str::parse)
        .transpose()
        .map(|address| address.map_or_else(ServerConfig::default, ServerConfig::new))
        .map_err(ServerError::InvalidListenAddress)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let config = config()?;
    let index = open_index(&config)?;
    let (context, session) = index.clone().into_session_parts();
    let dispatcher = Arc::new(Dispatcher::new(context, session));
    let mut server = match start(config.server, dispatcher).await {
        Ok(server) => server,
        Err(error) => {
            index.close()?;
            return Err(error.into());
        }
    };

    let result = tokio::select! {
        result = server.wait() => match result {
            Ok(()) => Err(ServerError::UnexpectedExit),
            Err(error) => Err(error),
        },
        signal = tokio::signal::ctrl_c() => {
            let signal = signal.map_err(ServerError::Signal);
            let shutdown = server.shutdown().await;
            signal?;
            shutdown
        }
    };
    index.close()?;
    result?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{config_from_address, default_server_config, ServerConfig};
    use std::net::SocketAddr;

    #[test]
    fn configured_address_is_parsed() {
        let expected = SocketAddr::from(([127, 0, 0, 1], 50051));
        assert_eq!(
            config_from_address(Some("127.0.0.1:50051")).unwrap(),
            ServerConfig::new(expected)
        );
        assert_eq!(config_from_address(None).unwrap(), ServerConfig::default());
        assert!(config_from_address(Some("not-an-address")).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn configured_unix_socket_is_parsed() {
        assert_eq!(
            config_from_address(Some("unix:///tmp/searchkerneld.sock")).unwrap(),
            ServerConfig::unix("/tmp/searchkerneld.sock")
        );
        assert!(config_from_address(Some("unix://")).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn default_server_uses_a_unix_socket() {
        assert_eq!(
            default_server_config(),
            ServerConfig::unix("/tmp/searchkerneld.sock")
        );
    }
}
