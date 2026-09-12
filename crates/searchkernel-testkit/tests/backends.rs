use searchkernel_core::__flatbuffers::{root, FlatBufferBuilder};
use searchkernel_core::{
    api, AnalyzeRequestIr, DeleteRequestIr, DeleteResult, GetRequestIr, GetResult, IndexContext,
    InsertRequestIr, InsertResult, OperationIr, PatchRequestIr, PatchResult, SearchRequestIr,
    SearchResult, UpsertRequestIr, UpsertResult,
};
use searchkernel_testkit::{
    Backend, ExpectedCall, RecordingBackend, ScriptedBackend, ScriptedCallError, ScriptedResult,
    TestkitError,
};

fn context() -> IndexContext {
    let mut builder = FlatBufferBuilder::new();
    let name = builder.create_string("title");
    let stored = api::OptionalBool::new(true);
    let searchable = api::OptionalBool::new(true);
    let analyzer_name = builder.create_string("standard");
    let analyzer = api::Analyzer::create(
        &mut builder,
        &api::AnalyzerArgs {
            name: Some(analyzer_name),
            version: 1,
            options: None,
        },
    );
    let field = api::FieldDefinition::create(
        &mut builder,
        &api::FieldDefinitionArgs {
            name: Some(name),
            kind: api::FieldKind::Text,
            cardinality: api::FieldCardinality::Single,
            stored: Some(&stored),
            searchable: Some(&searchable),
            analyzer: Some(analyzer),
            ..Default::default()
        },
    );
    let fields = builder.create_vector(&[field]);
    let definition = api::IndexDefinition::create(
        &mut builder,
        &api::IndexDefinitionArgs {
            fields: Some(fields),
            ..Default::default()
        },
    );
    builder.finish(definition, None);
    let bytes = builder.finished_data().to_vec();
    IndexContext::from_api(root::<api::IndexDefinition>(&bytes).unwrap()).unwrap()
}

fn insert() -> InsertRequestIr {
    let mut builder = FlatBufferBuilder::new();
    let id = builder.create_string("a");
    let document = api::Document::create(
        &mut builder,
        &api::DocumentArgs {
            id: Some(id),
            ..Default::default()
        },
    );
    let request = api::InsertRequest::create(
        &mut builder,
        &api::InsertRequestArgs {
            document: Some(document),
        },
    );
    builder.finish(request, None);
    let bytes = builder.finished_data().to_vec();
    context()
        .insert_to_ir(root::<api::InsertRequest>(&bytes).unwrap())
        .unwrap()
}

fn upsert() -> UpsertRequestIr {
    let mut builder = FlatBufferBuilder::new();
    let id = builder.create_string("a");
    let document = api::Document::create(
        &mut builder,
        &api::DocumentArgs {
            id: Some(id),
            ..Default::default()
        },
    );
    let request = api::UpsertRequest::create(
        &mut builder,
        &api::UpsertRequestArgs {
            document: Some(document),
        },
    );
    builder.finish(request, None);
    let bytes = builder.finished_data().to_vec();
    context()
        .upsert_to_ir(root::<api::UpsertRequest>(&bytes).unwrap())
        .unwrap()
}

fn patch() -> PatchRequestIr {
    let mut builder = FlatBufferBuilder::new();
    let id = builder.create_string("a");
    let field = builder.create_string("title");
    let unset = api::UnsetField::create(&mut builder, &api::UnsetFieldArgs { field: Some(field) });
    let operation = api::PatchOperation::create(
        &mut builder,
        &api::PatchOperationArgs {
            operation_type: api::PatchOperationKind::UnsetField,
            operation: Some(unset.as_union_value()),
        },
    );
    let operations = builder.create_vector(&[operation]);
    let request = api::PatchRequest::create(
        &mut builder,
        &api::PatchRequestArgs {
            id: Some(id),
            operations: Some(operations),
        },
    );
    builder.finish(request, None);
    let bytes = builder.finished_data().to_vec();
    context()
        .patch_to_ir(root::<api::PatchRequest>(&bytes).unwrap())
        .unwrap()
}

fn delete() -> DeleteRequestIr {
    let mut builder = FlatBufferBuilder::new();
    let id = builder.create_string("a");
    let request =
        api::DeleteRequest::create(&mut builder, &api::DeleteRequestArgs { id: Some(id) });
    builder.finish(request, None);
    let bytes = builder.finished_data().to_vec();
    context()
        .delete_to_ir(root::<api::DeleteRequest>(&bytes).unwrap())
        .unwrap()
}

fn get() -> GetRequestIr {
    let mut builder = FlatBufferBuilder::new();
    let id = builder.create_string("a");
    let request = api::GetRequest::create(
        &mut builder,
        &api::GetRequestArgs {
            id: Some(id),
            ..Default::default()
        },
    );
    builder.finish(request, None);
    let bytes = builder.finished_data().to_vec();
    context()
        .get_to_ir(root::<api::GetRequest>(&bytes).unwrap())
        .unwrap()
}

fn search() -> SearchRequestIr {
    let mut builder = FlatBufferBuilder::new();
    let request = api::SearchRequest::create(&mut builder, &api::SearchRequestArgs::default());
    builder.finish(request, None);
    let bytes = builder.finished_data().to_vec();
    context()
        .search_to_ir(root::<api::SearchRequest>(&bytes).unwrap())
        .unwrap()
}

fn analyze() -> AnalyzeRequestIr {
    let mut builder = FlatBufferBuilder::new();
    let name = builder.create_string("standard");
    let analyzer = api::Analyzer::create(
        &mut builder,
        &api::AnalyzerArgs {
            name: Some(name),
            version: 1,
            options: None,
        },
    );
    let text = builder.create_string("text");
    let request = api::AnalyzeRequest::create(
        &mut builder,
        &api::AnalyzeRequestArgs {
            target_type: api::AnalyzeTarget::Analyzer,
            target: Some(analyzer.as_union_value()),
            text: Some(text),
            ..Default::default()
        },
    );
    builder.finish(request, None);
    let bytes = builder.finished_data().to_vec();
    context()
        .analyze_to_ir(root::<api::AnalyzeRequest>(&bytes).unwrap())
        .unwrap()
}

#[test]
fn recording_preserves_exact_order_and_can_drain() {
    let backend = RecordingBackend::new();
    assert!(Backend::insert(&backend, insert()).is_err());
    assert!(Backend::delete(&backend, delete()).is_err());
    assert!(Backend::search(&backend, search()).is_err());
    assert_eq!(backend.calls().len(), 3);
    assert!(matches!(backend.last_call(), Some(OperationIr::Search(_))));
    let drained = backend.take_calls();
    assert!(matches!(
        drained.as_slice(),
        [
            OperationIr::Insert(_),
            OperationIr::Delete(_),
            OperationIr::Search(_)
        ]
    ));
    assert!(backend.calls().is_empty());
    backend.clear();
}

#[test]
fn recording_supports_all_backend_variants() {
    let backend = RecordingBackend::default();
    assert!(Backend::upsert(&backend, upsert()).is_err());
    assert!(Backend::patch(&backend, patch()).is_err());
    assert!(Backend::delete(&backend, delete()).is_err());
    assert!(Backend::get(&backend, get()).is_err());
    assert!(Backend::search(&backend, search()).is_err());
    assert!(Backend::analyze(&backend, analyze()).is_err());
    assert_eq!(backend.calls().len(), 6);
}

#[test]
fn scripted_backend_matches_typed_requests_and_returns_owned_results() {
    let expected = insert();
    let backend = ScriptedBackend::new(vec![ExpectedCall::success(
        OperationIr::Insert(expected.clone()),
        ScriptedResult::Insert(InsertResult { id: "a".to_owned() }),
    )]);
    assert_eq!(backend.call_insert(expected).unwrap().id, "a");
    assert_eq!(backend.finish(), Ok(()));
}

#[test]
fn scripted_backend_returns_backend_errors_without_executing() {
    let request = delete();
    let error = searchkernel_core::PortableError::new(
        searchkernel_core::ErrorCode::Unavailable,
        "temporarily unavailable",
    );
    let backend = ScriptedBackend::new(vec![ExpectedCall::error(
        OperationIr::Delete(request.clone()),
        error.clone(),
    )]);
    assert_eq!(
        backend.call_delete(request),
        Err(ScriptedCallError::Backend(error))
    );
}

#[test]
fn scripted_backend_reports_order_mismatch_and_exhaustion() {
    let backend = ScriptedBackend::new(vec![ExpectedCall::success(
        OperationIr::Get(get()),
        ScriptedResult::Get(GetResult { document: None }),
    )]);
    let error = backend.call_search(search()).unwrap_err();
    assert!(matches!(
        error,
        ScriptedCallError::Misuse(error)
            if matches!(*error, TestkitError::UnexpectedCall { position: 0, .. })
    ));
    let error = backend.call_get(get()).unwrap();
    assert!(error.document.is_none());
    let error = backend.call_get(get()).unwrap_err();
    assert!(matches!(
        error,
        ScriptedCallError::Misuse(error)
            if matches!(*error, TestkitError::ScriptExhausted { position: 1, .. })
    ));
}

#[test]
fn scripted_backend_reports_request_mismatch_and_unused_entries() {
    let backend = ScriptedBackend::new(vec![ExpectedCall::success(
        OperationIr::Patch(patch()),
        ScriptedResult::Patch(PatchResult { id: "a".to_owned() }),
    )]);
    let different = {
        let mut builder = FlatBufferBuilder::new();
        let id = builder.create_string("b");
        let field = builder.create_string("title");
        let unset =
            api::UnsetField::create(&mut builder, &api::UnsetFieldArgs { field: Some(field) });
        let operation = api::PatchOperation::create(
            &mut builder,
            &api::PatchOperationArgs {
                operation_type: api::PatchOperationKind::UnsetField,
                operation: Some(unset.as_union_value()),
            },
        );
        let operations = builder.create_vector(&[operation]);
        let request = api::PatchRequest::create(
            &mut builder,
            &api::PatchRequestArgs {
                id: Some(id),
                operations: Some(operations),
            },
        );
        builder.finish(request, None);
        let bytes = builder.finished_data().to_vec();
        context()
            .patch_to_ir(root::<api::PatchRequest>(&bytes).unwrap())
            .unwrap()
    };
    assert!(matches!(
        backend.call_patch(different),
        Err(ScriptedCallError::Misuse(error))
            if matches!(*error, TestkitError::ExpectedDifferentRequest { position: 0, .. })
    ));
    assert!(matches!(
        backend.finish(),
        Err(TestkitError::UnusedScriptedCalls {
            position: 0,
            remaining: 1
        })
    ));
}

#[test]
fn scripted_trait_supports_all_seven_families() {
    let backend = ScriptedBackend::new(vec![
        ExpectedCall::success(
            OperationIr::Insert(insert()),
            ScriptedResult::Insert(InsertResult { id: "a".into() }),
        ),
        ExpectedCall::success(
            OperationIr::Upsert(upsert()),
            ScriptedResult::Upsert(UpsertResult { id: "a".into() }),
        ),
        ExpectedCall::success(
            OperationIr::Patch(patch()),
            ScriptedResult::Patch(PatchResult { id: "a".into() }),
        ),
        ExpectedCall::success(
            OperationIr::Delete(delete()),
            ScriptedResult::Delete(DeleteResult {
                id: "a".into(),
                deleted: true,
            }),
        ),
        ExpectedCall::success(
            OperationIr::Get(get()),
            ScriptedResult::Get(GetResult { document: None }),
        ),
        ExpectedCall::success(
            OperationIr::Search(search()),
            ScriptedResult::Search(SearchResult {
                hits: vec![],
                total_hits: searchkernel_core::TotalHits {
                    value: 0,
                    relation: searchkernel_core::TotalHitsRelation::Exact,
                },
                facets: vec![],
            }),
        ),
        ExpectedCall::success(
            OperationIr::Analyze(analyze()),
            ScriptedResult::Analyze(searchkernel_core::AnalyzeResult { tokens: vec![] }),
        ),
    ]);
    assert!(Backend::insert(&backend, insert()).is_ok());
    assert!(Backend::upsert(&backend, upsert()).is_ok());
    assert!(Backend::patch(&backend, patch()).is_ok());
    assert!(Backend::delete(&backend, delete()).is_ok());
    assert!(Backend::get(&backend, get()).is_ok());
    assert!(Backend::search(&backend, search()).is_ok());
    assert!(Backend::analyze(&backend, analyze()).is_ok());
    assert_eq!(backend.remaining(), 0);
}
