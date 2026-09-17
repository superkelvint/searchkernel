include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../core/crates/searchkernel-ir/tests/semantics.rs"
));

#[test]
fn frozen_timeout_corpus_is_executed_by_search_and_inspection() {
    let scenarios: serde_json::Value =
        serde_json::from_str(include_str!("../../scenarios/timeout.json"))
            .expect("valid frozen timeout scenarios");
    let expected: serde_json::Value =
        serde_json::from_str(include_str!("../../expected/timeout.json"))
            .expect("valid frozen timeout expectations");
    assert_eq!(scenarios["operation"], "search");

    let context = context(Some(&["title"]));
    for case in scenarios["cases"].as_array().expect("timeout cases array") {
        let name = case["name"].as_str().expect("timeout case name");
        let timeout = case["request"]["timeout"].as_u64();
        let expected_timeout = expected[name]["timeout_ms"].as_u64();
        assert_eq!(timeout, expected_timeout, "fixture expectation: {name}");

        let bytes = timeout_request_bytes(timeout);
        let request = searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&bytes)
            .expect("valid timeout request");
        assert_eq!(
            context
                .search_to_ir(request)
                .expect("search request conversion")
                .timeout()
                .map(|value| value.as_millis() as u64),
            expected_timeout,
            "search conversion: {name}"
        );

        let request = searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&bytes)
            .expect("valid timeout request");
        assert_eq!(
            context
                .inspect_query(request)
                .expect("inspection conversion")
                .timeout()
                .map(|value| value.as_millis() as u64),
            expected_timeout,
            "inspection conversion: {name}"
        );
    }
}
