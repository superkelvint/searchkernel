#[cfg(not(feature = "native"))]
fn main() {
    eprintln!("searchkerneld requires the `native` feature");
}

#[cfg(feature = "native")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    native_main::run()
}

#[cfg(feature = "native")]
mod native_main {
    use std::{env, error::Error, path::PathBuf, sync::Arc};

    use searchkerneld::{start_multi, IndexRegistry, ServerConfig, ServerError};

    const DEFAULT_UNIX_SOCKET: &str = "/tmp/searchkerneld.sock";

    #[derive(Debug)]
    struct Config {
        data_root: PathBuf,
        server: ServerConfig,
    }

    fn config() -> Result<Config, Box<dyn Error>> {
        let mut args = env::args_os().skip(1);
        let mut data_root = None;
        let mut listen = None;
        let mut web_listen = None;
        let mut execution_limit = None;
        let mut admission_limit = None;

        while let Some(argument) = args.next() {
            match argument.to_string_lossy().as_ref() {
                "--data-root" => {
                    data_root = Some(
                        args.next()
                            .ok_or("--data-root requires a path")
                            .map(PathBuf::from)?,
                    )
                }
                "--execution-limit" => {
                    execution_limit = Some(
                        args.next()
                            .ok_or("--execution-limit requires a positive integer")?
                            .to_string_lossy()
                            .parse::<usize>()?,
                    );
                }
                "--admission-limit" => {
                    admission_limit = Some(
                        args.next()
                            .ok_or("--admission-limit requires a positive integer")?
                            .to_string_lossy()
                            .parse::<usize>()?,
                    );
                }
                "--listen" => {
                    listen = Some(args.next().ok_or("--listen requires an address")?);
                }
                "--web-listen" => {
                    web_listen = Some(args.next().ok_or("--web-listen requires an address")?);
                }
                value => return Err(format!("unknown argument {value}").into()),
            }
        }

        let data_root = data_root
            .or_else(|| env::var_os("SEARCHKERNEL_DATA_ROOT").map(PathBuf::from))
            .ok_or("--data-root or SEARCHKERNEL_DATA_ROOT is required")?;
        let server = match listen {
            Some(value) => config_from_address(Some(&value.to_string_lossy()))?,
            None => match env::var("SEARCHKERNEL_GRPC_ADDR") {
                Ok(value) => config_from_address(Some(&value))?,
                Err(env::VarError::NotPresent) => default_server_config(),
                Err(error) => return Err(ServerError::Environment(error).into()),
            },
        };
        let web_listen_addr = match web_listen {
            Some(value) => Some(parse_web_listen_address(&value.to_string_lossy())?),
            None => match env::var("SEARCHKERNEL_GRPC_WEB_ADDR") {
                Ok(value) => Some(parse_web_listen_address(&value)?),
                Err(env::VarError::NotPresent) => None,
                Err(error) => return Err(ServerError::Environment(error).into()),
            },
        };
        let execution_limit = match execution_limit {
            Some(value) => value,
            None => match env::var("SEARCHKERNEL_EXECUTION_LIMIT") {
                Ok(value) => value.parse::<usize>()?,
                Err(env::VarError::NotPresent) => server.execution_limit,
                Err(error) => return Err(error.into()),
            },
        };
        let admission_limit = match admission_limit {
            Some(value) => value,
            None => match env::var("SEARCHKERNEL_ADMISSION_LIMIT") {
                Ok(value) => value.parse::<usize>()?,
                Err(env::VarError::NotPresent) => server.admission_limit,
                Err(error) => return Err(error.into()),
            },
        };

        let server = if let Some(web_listen_addr) = web_listen_addr {
            server.with_web_listen_addr(web_listen_addr)
        } else {
            server
        };

        Ok(Config {
            data_root,
            server: server
                .with_execution_limit(execution_limit)
                .with_admission_limit(admission_limit),
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

    fn parse_web_listen_address(value: &str) -> Result<std::net::SocketAddr, ServerError> {
        value.parse().map_err(ServerError::InvalidListenAddress)
    }

    #[cfg(unix)]
    async fn graceful_shutdown_signal() -> std::io::Result<()> {
        use tokio::signal::unix::{signal, SignalKind};

        let mut terminate = signal(SignalKind::terminate())?;
        tokio::select! {
            result = tokio::signal::ctrl_c() => result,
            _ = terminate.recv() => Ok(()),
        }
    }

    #[cfg(not(unix))]
    async fn graceful_shutdown_signal() -> std::io::Result<()> {
        tokio::signal::ctrl_c().await
    }

    #[tokio::main]
    pub async fn run() -> Result<(), Box<dyn Error>> {
        let config = config()?;
        let registry = Arc::new(IndexRegistry::new(config.data_root)?);
        let mut server = start_multi(config.server, registry.clone()).await?;

        let signal = graceful_shutdown_signal();
        tokio::pin!(signal);
        let result = tokio::select! {
            result = server.wait() => match result {
                Ok(()) => Err(ServerError::UnexpectedExit),
                Err(error) => Err(error),
            },
            signal = &mut signal => {
                let signal = signal.map_err(ServerError::Signal);
                let shutdown = server.shutdown().await;
                signal?;
                shutdown
            }
        };
        registry.close_all()?;
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

        #[test]
        fn execution_limit_can_be_configured() {
            let config = ServerConfig::new(SocketAddr::from(([127, 0, 0, 1], 50051)))
                .with_execution_limit(3);
            assert_eq!(config.execution_limit, 3);
        }

        #[test]
        fn admission_limit_can_be_configured() {
            let config = ServerConfig::new(SocketAddr::from(([127, 0, 0, 1], 50051)))
                .with_admission_limit(3);
            assert_eq!(config.admission_limit, 3);
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
}
