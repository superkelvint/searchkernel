use searchkernel_client::{AsyncClient, AsyncTransport, Client, Transport};
use std::future::Future;

#[derive(Debug)]
struct TestError;

struct TestTransport;

impl Transport for TestTransport {
    type Error = TestError;

    fn call(&self, request: &[u8]) -> Result<Vec<u8>, Self::Error> {
        Ok(request.to_vec())
    }
}

struct TestAsyncTransport;

impl AsyncTransport for TestAsyncTransport {
    type Error = TestError;

    fn call(&self, request: &[u8]) -> impl Future<Output = Result<Vec<u8>, Self::Error>> + Send {
        let request = request.to_vec();
        async move { Ok(request) }
    }
}

fn assert_send<T: Send>(_: T) {}
fn assert_send_sync<T: Send + Sync>() {}

fn assert_async_client_operation_is_send<T: AsyncTransport>(client: &AsyncClient<T>) {
    assert_send(client.search(Default::default()));
}

#[test]
fn transports_and_clients_are_send_and_sync() {
    assert_send_sync::<TestTransport>();
    assert_send_sync::<TestAsyncTransport>();
    assert_send_sync::<Client<TestTransport>>();
    assert_send_sync::<AsyncClient<TestAsyncTransport>>();
}

#[test]
fn async_client_operations_return_send_futures() {
    let client = AsyncClient::new(TestAsyncTransport);
    assert_async_client_operation_is_send(&client);
}
