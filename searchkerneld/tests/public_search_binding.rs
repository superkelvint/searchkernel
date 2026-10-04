use searchkernel_protocol::{protocol::v1 as wire, wire::v1 as api};
use searchkerneld::search_endpoints::{PublicSearchConfig, PublicSearchError, PublicSearchRequest};

fn config() -> PublicSearchConfig {
    serde_json::from_str(
        r#"{
      "behindRateLimitedProxy":true,
      "endpoints":{"products":{"target":"products","generation":"0123456789abcdef0123456789abcdef",
        "fields":["title","brand","price"],"filterFields":{"brand":"string","price":"int64"},
        "facets":["brand"],"highlights":["title"],"sort":[{"field":"price","descending":false}]}}
    }"#,
    )
    .unwrap()
}
fn request() -> PublicSearchRequest {
    serde_json::from_str(r#"{"endpoint":"products","query":"wireless","limit":7,"offset":14,
      "facets":["brand"],"facetLimit":10,"snippets":[],"filters":[
        {"op":"or","clauses":[{"op":"eq","field":"brand","value":"Acme"},{"op":"eq","field":"brand","value":"Bravo"}]},
        {"op":"gte","field":"price","value":10}]}
    "#).unwrap()
}
#[test]
fn all_union_discriminators_precede_payloads_in_canonical_frame() {
    let config = config().compile().unwrap();
    let bytes = config
        .bind_post(&request())
        .expect("official BFBS parser accepts every nested union");
    let envelope = flatbuffers::root::<wire::RequestEnvelope<'_>>(&bytes).unwrap();
    assert_eq!(envelope.target().unwrap().name(), Some("products"));
    let search = envelope
        .payload_as_search_kernel_wire_search_request()
        .unwrap();
    assert_eq!(search.limit().unwrap().value(), 7);
    assert_eq!(search.offset().unwrap().value(), 14);
    assert_eq!(
        search.query().unwrap().source_type(),
        api::QuerySourceKind::SearchKernelTextQuery
    );
    assert_eq!(search.filters().unwrap().structured().unwrap().len(), 2);
    assert_eq!(search.grouping().unwrap().len(), 1);
    assert_eq!(search.sort().unwrap().len(), 1);
}
#[test]
fn malformed_and_unbounded_public_shapes_fail_before_dispatch() {
    let config = config().compile().unwrap();
    let mut request = request();
    request.limit = 101;
    assert_eq!(
        config.bind_post(&request).unwrap_err(),
        PublicSearchError::InvalidRequest
    );
    request.limit = 7;
    request.facets = vec!["private_field".into()];
    assert_eq!(
        config.bind_post(&request).unwrap_err(),
        PublicSearchError::Unsupported
    );
    request.facets.clear();
    request.endpoint = "missing".into();
    assert_eq!(
        config.bind_post(&request).unwrap_err(),
        PublicSearchError::UnknownIndex
    );
    assert!(serde_json::from_str::<PublicSearchRequest>(r#"{"endpoint":"products","query":"a","limit":1,"offset":0,"facets":[],"facetLimit":10,"filters":[],"snippets":[],"properties":{}}"#).is_err());
}

#[test]
fn nested_highlight_configuration_is_explicitly_outside_the_profile() {
    let mut config = config();
    let endpoint = config.endpoints.get_mut("products").unwrap();
    endpoint.fields.push("address.street".into());
    endpoint.highlights.push("address.street".into());
    assert_eq!(
        config.compile().unwrap_err(),
        PublicSearchError::InvalidRequest
    );
}

#[cfg(feature = "native")]
#[tokio::test]
async fn production_get_uses_endpoint_policy_before_dispatch() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let root = std::env::temp_dir().join(format!("endpoint-get-regression-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let registry = std::sync::Arc::new(searchkerneld::IndexRegistry::new(&root).unwrap());
    let server = searchkerneld::search_endpoints::start_public_search(
        "127.0.0.1:0".parse().unwrap(),
        config().compile().unwrap(),
        registry.clone(),
    )
    .await
    .unwrap();
    let mut stream = tokio::net::TcpStream::connect(server.local_addr())
        .await
        .unwrap();
    stream.write_all(b"GET /search/products?q=wireless&limit=101 HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n").await.unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).await.unwrap();
    server.shutdown().await;
    registry.close_all().unwrap();
    std::fs::remove_dir_all(root).unwrap();
    assert!(response.starts_with("HTTP/1.1 400"), "{response}");
    assert!(response.contains("invalid_request"), "{response}");
}

#[test]
fn deployment_policy_compiles_into_both_transport_binders() {
    let mut deployment = config();
    deployment.endpoints.get_mut("products").unwrap().policy =
        serde_json::from_value(serde_json::json!({
            "maxQueryBytes": 8, "defaultLimit": 3, "maxLimit": 7, "maxOffset": 14,
            "maxFacetLimit": 10, "maxFilters": 2, "maxFilterNodes": 4, "maxFilterDepth": 1,
            "timeoutMs": 1234
        }))
        .unwrap();
    let router = deployment.compile().unwrap();
    let mut post = request();
    post.query = "wireless".into();
    let bound = router.bind_post(&post).unwrap();
    let envelope = wire::root_as_request_envelope(&bound).unwrap();
    assert_eq!(
        envelope.target().unwrap().generation(),
        Some("0123456789abcdef0123456789abcdef")
    );
    assert_eq!(
        envelope
            .payload_as_search_kernel_wire_search_request()
            .unwrap()
            .timeout_ms()
            .unwrap()
            .value(),
        1234
    );
    let defaulted = router.bind_get("/search/products", None).unwrap();
    assert_eq!(
        wire::root_as_request_envelope(&defaulted)
            .unwrap()
            .payload_as_search_kernel_wire_search_request()
            .unwrap()
            .limit()
            .unwrap()
            .value(),
        3
    );
    for (query, limit, offset) in [
        ("too-long-query", 7, 14),
        ("wireless", 8, 14),
        ("wireless", 7, 15),
    ] {
        post.query = query.into();
        post.limit = limit;
        post.offset = offset;
        assert_eq!(
            router.bind_post(&post).unwrap_err(),
            PublicSearchError::InvalidRequest
        );
        assert!(router
            .bind_get(
                "/search/products",
                Some(&format!("q={query}&limit={limit}&offset={offset}"))
            )
            .is_err());
    }
    post = request();
    post.facet_limit = 11;
    assert_eq!(
        router.bind_post(&post).unwrap_err(),
        PublicSearchError::InvalidRequest
    );
    post = request();
    post.filters.push(post.filters[1].clone());
    assert_eq!(
        router.bind_post(&post).unwrap_err(),
        PublicSearchError::InvalidRequest
    );
}

#[test]
fn aliases_compile_to_distinct_fixed_target_generations() {
    let mut deployment = config();
    let mut second = deployment.endpoints["products"].clone();
    second.target = "other".into();
    second.generation = "other-generation".into();
    deployment.endpoints.insert("second".into(), second);
    let router = deployment.compile().unwrap();
    for (alias, target, generation) in [
        ("products", "products", "0123456789abcdef0123456789abcdef"),
        ("second", "other", "other-generation"),
    ] {
        let get = router
            .bind_get(
                &format!("/search/{alias}"),
                Some("q=wireless&limit=7&offset=14"),
            )
            .unwrap();
        let envelope = wire::root_as_request_envelope(&get).unwrap();
        assert_eq!(envelope.target().unwrap().name(), Some(target));
        assert_eq!(envelope.target().unwrap().generation(), Some(generation));
        let mut post = request();
        post.endpoint = alias.into();
        let post = router.bind_post(&post).unwrap();
        let envelope = wire::root_as_request_envelope(&post).unwrap();
        assert_eq!(envelope.target().unwrap().name(), Some(target));
        assert_eq!(envelope.target().unwrap().generation(), Some(generation));
    }
}

#[test]
fn invalid_compiled_policy_and_exposure_fail_startup() {
    for policy in [
        serde_json::json!({"defaultLimit":8, "maxLimit":7}),
        serde_json::json!({"timeoutMs":0}),
        serde_json::json!({"maxFilterDepth":17}),
    ] {
        let mut deployment = config();
        deployment.endpoints.get_mut("products").unwrap().policy =
            serde_json::from_value(policy).unwrap();
        assert_eq!(
            deployment.compile().unwrap_err(),
            PublicSearchError::InvalidRequest
        );
    }
    for serving in [
        serde_json::json!({"maxBatchRequests":0}),
        serde_json::json!({"executionLimit":0}),
        serde_json::json!({"readTimeoutMs":0}),
    ] {
        let mut deployment = config();
        deployment.serving = serde_json::from_value(serving).unwrap();
        assert_eq!(
            deployment.compile().unwrap_err(),
            PublicSearchError::InvalidRequest
        );
    }
    let mut deployment = config();
    deployment
        .endpoints
        .get_mut("products")
        .unwrap()
        .generation
        .clear();
    assert_eq!(
        deployment.compile().unwrap_err(),
        PublicSearchError::InvalidRequest
    );
    let mut deployment = config();
    deployment
        .endpoints
        .get_mut("products")
        .unwrap()
        .facets
        .push("private".into());
    assert_eq!(
        deployment.compile().unwrap_err(),
        PublicSearchError::InvalidRequest
    );
}

#[test]
fn configured_filter_value_bytes_bound_the_canonical_binder() {
    let mut deployment = config();
    deployment.endpoints.get_mut("products").unwrap().policy =
        serde_json::from_value(serde_json::json!({"maxFilterValueBytes":4})).unwrap();
    let router = deployment.compile().unwrap();
    let mut request = request();
    request.filters =
        serde_json::from_value(serde_json::json!([{"op":"eq","field":"brand","value":"Acme"}]))
            .unwrap();
    assert!(router.bind_post(&request).is_ok());
    request.filters =
        serde_json::from_value(serde_json::json!([{"op":"eq","field":"brand","value":"Bravo"}]))
            .unwrap();
    assert_eq!(
        router.bind_post(&request).unwrap_err(),
        PublicSearchError::InvalidRequest
    );
}
