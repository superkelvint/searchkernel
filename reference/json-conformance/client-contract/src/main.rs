use searchkernel_client::model::{
    AnalyzeRequest, AnalyzeResult, DeleteRequest, DeleteResult, DescribeRequest,
    DescribeResult, Error, FilterExpr, GetRequest, GetResult, InsertRequest,
    InsertResult, InspectQueryRequest, InspectQueryResult, PatchRequest, PatchResult,
    QueryExpr, RankingPolicy, SearchRequest, SearchResult, UpsertRequest, UpsertResult,
    Value,
};
use searchkernel_client::{AsyncClient, AsyncTransport, Client, ClientError, Transport};
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

fn assert_default<T: Default>() {}

fn assert_decoded_result<T>()
where
    T: Default + PartialEq + std::fmt::Debug,
{
}

async fn assert_async_client_surface() {
    let client = AsyncClient::new(TestAsyncTransport);

    let _: Result<InsertResult, ClientError<TestError>> =
        client.insert(InsertRequest::default()).await;
    let _: Result<UpsertResult, ClientError<TestError>> =
        client.upsert(UpsertRequest::default()).await;
    let _: Result<PatchResult, ClientError<TestError>> =
        client.patch(PatchRequest::default()).await;
    let _: Result<DeleteResult, ClientError<TestError>> =
        client.delete(DeleteRequest::default()).await;
    let _: Result<GetResult, ClientError<TestError>> = client.get(GetRequest::default()).await;
    let _: Result<SearchResult, ClientError<TestError>> =
        client.search(SearchRequest::default()).await;
    let _: Result<AnalyzeResult, ClientError<TestError>> =
        client.analyze(AnalyzeRequest::default()).await;
    let _: Result<DescribeResult, ClientError<TestError>> =
        client.describe(DescribeRequest::default()).await;
    let _: Result<InspectQueryResult, ClientError<TestError>> =
        client.inspect_query(InspectQueryRequest::default()).await;
}

fn main() {
    let _client = Client::new(TestTransport);
    let _ = assert_async_client_surface;

    let _: fn(&Client<TestTransport>, InsertRequest) -> Result<InsertResult, ClientError<TestError>> =
        Client::insert;
    let _: fn(&Client<TestTransport>, UpsertRequest) -> Result<UpsertResult, ClientError<TestError>> =
        Client::upsert;
    let _: fn(&Client<TestTransport>, PatchRequest) -> Result<PatchResult, ClientError<TestError>> =
        Client::patch;
    let _: fn(&Client<TestTransport>, DeleteRequest) -> Result<DeleteResult, ClientError<TestError>> =
        Client::delete;
    let _: fn(&Client<TestTransport>, GetRequest) -> Result<GetResult, ClientError<TestError>> =
        Client::get;
    let _: fn(&Client<TestTransport>, SearchRequest) -> Result<SearchResult, ClientError<TestError>> =
        Client::search;
    let _: fn(&Client<TestTransport>, AnalyzeRequest) -> Result<AnalyzeResult, ClientError<TestError>> =
        Client::analyze;
    let _: fn(&Client<TestTransport>, DescribeRequest) -> Result<DescribeResult, ClientError<TestError>> =
        Client::describe;
    let _: fn(
        &Client<TestTransport>,
        InspectQueryRequest,
    ) -> Result<InspectQueryResult, ClientError<TestError>> = Client::inspect_query;

    assert_default::<InsertRequest>();
    assert_default::<UpsertRequest>();
    assert_default::<PatchRequest>();
    assert_default::<DeleteRequest>();
    assert_default::<GetRequest>();
    assert_default::<SearchRequest>();
    assert_default::<AnalyzeRequest>();
    assert_default::<DescribeRequest>();
    assert_default::<InspectQueryRequest>();

    // The behavioral verifier compares decoded results with Default and with
    // another decoded payload. Freeze those capabilities into the generated
    // client contract so a result type cannot evade payload-decoding tests.
    assert_decoded_result::<InsertResult>();
    assert_decoded_result::<UpsertResult>();
    assert_decoded_result::<PatchResult>();
    assert_decoded_result::<DeleteResult>();
    assert_decoded_result::<GetResult>();
    assert_decoded_result::<SearchResult>();
    assert_decoded_result::<AnalyzeResult>();
    assert_decoded_result::<DescribeResult>();
    assert_decoded_result::<InspectQueryResult>();

    // Freeze representative generated fields, rather than only freezing type
    // names. These accesses ensure the client surface is schema-derived and
    // cannot regress to arbitrary JSON maps.
    let _ = InsertRequest {
        document: None,
    };
    let _ = SearchRequest {
        query: Some("rust".to_owned()),
        limit: Some(10),
        ..Default::default()
    };
    let _: String = InsertResult::default().id;
    let _: bool = DeleteResult::default().deleted;
    let _: Vec<_> = SearchResult::default().hits;
    let _: u64 = SearchResult::default().total_hits.value;
    let _: Vec<_> = AnalyzeResult::default().tokens;
    let _: Option<_> = GetResult::default().document;
    let _: Option<_> = DescribeResult::default().index;
    let _: Option<_> = InspectQueryResult::default().resolved_request;

    let _: Option<Value> = None;
    let _: Option<FilterExpr> = None;
    let _: Option<QueryExpr> = None;
    let _: Option<RankingPolicy> = None;
    let _: Option<Error> = None;
}
