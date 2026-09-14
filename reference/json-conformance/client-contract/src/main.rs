use searchkernel_client::model::{
    AnalyzeRequest, AnalyzeResult, DeleteRequest, DeleteResult, DescribeRequest,
    DescribeResult, Error, FilterExpr, GetRequest, GetResult, InsertRequest,
    InsertResult, InspectQueryRequest, InspectQueryResult, PatchRequest, PatchResult,
    QueryExpr, RankingPolicy, SearchRequest, SearchResult, UpsertRequest, UpsertResult,
    Value,
};
use searchkernel_client::{Client, ClientError, Transport};

#[derive(Debug)]
struct TestError;

struct TestTransport;

impl Transport for TestTransport {
    type Error = TestError;

    fn call(&self, request: &[u8]) -> Result<Vec<u8>, Self::Error> {
        Ok(request.to_vec())
    }
}

fn assert_default<T: Default>() {}

fn assert_decoded_result<T>()
where
    T: Default + PartialEq + std::fmt::Debug,
{
}

fn main() {
    let _client = Client::new(TestTransport);

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
