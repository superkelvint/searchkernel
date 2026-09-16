#![cfg(feature = "hardening")]

use searchkernel_vespa_engine::{
    hardening, Aggregation, ConfigCompiler, Document, Engine, ErrorKind, FacetRequest, Field,
    FieldType, FieldValue, GroupingOrder, GroupingRequest, Query, RankProfile, ScalarValue, Schema,
    SearchRequest, SummaryRequest, Value,
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

fn grouping_request() -> GroupingRequest {
    GroupingRequest::new(Query::True, "category")
        .aggregation(Aggregation::Sum("qty".to_owned()))
        .aggregation(Aggregation::Min("qty".to_owned()))
        .aggregation(Aggregation::Max("qty".to_owned()))
        .aggregation(Aggregation::Average("qty".to_owned()))
}

fn expected_error_kind(fault: hardening::ResultFault) -> ErrorKind {
    match fault {
        hardening::ResultFault::DocumentAccessorError
        | hardening::ResultFault::MatchFeatureValueError
        | hardening::ResultFault::GroupAccessorError
        | hardening::ResultFault::AggregationAccessorError => ErrorKind::Internal,
        _ => ErrorKind::InvalidArgument,
    }
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
    assert_eq!(error.kind(), expected_error_kind(fault));
    let after = hardening::result_destroy_count(engine).expect("read result destroy count");
    assert_eq!(
        after,
        before + 1,
        "search result was not destroyed on error"
    );
}

fn assert_grouping_fault(engine: &Engine, fault: hardening::ResultFault) {
    let before = hardening::result_destroy_count(engine).expect("read result destroy count");
    hardening::set_result_fault(engine, fault).expect("set result fault");
    let error = engine
        .group(&grouping_request())
        .expect_err("malformed grouping result unexpectedly decoded");
    hardening::clear_result_fault(engine).expect("clear result fault");
    assert!(!error.to_string().is_empty());
    assert_eq!(error.kind(), expected_error_kind(fault));
    let after = hardening::result_destroy_count(engine).expect("read result destroy count");
    assert_eq!(
        after,
        before + 1,
        "grouping result was not destroyed on error"
    );
}

#[test]
fn native_result_decoding_preserves_mixed_values_features_facets_and_grouping() {
    let (engine, config, data) = fixture();
    let result = engine
        .search(search_request())
        .expect("decode mixed search result");
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
    assert_eq!(result.facets.len(), 2);
    assert_eq!(
        result.facets[0].groups[0].key,
        Value::String("books".into())
    );
    assert_eq!(result.facets[1].groups[0].key, Value::Integer(7));

    let grouping = engine
        .group(&grouping_request())
        .expect("decode grouping result");
    assert_eq!(grouping.groups.len(), 1);
    assert_eq!(grouping.groups[0].aggregations.len(), 4);
    assert_eq!(grouping.groups[0].key, Value::String("books".into()));

    engine.close().expect("close result-decoding engine");
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
fn native_result_decoding_rejects_malformed_grouping_values_and_releases_results() {
    let (engine, config, data) = fixture();
    for fault in [
        hardening::ResultFault::NullResult,
        hardening::ResultFault::UnsupportedGroupKey,
        hardening::ResultFault::InvalidGroupKey,
        hardening::ResultFault::GroupAccessorError,
        hardening::ResultFault::UnknownAggregationKind,
        hardening::ResultFault::AggregationAccessorError,
    ] {
        assert_grouping_fault(&engine, fault);
    }

    engine
        .group(&grouping_request())
        .expect("healthy grouping after malformed results");
    engine
        .close()
        .expect("close grouping result-decoding engine");
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
