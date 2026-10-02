use super::*;

#[test]
fn structured_grouping_validation_is_shared_between_ingresses() {
    let definition = scripted_definition();
    let context = context_for(&definition);
    let grouping = || GroupingPlan::new().level(GroupingPlan::by("brand"));

    let recording = Arc::new(RecordingBackend::new());
    let (_runtime, index) = runtime_index(definition.clone(), recording.clone());
    assert!(index
        .browse()
        .grouping(grouping())
        .execute::<serde_json::Value>()
        .is_err());
    let OperationIr::Search(rust_request) = recording.last_call().unwrap() else {
        panic!("valid grouping should reach the recording backend");
    };
    let bytes = portable_grouping_request("brand");
    let request = searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&bytes).unwrap();
    let portable_request = searchkernel_ir::wire::v1::search_to_ir(&context, request).unwrap();
    assert_eq!(rust_request, portable_request);

    let recording = Arc::new(RecordingBackend::new());
    let (_runtime, index) = runtime_index(definition.clone(), recording);
    let rust_error = index
        .browse()
        .grouping(GroupingPlan::new().level(GroupingPlan::by("")))
        .execute::<serde_json::Value>()
        .unwrap_err();
    let bytes = portable_grouping_request("");
    let request = searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&bytes).unwrap();
    let portable_error = searchkernel_ir::wire::v1::search_to_ir(&context, request).unwrap_err();
    assert_eq!(rust_error.code(), crate::ErrorCode::InvalidArgument);
    assert_eq!(
        portable_error.code,
        searchkernel_ir::ErrorCode::InvalidArgument
    );
    assert_eq!(rust_error.details()[0].code, portable_error.details[0].code);
    assert_eq!(rust_error.details()[0].path, portable_error.details[0].path);
}

#[test]
fn public_facets_validate_schema_fields_and_duplicates_in_search_resolution() {
    let definition = IndexDefinition::new()
        .field(Field::text("title").stored(true))
        .field(Field::keyword("brand").facetable(true));
    let (_runtime, index) = runtime_index(definition, Arc::new(RecordingBackend::new()));

    for (facets, detail_code) in [
        (vec!["does_not_exist"], "unknown_field"),
        (vec!["title"], "field_not_facetable"),
        (vec!["brand", "brand"], "duplicate_facet_field"),
    ] {
        let error = index
            .search("x")
            .search_fields(["title"])
            .facets(facets)
            .inspect()
            .expect_err("invalid facet semantics must fail before backend execution");
        assert_eq!(error.details()[0].code, detail_code);
    }
}

#[test]
fn public_structured_grouping_validates_schema_fields_in_search_resolution() {
    let definition = IndexDefinition::new()
        .field(Field::text("title").stored(true))
        .field(Field::keyword("brand").facetable(true));
    let context = context_for(&definition);
    let (_runtime, index) = runtime_index(definition, Arc::new(RecordingBackend::new()));

    for (field, detail_code) in [
        ("does_not_exist", "unknown_field"),
        ("title", "field_not_facetable"),
    ] {
        let error = index
            .browse()
            .grouping(GroupingPlan::new().level(GroupingPlan::by(field)))
            .inspect()
            .expect_err("invalid grouping fields must fail before backend execution");
        assert_eq!(error.details()[0].code, detail_code);

        let bytes = portable_grouping_request(field);
        let request = searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&bytes).unwrap();
        let portable_error = searchkernel_ir::wire::v1::search_to_ir(&context, request)
            .expect_err("portable grouping fields must use the same schema validation");
        assert_eq!(portable_error.details[0].code, detail_code);
    }
}
