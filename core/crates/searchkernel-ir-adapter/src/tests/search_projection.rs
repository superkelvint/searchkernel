use super::*;

#[test]
fn empty_projection_lowers_to_no_summary_fields() {
    let request = request(
        vec![SearchFieldIr::new(
            field("title"),
            SearchFieldWeight::new(1.0).unwrap(),
        )],
        Vec::new(),
        None,
    );
    let language = analysis_options(request.language()).unwrap();
    let query = lower_search_query(&request, language).unwrap();
    let lowered = lower_search_options(&request, query, language).unwrap();

    assert_eq!(
        lowered.summary,
        searchkernel_vespa_engine::SummaryRequest::None
    );
}

#[test]
fn summary_class_lowers_independently_from_projection() {
    let title = field("title");
    let base = SearchRequestIr::new(
        QueryIr::Text("wireless headphones".into()),
        vec![SearchFieldIr::new(
            title.clone(),
            SearchFieldWeight::new(1.0).unwrap(),
        )],
        None,
        Vec::new(),
        20,
        0,
        ProjectionIr::new(vec![title]),
        HighlightingIr::Disabled,
        None,
        PrefixPolicy::Disabled,
        TypoPolicy::Disabled,
    );
    let language = analysis_options(base.language()).unwrap();
    let query = lower_search_query(&base, language).unwrap();

    let default_class = lower_search_options(&base, query.clone(), language).unwrap();
    assert_eq!(default_class.summary_class, None);
    assert_eq!(
        default_class.summary,
        searchkernel_vespa_engine::SummaryRequest::Fields(vec!["title".into()])
    );

    for name in ["default", "some-class"] {
        let request = base.clone().with_presentation(PresentationIr {
            summary: Some(name.to_owned()),
            ..PresentationIr::default()
        });
        let lowered = lower_search_options(&request, query.clone(), language).unwrap();
        assert_eq!(lowered.summary_class.as_deref(), Some(name));
        assert_eq!(
            lowered.summary,
            searchkernel_vespa_engine::SummaryRequest::Fields(vec!["title".into()])
        );
    }
}

#[test]
fn summary_class_survives_explicit_empty_projection_without_fetching_docsums() {
    let request = request(
        vec![SearchFieldIr::new(
            field("title"),
            SearchFieldWeight::new(1.0).unwrap(),
        )],
        Vec::new(),
        None,
    )
    .with_presentation(PresentationIr {
        summary: Some("some-class".to_owned()),
        ..PresentationIr::default()
    });
    let language = analysis_options(request.language()).unwrap();
    let query = lower_search_query(&request, language).unwrap();
    let lowered = lower_search_options(&request, query, language).unwrap();

    assert_eq!(lowered.summary_class.as_deref(), Some("some-class"));
    assert_eq!(
        lowered.summary,
        searchkernel_vespa_engine::SummaryRequest::None
    );
}

#[test]
fn named_summary_and_highlighting_lower_together() {
    let title = field("title");
    let request = SearchRequestIr::new(
        QueryIr::Text("brown fox".into()),
        vec![SearchFieldIr::new(
            title.clone(),
            SearchFieldWeight::new(1.0).unwrap(),
        )],
        None,
        Vec::new(),
        10,
        0,
        ProjectionIr::new(vec![title.clone()]),
        HighlightingIr::Requested {
            fields: vec![title.clone()],
            multi_valued_fields: Vec::new(),
        },
        None,
        PrefixPolicy::Disabled,
        TypoPolicy::Disabled,
    )
    .with_presentation(PresentationIr {
        summary: Some("custom".to_owned()),
        ..PresentationIr::default()
    });
    let language = analysis_options(request.language()).unwrap();
    let query = lower_search_query(&request, language).unwrap();
    let lowered = lower_search_options(&request, query, language).unwrap();

    assert_eq!(lowered.summary_class.as_deref(), Some("custom"));
    assert_eq!(
        lowered.summary,
        searchkernel_vespa_engine::SummaryRequest::Fields(vec!["title".into()])
    );
    assert_eq!(lowered.highlight_fields, vec!["title"]);
}

#[test]
fn highlighting_compatibility_survives_portable_flatbuffers_resolution() {
    let title = field("title");
    let request = SearchRequestIr::new(
        typed_contains(
            "title",
            QueryExprKindIr::Fuzzy {
                value: RequestValueIr::String("runnng".to_owned()),
                max_edit_distance: 2,
                prefix_length: 0,
                prefix: false,
            },
        ),
        Vec::new(),
        None,
        Vec::new(),
        10,
        0,
        ProjectionIr::new(Vec::new()),
        HighlightingIr::Requested {
            fields: vec![title.clone()],
            multi_valued_fields: Vec::new(),
        },
        None,
        PrefixPolicy::Disabled,
        TypoPolicy::Disabled,
    );
    let definition = IndexDefinitionIr::new(
        None,
        vec![FieldIr::new(
            title,
            DataTypeIr::Primitive(PrimitiveDataTypeIr::String),
            None,
            FieldCapabilities::new(true, true, false, false, false),
        )],
        Vec::new(),
    )
    .expect("highlighting parity definition");

    let bytes = searchkernel_ir::wire::v1::search_request_ir_to_api(&request);
    let api_request = searchkernel_ir::__flatbuffers::root::<
        searchkernel_ir::wire::v1::schema::SearchRequest,
    >(&bytes)
    .expect("encoded highlighting request");
    let portable = IndexContext::from_definition_ir(definition)
        .search_to_ir(api_request)
        .expect("portable highlighting request should resolve");
    assert_eq!(portable, request);

    for candidate in [&request, &portable] {
        let error = validate_search_request(candidate)
            .expect_err("same-field fuzzy highlighting must be rejected consistently");
        assert_eq!(error.code, ErrorCode::Unsupported);
        assert_eq!(error.details[0].code, "highlighting_query_not_supported");
    }
}
