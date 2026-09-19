use searchkerneld::{start, ServerConfig, ServerError};

fn config_from_environment() -> Result<ServerConfig, ServerError> {
    match std::env::var("SEARCHKERNEL_GRPC_ADDR") {
        Ok(value) => config_from_address(Some(&value)),
        Err(std::env::VarError::NotPresent) => config_from_address(None),
        Err(error) => Err(ServerError::Environment(error)),
    }
}

fn config_from_address(value: Option<&str>) -> Result<ServerConfig, ServerError> {
    value
        .map(str::parse)
        .transpose()
        .map(|address| address.map_or_else(ServerConfig::default, ServerConfig::new))
        .map_err(ServerError::InvalidListenAddress)
}

#[tokio::main]
async fn main() -> Result<(), ServerError> {
    let mut server = start(config_from_environment()?).await?;

    tokio::select! {
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
    }
}

#[cfg(test)]
mod tests {
    use super::{config_from_address, ServerConfig};
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
}
