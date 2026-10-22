#![cfg(all(feature = "native", feature = "hardening"))]

use searchkernel_vespa_engine::{
    hardening, ConfigCompiler, Document, Engine, ErrorKind, FacetRequest, Field, FieldType,
    FieldValue, GroupingAggregation, GroupingExpression, GroupingLevel, GroupingOrder,
    GroupingPlan, GroupingResultValue, MatchFeatureValue, Query, RankProfile, ScalarValue, Schema,
    SearchRequest, SummaryRequest, TensorValue, Value,
};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

fn temporary_path(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("searchkernel-result-decoding-{label}-{nonce}"))
}

fn fixture() -> (Engine, PathBuf, PathBuf) {
    let config = temporary_path("config");
    let data = temporary_path("data");
    let schema = Schema::new("typea")
        .field(Field::new("title", FieldType::Text).indexed().summary())
        .field(
            Field::new("category", FieldType::String)
                .attribute()
                .summary(),
        )
        .field(Field::new("qty", FieldType::Integer).attribute().summary())
        .field(Field::new("rating", FieldType::Float).attribute().summary())
        .field(
            Field::new("active", FieldType::Boolean)
                .attribute()
                .summary(),
        )
        .field(
            Field::new("stored_verification_id", FieldType::Text)
                .indexed()
                .summary(),
        )
        .rank_profile(
            RankProfile::new("features")
                .first_phase("attribute(qty)")
                .match_feature("query(test_feature)")
                .match_feature("attribute(qty)"),
        )
        .rank_profile(
            RankProfile::new("data_features")
                .query_input("embedding", "tensor<float>(x[2])")
                .first_phase("attribute(qty)")
                .match_feature("query(embedding)"),
        );
    ConfigCompiler
        .compile_to(&schema, &config)
        .expect("compile result-decoding config");
    let engine = Engine::open(&data, &config).expect("open result-decoding engine");
    engine
        .put(
            Document::new("decode-1")
                .field("title", "alpha")
                .field("category", "books")
                .field("qty", 7_i64)
                .field("rating", 4.5_f64)
                .field("active", true)
                .field("stored_verification_id", "decode-1"),
        )
        .expect("put result-decoding document");
    (engine, config, data)
}

fn search_request() -> SearchRequest {
    SearchRequest::new(Query::True)
        .rank_profile("features")
        .query_feature("test_feature", 2.5)
        .fields(["title", "category", "qty", "rating", "active"])
        .facet(FacetRequest::new("category", 4, GroupingOrder::None))
        .facet(FacetRequest::new("qty", 4, GroupingOrder::None))
}

fn identity_search_request() -> SearchRequest {
    SearchRequest::new(Query::True)
        .summary(SummaryRequest::Fields(vec![
            "stored_verification_id".to_owned()
        ]))
        .limit(10)
}

fn data_feature_search_request() -> SearchRequest {
    SearchRequest::new(Query::True)
        .rank_profile("data_features")
        .query_tensor(
            "embedding",
            TensorValue::dense("tensor<float>(x[2])", [1.25, -2.5]),
        )
}

fn grouping_plan() -> GroupingPlan {
    GroupingPlan::new().level(
        GroupingLevel::new(GroupingExpression::attribute("category"))
            .aggregation(GroupingAggregation::Count)
            .aggregation(GroupingAggregation::Average(GroupingExpression::attribute(
                "qty",
            ))),
    )
}

fn assert_search_fault(engine: &Engine, fault: hardening::ResultFault) {
    let before = hardening::result_destroy_count(engine).expect("read result destroy count");
    hardening::set_result_fault(engine, fault).expect("set result fault");
    let error = match engine.search(search_request()) {
        Ok(_) => panic!("malformed search result unexpectedly decoded: {fault:?}"),
        Err(error) => error,
    };
    hardening::clear_result_fault(engine).expect("clear result fault");
    assert!(!error.to_string().is_empty());
    assert_eq!(error.kind(), ErrorKind::Internal, "{fault:?}: {error}");
    let after = hardening::result_destroy_count(engine).expect("read result destroy count");
    assert_eq!(
        after,
        before + 1,
        "search result was not destroyed on error"
    );
}

#[test]
fn native_result_decoding_preserves_mixed_values_features_facets_and_grouping() {
    let (engine, config, data) = fixture();
    let result = engine
        .search(search_request())
        .expect("decode mixed search result");
    assert_eq!(
        result.total_hits_relation,
        searchkernel_vespa_engine::TotalHitsRelation::Unknown
    );
    assert_eq!(result.hits.len(), 1);
    assert_eq!(result.hits[0].document_id, "decode-1");
    assert_eq!(
        result.hits[0].fields["title"],
        FieldValue::Scalar(ScalarValue::String("alpha".into()))
    );
    assert_eq!(
        result.hits[0].fields["qty"],
        FieldValue::Scalar(ScalarValue::Integer(7))
    );
    assert_eq!(
        result.hits[0].fields["rating"],
        FieldValue::Scalar(ScalarValue::Float(4.5))
    );
    assert_eq!(
        result.hits[0].fields["active"],
        FieldValue::Scalar(ScalarValue::Boolean(true))
    );
    assert_eq!(result.hits[0].match_features.len(), 2);
    assert_eq!(
        result.hits[0].match_features["query(test_feature)"],
        MatchFeatureValue::Double(2.5)
    );
    assert_eq!(
        result.hits[0].match_features["attribute(qty)"],
        MatchFeatureValue::Double(7.0)
    );
    assert_eq!(result.coverage.active, 1);
    assert_eq!(result.coverage.covered, 1);
    assert_eq!(result.coverage.target_active, 1);
    assert_eq!(result.coverage.degradation_flags, 0);
    assert!(result.issues.is_empty());
    assert_eq!(result.facets.len(), 2);
    assert_eq!(
        result.facets[0].groups[0].key,
        Value::String("books".into())
    );
    assert_eq!(result.facets[1].groups[0].key, Value::Integer(7));

    let grouping = engine
        .group(&Query::True, &grouping_plan())
        .expect("decode grouping result");
    assert_eq!(grouping.key, None);
    assert!(grouping.outputs.is_empty());
    assert_eq!(grouping.groups.len(), 1);
    assert_eq!(
        grouping.groups[0].key,
        Some(GroupingResultValue::String("books".into()))
    );
    assert_eq!(
        grouping.groups[0].outputs,
        vec![
            searchkernel_vespa_engine::GroupingOutput::Value(GroupingResultValue::Integer(1)),
            searchkernel_vespa_engine::GroupingOutput::Value(GroupingResultValue::Float(7.0)),
        ]
    );

    engine.close().expect("close result-decoding engine");
    let _ = fs::remove_dir_all(config);
    let _ = fs::remove_dir_all(data);
}

#[test]
fn native_result_decoding_preserves_opaque_match_feature_bytes() {
    let (engine, config, data) = fixture();
    let result = engine
        .search(data_feature_search_request())
        .expect("decode opaque data match feature");
    assert_eq!(result.hits.len(), 1);
    assert_eq!(
        result.hits[0].match_features["query(embedding)"],
        MatchFeatureValue::Data(vec![
            0x06, 0x01, 0x01, 0x01, 0x78, 0x02, 0x3f, 0xa0, 0x00, 0x00, 0xc0, 0x20, 0x00, 0x00,
        ])
    );

    engine.close().expect("close data feature engine");
    let _ = fs::remove_dir_all(config);
    let _ = fs::remove_dir_all(data);
}

#[test]
fn native_result_decoding_rejects_malformed_document_values() {
    let (engine, config, data) = fixture();
    for fault in [
        hardening::ResultFault::NullDocumentFieldName,
        hardening::ResultFault::InvalidDocumentFieldName,
        hardening::ResultFault::NullDocumentString,
        hardening::ResultFault::InvalidDocumentString,
        hardening::ResultFault::UnknownDocumentKind,
        hardening::ResultFault::DocumentAccessorError,
    ] {
        hardening::set_result_fault(&engine, fault).expect("set document result fault");
        let error = engine
            .get("decode-1")
            .expect_err("malformed document result unexpectedly decoded");
        hardening::clear_result_fault(&engine).expect("clear document result fault");
        assert_eq!(error.kind(), ErrorKind::Internal, "{fault:?}: {error}");
        assert!(!error.to_string().is_empty());
        assert!(engine.get("decode-1").expect("healthy get").is_some());
    }

    engine
        .close()
        .expect("close document result-decoding engine");
    let _ = fs::remove_dir_all(config);
    let _ = fs::remove_dir_all(data);
}

#[test]
fn native_result_decoding_rejects_malformed_search_values_and_releases_results() {
    let (engine, config, data) = fixture();
    for fault in [
        hardening::ResultFault::NullResult,
        hardening::ResultFault::NullHitId,
        hardening::ResultFault::InvalidHitId,
        hardening::ResultFault::NullSummaryName,
        hardening::ResultFault::InvalidSummaryName,
        hardening::ResultFault::NullSummaryString,
        hardening::ResultFault::InvalidSummaryString,
        hardening::ResultFault::UnknownSummaryKind,
        hardening::ResultFault::NullMatchFeatureName,
        hardening::ResultFault::InvalidMatchFeatureName,
        hardening::ResultFault::DuplicateMatchFeatureName,
        hardening::ResultFault::MatchFeatureValueError,
        hardening::ResultFault::UnsupportedGroupKey,
        hardening::ResultFault::InvalidGroupKey,
        hardening::ResultFault::GroupAccessorError,
    ] {
        assert_search_fault(&engine, fault);
    }

    engine
        .search(search_request())
        .expect("healthy search after malformed results");
    engine.close().expect("close search result-decoding engine");
    let _ = fs::remove_dir_all(config);
    let _ = fs::remove_dir_all(data);
}

#[test]
fn native_result_decoding_preserves_each_coverage_flag_and_issue_message() {
    for fault in [
        searchkernel_vespa_bindings::SK_DEGRADATION_MATCH_PHASE,
        searchkernel_vespa_bindings::SK_DEGRADATION_TIMEOUT,
        searchkernel_vespa_bindings::SK_DEGRADATION_ANN_TIMEOUT,
    ] {
        let result =
            hardening::search_reply_metadata(fault, &[]).expect("decode coverage metadata result");
        assert_eq!(result.coverage.degradation_flags, fault);
        assert_eq!(result.coverage.active, 17);
        assert_eq!(result.coverage.covered, 11);
        assert_eq!(result.coverage.target_active, 19);
    }

    let result =
        hardening::search_reply_metadata(0, &["first native issue", "second native issue"])
            .expect("decode issue metadata result");
    assert_eq!(
        result.issues,
        vec!["first native issue", "second native issue"]
    );
}

#[test]
fn native_grouping_tree_decoder_rejects_malformed_recursive_results_and_releases_them() {
    let (engine, config, data) = fixture();
    let faults = [
        hardening::ResultFault::InvalidGroupingString,
        hardening::ResultFault::UnknownGroupingValueKind,
        hardening::ResultFault::HitListGroupingOutput,
        hardening::ResultFault::UnknownGroupingOutputKind,
        hardening::ResultFault::GroupingTreeGroupAccessorError,
        hardening::ResultFault::GroupingTreeRootAccessorError,
        hardening::ResultFault::GroupingTreeKeyAccessorError,
        hardening::ResultFault::GroupingTreeOutputAccessorError,
        hardening::ResultFault::GroupingOutputValueAccessorError,
        hardening::ResultFault::NullGroupingChild,
        hardening::ResultFault::NullGroupingOutput,
        hardening::ResultFault::NullGroupingValue,
    ];

    for fault in faults {
        let before = hardening::result_destroy_count(&engine).expect("read tree destroy count");
        hardening::set_result_fault(&engine, fault).expect("set grouping tree fault");
        let error = engine
            .group(&Query::True, &grouping_plan())
            .expect_err("malformed grouping tree unexpectedly decoded");
        hardening::clear_result_fault(&engine).expect("clear grouping tree fault");
        assert_eq!(error.kind(), ErrorKind::Internal, "{fault:?}: {error}");
        assert!(!error.to_string().is_empty());
        let after = hardening::result_destroy_count(&engine).expect("read tree destroy count");
        assert_eq!(
            after,
            before + 1,
            "grouping tree was not destroyed exactly once"
        );

        let healthy = engine
            .group(&Query::True, &grouping_plan())
            .expect("healthy grouping after malformed tree");
        assert!(!healthy.groups.is_empty());
    }

    engine
        .close()
        .expect("close grouping tree hardening engine");
    let _ = fs::remove_dir_all(config);
    let _ = fs::remove_dir_all(data);
}

#[test]
fn native_search_keeps_surviving_hit_data_aligned_after_concurrent_delete() {
    let (engine, config, data) = fixture();
    engine
        .put(
            Document::new("decode-2")
                .field("title", "beta")
                .field("category", "music")
                .field("qty", 22_i64)
                .field("rating", 3.5_f64)
                .field("active", false)
                .field("stored_verification_id", "decode-2"),
        )
        .expect("put second result-decoding document");

    let engine = Arc::new(engine);
    hardening::set(&engine, "search_after_match", 1, hardening::Action::Block)
        .expect("set post-match barrier");
    let searching = Arc::clone(&engine);
    let search = std::thread::spawn(move || searching.search(search_request()));

    for _ in 0..100 {
        if hardening::is_blocked(&engine, "search_after_match").expect("read post-match barrier") {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(
        hardening::is_blocked(&engine, "search_after_match").expect("confirm post-match barrier"),
        "search did not reach the post-match barrier"
    );
    assert!(engine
        .delete("decode-1")
        .expect("delete first matched document"));
    hardening::release(&engine, "search_after_match").expect("release post-match barrier");
    let result = search
        .join()
        .expect("search thread")
        .expect("search after concurrent delete");
    hardening::clear(&engine, "search_after_match").expect("clear post-match barrier");

    assert_eq!(result.hits.len(), 1);
    let hit = &result.hits[0];
    assert_eq!(hit.document_id, "decode-2");
    assert_eq!(
        hit.fields["qty"],
        FieldValue::Scalar(ScalarValue::Integer(22))
    );
    assert_eq!(hit.match_features["attribute(qty)"], 22.0);

    let engine =
        Arc::try_unwrap(engine).unwrap_or_else(|_| panic!("no outstanding engine references"));
    engine.close().expect("close concurrent-delete engine");
    let _ = fs::remove_dir_all(config);
    let _ = fs::remove_dir_all(data);
}

#[test]
fn native_search_keeps_docsums_associated_after_delete_during_docsum_request() {
    let (engine, config, data) = fixture();
    engine
        .put(
            Document::new("decode-2")
                .field("title", "beta")
                .field("category", "music")
                .field("qty", 22_i64)
                .field("rating", 3.5_f64)
                .field("active", false)
                .field("stored_verification_id", "decode-2"),
        )
        .expect("put second docsums-association document");

    let engine = Arc::new(engine);
    hardening::set(
        &engine,
        "search_after_id_resolution",
        1,
        hardening::Action::Block,
    )
    .expect("set post-ID-resolution barrier");
    let searching = Arc::clone(&engine);
    let search = std::thread::spawn(move || searching.search(identity_search_request()));

    for _ in 0..100 {
        if hardening::is_blocked(&engine, "search_after_id_resolution")
            .expect("read post-ID-resolution barrier")
        {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(
        hardening::is_blocked(&engine, "search_after_id_resolution")
            .expect("confirm post-ID-resolution barrier"),
        "search did not resolve hit IDs before the barrier"
    );
    assert!(engine
        .delete("decode-1")
        .expect("delete first docsums-association document"));
    hardening::release(&engine, "search_after_id_resolution")
        .expect("release post-ID-resolution barrier");
    let result = search
        .join()
        .expect("docsums-association search thread")
        .expect("search after delete during docsum request");
    hardening::clear(&engine, "search_after_id_resolution")
        .expect("clear post-ID-resolution barrier");

    assert_eq!(result.hits.len(), 2);
    // Vespa returns an empty entry for the deleted GID, preserving the
    // request position. A compacted response would incorrectly put decode-2's
    // summary in the deleted hit's slot.
    let deleted = result
        .hits
        .iter()
        .find(|hit| hit.document_id == "decode-1")
        .expect("stale matched hit for deleted document");
    assert!(
        !deleted.fields.contains_key("stored_verification_id"),
        "deleted hit received a summary from another hit: {deleted:?}"
    );
    let surviving = result
        .hits
        .iter()
        .find(|hit| hit.document_id == "decode-2")
        .expect("surviving matched hit");
    assert_eq!(
        surviving.fields.get("stored_verification_id"),
        Some(&FieldValue::Scalar(ScalarValue::String("decode-2".into())))
    );

    let engine =
        Arc::try_unwrap(engine).unwrap_or_else(|_| panic!("no outstanding engine references"));
    engine.close().expect("close docsums-association engine");
    let _ = fs::remove_dir_all(config);
    let _ = fs::remove_dir_all(data);
}

#[test]
fn caller_validation_remains_invalid_argument() {
    let (engine, config, data) = fixture();
    for error in [
        engine.get("invalid\0id").unwrap_err(),
        engine.put(Document::new("empty")).unwrap_err(),
        engine
            .put(Document::new("bad-field").field("invalid\0field", 1_i64))
            .unwrap_err(),
    ] {
        assert_eq!(error.kind(), ErrorKind::InvalidArgument);
        assert_eq!(error.detail_code(), None);
    }
    engine.close().unwrap();
    fs::remove_dir_all(config).unwrap();
    fs::remove_dir_all(data).unwrap();
}
