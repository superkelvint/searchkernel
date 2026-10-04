use searchkernel_protocol::{protocol::v1 as wire, wire::v1 as api};
use searchkerneld::public_search::{PublicSearchConfig, PublicSearchError, PublicSearchRequest};

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
    let config = config();
    config.validate().unwrap();
    let bytes = config
        .bind(&request())
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
    let config = config();
    let mut request = request();
    request.limit = 101;
    assert_eq!(
        config.bind(&request).unwrap_err(),
        PublicSearchError::InvalidRequest
    );
    request.limit = 7;
    request.facets = vec!["private_field".into()];
    assert_eq!(
        config.bind(&request).unwrap_err(),
        PublicSearchError::Unsupported
    );
    request.facets.clear();
    request.endpoint = "missing".into();
    assert_eq!(
        config.bind(&request).unwrap_err(),
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
        config.validate().unwrap_err(),
        PublicSearchError::InvalidRequest
    );
}
