include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../core/crates/searchkernel-ir/tests/semantics.rs"
));

fn context(searchable: Option<&[&str]>) -> searchkernel_ir::IndexContext {
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let primitive = api::PrimitiveDataType::create(
        &mut builder,
        &api::PrimitiveDataTypeArgs {
            kind: api::PrimitiveDataTypeKind::String,
        },
    );
    let data_type = api::DataType::create(
        &mut builder,
        &api::DataTypeArgs {
            kind_type: api::DataTypeKind::PrimitiveDataType,
            kind: Some(primitive.as_union_value()),
        },
    );
    let fields = searchable
        .unwrap_or(&[])
        .iter()
        .map(|name| {
            let name = builder.create_string(name);
            let searchable = api::OptionalBool::new(true);
            api::FieldDefinition::create(
                &mut builder,
                &api::FieldDefinitionArgs {
                    name: Some(name),
                    data_type: Some(data_type),
                    searchable: Some(&searchable),
                    ..Default::default()
                },
            )
        })
        .collect::<Vec<_>>();
    let fields = builder.create_vector(&fields);
    let definition = api::IndexDefinition::create(
        &mut builder,
        &api::IndexDefinitionArgs {
            fields: Some(fields),
            ..Default::default()
        },
    );
    builder.finish(definition, None);
    let definition =
        searchkernel_ir::__flatbuffers::root::<api::IndexDefinition>(builder.finished_data())
            .expect("valid index definition");
    searchkernel_ir::IndexContext::from_api(definition).expect("valid index context")
}

fn timeout_request_bytes(timeout: Option<u64>) -> Vec<u8> {
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let text = builder.create_string("query");
    let query = api::SearchKernelTextQuery::create(
        &mut builder,
        &api::SearchKernelTextQueryArgs { text: Some(text) },
    );
    let query = api::QuerySource::create(
        &mut builder,
        &api::QuerySourceArgs {
            source_type: api::QuerySourceKind::SearchKernelTextQuery,
            source: Some(query.as_union_value()),
        },
    );
    let timeout = timeout.map(api::OptionalUInt64::new);
    let request = api::SearchRequest::create(
        &mut builder,
        &api::SearchRequestArgs {
            query: Some(query),
            timeout_ms: timeout.as_ref(),
            ..Default::default()
        },
    );
    builder.finish(request, None);
    builder.finished_data().to_vec()
}

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
