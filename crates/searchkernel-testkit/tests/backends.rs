use searchkernel_core::{
    AnalyzeRequestIr, AnalyzerIdentity, DeleteRequestIr, DeleteResult, DocumentId, GetRequestIr,
    GetResult, InsertRequestIr, InsertResult, OperationIr, PatchRequestIr, PatchResult,
    ProjectionIr, SearchRequestIr, SearchResult, UpsertRequestIr, UpsertResult,
};
use searchkernel_testkit::{
    Backend, ExpectedCall, RecordingBackend, ScriptedBackend, ScriptedCallError, ScriptedResult,
    TestkitError,
};

fn id(value: &str) -> DocumentId {
    DocumentId::parse(value).unwrap()
}
fn insert() -> InsertRequestIr {
    InsertRequestIr {
        document: searchkernel_core::DocumentIr {
            id: id("a"),
            fields: Default::default(),
        },
    }
}
fn upsert() -> UpsertRequestIr {
    UpsertRequestIr {
        document: insert().document,
    }
}
fn patch() -> PatchRequestIr {
    PatchRequestIr {
        id: id("a"),
        operations: vec![],
    }
}
fn delete() -> DeleteRequestIr {
    DeleteRequestIr { id: id("a") }
}
fn get() -> GetRequestIr {
    GetRequestIr {
        id: id("a"),
        projection: ProjectionIr { fields: vec![] },
    }
}
fn search() -> SearchRequestIr {
    SearchRequestIr {
        query: searchkernel_core::QueryIr::MatchAll,
        search_fields: vec![],
        filter: None,
        sort: vec![],
        facets: vec![],
        ranking: searchkernel_core::RankingPolicyIr::DefaultRelevanceV1,
        limit: 1,
        offset: 0,
        projection: ProjectionIr { fields: vec![] },
        highlighting: searchkernel_core::HighlightingIr::Disabled,
        language: None,
        prefix_policy: searchkernel_core::PrefixPolicy::Disabled,
        typo_policy: searchkernel_core::TypoPolicy::Disabled,
    }
}
fn analyze() -> AnalyzeRequestIr {
    AnalyzeRequestIr {
        analyzer: AnalyzerIdentity::StandardV1,
        text: "text".to_owned(),
        language: None,
    }
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
    let mut different = patch();
    different.id = id("b");
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
