use std::net::SocketAddr;

use searchkerneld::{rpc, start, ServerConfig};
use tonic::{Code, Request};

#[tokio::test]
async fn generated_client_reaches_temporary_execute_service() {
    let server = start(ServerConfig::new(SocketAddr::from(([127, 0, 0, 1], 0))))
        .await
        .expect("start gRPC server");
    let address = server.local_addr();

    let mut client =
        rpc::search_kernel_client::SearchKernelClient::connect(format!("http://{address}"))
            .await
            .expect("connect generated gRPC client");

    let error = client
        .execute(Request::new(rpc::FlatBufferFrame {
            payload: vec![0xde, 0xad, 0xbe, 0xef],
        }))
        .await
        .expect_err("temporary Execute implementation must be unimplemented");
    assert_eq!(error.code(), Code::Unimplemented);

    server
        .shutdown()
        .await
        .expect("gracefully stop gRPC server");
}

#[tokio::test]
async fn ephemeral_server_stops_cleanly() {
    let server = start(ServerConfig::new(SocketAddr::from(([127, 0, 0, 1], 0))))
        .await
        .expect("start gRPC server");
    assert_ne!(server.local_addr().port(), 0);

    server
        .shutdown()
        .await
        .expect("gracefully stop gRPC server");
}
