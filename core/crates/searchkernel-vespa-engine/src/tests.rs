#[cfg(test)]
use super::error::{check, cstring, native_error, path_cstring};
use super::native::{
    length_u32, require_ptr, text_occurrences, text_query_terms, validate_term_query, NativeError,
};
use super::search::validate_search_ranking_inputs;
use super::{
    Document, DocumentAnalysis, DocumentTerm, Error, ErrorKind, Query, QueryAnalysis, QueryTerm,
    SearchRequest, SortField, SortOrder, Source, SummaryRequest, TensorValue,
};
use std::time::Duration;

#[test]
fn boundary_helpers_preserve_native_error_information() {
    let error = NativeError::new();
    assert!(check(0, &error).is_ok());
    assert_eq!(
        native_error(&error, 17).to_string(),
        "native operation failed with status 17"
    );
    assert_eq!(native_error(&error, 17).code(), 17);
    assert_eq!(native_error(&error, 17).kind(), ErrorKind::Internal);

    let mut error = NativeError::new();
    error.0[..15].copy_from_slice(b"native failure\0");
    let checked = check(3, &error).expect_err("nonzero status must fail");
    assert_eq!(checked.to_string(), "native failure");
    assert_eq!(checked.code(), 3);
    assert_eq!(checked.kind(), ErrorKind::ResourceExhausted);

    let valid = cstring("field").expect("ordinary strings are portable");
    assert_eq!(valid.to_bytes(), b"field");
    let nul = cstring("bad\0field").expect_err("interior NUL must be rejected");
    assert!(nul.to_string().contains("interior NUL"));

    let valid_path =
        path_cstring(std::path::Path::new("config")).expect("ordinary paths are portable");
    assert_eq!(valid_path.to_bytes(), b"config");
    let invalid_path = path_cstring(std::path::Path::new("bad\0path"))
        .expect_err("interior NUL in a path must be rejected");
    assert!(invalid_path.to_string().contains("interior NUL"));
}

#[test]
fn boundary_helpers_enforce_native_size_and_pointer_contracts() {
    assert_eq!(length_u32(0, "items").unwrap(), 0);
    assert_eq!(length_u32(u32::MAX as usize, "items").unwrap(), u32::MAX);
    let overflow =
        length_u32(u32::MAX as usize + 1, "items").expect_err("values above u32 must be rejected");
    assert!(overflow.to_string().contains("items count"));

    assert!(require_ptr::<u8>(std::ptr::null_mut(), "result").is_err());
    let mut value = 1_u8;
    assert!(require_ptr(&mut value, "result").is_ok());
}

#[test]
fn errors_preserve_codes_kinds_and_messages() {
    for (code, kind) in [
        (1, ErrorKind::InvalidArgument),
        (3, ErrorKind::ResourceExhausted),
        (6, ErrorKind::Type),
        (77, ErrorKind::Internal),
    ] {
        let error = Error::native(code, None, "native failure");
        assert_eq!(error.code(), code);
        assert_eq!(error.kind(), kind);
        assert_eq!(error.to_string(), "native failure");
    }

    let local = Error::message("wrapper failure");
    assert_eq!(local.code(), 0);
    assert_eq!(local.kind(), ErrorKind::InvalidArgument);
    assert_eq!(local.to_string(), "wrapper failure");

    let nul = std::ffi::CString::new(b"bad\0value".as_slice()).expect_err("interior NUL");
    let nul_error = Error::from_nul(nul);
    assert_eq!(nul_error.kind(), ErrorKind::InvalidArgument);
    assert!(nul_error.to_string().contains("interior NUL"));
}

#[test]
fn native_error_decodes_stable_detail_code_without_using_message_text() {
    let mut error = NativeError::new();
    let diagnostic = b"SK_DETAIL_V1:facet_key_type_not_supported:unsupported";
    error.0[..diagnostic.len()].copy_from_slice(diagnostic);
    let decoded = check(4, &error).expect_err("native unsupported status must fail");
    assert_eq!(decoded.kind(), ErrorKind::Unsupported);
    assert_eq!(decoded.detail_code(), Some("facet_key_type_not_supported"));
    assert_eq!(decoded.to_string(), "unsupported");
}

#[test]
fn fluent_requests_and_attribute_builders_preserve_configuration() {
    let ascending = SortField::ascending("title");
    let descending = SortField::new("score", SortOrder::Descending);
    assert_eq!(ascending.order, SortOrder::Ascending);
    assert_eq!(descending.field, "score");

    let search = SearchRequest::new(Query::True)
        .rank_profile("title_rank")
        .limit(4)
        .offset(2)
        .sort(ascending.clone())
        .fields(["title", "body"])
        .timeout(Duration::from_millis(200));
    assert_eq!(search.rank_profile, "title_rank");
    assert_eq!(search.limit, 4);
    assert_eq!(search.offset, 2);
    assert_eq!(search.sort, vec![ascending]);
    assert_eq!(
        search.summary,
        SummaryRequest::Fields(vec!["title".into(), "body".into()])
    );
    assert_eq!(search.timeout, Duration::from_millis(200));

    let search = SearchRequest::new(Query::True)
        .rank_profile("local_rank")
        .limit(3)
        .offset(1)
        .sort(descending.clone())
        .summary(SummaryRequest::Default);
    assert_eq!(search.rank_profile, "local_rank");
    assert_eq!(search.limit, 3);
    assert_eq!(search.offset, 1);
    assert_eq!(search.sort, vec![descending]);
    assert_eq!(search.summary, SummaryRequest::Default);

    let yql = SearchRequest::from_yql("select * from typea where title contains \"fox\"")
        .expect("supported YQL should parse");
    assert_eq!(yql.source, Source::Named("typea".into()));
    assert_eq!(yql.query, Query::term("title", "fox"));

    let document = Document::new("id:ns:type::1").field("rating", 4_i64);
    assert_eq!(document.id(), "id:ns:type::1");
    assert_eq!(document.fields().len(), 1);

    let update = crate::DocumentUpdate::new("id:ns:type::1").assign("rating", 5_i64);
    assert_eq!(update.id(), "id:ns:type::1");
    assert_eq!(update.fields().len(), 1);
}

#[test]
fn ranking_request_inputs_are_validated_before_native_calls() {
    let query_feature = SearchRequest::new(Query::True).query_feature("boost", 2.0);
    validate_search_ranking_inputs(&query_feature).expect("finite query feature");

    let rank_property = SearchRequest::new(Query::True).rank_property("model", "v1");
    validate_search_ranking_inputs(&rank_property).expect("non-empty rank property");

    let collision = SearchRequest::new(Query::True)
        .query_feature("boost", 2.0)
        .rank_property("boost", "3");
    let error = validate_search_ranking_inputs(&collision)
        .expect_err("query-feature/rank-property collisions must be rejected");
    assert!(error.to_string().contains("collision"));

    let tensor_feature_collision = SearchRequest::new(Query::True)
        .query_tensor(
            "embedding",
            TensorValue::dense("tensor<float>(x[1])", [1.0]),
        )
        .query_feature("embedding", 2.0);
    let error = validate_search_ranking_inputs(&tensor_feature_collision)
        .expect_err("query tensor/query-feature collisions must be rejected");
    assert!(error.to_string().contains("collision"));

    let tensor_rank_property_collision = SearchRequest::new(Query::True)
        .query_tensor(
            "embedding",
            TensorValue::dense("tensor<float>(x[1])", [1.0]),
        )
        .rank_property("embedding", "3");
    let error = validate_search_ranking_inputs(&tensor_rank_property_collision)
        .expect_err("query tensor/rank-property collisions must be rejected");
    assert!(error.to_string().contains("collision"));

    let reserved =
        SearchRequest::new(Query::True).rank_property("vespa.matching.numthreadspersearch", "2");
    let error = validate_search_ranking_inputs(&reserved)
        .expect_err("Search Kernel-owned rank properties must be rejected");
    assert!(error.to_string().contains("reserved"));

    let reserved =
        SearchRequest::new(Query::True).query_feature("vespa.matching.numthreadspersearch", 2.0);
    let error = validate_search_ranking_inputs(&reserved)
        .expect_err("Search Kernel-owned query features must be rejected");
    assert!(error.to_string().contains("reserved"));
}

#[test]
fn document_analysis_converts_to_native_soa_occurrences() {
    let analysis = DocumentAnalysis {
        terms: vec![DocumentTerm {
            original: "fox".to_owned(),
            term: "fox".to_owned(),
            byte_start: 5,
            byte_len: 3,
            position: 2,
        }],
    };

    let occurrences = text_occurrences(&analysis).expect("valid occurrence");
    assert_eq!(occurrences.term_bytes, b"fox");
    assert_eq!(occurrences.term_lengths, [3]);
    assert_eq!(occurrences.byte_starts, [5]);
    assert_eq!(occurrences.byte_lengths, [3]);
}

#[test]
fn query_analysis_converts_one_stem_per_native_term() {
    let analysis = QueryAnalysis {
        terms: vec![QueryTerm {
            stems: vec!["running".to_owned()],
            position: 4,
        }],
    };

    let terms = text_query_terms(&analysis).expect("valid query term");
    assert_eq!(terms.term_bytes, b"running");
    assert_eq!(terms.term_lengths, [7]);
}

#[test]
fn native_lengths_reject_u32_overflow() {
    if usize::BITS > 32 {
        assert!(length_u32(u32::MAX as usize + 1, "test").is_err());
    }
}

#[test]
fn query_conversion_rejects_zero_stems() {
    let analysis = QueryAnalysis {
        terms: vec![QueryTerm {
            stems: Vec::new(),
            position: 0,
        }],
    };

    assert!(text_query_terms(&analysis).is_err());
}

#[test]
fn query_conversion_rejects_multiple_stems() {
    let analysis = QueryAnalysis {
        terms: vec![QueryTerm {
            stems: vec!["first".to_owned(), "second".to_owned()],
            position: 0,
        }],
    };

    assert!(text_query_terms(&analysis).is_err());
}

#[test]
fn term_queries_require_one_analyzed_position() {
    let no_terms = QueryAnalysis { terms: Vec::new() };
    assert!(validate_term_query(&no_terms).is_err());

    let multiple = QueryAnalysis {
        terms: vec![
            QueryTerm {
                stems: vec!["one".to_owned()],
                position: 0,
            },
            QueryTerm {
                stems: vec!["two".to_owned()],
                position: 1,
            },
        ],
    };
    assert!(validate_term_query(&multiple).is_err());

    let one = QueryAnalysis {
        terms: vec![QueryTerm {
            stems: vec!["one".to_owned()],
            position: 0,
        }],
    };
    assert!(validate_term_query(&one).is_ok());
}
