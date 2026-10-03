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

    use http::HeaderValue;
    use searchkernel::Index;
    use searchkernel_dispatch::Dispatcher;
    use searchkernel_ir::validate_index_target;
    use searchkerneld::{start, start_multi, IndexRegistry, ServerConfig, ServerError};

    const DEFAULT_UNIX_SOCKET: &str = "/tmp/searchkerneld.sock";

    #[derive(Debug)]
    enum Mode {
        Multi {
            data_root: PathBuf,
        },
        Migrate {
            source_path: PathBuf,
            data_root: PathBuf,
            target: String,
        },
        LegacySingle {
            index_path: PathBuf,
            definition_path: Option<PathBuf>,
            definition_json_path: Option<PathBuf>,
        },
    }

    #[derive(Debug)]
    struct Config {
        mode: Mode,
        server: ServerConfig,
    }

    fn config() -> Result<Config, Box<dyn Error>> {
        let mut args = env::args_os().skip(1);
        let mut data_root = None;
        let mut migrate_index_path = None;
        let mut migrate_target = None;
        let mut index_path = None;
        let mut definition_path = None;
        let mut definition_json_path = None;
        let mut listen = None;
        let mut web_listen = None;
        let mut web_origins = Vec::new();
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
                "--migrate-index" => {
                    migrate_index_path = Some(
                        args.next()
                            .ok_or("--migrate-index requires a path")
                            .map(PathBuf::from)?,
                    )
                }
                "--target" => {
                    migrate_target = Some(
                        args.next()
                            .ok_or("--target requires a logical index name")?
                            .to_string_lossy()
                            .into_owned(),
                    )
                }
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
                "--definition-json" => {
                    definition_json_path = Some(
                        args.next()
                            .ok_or("--definition-json requires a path")
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
                "--web-origin" => {
                    web_origins.push(
                        args.next()
                            .ok_or("--web-origin requires an exact browser origin")?
                            .to_string_lossy()
                            .into_owned(),
                    );
                }
                value => return Err(format!("unknown argument {value}").into()),
            }
        }

        let data_root =
            data_root.or_else(|| env::var_os("SEARCHKERNEL_DATA_ROOT").map(PathBuf::from));
        let index_path =
            index_path.or_else(|| env::var_os("SEARCHKERNEL_INDEX_PATH").map(PathBuf::from));
        let definition_path = definition_path
            .or_else(|| env::var_os("SEARCHKERNEL_INDEX_DEFINITION").map(PathBuf::from));
        let definition_json_path = definition_json_path
            .or_else(|| env::var_os("SEARCHKERNEL_INDEX_DEFINITION_JSON").map(PathBuf::from));

        if definition_path.is_some() && definition_json_path.is_some() {
            return Err("--definition and --definition-json are mutually exclusive".into());
        }
        let mode = if migrate_index_path.is_some() || migrate_target.is_some() {
            if index_path.is_some() || definition_path.is_some() || definition_json_path.is_some() {
                return Err(
                    "--migrate-index cannot be combined with legacy --index/--definition options"
                        .into(),
                );
            }
            let source_path =
                migrate_index_path.ok_or("--migrate-index is required with --target")?;
            let data_root = data_root.ok_or("--data-root is required with --migrate-index")?;
            let target = migrate_target.ok_or("--target is required with --migrate-index")?;
            validate_index_target(&target)?;
            Mode::Migrate {
                source_path,
                data_root,
                target,
            }
        } else {
            if migrate_target.is_some() {
                return Err("--target is only valid with --migrate-index".into());
            }
            if data_root.is_some()
                && (index_path.is_some()
                    || definition_path.is_some()
                    || definition_json_path.is_some())
            {
                return Err(
                    "--data-root cannot be combined with legacy --index/--definition options"
                        .into(),
                );
            }

            match (data_root, index_path) {
                (Some(data_root), None) => Mode::Multi { data_root },
                (None, Some(index_path)) => Mode::LegacySingle {
                    index_path,
                    definition_path,
                    definition_json_path,
                },
                (None, None) => {
                    return Err(
                        "--data-root/SEARCHKERNEL_DATA_ROOT is required for multi-index mode; legacy --index/SEARCHKERNEL_INDEX_PATH remains temporarily supported"
                            .into(),
                    )
                }
                (Some(_), Some(_)) => unreachable!("mixed daemon modes rejected above"),
            }
        };

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
        if web_origins.is_empty() {
            match env::var("SEARCHKERNEL_GRPC_WEB_ORIGIN") {
                Ok(value) => web_origins.push(value),
                Err(env::VarError::NotPresent) => {}
                Err(error) => return Err(ServerError::Environment(error).into()),
            }
        }
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

        let mut server = if let Some(web_listen_addr) = web_listen_addr {
            server.with_web_listen_addr(web_listen_addr)
        } else {
            server
        };
        for origin in web_origins {
            let origin = origin
                .parse::<HeaderValue>()
                .map_err(|_| format!("invalid --web-origin value {origin:?}"))?;
            if origin.as_bytes() == b"*" {
                return Err("--web-origin does not accept wildcard origins".into());
            }
            server = server.with_web_allowed_origin(origin);
        }

        Ok(Config {
            mode,
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

    fn open_legacy_index(
        index_path: &PathBuf,
        definition_path: Option<&PathBuf>,
        definition_json_path: Option<&PathBuf>,
    ) -> Result<Index, Box<dyn Error>> {
        if index_path.is_dir() {
            if definition_path.is_some() || definition_json_path.is_some() {
                return Err(
                    "--definition and --definition-json are only valid when creating an index"
                        .into(),
                );
            }
            return Ok(Index::open(index_path)?);
        }

        let definition = match (definition_path, definition_json_path) {
            (Some(path), None) => std::fs::read(path)?,
            (None, Some(path)) => {
                let json = std::fs::read(path)?;
                searchkernel_protocol::json::index_definition_json_to_flatbuffer(&json)?
            }
            (None, None) => return Err(
                "--definition or --definition-json is required when creating a legacy single index"
                    .into(),
            ),
            (Some(_), Some(_)) => unreachable!("mutually exclusive definitions checked in config"),
        };
        Ok(Index::create_from_definition_bytes(
            index_path,
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

    async fn wait_for_shutdown(
        mut server: searchkerneld::RunningServer,
    ) -> Result<(), ServerError> {
        let signal = graceful_shutdown_signal();
        tokio::pin!(signal);
        tokio::select! {
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
        }
    }

    #[tokio::main]
    pub async fn run() -> Result<(), Box<dyn Error>> {
        let config = config()?;
        match config.mode {
            Mode::Multi { data_root } => {
                let registry = Arc::new(IndexRegistry::new(data_root)?);
                let server = start_multi(config.server, registry.clone()).await?;
                let result = wait_for_shutdown(server).await;
                registry.close_all()?;
                result?;
            }
            Mode::Migrate {
                source_path,
                data_root,
                target,
            } => {
                std::fs::create_dir_all(&data_root)?;
                Index::migrate_storage(&source_path, data_root.join(&target))?;
            }
            Mode::LegacySingle {
                index_path,
                definition_path,
                definition_json_path,
            } => {
                let index = open_legacy_index(
                    &index_path,
                    definition_path.as_ref(),
                    definition_json_path.as_ref(),
                )?;
                let (context, session) = index.clone().into_session_parts();
                let dispatcher = Arc::new(Dispatcher::new(context, session));
                let server = match start(config.server, dispatcher).await {
                    Ok(server) => server,
                    Err(error) => {
                        index.close()?;
                        return Err(error.into());
                    }
                };
                let result = wait_for_shutdown(server).await;
                index.close()?;
                result?;
            }
        }
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
