use super::*;

#[test]
fn scripted_backend_get_converts_documents_and_projections() {
    let definition = scripted_definition();
    let context = context_for(&definition);
    let projection = vec!["title".to_owned()];
    let document = searchkernel_ir::DocumentResult {
        id: "doc-1".to_owned(),
        fields: BTreeMap::from([
            (
                "title".to_owned(),
                searchkernel_ir::Value::String("fox".to_owned()),
            ),
            (
                "brand".to_owned(),
                searchkernel_ir::Value::String("acme".to_owned()),
            ),
        ]),
    };
    let backend = Arc::new(ScriptedBackend::new([
        ExpectedCall::success(
            OperationIr::Get(expected_get(&context, "doc-1", None)),
            ScriptedResult::Get(searchkernel_ir::GetResult {
                document: Some(document),
            }),
        ),
        ExpectedCall::success(
            OperationIr::Get(expected_get(&context, "doc-1", Some(&projection))),
            ScriptedResult::Get(searchkernel_ir::GetResult {
                document: Some(searchkernel_ir::DocumentResult {
                    id: "doc-1".to_owned(),
                    fields: BTreeMap::from([(
                        "title".to_owned(),
                        searchkernel_ir::Value::String("fox".to_owned()),
                    )]),
                }),
            }),
        ),
        ExpectedCall::success(
            OperationIr::Get(expected_get(&context, "missing", None)),
            ScriptedResult::Get(searchkernel_ir::GetResult { document: None }),
        ),
    ]));
    let (_runtime, index) = runtime_index(definition, backend.clone());

    let full: Option<serde_json::Value> = index.get("doc-1").unwrap();
    assert_eq!(full, Some(json!({"title": "fox", "brand": "acme"})));
    let projected: Option<serde_json::Value> =
        index.get_with_projection("doc-1", ["title"]).unwrap();
    assert_eq!(projected, Some(json!({"title": "fox"})));
    let missing: Option<serde_json::Value> = index.get("missing").unwrap();
    assert_eq!(missing, None);
    backend.finish().unwrap();
}

#[test]
fn scripted_backend_executes_analyze_and_converts_tokens() {
    let definition = scripted_definition();
    let context = context_for(&definition);
    let (_probe_runtime, probe) =
        runtime_index(definition.clone(), Arc::new(RecordingBackend::new()));
    let probe_request = probe.analyze("Running").field("title").language("EN-us");
    let expected = expected_analyze(&context, &probe_request);
    let backend = Arc::new(ScriptedBackend::new([ExpectedCall::success(
        OperationIr::Analyze(expected),
        ScriptedResult::Analyze(searchkernel_ir::AnalyzeResult {
            tokens: vec![searchkernel_ir::AnalyzedToken {
                surface: "Running".to_owned(),
                term: "run".to_owned(),
                position: 1,
                start_byte: 0,
                end_byte: 7,
            }],
        }),
    )]));
    let (_runtime, index) = runtime_index(definition, backend.clone());

    let result = index
        .analyze("Running")
        .field("title")
        .language("EN-us")
        .execute()
        .unwrap();
    assert_eq!(result.tokens[0].term, "run");
    assert_eq!(result.tokens[0].start_byte, 0);
    assert_eq!(result.tokens[0].end_byte, 7);
    backend.finish().unwrap();
}

#[test]
fn scripted_backend_errors_preserve_code_message_and_details() {
    let definition = scripted_definition();
    let context = context_for(&definition);
    let (_probe_runtime, probe) =
        runtime_index(definition.clone(), Arc::new(RecordingBackend::new()));
    let mut backend_error = searchkernel_ir::PortableError::new(
        searchkernel_ir::ErrorCode::Unavailable,
        "backend unavailable",
    )
    .with_detail("backend_timeout", Some("search"), "request timed out");
    backend_error.retryable = true;
    let backend = Arc::new(ScriptedBackend::new([ExpectedCall::error(
        OperationIr::Search(expected_search(&context, &probe.search("fox"))),
        backend_error,
    )]));
    let (_runtime, index) = runtime_index(definition, backend.clone());

    let error = index
        .search("fox")
        .execute::<serde_json::Value>()
        .expect_err("scripted backend should fail the search");
    assert_eq!(error.code(), crate::ErrorCode::Unavailable);
    assert!(error.retryable());
    assert_eq!(error.details()[0].code, "backend_timeout");
    assert_eq!(error.details()[0].path.as_deref(), Some("search"));
    assert_eq!(error.details()[0].message, "request timed out");
    assert_eq!(error.to_string(), "backend unavailable");
    backend.finish().unwrap();
}

#[test]
fn invalid_values_do_not_reach_the_backend() {
    let (_runtime, index, backend) = test_index();

    let error = index
        .insert("nan", &f64::NAN)
        .expect_err("non-finite JSON values must be rejected");
    assert!(
        matches!(error, Error::Serialization(_)),
        "unexpected error: {error}"
    );
    assert!(backend.calls().is_empty());

    let error = index
        .patch("nan", Patch::new().set("price", f64::NAN))
        .expect_err("invalid patch values must be rejected");
    assert_eq!(error.code(), crate::ErrorCode::InvalidArgument);
    assert!(backend.calls().is_empty());
}

#[test]
fn backend_errors_are_returned_without_being_rewritten() {
    let (_runtime, index, backend) = test_index();
    let error = index
        .insert("doc-1", &json!({"title": "fox"}))
        .expect_err("recording backend intentionally rejects writes");
    assert_eq!(error.code(), crate::ErrorCode::Unsupported);
    assert_eq!(backend.calls().len(), 1);

    let error = index
        .search("fox")
        .limit(5)
        .execute::<serde_json::Value>()
        .expect_err("recording backend intentionally rejects searches");
    assert_eq!(error.code(), crate::ErrorCode::Unsupported);
    assert_eq!(backend.calls().len(), 2);
}
