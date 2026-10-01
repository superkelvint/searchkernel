use searchkernel_ir::*;
use searchkernel_ir::wire::v1::*;
use searchkernel_ir::wire::v1::schema as api;
use std::collections::BTreeMap;

fn index_context() -> IndexContext {
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let name = builder.create_string("brand");
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
    let filterable = api::OptionalBool::new(true);
    let matching = api::MatchingConfig::create(
        &mut builder,
        &api::MatchingConfigArgs {
            mode: api::MatchingMode::Exact,
            ..Default::default()
        },
    );
    let field = api::FieldDefinition::create(
        &mut builder,
        &api::FieldDefinitionArgs {
            name: Some(name),
            data_type: Some(data_type),
            filterable: Some(&filterable),
            matching: Some(matching),
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
    let definition =
        searchkernel_ir::__flatbuffers::root::<api::IndexDefinition>(builder.finished_data())
            .expect("valid filter expansion schema");
    IndexContext::from_api(definition).expect("valid filter expansion context")
}

fn strings(prefix: &str, count: usize) -> Value {
    Value::List(
        (0..count)
            .map(|i| Value::String(format!("{prefix}-{i}")))
            .collect(),
    )
}

fn rust_request(source: String, bindings: BTreeMap<String, Value>) -> SearchInput {
    SearchInput {
        query: QuerySourceInput::MatchAll,
        search_fields: None,
        filters: Vec::new(),
        textual_filters: vec![TextualFilterInput { source, bindings }],
        sorts: Vec::new(),
        grouping: Vec::new(),
        ranking: None,
        query_inputs: BTreeMap::new(),
        rank_properties: BTreeMap::new(),
        query_model: QueryModelInput::default(),
        projection: None,
        highlighting: None,
        presentation: PresentationInput::default(),
        properties: BTreeMap::new(),
        query_profile: None,
        search_chain: None,
        timeout: None,
        limit: None,
        offset: None,
    }
}

fn string_list_value<'a>(
    builder: &mut searchkernel_ir::__flatbuffers::FlatBufferBuilder<'a>,
    prefix: &str,
    count: usize,
) -> searchkernel_ir::__flatbuffers::WIPOffset<api::Value<'a>> {
    let values = (0..count)
        .map(|i| {
            let text = builder.create_string(&format!("{prefix}-{i}"));
            let value =
                api::StringValue::create(builder, &api::StringValueArgs { value: Some(text) });
            api::Value::create(
                builder,
                &api::ValueArgs {
                    kind_type: api::ValueKind::StringValue,
                    kind: Some(value.as_union_value()),
                },
            )
        })
        .collect::<Vec<_>>();
    let values = builder.create_vector(&values);
    let list = api::ValueList::create(
        builder,
        &api::ValueListArgs {
            values: Some(values),
        },
    );
    api::Value::create(
        builder,
        &api::ValueArgs {
            kind_type: api::ValueKind::ValueList,
            kind: Some(list.as_union_value()),
        },
    )
}

fn portable_request(source: &str, bindings: &[(String, usize)]) -> Vec<u8> {
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let entries = bindings
        .iter()
        .map(|(name, count)| {
            let key = builder.create_string(name);
            let value = string_list_value(&mut builder, name, *count);
            api::ValueEntry::create(
                &mut builder,
                &api::ValueEntryArgs {
                    key: Some(key),
                    value: Some(value),
                },
            )
        })
        .collect::<Vec<_>>();
    let entries = builder.create_vector(&entries);
    let source = builder.create_string(source);
    let clause = api::FilterClause::create(
        &mut builder,
        &api::FilterClauseArgs {
            source: Some(source),
            bindings: Some(entries),
        },
    );
    let textual = builder.create_vector(&[clause]);
    let filters = api::FilterRequest::create(
        &mut builder,
        &api::FilterRequestArgs {
            textual: Some(textual),
            ..Default::default()
        },
    );
    let match_all = api::MatchAllQuery::create(&mut builder, &api::MatchAllQueryArgs {});
    let query = api::QuerySource::create(
        &mut builder,
        &api::QuerySourceArgs {
            source_type: api::QuerySourceKind::MatchAllQuery,
            source: Some(match_all.as_union_value()),
        },
    );
    let request = api::SearchRequest::create(
        &mut builder,
        &api::SearchRequestArgs {
            query: Some(query),
            filters: Some(filters),
            ..Default::default()
        },
    );
    builder.finish(request, None);
    builder.finished_data().to_vec()
}

fn repeated_source(name: &str, count: usize) -> String {
    std::iter::repeat_n(format!("brand IN (:{name})"), count)
        .collect::<Vec<_>>()
        .join(" OR ")
}

fn distinct_source(count: usize) -> String {
    (0..count)
        .map(|i| format!("brand IN (:p{i})"))
        .collect::<Vec<_>>()
        .join(" OR ")
}

fn portable_result(context: &IndexContext, bytes: &[u8]) -> Result<SearchRequestIr> {
    let request = searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(bytes)
        .expect("portable filter expansion request decodes");
    context.search_to_ir(request)
}

fn assert_expansion_error(error: &PortableError) {
    assert_eq!(error.code, ErrorCode::InvalidArgument);
    let detail = &error.details[0];
    assert_eq!(detail.code, "expression_too_complex");
    assert_eq!(detail.path.as_deref(), Some("filters.textual[0].source"));
    assert_eq!(
        detail.metadata.get("limit").map(String::as_str),
        Some("InValues")
    );
}

#[test]
fn repeated_list_parameter_references_are_bounded_across_rust_and_portable_ingress() {
    let context = index_context();
    let per_list = searchkernel_filter::limits::MAX_IN_VALUES;
    let aggregate = searchkernel_filter::limits::MAX_TOKENS;
    assert_eq!(aggregate % per_list, 0);
    let exact_references = aggregate / per_list;

    let exact_source = repeated_source("brands", exact_references);
    let exact_bindings = BTreeMap::from([("brands".to_owned(), strings("brands", per_list))]);
    let rust = resolve_search(&context, rust_request(exact_source.clone(), exact_bindings))
        .expect("aggregate expansion at the token budget must resolve");
    let portable = portable_result(
        &context,
        &portable_request(&exact_source, &[("brands".to_owned(), per_list)]),
    )
    .expect("portable aggregate expansion at the token budget must resolve");
    assert_eq!(rust, portable);

    let over_source = repeated_source("brands", exact_references + 1);
    let over_bindings = BTreeMap::from([("brands".to_owned(), strings("brands", per_list))]);
    let rust_error =
        resolve_search(&context, rust_request(over_source.clone(), over_bindings)).unwrap_err();
    let portable_error = portable_result(
        &context,
        &portable_request(&over_source, &[("brands".to_owned(), per_list)]),
    )
    .unwrap_err();
    assert_eq!(rust_error, portable_error);
    assert_expansion_error(&rust_error);
}

#[test]
fn multiple_list_parameters_enforce_exact_aggregate_boundary_and_one_over() {
    let context = index_context();
    let per_list = searchkernel_filter::limits::MAX_IN_VALUES;
    let aggregate = searchkernel_filter::limits::MAX_TOKENS;
    assert_eq!(aggregate % per_list, 0);
    let full_lists = aggregate / per_list;

    let exact_source = distinct_source(full_lists);
    let exact_bindings = (0..full_lists)
        .map(|i| (format!("p{i}"), strings(&format!("p{i}"), per_list)))
        .collect::<BTreeMap<_, _>>();
    let exact_portable = (0..full_lists)
        .map(|i| (format!("p{i}"), per_list))
        .collect::<Vec<_>>();
    let rust = resolve_search(&context, rust_request(exact_source.clone(), exact_bindings))
        .expect("distinct parameter expansion at the aggregate budget must resolve");
    let portable = portable_result(&context, &portable_request(&exact_source, &exact_portable))
        .expect("portable distinct expansion at the aggregate budget must resolve");
    assert_eq!(rust, portable);

    let over_source = distinct_source(full_lists + 1);
    let mut over_bindings = (0..full_lists)
        .map(|i| (format!("p{i}"), strings(&format!("p{i}"), per_list)))
        .collect::<BTreeMap<_, _>>();
    over_bindings.insert(format!("p{full_lists}"), strings("overflow", 1));
    let mut over_portable = (0..full_lists)
        .map(|i| (format!("p{i}"), per_list))
        .collect::<Vec<_>>();
    over_portable.push((format!("p{full_lists}"), 1));

    let rust_error =
        resolve_search(&context, rust_request(over_source.clone(), over_bindings)).unwrap_err();
    let portable_error =
        portable_result(&context, &portable_request(&over_source, &over_portable)).unwrap_err();
    assert_eq!(rust_error, portable_error);
    assert_expansion_error(&rust_error);
}

#[test]
fn aggregate_budget_is_shared_across_nested_boolean_branches() {
    let context = index_context();
    let per_list = searchkernel_filter::limits::MAX_IN_VALUES;
    let aggregate = searchkernel_filter::limits::MAX_TOKENS;
    assert_eq!(aggregate % per_list, 0);
    let exact_references = aggregate / per_list;
    let left_references = exact_references / 2;
    let right_references = exact_references - left_references;

    let exact_source = format!(
        "({}) AND NOT ({})",
        repeated_source("brands", left_references),
        repeated_source("brands", right_references)
    );
    let exact_bindings = BTreeMap::from([("brands".to_owned(), strings("brands", per_list))]);
    resolve_search(&context, rust_request(exact_source, exact_bindings))
        .expect("nested aggregate expansion at the token budget must resolve");

    let over_source = format!(
        "({}) AND NOT ({})",
        repeated_source("brands", left_references),
        repeated_source("brands", right_references + 1)
    );
    let over_bindings = BTreeMap::from([("brands".to_owned(), strings("brands", per_list))]);
    let error = resolve_search(&context, rust_request(over_source, over_bindings))
        .expect_err("nested boolean branches must share one aggregate expansion budget");
    assert_expansion_error(&error);
}
