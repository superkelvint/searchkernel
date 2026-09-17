use searchkernel_ir::{api, *};
use std::collections::BTreeMap;

fn definition_bytes(
    default_fields: Option<&[&str]>,
    cardinality: api::FieldCardinality,
) -> Vec<u8> {
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let name = builder.create_string("title");
    let stored = api::OptionalBool::new(true);
    let searchable = api::OptionalBool::new(true);
    let filterable = api::OptionalBool::new(false);
    let sortable = api::OptionalBool::new(false);
    let facetable = api::OptionalBool::new(false);
    let field = api::FieldDefinition::create(
        &mut builder,
        &api::FieldDefinitionArgs {
            name: Some(name),
            kind: api::FieldKind::Text,
            cardinality,
            stored: Some(&stored),
            searchable: Some(&searchable),
            filterable: Some(&filterable),
            sortable: Some(&sortable),
            facetable: Some(&facetable),
            analyzer: None,
        },
    );
    let fields = builder.create_vector(&[field]);
    let default_search_fields = default_fields.map(|names| {
        let search_fields = names
            .iter()
            .map(|name| {
                let field = builder.create_string(name);
                let weight = api::OptionalDouble::new(1.0);
                api::SearchField::create(
                    &mut builder,
                    &api::SearchFieldArgs {
                        field: Some(field),
                        weight: Some(&weight),
                    },
                )
            })
            .collect::<Vec<_>>();
        let search_fields = builder.create_vector(&search_fields);
        api::SearchFieldSelection::create(
            &mut builder,
            &api::SearchFieldSelectionArgs {
                fields: Some(search_fields),
            },
        )
    });
    let root = api::IndexDefinition::create(
        &mut builder,
        &api::IndexDefinitionArgs {
            name: None,
            fields: Some(fields),
            default_search_fields,
        },
    );
    builder.finish(root, None);
    builder.finished_data().to_vec()
}

fn context(default_fields: Option<&[&str]>) -> IndexContext {
    let bytes = definition_bytes(default_fields, api::FieldCardinality::Single);
    let definition = searchkernel_ir::__flatbuffers::root::<api::IndexDefinition>(&bytes).unwrap();
    IndexContext::from_api(definition).unwrap()
}

#[allow(clippy::too_many_arguments)]
fn custom_definition_bytes(
    name: &str,
    kind: api::FieldKind,
    cardinality: api::FieldCardinality,
    stored: Option<bool>,
    searchable: Option<bool>,
    filterable: Option<bool>,
    sortable: Option<bool>,
    facetable: Option<bool>,
    analyzer: Option<(&str, u32)>,
) -> Vec<u8> {
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let field_name = builder.create_string(name);
    let stored_value = stored.map(api::OptionalBool::new);
    let searchable_value = searchable.map(api::OptionalBool::new);
    let filterable_value = filterable.map(api::OptionalBool::new);
    let sortable_value = sortable.map(api::OptionalBool::new);
    let facetable_value = facetable.map(api::OptionalBool::new);
    let analyzer = analyzer.map(|(name, version)| {
        let name = builder.create_string(name);
        api::Analyzer::create(
            &mut builder,
            &api::AnalyzerArgs {
                name: Some(name),
                version,
                options: None,
            },
        )
    });
    let field = api::FieldDefinition::create(
        &mut builder,
        &api::FieldDefinitionArgs {
            name: Some(field_name),
            kind,
            cardinality,
            stored: stored_value.as_ref(),
            searchable: searchable_value.as_ref(),
            filterable: filterable_value.as_ref(),
            sortable: sortable_value.as_ref(),
            facetable: facetable_value.as_ref(),
            analyzer,
        },
    );
    let fields = builder.create_vector(&[field]);
    let root = api::IndexDefinition::create(
        &mut builder,
        &api::IndexDefinitionArgs {
            name: None,
            fields: Some(fields),
            default_search_fields: None,
        },
    );
    builder.finish(root, None);
    builder.finished_data().to_vec()
}

fn result_code<T: std::fmt::Debug>(result: searchkernel_ir::Result<T>) -> (String, Option<String>) {
    let error = result.expect_err("expected a portable error");
    let detail = error.details.first().expect("expected an error detail");
    (detail.code.clone(), detail.path.clone())
}

fn root_definition(bytes: &[u8]) -> api::IndexDefinition<'_> {
    searchkernel_ir::__flatbuffers::root::<api::IndexDefinition>(bytes).unwrap()
}

fn definition_inner_vector_absent_bytes() -> Vec<u8> {
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let name = builder.create_string("title");
    let field = api::FieldDefinition::create(
        &mut builder,
        &api::FieldDefinitionArgs {
            name: Some(name),
            kind: api::FieldKind::Text,
            cardinality: api::FieldCardinality::Single,
            stored: Some(&api::OptionalBool::new(true)),
            searchable: Some(&api::OptionalBool::new(true)),
            filterable: Some(&api::OptionalBool::new(false)),
            sortable: Some(&api::OptionalBool::new(false)),
            facetable: Some(&api::OptionalBool::new(false)),
            analyzer: None,
        },
    );
    let fields = builder.create_vector(&[field]);
    let selection = api::SearchFieldSelection::create(
        &mut builder,
        &api::SearchFieldSelectionArgs { fields: None },
    );
    let root = api::IndexDefinition::create(
        &mut builder,
        &api::IndexDefinitionArgs {
            name: None,
            fields: Some(fields),
            default_search_fields: Some(selection),
        },
    );
    builder.finish(root, None);
    builder.finished_data().to_vec()
}

fn request_bytes(
    search_fields: Option<&[&str]>,
    projection: Option<&[&str]>,
    highlighting: Option<&[&str]>,
) -> Vec<u8> {
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let query = builder.create_string("");
    let search_fields = search_fields.map(|names| {
        let fields = names
            .iter()
            .map(|name| {
                let field = builder.create_string(name);
                let weight = api::OptionalDouble::new(1.0);
                api::SearchField::create(
                    &mut builder,
                    &api::SearchFieldArgs {
                        field: Some(field),
                        weight: Some(&weight),
                    },
                )
            })
            .collect::<Vec<_>>();
        let fields = builder.create_vector(&fields);
        api::SearchFieldSelection::create(
            &mut builder,
            &api::SearchFieldSelectionArgs {
                fields: Some(fields),
            },
        )
    });
    let projection = projection.map(|names| {
        let fields = names
            .iter()
            .map(|name| builder.create_string(name))
            .collect::<Vec<_>>();
        let fields = builder.create_vector(&fields);
        api::FieldProjection::create(
            &mut builder,
            &api::FieldProjectionArgs {
                fields: Some(fields),
            },
        )
    });
    let highlighting = highlighting.map(|names| {
        let fields = names
            .iter()
            .map(|name| builder.create_string(name))
            .collect::<Vec<_>>();
        let fields = builder.create_vector(&fields);
        api::HighlightRequest::create(
            &mut builder,
            &api::HighlightRequestArgs {
                fields: Some(fields),
            },
        )
    });
    let root = api::SearchRequest::create(
        &mut builder,
        &api::SearchRequestArgs {
            query: Some(query),
            search_fields,
            projection,
            highlighting,
            ..Default::default()
        },
    );
    builder.finish(root, None);
    builder.finished_data().to_vec()
}

fn request_inner_vectors_absent_bytes() -> Vec<u8> {
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let query = builder.create_string("");
    let search_fields = api::SearchFieldSelection::create(
        &mut builder,
        &api::SearchFieldSelectionArgs { fields: None },
    );
    let projection =
        api::FieldProjection::create(&mut builder, &api::FieldProjectionArgs { fields: None });
    let highlighting =
        api::HighlightRequest::create(&mut builder, &api::HighlightRequestArgs { fields: None });
    let root = api::SearchRequest::create(
        &mut builder,
        &api::SearchRequestArgs {
            query: Some(query),
            search_fields: Some(search_fields),
            projection: Some(projection),
            highlighting: Some(highlighting),
            ..Default::default()
        },
    );
    builder.finish(root, None);
    builder.finished_data().to_vec()
}

fn timeout_request_bytes(timeout: Option<u64>) -> Vec<u8> {
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let timeout = timeout.map(api::OptionalUInt64::new);
    let root = api::SearchRequest::create(
        &mut builder,
        &api::SearchRequestArgs {
            timeout: timeout.as_ref(),
            ..Default::default()
        },
    );
    builder.finish(root, None);
    builder.finished_data().to_vec()
}

fn none_document_bytes() -> Vec<u8> {
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let id = builder.create_string("doc-1");
    let key = builder.create_string("title");
    let value = api::Value::create(
        &mut builder,
        &api::ValueArgs {
            kind_type: api::ValueKind::NONE,
            kind: None,
        },
    );
    let entry = api::ValueEntry::create(
        &mut builder,
        &api::ValueEntryArgs {
            key: Some(key),
            value: Some(value),
        },
    );
    let fields = builder.create_vector(&[entry]);
    let root = api::Document::create(
        &mut builder,
        &api::DocumentArgs {
            id: Some(id),
            fields: Some(fields),
        },
    );
    builder.finish(root, None);
    builder.finished_data().to_vec()
}

fn rich_field<'a>(
    builder: &mut searchkernel_ir::__flatbuffers::FlatBufferBuilder<'a>,
    name: &str,
    kind: api::FieldKind,
    cardinality: api::FieldCardinality,
) -> searchkernel_ir::__flatbuffers::WIPOffset<api::FieldDefinition<'a>> {
    let name = builder.create_string(name);
    api::FieldDefinition::create(
        builder,
        &api::FieldDefinitionArgs {
            name: Some(name),
            kind,
            cardinality,
            ..Default::default()
        },
    )
}

fn rich_definition_bytes() -> Vec<u8> {
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let fields = [
        rich_field(
            &mut builder,
            "title",
            api::FieldKind::Text,
            api::FieldCardinality::Single,
        ),
        rich_field(
            &mut builder,
            "brand",
            api::FieldKind::Keyword,
            api::FieldCardinality::Single,
        ),
        rich_field(
            &mut builder,
            "price",
            api::FieldKind::Int64,
            api::FieldCardinality::Single,
        ),
        rich_field(
            &mut builder,
            "tags",
            api::FieldKind::Keyword,
            api::FieldCardinality::Multi,
        ),
        rich_field(
            &mut builder,
            "available",
            api::FieldKind::Bool,
            api::FieldCardinality::Single,
        ),
    ];
    let fields = builder.create_vector(&fields);
    let name = builder.create_string("products");
    let root = api::IndexDefinition::create(
        &mut builder,
        &api::IndexDefinitionArgs {
            name: Some(name),
            fields: Some(fields),
            default_search_fields: None,
        },
    );
    builder.finish(root, None);
    builder.finished_data().to_vec()
}

fn string_value<'a>(
    builder: &mut searchkernel_ir::__flatbuffers::FlatBufferBuilder<'a>,
    value: &str,
) -> searchkernel_ir::__flatbuffers::WIPOffset<api::Value<'a>> {
    let value = builder.create_string(value);
    let value = api::StringValue::create(builder, &api::StringValueArgs { value: Some(value) });
    api::Value::create(
        builder,
        &api::ValueArgs {
            kind_type: api::ValueKind::StringValue,
            kind: Some(value.as_union_value()),
        },
    )
}

fn int_value<'a>(
    builder: &mut searchkernel_ir::__flatbuffers::FlatBufferBuilder<'a>,
    value: i64,
) -> searchkernel_ir::__flatbuffers::WIPOffset<api::Value<'a>> {
    let value = api::Int64Value::create(builder, &api::Int64ValueArgs { value });
    api::Value::create(
        builder,
        &api::ValueArgs {
            kind_type: api::ValueKind::Int64Value,
            kind: Some(value.as_union_value()),
        },
    )
}

fn float_value<'a>(
    builder: &mut searchkernel_ir::__flatbuffers::FlatBufferBuilder<'a>,
    value: f64,
) -> searchkernel_ir::__flatbuffers::WIPOffset<api::Value<'a>> {
    let value = api::Float64Value::create(builder, &api::Float64ValueArgs { value });
    api::Value::create(
        builder,
        &api::ValueArgs {
            kind_type: api::ValueKind::Float64Value,
            kind: Some(value.as_union_value()),
        },
    )
}

fn uint_value<'a>(
    builder: &mut searchkernel_ir::__flatbuffers::FlatBufferBuilder<'a>,
    value: u64,
) -> searchkernel_ir::__flatbuffers::WIPOffset<api::Value<'a>> {
    let value = api::UInt64Value::create(builder, &api::UInt64ValueArgs { value });
    api::Value::create(
        builder,
        &api::ValueArgs {
            kind_type: api::ValueKind::UInt64Value,
            kind: Some(value.as_union_value()),
        },
    )
}

enum StandaloneValue {
    Float(f64),
    UInt(u64),
    Bool(bool),
}

fn standalone_value_bytes(value: StandaloneValue) -> Vec<u8> {
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let value = match value {
        StandaloneValue::Float(value) => float_value(&mut builder, value),
        StandaloneValue::UInt(value) => uint_value(&mut builder, value),
        StandaloneValue::Bool(value) => bool_value(&mut builder, value),
    };
    builder.finish(value, None);
    builder.finished_data().to_vec()
}

fn bool_value<'a>(
    builder: &mut searchkernel_ir::__flatbuffers::FlatBufferBuilder<'a>,
    value: bool,
) -> searchkernel_ir::__flatbuffers::WIPOffset<api::Value<'a>> {
    let value = api::BoolValue::create(builder, &api::BoolValueArgs { value });
    api::Value::create(
        builder,
        &api::ValueArgs {
            kind_type: api::ValueKind::BoolValue,
            kind: Some(value.as_union_value()),
        },
    )
}

fn rich_document_bytes() -> Vec<u8> {
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let id = builder.create_string("sku-1");
    let mut entries = Vec::new();
    for (name, value) in [
        ("title", string_value(&mut builder, "headphones")),
        ("brand", string_value(&mut builder, "Acme")),
        ("price", int_value(&mut builder, 42)),
        ("available", bool_value(&mut builder, true)),
    ] {
        let key = builder.create_string(name);
        entries.push(api::ValueEntry::create(
            &mut builder,
            &api::ValueEntryArgs {
                key: Some(key),
                value: Some(value),
            },
        ));
    }
    let tag_values = [
        string_value(&mut builder, "audio"),
        string_value(&mut builder, "sale"),
    ];
    let tag_values = builder.create_vector(&tag_values);
    let tags = api::ValueList::create(
        &mut builder,
        &api::ValueListArgs {
            values: Some(tag_values),
        },
    );
    let tags = api::Value::create(
        &mut builder,
        &api::ValueArgs {
            kind_type: api::ValueKind::ValueList,
            kind: Some(tags.as_union_value()),
        },
    );
    let tags_key = builder.create_string("tags");
    entries.push(api::ValueEntry::create(
        &mut builder,
        &api::ValueEntryArgs {
            key: Some(tags_key),
            value: Some(tags),
        },
    ));
    let fields = builder.create_vector(&entries);
    let root = api::Document::create(
        &mut builder,
        &api::DocumentArgs {
            id: Some(id),
            fields: Some(fields),
        },
    );
    builder.finish(root, None);
    builder.finished_data().to_vec()
}

fn rich_search_request_bytes() -> Vec<u8> {
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let query = builder.create_string("headphones");
    let field = builder.create_string("price");
    let comparison_value = int_value(&mut builder, 10);
    let comparison = api::FilterComparison::create(
        &mut builder,
        &api::FilterComparisonArgs {
            field: Some(field),
            op: api::ComparisonOp::Gte,
            value: Some(comparison_value),
        },
    );
    let filter = api::FilterExpr::create(
        &mut builder,
        &api::FilterExprArgs {
            expr_type: api::FilterExprKind::FilterComparison,
            expr: Some(comparison.as_union_value()),
        },
    );
    let sort_field = builder.create_string("price");
    let sort = api::SortExpr::create(
        &mut builder,
        &api::SortExprArgs {
            field: Some(sort_field),
            direction: api::SortDirection::Unspecified,
            missing: None,
        },
    );
    let sort = builder.create_vector(&[sort]);
    let facet_field = builder.create_string("brand");
    let facet = api::FacetRequest::create(
        &mut builder,
        &api::FacetRequestArgs {
            field: Some(facet_field),
            limit: None,
            sort: None,
        },
    );
    let facets = builder.create_vector(&[facet]);
    let ranking = api::Bm25Ranking::create(&mut builder, &api::Bm25RankingArgs {});
    let ranking = api::RankingPolicy::create(
        &mut builder,
        &api::RankingPolicyArgs {
            policy_type: api::RankingPolicyKind::Bm25Ranking,
            policy: Some(ranking.as_union_value()),
        },
    );
    let limit = api::OptionalUInt32::new(0);
    let offset = api::OptionalUInt32::new(5);
    let language = builder.create_string("EN-us");
    let root = api::SearchRequest::create(
        &mut builder,
        &api::SearchRequestArgs {
            query: Some(query),
            filter: Some(filter),
            sort: Some(sort),
            facets: Some(facets),
            ranking: Some(ranking),
            limit: Some(&limit),
            offset: Some(&offset),
            language: Some(language),
            ..Default::default()
        },
    );
    builder.finish(root, None);
    builder.finished_data().to_vec()
}

#[allow(dead_code)]
fn all_kinds_definition() -> IndexDefinitionIr {
    let names = ["text", "keyword", "int", "float", "bool"]
        .into_iter()
        .map(|name| FieldName::parse(name).unwrap())
        .collect::<Vec<_>>();
    let fields = vec![
        FieldIr::new(
            names[0].clone(),
            FieldKind::Text,
            FieldCardinality::Single,
            Some(AnalyzerIdentity::StandardV1),
            FieldCapabilities::new(true, true, false, false, false),
        ),
        FieldIr::new(
            names[1].clone(),
            FieldKind::Keyword,
            FieldCardinality::Multi,
            None,
            FieldCapabilities::new(true, false, true, false, true),
        ),
        FieldIr::new(
            names[2].clone(),
            FieldKind::Int64,
            FieldCardinality::Single,
            None,
            FieldCapabilities::new(true, false, true, true, false),
        ),
        FieldIr::new(
            names[3].clone(),
            FieldKind::Float64,
            FieldCardinality::Single,
            None,
            FieldCapabilities::new(true, false, true, true, false),
        ),
        FieldIr::new(
            names[4].clone(),
            FieldKind::Bool,
            FieldCardinality::Single,
            None,
            FieldCapabilities::new(true, false, true, true, true),
        ),
    ];
    IndexDefinitionIr::new(
        Some(IndexName::parse("all-kinds").unwrap()),
        fields,
        vec![SearchFieldIr::new(
            names[0].clone(),
            SearchFieldWeight::new(0.0).unwrap(),
        )],
        RankingPolicyIr::Bm25,
        PrefixPolicy::Disabled,
        TypoPolicy::Disabled,
    )
    .unwrap()
}

fn query_term_offset<'a>(
    builder: &mut searchkernel_ir::__flatbuffers::FlatBufferBuilder<'a>,
    field_name: &str,
    text: &str,
) -> searchkernel_ir::__flatbuffers::WIPOffset<api::QueryExpr<'a>> {
    let field = builder.create_string(field_name);
    let value = string_value(builder, text);
    let term = api::TermQuery::create(
        builder,
        &api::TermQueryArgs {
            field: Some(field),
            value: Some(value),
        },
    );
    api::QueryExpr::create(
        builder,
        &api::QueryExprArgs {
            expr_type: api::QueryExprKind::TermQuery,
            expr: Some(term.as_union_value()),
        },
    )
}

fn filter_comparison_offset<'a>(
    builder: &mut searchkernel_ir::__flatbuffers::FlatBufferBuilder<'a>,
    field_name: &str,
    value: i64,
) -> searchkernel_ir::__flatbuffers::WIPOffset<api::FilterExpr<'a>> {
    filter_comparison_offset_op(builder, field_name, value, api::ComparisonOp::Gte)
}

fn filter_comparison_offset_op<'a>(
    builder: &mut searchkernel_ir::__flatbuffers::FlatBufferBuilder<'a>,
    field_name: &str,
    value: i64,
    op: api::ComparisonOp,
) -> searchkernel_ir::__flatbuffers::WIPOffset<api::FilterExpr<'a>> {
    let field = builder.create_string(field_name);
    let value = int_value(builder, value);
    let comparison = api::FilterComparison::create(
        builder,
        &api::FilterComparisonArgs {
            field: Some(field),
            op,
            value: Some(value),
        },
    );
    api::FilterExpr::create(
        builder,
        &api::FilterExprArgs {
            expr_type: api::FilterExprKind::FilterComparison,
            expr: Some(comparison.as_union_value()),
        },
    )
}

#[test]
fn owned_ir_types_have_final_names() {
    let definition = index_definition_to_ir(root_definition(&rich_definition_bytes())).unwrap();
    assert_eq!(definition.fields()[0].name().as_str(), "title");
}

#[test]
fn ordinary_result_values_are_owned() {
    let value = Value::Object(std::collections::BTreeMap::from([(
        "title".to_owned(),
        Value::String("headphones".to_owned()),
    )]));
    let cloned = value.clone();
    drop(value);
    assert!(matches!(cloned, Value::Object(_)));
}

#[test]
fn model_validators_cover_boundary_values() {
    for invalid in ["", "_title", "title-name", "title name", "1title"] {
        assert!(FieldName::parse(invalid).is_err(), "accepted {invalid:?}");
    }
    for valid in ["title", "Title_2", "x9"] {
        assert!(FieldName::parse(valid).is_ok(), "rejected {valid:?}");
    }
    assert!(DocumentId::parse("").is_err());
    assert!(IndexName::parse("").is_err());
    assert!(LanguageHint::parse("").is_err());
    assert!(LanguageHint::parse("e").is_err());
    assert_eq!(LanguageHint::parse("EN-us").unwrap().as_str(), "en-us");
    assert!(SearchFieldWeight::new(-1.0).is_err());
    assert!(SearchFieldWeight::new(f64::NAN).is_err());
    assert!(SearchFieldWeight::new(f64::INFINITY).is_err());
    assert_eq!(SearchFieldWeight::new(0.0).unwrap().get(), 0.0);
    assert_eq!(FieldName::parse("title").unwrap().to_string(), "title");
    assert_eq!(DocumentId::parse("doc-1").unwrap().to_string(), "doc-1");
    assert_eq!(
        IndexName::parse("products").unwrap().to_string(),
        "products"
    );
    assert_eq!(LanguageHint::parse("EN-us").unwrap().as_str(), "en-us");
}

#[test]
fn model_accessors_and_error_display_preserve_owned_values() {
    let field_name = FieldName::parse("title").unwrap();
    let capabilities = FieldCapabilities::new(true, true, false, true, false);
    assert!(capabilities.stored());
    assert!(capabilities.searchable());
    assert!(!capabilities.filterable());
    assert!(capabilities.sortable());
    assert!(!capabilities.facetable());

    let field = FieldIr::new(
        field_name.clone(),
        FieldKind::Text,
        FieldCardinality::Single,
        Some(AnalyzerIdentity::StandardV1),
        capabilities,
    );
    assert_eq!(field.name().as_str(), "title");
    assert_eq!(field.kind(), FieldKind::Text);
    assert_eq!(field.cardinality(), FieldCardinality::Single);
    assert_eq!(field.analyzer(), Some(AnalyzerIdentity::StandardV1));
    assert_eq!(field.capabilities(), &capabilities);

    let weight = SearchFieldWeight::new(2.5).unwrap();
    let search_field = SearchFieldIr::new(field_name.clone(), weight);
    assert_eq!(search_field.field().as_str(), "title");
    assert_eq!(search_field.weight().get(), 2.5);

    let definition = IndexDefinitionIr::new(
        Some(IndexName::parse("products").unwrap()),
        vec![field],
        vec![search_field],
        RankingPolicyIr::Bm25,
        PrefixPolicy::Disabled,
        TypoPolicy::Disabled,
    )
    .unwrap();
    assert_eq!(definition.name().unwrap().as_str(), "products");
    assert_eq!(definition.fields().len(), 1);
    assert_eq!(definition.default_search_fields().len(), 1);
    assert_eq!(definition.default_ranking(), RankingPolicyIr::Bm25);
    assert_eq!(definition.default_prefix_policy(), PrefixPolicy::Disabled);
    assert_eq!(definition.default_typo_policy(), TypoPolicy::Disabled);

    let sort = SortIr::new(
        field_name.clone(),
        SortDirection::Desc,
        MissingPlacement::Last,
    );
    assert_eq!(sort.field().as_str(), "title");
    assert_eq!(sort.direction(), SortDirection::Desc);
    assert_eq!(sort.missing(), MissingPlacement::Last);
    let facet = FacetIr::new(field_name.clone(), 7, FacetSort::ValueDesc);
    assert_eq!(facet.field().as_str(), "title");
    assert_eq!(facet.limit(), 7);
    assert_eq!(facet.sort(), FacetSort::ValueDesc);
    let comparison = ComparisonIr::new(
        field_name.clone(),
        ComparisonOp::Gte,
        ScalarValueIr::Int64(42),
    );
    assert_eq!(comparison.field().as_str(), "title");
    assert_eq!(comparison.op(), ComparisonOp::Gte);
    assert_eq!(comparison.value(), &ScalarValueIr::Int64(42));
    let range = RangeIr::new(field_name, Some(ScalarValueIr::Int64(1)), None, true, false);
    assert_eq!(range.lower(), Some(&ScalarValueIr::Int64(1)));
    assert!(range.upper().is_none());
    assert!(range.include_lower());
    assert!(!range.include_upper());

    let error = PortableError::new(ErrorCode::Conflict, "conflict").with_detail(
        "duplicate",
        Some("document.id"),
        "document already exists",
    );
    assert_eq!(error.to_string(), "conflict");
    assert_eq!(error.details.len(), 1);
    assert_eq!(error.details[0].path.as_deref(), Some("document.id"));
}

#[test]
fn schema_defaults_cover_all_field_kinds() {
    let cases = [
        (
            api::FieldKind::Text,
            FieldKind::Text,
            true,
            true,
            false,
            false,
            false,
        ),
        (
            api::FieldKind::Keyword,
            FieldKind::Keyword,
            true,
            false,
            true,
            true,
            true,
        ),
        (
            api::FieldKind::Int64,
            FieldKind::Int64,
            true,
            false,
            true,
            true,
            false,
        ),
        (
            api::FieldKind::Float64,
            FieldKind::Float64,
            true,
            false,
            true,
            true,
            false,
        ),
        (
            api::FieldKind::Bool,
            FieldKind::Bool,
            true,
            false,
            true,
            true,
            true,
        ),
    ];
    for (wire_kind, kind, stored, searchable, filterable, sortable, facetable) in cases {
        let bytes = custom_definition_bytes(
            "field",
            wire_kind,
            api::FieldCardinality::Single,
            None,
            None,
            None,
            None,
            None,
            None,
        );
        let definition = index_definition_to_ir(root_definition(&bytes)).unwrap();
        let field = &definition.fields()[0];
        assert_eq!(field.kind(), kind);
        assert_eq!(field.capabilities().stored(), stored);
        assert_eq!(field.capabilities().searchable(), searchable);
        assert_eq!(field.capabilities().filterable(), filterable);
        assert_eq!(field.capabilities().sortable(), sortable);
        assert_eq!(field.capabilities().facetable(), facetable);
    }
}

#[test]
fn schema_rejects_invalid_names_capabilities_and_analyzers() {
    let cases = [
        ("", "invalid_field_name", "fields[0].name"),
        ("title-name", "invalid_field_name", "fields[0].name"),
    ];
    for (name, code, path) in cases {
        let bytes = custom_definition_bytes(
            name,
            api::FieldKind::Text,
            api::FieldCardinality::Single,
            None,
            None,
            None,
            None,
            None,
            None,
        );
        let (actual_code, actual_path) =
            result_code(index_definition_to_ir(root_definition(&bytes)));
        assert_eq!(
            (actual_code, actual_path),
            (code.to_owned(), Some(path.to_owned()))
        );
    }

    let invalid_capabilities = [
        (
            api::FieldKind::Text,
            api::FieldCardinality::Single,
            (None, None, Some(true), None, None),
            "fields[0].filterable",
        ),
        (
            api::FieldKind::Keyword,
            api::FieldCardinality::Single,
            (None, Some(true), None, None, None),
            "fields[0].searchable",
        ),
        (
            api::FieldKind::Keyword,
            api::FieldCardinality::Multi,
            (None, None, None, Some(true), None),
            "fields[0].sortable",
        ),
    ];
    for (kind, cardinality, (stored, searchable, filterable, sortable, facetable), path) in
        invalid_capabilities
    {
        let bytes = custom_definition_bytes(
            "field",
            kind,
            cardinality,
            stored,
            searchable,
            filterable,
            sortable,
            facetable,
            None,
        );
        let (code, actual_path) = result_code(index_definition_to_ir(root_definition(&bytes)));
        assert_eq!(code, "invalid_capability");
        assert_eq!(actual_path.as_deref(), Some(path));
    }

    let bytes = custom_definition_bytes(
        "title",
        api::FieldKind::Text,
        api::FieldCardinality::Single,
        None,
        None,
        None,
        None,
        None,
        Some(("other", 1)),
    );
    let (code, path) = result_code(index_definition_to_ir(root_definition(&bytes)));
    assert_eq!(code, "unsupported_analyzer");
    assert_eq!(path.as_deref(), Some("fields[0].analyzer"));
}

#[test]
fn schema_rejects_empty_duplicate_and_unknown_field_definitions() {
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let fields =
        builder
            .create_vector::<searchkernel_ir::__flatbuffers::ForwardsUOffset<api::FieldDefinition>>(
                &[],
            );
    let root = api::IndexDefinition::create(
        &mut builder,
        &api::IndexDefinitionArgs {
            fields: Some(fields),
            ..Default::default()
        },
    );
    builder.finish(root, None);
    let bytes = builder.finished_data().to_vec();
    let (code, path) = result_code(index_definition_to_ir(root_definition(&bytes)));
    assert_eq!(code, "invalid_schema");
    assert_eq!(path.as_deref(), Some("fields"));

    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let first = rich_field(
        &mut builder,
        "title",
        api::FieldKind::Text,
        api::FieldCardinality::Single,
    );
    let second = rich_field(
        &mut builder,
        "title",
        api::FieldKind::Text,
        api::FieldCardinality::Single,
    );
    let fields = builder.create_vector(&[first, second]);
    let root = api::IndexDefinition::create(
        &mut builder,
        &api::IndexDefinitionArgs {
            fields: Some(fields),
            ..Default::default()
        },
    );
    builder.finish(root, None);
    let bytes = builder.finished_data().to_vec();
    let (code, path) = result_code(index_definition_to_ir(root_definition(&bytes)));
    assert_eq!(code, "duplicate_field");
    assert_eq!(path.as_deref(), Some("fields[1].name"));

    for (kind, code) in [
        (api::FieldKind::Unspecified, "invalid_field_kind"),
        (api::FieldKind(9), "invalid_field_kind"),
    ] {
        let bytes = custom_definition_bytes(
            "field",
            kind,
            api::FieldCardinality::Single,
            None,
            None,
            None,
            None,
            None,
            None,
        );
        let (actual, _) = result_code(index_definition_to_ir(root_definition(&bytes)));
        assert_eq!(actual, code);
    }
    let bytes = custom_definition_bytes(
        "field",
        api::FieldKind::Text,
        api::FieldCardinality(9),
        None,
        None,
        None,
        None,
        None,
        None,
    );
    let (code, path) = result_code(index_definition_to_ir(root_definition(&bytes)));
    assert_eq!(code, "invalid_cardinality");
    assert_eq!(path.as_deref(), Some("fields[0].cardinality"));
}

#[test]
fn scalar_conversion_covers_numeric_validation_and_type_errors() {
    let float_definition = custom_definition_bytes(
        "value",
        api::FieldKind::Float64,
        api::FieldCardinality::Single,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    let float_context = IndexContext::from_api(root_definition(&float_definition)).unwrap();
    let bytes = standalone_value_bytes(StandaloneValue::Float(1.5));
    let value = searchkernel_ir::__flatbuffers::root::<api::Value>(&bytes).unwrap();
    assert!(matches!(
        float_context.value_to_ir("value", value).unwrap(),
        FieldValueIr::Single(ScalarValueIr::Float64(value)) if value == 1.5
    ));

    let bytes = standalone_value_bytes(StandaloneValue::Float(f64::NAN));
    let value = searchkernel_ir::__flatbuffers::root::<api::Value>(&bytes).unwrap();
    let (code, path) = result_code(float_context.value_to_ir("value", value));
    assert_eq!(code, "non_finite_number");
    assert_eq!(path.as_deref(), Some("value"));

    let text_context = context(None);
    let bytes = standalone_value_bytes(StandaloneValue::UInt(4));
    let value = searchkernel_ir::__flatbuffers::root::<api::Value>(&bytes).unwrap();
    let (code, path) = result_code(text_context.value_to_ir("title", value));
    assert_eq!(code, "type_mismatch");
    assert_eq!(path.as_deref(), Some("value"));

    let bytes = standalone_value_bytes(StandaloneValue::Bool(true));
    let value = searchkernel_ir::__flatbuffers::root::<api::Value>(&bytes).unwrap();
    let (code, path) = result_code(text_context.value_to_ir("title", value));
    assert_eq!(code, "type_mismatch");
    assert_eq!(path.as_deref(), Some("value"));
}

#[test]
fn malformed_value_list_union_is_missing_value() {
    let context = IndexContext::from_api(root_definition(&rich_definition_bytes())).unwrap();
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let value = api::Value::create(
        &mut builder,
        &api::ValueArgs {
            kind_type: api::ValueKind::ValueList,
            kind: None,
        },
    );
    builder.finish(value, None);
    let bytes = builder.finished_data().to_vec();
    // This deliberately bypasses verification so the converter can classify
    // the malformed union payload instead of rejecting the buffer first.
    let value = unsafe { searchkernel_ir::__flatbuffers::root_unchecked::<api::Value>(&bytes) };

    let (code, path) = result_code(context.value_to_ir("tags", value));
    assert_eq!(code, "missing_value");
    assert_eq!(path.as_deref(), Some("value"));
}

#[test]
fn analyze_and_crud_operations_validate_presence_and_targets() {
    let definition = rich_definition_bytes();
    let context = IndexContext::from_api(root_definition(&definition)).unwrap();
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let analyzer_name = builder.create_string("standard");
    let analyzer = api::Analyzer::create(
        &mut builder,
        &api::AnalyzerArgs {
            name: Some(analyzer_name),
            version: 1,
            options: None,
        },
    );
    let text = builder.create_string("Headphones");
    let request = api::AnalyzeRequest::create(
        &mut builder,
        &api::AnalyzeRequestArgs {
            target_type: api::AnalyzeTarget::Analyzer,
            target: Some(analyzer.as_union_value()),
            text: Some(text),
            language: None,
        },
    );
    builder.finish(request, None);
    let bytes = builder.finished_data().to_vec();
    let request = searchkernel_ir::__flatbuffers::root::<api::AnalyzeRequest>(&bytes).unwrap();
    assert_eq!(context.analyze_to_ir(request).unwrap().text(), "Headphones");

    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let field = builder.create_string("title");
    let target = api::AnalyzeFieldTarget::create(
        &mut builder,
        &api::AnalyzeFieldTargetArgs { field: Some(field) },
    );
    let request = api::AnalyzeRequest::create(
        &mut builder,
        &api::AnalyzeRequestArgs {
            target_type: api::AnalyzeTarget::AnalyzeFieldTarget,
            target: Some(target.as_union_value()),
            ..Default::default()
        },
    );
    builder.finish(request, None);
    let bytes = builder.finished_data().to_vec();
    let request = searchkernel_ir::__flatbuffers::root::<api::AnalyzeRequest>(&bytes).unwrap();
    assert_eq!(
        context.analyze_to_ir(request).unwrap().analyzer(),
        AnalyzerIdentity::StandardV1
    );

    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let insert =
        api::InsertRequest::create(&mut builder, &api::InsertRequestArgs { document: None });
    builder.finish(insert, None);
    let bytes = builder.finished_data().to_vec();
    let insert = searchkernel_ir::__flatbuffers::root::<api::InsertRequest>(&bytes).unwrap();
    let (code, path) = result_code(context.insert_to_ir(insert));
    assert_eq!(code, "missing_value");
    assert_eq!(path.as_deref(), Some("document"));

    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let delete = api::DeleteRequest::create(&mut builder, &api::DeleteRequestArgs { id: None });
    builder.finish(delete, None);
    let bytes = builder.finished_data().to_vec();
    let delete = searchkernel_ir::__flatbuffers::root::<api::DeleteRequest>(&bytes).unwrap();
    let (code, path) = result_code(context.delete_to_ir(delete));
    assert_eq!(code, "invalid_document_id");
    assert_eq!(path.as_deref(), Some("id"));
}

#[test]
fn request_validation_rejects_bad_search_fields_projection_and_highlighting() {
    let definition = rich_definition_bytes();
    let context = IndexContext::from_api(root_definition(&definition)).unwrap();

    for (fields, code, path) in [
        (
            &["brand"][..],
            "field_not_searchable",
            "search_fields.fields[0].field",
        ),
        (
            &["title", "title"][..],
            "duplicate_search_field",
            "search_fields.fields[1].field",
        ),
    ] {
        let bytes = request_bytes(Some(fields), None, None);
        let request = searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&bytes).unwrap();
        let (actual_code, actual_path) = result_code(context.search_to_ir(request));
        assert_eq!(actual_code, code);
        assert_eq!(actual_path.as_deref(), Some(path));
    }

    let bytes = request_bytes(None, Some(&["unknown"]), None);
    let request = searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&bytes).unwrap();
    let (code, path) = result_code(context.search_to_ir(request));
    assert_eq!(code, "unknown_field");
    assert_eq!(path.as_deref(), Some("projection.fields[0]"));

    let bytes = request_bytes(None, None, Some(&["brand"]));
    let request = searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&bytes).unwrap();
    let (code, path) = result_code(context.search_to_ir(request));
    assert_eq!(code, "invalid_highlight");
    assert_eq!(path.as_deref(), Some("highlighting.fields[0]"));

    let bytes = request_bytes(None, None, Some(&["title", "title"]));
    let request = searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&bytes).unwrap();
    let (code, path) = result_code(context.search_to_ir(request));
    assert_eq!(code, "duplicate_highlight_field");
    assert_eq!(path.as_deref(), Some("highlighting.fields[1]"));
}

#[test]
fn patch_validation_covers_unset_empty_duplicate_and_missing_values() {
    let definition = rich_definition_bytes();
    let context = IndexContext::from_api(root_definition(&definition)).unwrap();

    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let id = builder.create_string("doc-1");
    let request = api::PatchRequest::create(
        &mut builder,
        &api::PatchRequestArgs {
            id: Some(id),
            operations: None,
        },
    );
    builder.finish(request, None);
    let bytes = builder.finished_data().to_vec();
    let request = searchkernel_ir::__flatbuffers::root::<api::PatchRequest>(&bytes).unwrap();
    let (code, path) = result_code(context.patch_to_ir(request));
    assert_eq!(code, "empty_patch");
    assert_eq!(path.as_deref(), Some("operations"));

    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let field = builder.create_string("brand");
    let unset = api::UnsetField::create(&mut builder, &api::UnsetFieldArgs { field: Some(field) });
    let operation = api::PatchOperation::create(
        &mut builder,
        &api::PatchOperationArgs {
            operation_type: api::PatchOperationKind::UnsetField,
            operation: Some(unset.as_union_value()),
        },
    );
    let operations = builder.create_vector(&[operation]);
    let id = builder.create_string("doc-1");
    let request = api::PatchRequest::create(
        &mut builder,
        &api::PatchRequestArgs {
            id: Some(id),
            operations: Some(operations),
        },
    );
    builder.finish(request, None);
    let bytes = builder.finished_data().to_vec();
    let request =
        unsafe { searchkernel_ir::__flatbuffers::root_unchecked::<api::PatchRequest>(&bytes) };
    assert!(matches!(
        context.patch_to_ir(request).unwrap().operations()[0],
        PatchOperationIr::Unset { ref field } if field.as_str() == "brand"
    ));

    // A field may only be mentioned once, regardless of operation kind.
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let field_a = builder.create_string("brand");
    let field_b = builder.create_string("brand");
    let unset_a = api::UnsetField::create(
        &mut builder,
        &api::UnsetFieldArgs {
            field: Some(field_a),
        },
    );
    let unset_b = api::UnsetField::create(
        &mut builder,
        &api::UnsetFieldArgs {
            field: Some(field_b),
        },
    );
    let operation_a = api::PatchOperation::create(
        &mut builder,
        &api::PatchOperationArgs {
            operation_type: api::PatchOperationKind::UnsetField,
            operation: Some(unset_a.as_union_value()),
        },
    );
    let operation_b = api::PatchOperation::create(
        &mut builder,
        &api::PatchOperationArgs {
            operation_type: api::PatchOperationKind::UnsetField,
            operation: Some(unset_b.as_union_value()),
        },
    );
    let operations = builder.create_vector(&[operation_a, operation_b]);
    let id = builder.create_string("doc-1");
    let request = api::PatchRequest::create(
        &mut builder,
        &api::PatchRequestArgs {
            id: Some(id),
            operations: Some(operations),
        },
    );
    builder.finish(request, None);
    let bytes = builder.finished_data().to_vec();
    let request = searchkernel_ir::__flatbuffers::root::<api::PatchRequest>(&bytes).unwrap();
    let (code, path) = result_code(context.patch_to_ir(request));
    assert_eq!(code, "duplicate_patch_field");
    assert_eq!(path.as_deref(), Some("operations[1]"));

    // Set without a value is distinct from a type mismatch.
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let field = builder.create_string("brand");
    let set = api::SetField::create(
        &mut builder,
        &api::SetFieldArgs {
            field: Some(field),
            value: None,
        },
    );
    let operation = api::PatchOperation::create(
        &mut builder,
        &api::PatchOperationArgs {
            operation_type: api::PatchOperationKind::SetField,
            operation: Some(set.as_union_value()),
        },
    );
    let operations = builder.create_vector(&[operation]);
    let id = builder.create_string("doc-1");
    let request = api::PatchRequest::create(
        &mut builder,
        &api::PatchRequestArgs {
            id: Some(id),
            operations: Some(operations),
        },
    );
    builder.finish(request, None);
    let bytes = builder.finished_data().to_vec();
    let request = searchkernel_ir::__flatbuffers::root::<api::PatchRequest>(&bytes).unwrap();
    let (code, path) = result_code(context.patch_to_ir(request));
    assert_eq!(code, "missing_value");
    assert_eq!(path.as_deref(), Some("operations[0].set.value"));

    // A discriminator without its union payload is rejected at the boundary.
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let operation = api::PatchOperation::create(
        &mut builder,
        &api::PatchOperationArgs {
            operation_type: api::PatchOperationKind::SetField,
            operation: None,
        },
    );
    let operations = builder.create_vector(&[operation]);
    let id = builder.create_string("doc-1");
    let request = api::PatchRequest::create(
        &mut builder,
        &api::PatchRequestArgs {
            id: Some(id),
            operations: Some(operations),
        },
    );
    builder.finish(request, None);
    let bytes = builder.finished_data().to_vec();
    let request =
        unsafe { searchkernel_ir::__flatbuffers::root_unchecked::<api::PatchRequest>(&bytes) };
    let (code, path) = result_code(context.patch_to_ir(request));
    assert_eq!(code, "invalid_patch_operation");
    assert_eq!(path.as_deref(), Some("operations[0]"));
}

#[test]
fn reverse_encoding_covers_ir_variants_and_error_codes() {
    let definition = all_kinds_definition();
    let names = definition
        .fields()
        .iter()
        .map(|field| field.name().clone())
        .collect::<Vec<_>>();
    let bytes = index_definition_ir_to_api(&definition);
    let encoded = searchkernel_ir::__flatbuffers::root::<api::IndexDefinition>(&bytes).unwrap();
    assert_eq!(encoded.fields().unwrap().len(), 5);
    assert!(encoded.fields().unwrap().get(0).analyzer().is_some());
    assert_eq!(
        encoded
            .default_search_fields()
            .unwrap()
            .fields()
            .unwrap()
            .get(0)
            .weight()
            .unwrap()
            .value(),
        0.0
    );

    let request = SearchRequestIr::new(
        QueryIr::Or(vec![
            QueryIr::Term {
                field: names[0].clone(),
                value: ScalarValueIr::String("one".to_owned()),
            },
            QueryIr::Not(Box::new(QueryIr::Term {
                field: names[0].clone(),
                value: ScalarValueIr::String("two".to_owned()),
            })),
        ]),
        definition.default_search_fields().to_vec(),
        Some(FilterIr::Or(vec![
            FilterIr::Exists {
                field: names[1].clone(),
            },
            FilterIr::Range(RangeIr::new(
                names[2].clone(),
                Some(ScalarValueIr::Int64(1)),
                None,
                false,
                true,
            )),
        ])),
        vec![SortIr::new(
            names[2].clone(),
            SortDirection::Asc,
            MissingPlacement::Last,
        )],
        vec![],
        RankingPolicyIr::DefaultRelevanceV1,
        20,
        0,
        ProjectionIr::new(names.clone()),
        HighlightingIr::Disabled,
        None,
        PrefixPolicy::Disabled,
        TypoPolicy::Disabled,
    );
    let bytes = search_request_ir_to_api(&request);
    let encoded = searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&bytes).unwrap();
    assert_eq!(
        encoded.query_expr().unwrap().expr_type(),
        api::QueryExprKind::QueryOr
    );
    assert_eq!(
        encoded.filter().unwrap().expr_type(),
        api::FilterExprKind::FilterOr
    );
    assert_eq!(encoded.sort().unwrap().len(), 1);
    assert_eq!(
        encoded.sort().unwrap().get(0).missing().unwrap().value(),
        api::MissingPlacement::Last
    );

    for code in [
        ErrorCode::InvalidArgument,
        ErrorCode::NotFound,
        ErrorCode::AlreadyExists,
        ErrorCode::FailedPrecondition,
        ErrorCode::Conflict,
        ErrorCode::Unsupported,
        ErrorCode::ResourceExhausted,
        ErrorCode::Internal,
        ErrorCode::Unavailable,
    ] {
        let error =
            PortableError::new(code, "failure").with_detail("detail", Some("path"), "explanation");
        let bytes = error_to_api(&error);
        let encoded = searchkernel_ir::__flatbuffers::root::<api::Error>(&bytes).unwrap();
        assert!(encoded.details().is_some());
    }
}

#[test]
fn public_conversion_methods_and_write_dispatch_are_exercised() {
    let definition_bytes = rich_definition_bytes();
    let definition = root_definition(&definition_bytes);
    let context = IndexContext::from_api(definition).unwrap();
    assert_eq!(context.describe().fields().len(), 5);
    let request_bytes = request_bytes(Some(&["title"]), None, None);
    let request =
        searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&request_bytes).unwrap();
    assert!(context.inspect_query(request).is_ok());

    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let id = builder.create_string("doc-1");
    let document = api::Document::create(
        &mut builder,
        &api::DocumentArgs {
            id: Some(id),
            fields: None,
        },
    );
    let insert = api::InsertRequest::create(
        &mut builder,
        &api::InsertRequestArgs {
            document: Some(document),
        },
    );
    builder.finish(insert, None);
    let insert_bytes = builder.finished_data().to_vec();
    let insert = searchkernel_ir::__flatbuffers::root::<api::InsertRequest>(&insert_bytes).unwrap();
    assert!(context.insert_to_ir(insert).is_ok());

    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let id = builder.create_string("doc-1");
    let document = api::Document::create(
        &mut builder,
        &api::DocumentArgs {
            id: Some(id),
            fields: None,
        },
    );
    let upsert = api::UpsertRequest::create(
        &mut builder,
        &api::UpsertRequestArgs {
            document: Some(document),
        },
    );
    builder.finish(upsert, None);
    let upsert_bytes = builder.finished_data().to_vec();
    let upsert = searchkernel_ir::__flatbuffers::root::<api::UpsertRequest>(&upsert_bytes).unwrap();
    assert!(context.upsert_to_ir(upsert).is_ok());

    let field = definition.fields().unwrap().get(0);
    assert!(field_to_ir(field).is_ok());
    assert_eq!(
        analyzer_to_ir(None, FieldKind::Text).unwrap(),
        Some(AnalyzerIdentity::StandardV1)
    );
    assert_eq!(context.query_to_ir("", None).unwrap(), QueryIr::MatchAll);
    assert_eq!(
        context.query_to_ir("text", None).unwrap(),
        QueryIr::Text("text".to_owned())
    );

    let insert = searchkernel_ir::__flatbuffers::root::<api::InsertRequest>(&insert_bytes).unwrap();
    assert!(matches!(
        context.write_to_ir(WriteRequestRef::Insert(insert)),
        Ok(WriteRequestIr::Insert(_))
    ));
    let upsert = searchkernel_ir::__flatbuffers::root::<api::UpsertRequest>(&upsert_bytes).unwrap();
    assert!(matches!(
        context.write_to_ir(WriteRequestRef::Upsert(upsert)),
        Ok(WriteRequestIr::Upsert(_))
    ));
    assert!(context
        .bulk_write_to_ir(Vec::<WriteRequestRef<'_>>::new())
        .unwrap()
        .is_empty());
    let search =
        searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&request_bytes).unwrap();
    assert!(context
        .search_fields_to_ir(search.search_fields().unwrap())
        .is_ok());
}

#[test]
fn wire_presence_is_consumed_into_effective_ir() {
    let derived = context(None);
    assert_eq!(derived.definition().default_search_fields().len(), 1);

    let explicit_empty = context(Some(&[]));
    assert!(explicit_empty
        .definition()
        .default_search_fields()
        .is_empty());

    let request = request_bytes(None, None, None);
    let request = searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&request).unwrap();
    let ir = derived.search_to_ir(request).unwrap();
    assert_eq!(ir.search_fields().len(), 1);
    assert_eq!(ir.projection().fields().len(), 1);
    assert_eq!(ir.highlighting(), &HighlightingIr::Disabled);

    let request = request_bytes(Some(&[]), Some(&[]), Some(&[]));
    let request = searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&request).unwrap();
    let ir = derived.search_to_ir(request).unwrap();
    assert!(ir.search_fields().is_empty());
    assert!(ir.projection().fields().is_empty());
    assert_eq!(
        ir.highlighting(),
        &HighlightingIr::Requested { fields: vec![] }
    );
}

#[test]
fn all_wrapper_wire_states_are_canonicalized() {
    let absent = context(None);
    let empty = context(Some(&[]));
    let inner_absent_bytes = definition_inner_vector_absent_bytes();
    let inner_absent = IndexContext::from_api(
        searchkernel_ir::__flatbuffers::root::<api::IndexDefinition>(&inner_absent_bytes).unwrap(),
    )
    .unwrap();
    assert_eq!(
        empty.definition().default_search_fields(),
        inner_absent.definition().default_search_fields()
    );
    assert_ne!(
        absent.definition().default_search_fields(),
        empty.definition().default_search_fields()
    );

    let absent_request = request_bytes(None, None, None);
    let absent_ir = absent
        .search_to_ir(
            searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&absent_request).unwrap(),
        )
        .unwrap();
    let empty_request = request_bytes(Some(&[]), Some(&[]), Some(&[]));
    let empty_ir = absent
        .search_to_ir(
            searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&empty_request).unwrap(),
        )
        .unwrap();
    let inner_request = request_inner_vectors_absent_bytes();
    let inner_ir = absent
        .search_to_ir(
            searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&inner_request).unwrap(),
        )
        .unwrap();
    assert_eq!(empty_ir.search_fields(), inner_ir.search_fields());
    assert_eq!(empty_ir.projection(), inner_ir.projection());
    assert_eq!(empty_ir.highlighting(), inner_ir.highlighting());
    assert_ne!(absent_ir.projection(), empty_ir.projection());
    assert_ne!(absent_ir.highlighting(), empty_ir.highlighting());

    let values = request_bytes(Some(&["title"]), Some(&["title"]), Some(&["title"]));
    let values_ir = absent
        .search_to_ir(searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&values).unwrap())
        .unwrap();
    assert_ne!(empty_ir.search_fields(), values_ir.search_fields());
    assert_ne!(empty_ir.projection(), values_ir.projection());
    assert_ne!(empty_ir.highlighting(), values_ir.highlighting());
}

#[test]
fn optional_scalars_preserve_absence_and_explicit_defaults() {
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let field_name = builder.create_string("title");
    let weight_zero = api::OptionalDouble::new(0.0);
    let weighted = api::SearchField::create(
        &mut builder,
        &api::SearchFieldArgs {
            field: Some(field_name),
            weight: Some(&weight_zero),
        },
    );
    let field_name = builder.create_string("title");
    let unweighted = api::SearchField::create(
        &mut builder,
        &api::SearchFieldArgs {
            field: Some(field_name),
            weight: None,
        },
    );
    let search_fields = builder.create_vector(&[weighted, unweighted]);
    let selection = api::SearchFieldSelection::create(
        &mut builder,
        &api::SearchFieldSelectionArgs {
            fields: Some(search_fields),
        },
    );
    let limit_zero = api::OptionalUInt32::new(0);
    let offset_zero = api::OptionalUInt32::new(0);
    let sort_field = builder.create_string("title");
    let missing_first = api::OptionalMissingPlacement::new(api::MissingPlacement::First);
    let sort = api::SortExpr::create(
        &mut builder,
        &api::SortExprArgs {
            field: Some(sort_field),
            direction: api::SortDirection::Asc,
            missing: Some(&missing_first),
        },
    );
    let sorts = builder.create_vector(&[sort]);
    let facet_field = builder.create_string("title");
    let facet_limit_zero = api::OptionalUInt32::new(0);
    let facet_sort_value = api::OptionalFacetSort::new(api::FacetSort::ValueAsc);
    let facet = api::FacetRequest::create(
        &mut builder,
        &api::FacetRequestArgs {
            field: Some(facet_field),
            limit: Some(&facet_limit_zero),
            sort: Some(&facet_sort_value),
        },
    );
    let facets = builder.create_vector(&[facet]);
    let query = builder.create_string("");
    let root = api::SearchRequest::create(
        &mut builder,
        &api::SearchRequestArgs {
            query: Some(query),
            search_fields: Some(selection),
            limit: Some(&limit_zero),
            offset: Some(&offset_zero),
            sort: Some(sorts),
            facets: Some(facets),
            ..Default::default()
        },
    );
    builder.finish(root, None);
    let request =
        searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(builder.finished_data())
            .unwrap();
    assert_eq!(request.limit().unwrap().value(), 0);
    assert_eq!(request.offset().unwrap().value(), 0);
    let fields = request.search_fields().unwrap().fields().unwrap();
    assert_eq!(fields.get(0).weight().unwrap().value(), 0.0);
    assert!(fields.get(1).weight().is_none());
    assert_eq!(
        request.sort().unwrap().get(0).missing().unwrap().value(),
        api::MissingPlacement::First
    );
    assert_eq!(request.facets().unwrap().get(0).limit().unwrap().value(), 0);
    assert_eq!(
        request.facets().unwrap().get(0).sort().unwrap().value(),
        api::FacetSort::ValueAsc
    );
}

#[test]
fn ordinary_null_string_is_equivalent_to_empty_string() {
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let absent = api::SearchRequest::create(&mut builder, &api::SearchRequestArgs::default());
    builder.finish(absent, None);
    let absent_bytes = builder.finished_data().to_vec();

    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let empty = builder.create_string("");
    let explicit = api::SearchRequest::create(
        &mut builder,
        &api::SearchRequestArgs {
            query: Some(empty),
            ..Default::default()
        },
    );
    builder.finish(explicit, None);
    let explicit_bytes = builder.finished_data().to_vec();

    let context = context(None);
    let absent_ir = context
        .search_to_ir(
            searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&absent_bytes).unwrap(),
        )
        .unwrap();
    let explicit_ir = context
        .search_to_ir(
            searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&explicit_bytes).unwrap(),
        )
        .unwrap();
    assert_eq!(absent_ir.query(), explicit_ir.query());
}

#[test]
fn ordinary_null_vector_is_equivalent_to_empty_vector() {
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let id = builder.create_string("doc-1");
    let absent = api::Document::create(
        &mut builder,
        &api::DocumentArgs {
            id: Some(id),
            fields: None,
        },
    );
    builder.finish(absent, None);
    let absent_bytes = builder.finished_data().to_vec();

    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let id = builder.create_string("doc-1");
    let fields = builder
        .create_vector::<searchkernel_ir::__flatbuffers::ForwardsUOffset<api::ValueEntry>>(&[]);
    let explicit = api::Document::create(
        &mut builder,
        &api::DocumentArgs {
            id: Some(id),
            fields: Some(fields),
        },
    );
    builder.finish(explicit, None);
    let explicit_bytes = builder.finished_data().to_vec();

    let context = context(None);
    let absent_ir = context
        .document_to_ir(
            searchkernel_ir::__flatbuffers::root::<api::Document>(&absent_bytes).unwrap(),
        )
        .unwrap();
    let explicit_ir = context
        .document_to_ir(
            searchkernel_ir::__flatbuffers::root::<api::Document>(&explicit_bytes).unwrap(),
        )
        .unwrap();
    assert_eq!(absent_ir.fields(), explicit_ir.fields());
}

#[test]
fn none_value_is_missing_for_single_and_multi_fields() {
    let single = context(None);
    let bytes = none_document_bytes();
    let document = searchkernel_ir::__flatbuffers::root::<api::Document>(&bytes).unwrap();
    let error = single.document_to_ir(document).unwrap_err();
    assert_eq!(error.details[0].code, "missing_value");

    let bytes = definition_bytes(None, api::FieldCardinality::Multi);
    let definition = searchkernel_ir::__flatbuffers::root::<api::IndexDefinition>(&bytes).unwrap();
    let multi = IndexContext::from_api(definition).unwrap();
    let bytes = none_document_bytes();
    let document = searchkernel_ir::__flatbuffers::root::<api::Document>(&bytes).unwrap();
    let error = multi.document_to_ir(document).unwrap_err();
    assert_eq!(error.details[0].code, "missing_value");
}

#[test]
fn timeout_preserves_absence_zero_normal_and_u64_boundary_values() {
    let context = context(Some(&["title"]));
    for (wire_value, expected) in [
        (None, None),
        (Some(0), Some(0)),
        (Some(250), Some(250)),
        (Some(u64::MAX), Some(u64::MAX)),
    ] {
        let bytes = timeout_request_bytes(wire_value);
        let request = searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&bytes).unwrap();
        let resolved = context.search_to_ir(request).unwrap();
        assert_eq!(
            resolved.timeout().map(|value| value.as_millis() as u64),
            expected
        );

        let inspected = context.inspect_query(request).unwrap();
        assert_eq!(
            inspected.timeout().map(|value| value.as_millis() as u64),
            wire_value
        );
    }
}

#[test]
fn inspect_query_encodes_the_complete_normalized_request() {
    let field = FieldName::parse("title").unwrap();
    let request = SearchRequestIr::new(
        QueryIr::And(vec![QueryIr::Term {
            field: field.clone(),
            value: ScalarValueIr::String("headphones".to_owned()),
        }]),
        vec![SearchFieldIr::new(
            field.clone(),
            SearchFieldWeight::new(2.0).unwrap(),
        )],
        Some(FilterIr::Comparison(ComparisonIr::new(
            field.clone(),
            ComparisonOp::Eq,
            ScalarValueIr::String("headphones".to_owned()),
        ))),
        vec![SortIr::new(
            field.clone(),
            SortDirection::Desc,
            MissingPlacement::First,
        )],
        vec![FacetIr::new(field.clone(), 5, FacetSort::ValueAsc)],
        RankingPolicyIr::Bm25,
        7,
        3,
        ProjectionIr::new(vec![field.clone()]),
        HighlightingIr::Requested {
            fields: vec![field.clone()],
        },
        Some(LanguageHint::parse("EN-us").unwrap()),
        PrefixPolicy::Disabled,
        TypoPolicy::Disabled,
    );
    let bytes = inspect_query_result_to_api(&request);
    let result = searchkernel_ir::__flatbuffers::root::<api::InspectQueryResult>(&bytes).unwrap();
    let request = result.resolved_request().unwrap();
    assert_eq!(request.query().unwrap(), "");
    assert_eq!(request.search_fields().unwrap().fields().unwrap().len(), 1);
    assert!(request.filter().is_some());
    assert_eq!(request.sort().unwrap().len(), 1);
    assert_eq!(request.facets().unwrap().len(), 1);
    assert_eq!(
        request.ranking().unwrap().policy_type(),
        api::RankingPolicyKind::Bm25Ranking
    );
    assert_eq!(request.limit().unwrap().value(), 7);
    assert_eq!(request.offset().unwrap().value(), 3);
    assert_eq!(request.projection().unwrap().fields().unwrap().len(), 1);
    assert_eq!(request.highlighting().unwrap().fields().unwrap().len(), 1);
    assert_eq!(request.language().unwrap(), "en-us");
    assert_eq!(
        request.query_expr().unwrap().expr_type(),
        api::QueryExprKind::QueryAnd
    );
}

#[test]
fn structured_queries_validate_and_flatten() {
    let definition = rich_definition_bytes();
    let context = IndexContext::from_api(root_definition(&definition)).unwrap();

    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let first = query_term_offset(&mut builder, "title", "phones");
    let second = query_term_offset(&mut builder, "title", "audio");
    let clauses = builder.create_vector(&[first, second]);
    let all = api::QueryAnd::create(
        &mut builder,
        &api::QueryAndArgs {
            clauses: Some(clauses),
        },
    );
    let expression = api::QueryExpr::create(
        &mut builder,
        &api::QueryExprArgs {
            expr_type: api::QueryExprKind::QueryAnd,
            expr: Some(all.as_union_value()),
        },
    );
    let request = api::SearchRequest::create(
        &mut builder,
        &api::SearchRequestArgs {
            query_expr: Some(expression),
            ..Default::default()
        },
    );
    builder.finish(request, None);
    let bytes = builder.finished_data().to_vec();
    let request = searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&bytes).unwrap();
    let ir = context.search_to_ir(request).unwrap();
    assert!(matches!(ir.query(), QueryIr::And(clauses) if clauses.len() == 2));

    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let term = query_term_offset(&mut builder, "title", "phones");
    let negation = api::QueryNot::create(&mut builder, &api::QueryNotArgs { clause: Some(term) });
    let expression = api::QueryExpr::create(
        &mut builder,
        &api::QueryExprArgs {
            expr_type: api::QueryExprKind::QueryNot,
            expr: Some(negation.as_union_value()),
        },
    );
    let request = api::SearchRequest::create(
        &mut builder,
        &api::SearchRequestArgs {
            query_expr: Some(expression),
            ..Default::default()
        },
    );
    builder.finish(request, None);
    let bytes = builder.finished_data().to_vec();
    let request = searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&bytes).unwrap();
    assert!(matches!(
        context.search_to_ir(request).unwrap().query(),
        QueryIr::Not(_)
    ));
}

#[test]
fn structured_query_errors_preserve_contract_paths() {
    let definition = rich_definition_bytes();
    let context = IndexContext::from_api(root_definition(&definition)).unwrap();
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let term = query_term_offset(&mut builder, "brand", "Acme");
    let clauses = builder.create_vector(&[term]);
    let all = api::QueryAnd::create(
        &mut builder,
        &api::QueryAndArgs {
            clauses: Some(clauses),
        },
    );
    let expression = api::QueryExpr::create(
        &mut builder,
        &api::QueryExprArgs {
            expr_type: api::QueryExprKind::QueryAnd,
            expr: Some(all.as_union_value()),
        },
    );
    let query = builder.create_string("legacy");
    let request = api::SearchRequest::create(
        &mut builder,
        &api::SearchRequestArgs {
            query: Some(query),
            query_expr: Some(expression),
            ..Default::default()
        },
    );
    builder.finish(request, None);
    let bytes = builder.finished_data().to_vec();
    let request = searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&bytes).unwrap();
    let (code, path) = result_code(context.search_to_ir(request));
    assert_eq!(code, "invalid_query");
    assert_eq!(path.as_deref(), Some("query"));
}

#[test]
fn filters_cover_ranges_boolean_flattening_and_defaults() {
    let definition = rich_definition_bytes();
    let context = IndexContext::from_api(root_definition(&definition)).unwrap();
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let comparison = filter_comparison_offset(&mut builder, "price", 10);
    let nested = builder.create_vector(&[comparison]);
    let nested = api::FilterAnd::create(
        &mut builder,
        &api::FilterAndArgs {
            clauses: Some(nested),
        },
    );
    let filter = api::FilterExpr::create(
        &mut builder,
        &api::FilterExprArgs {
            expr_type: api::FilterExprKind::FilterAnd,
            expr: Some(nested.as_union_value()),
        },
    );
    let request = api::SearchRequest::create(
        &mut builder,
        &api::SearchRequestArgs {
            filter: Some(filter),
            ..Default::default()
        },
    );
    builder.finish(request, None);
    let bytes = builder.finished_data().to_vec();
    let request = searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&bytes).unwrap();
    assert!(matches!(
        context.search_to_ir(request).unwrap().filter(),
        Some(FilterIr::Comparison(_))
    ));

    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let lower = int_value(&mut builder, 10);
    let upper = int_value(&mut builder, 20);
    let range_field = builder.create_string("price");
    let range = api::FilterRange::create(
        &mut builder,
        &api::FilterRangeArgs {
            field: Some(range_field),
            lower: Some(lower),
            upper: Some(upper),
            include_lower: None,
            include_upper: None,
        },
    );
    let filter = api::FilterExpr::create(
        &mut builder,
        &api::FilterExprArgs {
            expr_type: api::FilterExprKind::FilterRange,
            expr: Some(range.as_union_value()),
        },
    );
    let request = api::SearchRequest::create(
        &mut builder,
        &api::SearchRequestArgs {
            filter: Some(filter),
            ..Default::default()
        },
    );
    builder.finish(request, None);
    let bytes = builder.finished_data().to_vec();
    let request = searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&bytes).unwrap();
    let ir = context.search_to_ir(request).unwrap();
    let Some(FilterIr::Range(range)) = ir.filter() else {
        panic!("expected range");
    };
    assert!(range.include_lower() && range.include_upper());
}

#[test]
fn filters_and_sorting_cover_all_operator_and_union_variants() {
    let definition = rich_definition_bytes();
    let context = IndexContext::from_api(root_definition(&definition)).unwrap();
    for op in [
        api::ComparisonOp::Eq,
        api::ComparisonOp::Ne,
        api::ComparisonOp::Lt,
        api::ComparisonOp::Lte,
        api::ComparisonOp::Gt,
        api::ComparisonOp::Gte,
    ] {
        let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
        let filter = filter_comparison_offset_op(&mut builder, "price", 10, op);
        let request = api::SearchRequest::create(
            &mut builder,
            &api::SearchRequestArgs {
                filter: Some(filter),
                ..Default::default()
            },
        );
        builder.finish(request, None);
        let bytes = builder.finished_data().to_vec();
        let request = searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&bytes).unwrap();
        assert!(matches!(
            context.search_to_ir(request).unwrap().filter(),
            Some(FilterIr::Comparison(_))
        ));
    }

    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let exists_field = builder.create_string("brand");
    let exists = api::FilterExists::create(
        &mut builder,
        &api::FilterExistsArgs {
            field: Some(exists_field),
        },
    );
    let exists = api::FilterExpr::create(
        &mut builder,
        &api::FilterExprArgs {
            expr_type: api::FilterExprKind::FilterExists,
            expr: Some(exists.as_union_value()),
        },
    );
    let not = api::FilterNot::create(
        &mut builder,
        &api::FilterNotArgs {
            clause: Some(exists),
        },
    );
    let not = api::FilterExpr::create(
        &mut builder,
        &api::FilterExprArgs {
            expr_type: api::FilterExprKind::FilterNot,
            expr: Some(not.as_union_value()),
        },
    );
    let request = api::SearchRequest::create(
        &mut builder,
        &api::SearchRequestArgs {
            filter: Some(not),
            ..Default::default()
        },
    );
    builder.finish(request, None);
    let bytes = builder.finished_data().to_vec();
    let request = searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&bytes).unwrap();
    assert!(matches!(
        context.search_to_ir(request).unwrap().filter(),
        Some(FilterIr::Not(inner)) if matches!(**inner, FilterIr::Exists { .. })
    ));

    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let sort_field = builder.create_string("price");
    let missing = api::OptionalMissingPlacement::new(api::MissingPlacement::First);
    let sort = api::SortExpr::create(
        &mut builder,
        &api::SortExprArgs {
            field: Some(sort_field),
            direction: api::SortDirection::Desc,
            missing: Some(&missing),
        },
    );
    let sort = builder.create_vector(&[sort]);
    let facets = [
        (api::FacetSort::CountDesc, "brand"),
        (api::FacetSort::CountAsc, "brand"),
        (api::FacetSort::ValueAsc, "brand"),
        (api::FacetSort::ValueDesc, "brand"),
    ]
    .iter()
    .map(|(sort, field)| {
        let field = builder.create_string(field);
        let sort = api::OptionalFacetSort::new(*sort);
        api::FacetRequest::create(
            &mut builder,
            &api::FacetRequestArgs {
                field: Some(field),
                limit: None,
                sort: Some(&sort),
            },
        )
    })
    .collect::<Vec<_>>();
    // Facet fields must be unique in one request; inspect each variant separately.
    let facet = facets[0];
    let facets = builder.create_vector(&[facet]);
    let request = api::SearchRequest::create(
        &mut builder,
        &api::SearchRequestArgs {
            sort: Some(sort),
            facets: Some(facets),
            ..Default::default()
        },
    );
    builder.finish(request, None);
    let bytes = builder.finished_data().to_vec();
    let request = searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&bytes).unwrap();
    let request = context.search_to_ir(request).unwrap();
    assert_eq!(request.sort()[0].direction(), SortDirection::Desc);
    assert_eq!(request.sort()[0].missing(), MissingPlacement::First);
    assert_eq!(request.facets()[0].sort(), FacetSort::CountDesc);
}

#[test]
fn describe_always_exposes_effective_default_search_fields() {
    let definition = context(Some(&[])).definition().clone();
    let bytes = describe_result_to_api(&definition);
    let result = searchkernel_ir::__flatbuffers::root::<api::DescribeResult>(&bytes).unwrap();
    let index = result.index().unwrap();
    assert!(index.default_search_fields().is_some());
    assert_eq!(
        index
            .default_search_fields()
            .unwrap()
            .fields()
            .unwrap()
            .len(),
        0
    );
}

#[test]
fn result_encoders_preserve_owned_values() {
    let mut object = BTreeMap::new();
    object.insert("nested".to_owned(), Value::Bool(true));
    let mut fields = BTreeMap::new();
    fields.insert("null".to_owned(), Value::Null);
    fields.insert("bool".to_owned(), Value::Bool(true));
    fields.insert("int".to_owned(), Value::Int64(-4));
    fields.insert("uint".to_owned(), Value::UInt64(4));
    fields.insert("float".to_owned(), Value::Float64(1.25));
    fields.insert("string".to_owned(), Value::String("headphones".to_owned()));
    fields.insert("bytes".to_owned(), Value::Bytes(vec![1, 2, 3]));
    fields.insert(
        "list".to_owned(),
        Value::List(vec![Value::Bool(false), Value::UInt64(9)]),
    );
    fields.insert("object".to_owned(), Value::Object(object));
    let result = SearchResult {
        hits: vec![Hit {
            id: "doc-1".to_owned(),
            score: Some(1.5),
            fields,
            highlights: vec![HighlightResult {
                field: "title".to_owned(),
                fragments: vec![HighlightFragment {
                    text: "<hi>phones</hi>".to_owned(),
                    matches: vec![HighlightSpan {
                        start_byte: 4,
                        end_byte: 10,
                    }],
                }],
            }],
        }],
        total_hits: TotalHits {
            value: 1,
            relation: TotalHitsRelation::Exact,
        },
        facets: vec![FacetResult {
            field: "brand".to_owned(),
            values: vec![FacetValue {
                value: Value::String("Acme".to_owned()),
                count: 3,
            }],
        }],
    };
    let bytes = search_result_to_api(&result);
    let encoded = searchkernel_ir::__flatbuffers::root::<api::SearchResult>(&bytes).unwrap();
    assert_eq!(encoded.hits().unwrap().len(), 1);
    assert_eq!(encoded.hits().unwrap().get(0).id().unwrap(), "doc-1");
    assert_eq!(encoded.total_hits().unwrap().value(), 1);
    assert_eq!(
        encoded.hits().unwrap().get(0).highlights().unwrap().len(),
        1
    );
    assert_eq!(encoded.facets().unwrap().len(), 1);
}

#[test]
fn every_owned_result_encoder_emits_its_portable_result() {
    let insert = insert_result_to_api(&InsertResult {
        id: "doc-1".to_owned(),
    });
    assert_eq!(
        searchkernel_ir::__flatbuffers::root::<api::InsertResult>(&insert)
            .unwrap()
            .id(),
        Some("doc-1")
    );
    let upsert = upsert_result_to_api(&UpsertResult {
        id: "doc-1".to_owned(),
    });
    assert_eq!(
        searchkernel_ir::__flatbuffers::root::<api::UpsertResult>(&upsert)
            .unwrap()
            .id(),
        Some("doc-1")
    );
    let patch = patch_result_to_api(&PatchResult {
        id: "doc-1".to_owned(),
    });
    assert_eq!(
        searchkernel_ir::__flatbuffers::root::<api::PatchResult>(&patch)
            .unwrap()
            .id(),
        Some("doc-1")
    );
    let deleted = delete_result_to_api(&DeleteResult {
        id: "doc-1".to_owned(),
        deleted: true,
    });
    assert!(
        searchkernel_ir::__flatbuffers::root::<api::DeleteResult>(&deleted)
            .unwrap()
            .deleted()
    );
    let get = get_result_to_api(&GetResult { document: None });
    assert!(searchkernel_ir::__flatbuffers::root::<api::GetResult>(&get)
        .unwrap()
        .document()
        .is_none());
    let get = get_result_to_api(&GetResult {
        document: Some(DocumentResult {
            id: "doc-1".to_owned(),
            fields: BTreeMap::from([("title".to_owned(), Value::String("phones".to_owned()))]),
        }),
    });
    let get = searchkernel_ir::__flatbuffers::root::<api::GetResult>(&get).unwrap();
    assert_eq!(get.document().unwrap().id(), Some("doc-1"));
    let search = search_result_to_api(&SearchResult {
        hits: vec![Hit {
            id: "doc-2".to_owned(),
            score: None,
            fields: BTreeMap::new(),
            highlights: Vec::new(),
        }],
        total_hits: TotalHits {
            value: 2,
            relation: TotalHitsRelation::LowerBound,
        },
        facets: Vec::new(),
    });
    let search = searchkernel_ir::__flatbuffers::root::<api::SearchResult>(&search).unwrap();
    assert!(search.hits().unwrap().get(0).score().is_none());
    assert_eq!(
        search.total_hits().unwrap().relation(),
        api::TotalHitsRelation::LowerBound
    );
    let analyze = analyze_result_to_api(&AnalyzeResult {
        tokens: vec![AnalyzedToken {
            surface: "Phones".to_owned(),
            term: "phone".to_owned(),
            position: 0,
            start_byte: 0,
            end_byte: 6,
        }],
    });
    assert_eq!(
        searchkernel_ir::__flatbuffers::root::<api::AnalyzeResult>(&analyze)
            .unwrap()
            .tokens()
            .unwrap()
            .len(),
        1
    );
    let error = error_to_api(
        &PortableError::invalid("bad_request", Some("query"), "bad query").with_detail(
            "hint",
            Some("query"),
            "use a term",
        ),
    );
    let error = searchkernel_ir::__flatbuffers::root::<api::Error>(&error).unwrap();
    assert_eq!(error.message(), Some("bad query"));
    assert_eq!(error.details().unwrap().len(), 2);

    let mut error = PortableError::new(ErrorCode::Unavailable, "temporarily unavailable");
    error.retryable = true;
    let detail = searchkernel_ir::ErrorDetail {
        code: "retry".to_owned(),
        path: None,
        message: "try again".to_owned(),
        metadata: BTreeMap::from([("attempt".to_owned(), "1".to_owned())]),
    };
    error.details.push(detail);
    let encoded = error_to_api(&error);
    let encoded = searchkernel_ir::__flatbuffers::root::<api::Error>(&encoded).unwrap();
    assert!(encoded.retryable());
}

#[test]
fn documents_multi_values_and_request_defaults_are_normalized() {
    let definition = rich_definition_bytes();
    let definition =
        searchkernel_ir::__flatbuffers::root::<api::IndexDefinition>(&definition).unwrap();
    let context = IndexContext::from_api(definition).unwrap();

    let document = rich_document_bytes();
    let document = searchkernel_ir::__flatbuffers::root::<api::Document>(&document).unwrap();
    let document = context.document_to_ir(document).unwrap();
    assert_eq!(document.id().as_str(), "sku-1");
    assert!(
        matches!(document.fields()[&FieldName::parse("tags").unwrap()], FieldValueIr::Multi(ref values) if values.len() == 2)
    );

    let request = rich_search_request_bytes();
    let request = searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&request).unwrap();
    let request = context.search_to_ir(request).unwrap();
    assert!(matches!(request.query(), QueryIr::Text(text) if text == "headphones"));
    assert_eq!(request.search_fields().len(), 1);
    assert!(matches!(request.filter(), Some(FilterIr::Comparison(_))));
    assert_eq!(request.sort()[0].direction(), SortDirection::Asc);
    assert_eq!(request.sort()[0].missing(), MissingPlacement::Last);
    assert_eq!(request.facets()[0].limit(), 10);
    assert_eq!(request.facets()[0].sort(), FacetSort::CountDesc);
    assert_eq!(request.ranking(), RankingPolicyIr::Bm25);
    assert_eq!(request.limit(), 0);
    assert_eq!(request.offset(), 5);
    assert_eq!(request.projection().fields().len(), 5);
    assert_eq!(request.language().unwrap().as_str(), "en-us");
}

#[test]
fn api_to_ir_preserves_patch_operations() {
    let definition = rich_definition_bytes();
    let definition =
        searchkernel_ir::__flatbuffers::root::<api::IndexDefinition>(&definition).unwrap();
    let context = IndexContext::from_api(definition).unwrap();
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let operation_value = int_value(&mut builder, 99);
    let field = builder.create_string("price");
    let set = api::SetField::create(
        &mut builder,
        &api::SetFieldArgs {
            field: Some(field),
            value: Some(operation_value),
        },
    );
    let operation = api::PatchOperation::create(
        &mut builder,
        &api::PatchOperationArgs {
            operation_type: api::PatchOperationKind::SetField,
            operation: Some(set.as_union_value()),
        },
    );
    let operations = builder.create_vector(&[operation]);
    let id = builder.create_string("sku-1");
    let root = api::PatchRequest::create(
        &mut builder,
        &api::PatchRequestArgs {
            id: Some(id),
            operations: Some(operations),
        },
    );
    builder.finish(root, None);
    let bytes = builder.finished_data().to_vec();
    let request = searchkernel_ir::__flatbuffers::root::<api::PatchRequest>(&bytes).unwrap();
    let request = context.patch_to_ir(request).unwrap();
    assert!(
        matches!(request.operations()[0], PatchOperationIr::Set { ref field, value: FieldValueIr::Single(ScalarValueIr::Int64(99)) } if field.as_str() == "price")
    );
}

#[test]
fn ranking_discriminator_requires_a_union_payload() {
    let definition = rich_definition_bytes();
    let definition =
        searchkernel_ir::__flatbuffers::root::<api::IndexDefinition>(&definition).unwrap();
    let context = IndexContext::from_api(definition).unwrap();
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let ranking = api::RankingPolicy::create(
        &mut builder,
        &api::RankingPolicyArgs {
            policy_type: api::RankingPolicyKind::Bm25Ranking,
            policy: None,
        },
    );
    let root = api::SearchRequest::create(
        &mut builder,
        &api::SearchRequestArgs {
            ranking: Some(ranking),
            ..Default::default()
        },
    );
    builder.finish(root, None);
    let bytes = builder.finished_data().to_vec();
    // Bypass FlatBuffers verification to exercise the API-to-IR union guard itself.
    let request =
        unsafe { searchkernel_ir::__flatbuffers::root_unchecked::<api::SearchRequest>(&bytes) };
    let error = context.search_to_ir(request).unwrap_err();
    assert_eq!(error.details[0].code, "invalid_ranking");
}

#[test]
fn analyze_get_delete_and_patch_cover_valid_and_invalid_targets() {
    let definition = rich_definition_bytes();
    let context = IndexContext::from_api(root_definition(&definition)).unwrap();

    // Analyze a non-text field and an invalid language.
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let field = builder.create_string("brand");
    let target = api::AnalyzeFieldTarget::create(
        &mut builder,
        &api::AnalyzeFieldTargetArgs { field: Some(field) },
    );
    let language = builder.create_string("-");
    let request = api::AnalyzeRequest::create(
        &mut builder,
        &api::AnalyzeRequestArgs {
            target_type: api::AnalyzeTarget::AnalyzeFieldTarget,
            target: Some(target.as_union_value()),
            language: Some(language),
            ..Default::default()
        },
    );
    builder.finish(request, None);
    let bytes = builder.finished_data().to_vec();
    let request = searchkernel_ir::__flatbuffers::root::<api::AnalyzeRequest>(&bytes).unwrap();
    let (code, path) = result_code(context.analyze_to_ir(request));
    assert_eq!(code, "invalid_analyzer");
    assert_eq!(path.as_deref(), Some("target.field"));

    // An analyzer target with no union payload is malformed.
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let request = api::AnalyzeRequest::create(
        &mut builder,
        &api::AnalyzeRequestArgs {
            target_type: api::AnalyzeTarget::Analyzer,
            target: None,
            ..Default::default()
        },
    );
    builder.finish(request, None);
    let bytes = builder.finished_data().to_vec();
    let request =
        unsafe { searchkernel_ir::__flatbuffers::root_unchecked::<api::AnalyzeRequest>(&bytes) };
    let (code, path) = result_code(context.analyze_to_ir(request));
    assert_eq!(code, "invalid_analyzer");
    assert_eq!(path.as_deref(), Some("target"));

    // Valid get and delete requests preserve their IDs and effective projection.
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let id = builder.create_string("doc-1");
    let field = builder.create_string("title");
    let fields = builder.create_vector(&[field]);
    let projection = api::FieldProjection::create(
        &mut builder,
        &api::FieldProjectionArgs {
            fields: Some(fields),
        },
    );
    let request = api::GetRequest::create(
        &mut builder,
        &api::GetRequestArgs {
            id: Some(id),
            projection: Some(projection),
        },
    );
    builder.finish(request, None);
    let bytes = builder.finished_data().to_vec();
    let request = searchkernel_ir::__flatbuffers::root::<api::GetRequest>(&bytes).unwrap();
    let get = context.get_to_ir(request).unwrap();
    assert_eq!(get.id().as_str(), "doc-1");
    assert_eq!(get.projection().fields()[0].as_str(), "title");

    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let id = builder.create_string("doc-1");
    let request =
        api::DeleteRequest::create(&mut builder, &api::DeleteRequestArgs { id: Some(id) });
    builder.finish(request, None);
    let bytes = builder.finished_data().to_vec();
    let request = searchkernel_ir::__flatbuffers::root::<api::DeleteRequest>(&bytes).unwrap();
    assert_eq!(
        context.delete_to_ir(request).unwrap().id().as_str(),
        "doc-1"
    );
}

#[test]
fn query_and_filter_boolean_boundaries_are_validated() {
    let definition = rich_definition_bytes();
    let context = IndexContext::from_api(root_definition(&definition)).unwrap();

    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let clauses = builder
        .create_vector::<searchkernel_ir::__flatbuffers::ForwardsUOffset<api::QueryExpr>>(&[]);
    let query = api::QueryOr::create(
        &mut builder,
        &api::QueryOrArgs {
            clauses: Some(clauses),
        },
    );
    let expression = api::QueryExpr::create(
        &mut builder,
        &api::QueryExprArgs {
            expr_type: api::QueryExprKind::QueryOr,
            expr: Some(query.as_union_value()),
        },
    );
    let bytes = {
        let request = api::SearchRequest::create(
            &mut builder,
            &api::SearchRequestArgs {
                query_expr: Some(expression),
                ..Default::default()
            },
        );
        builder.finish(request, None);
        builder.finished_data().to_vec()
    };
    let request = searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&bytes).unwrap();
    let (code, path) = result_code(context.search_to_ir(request));
    assert_eq!(code, "invalid_query");
    assert_eq!(path.as_deref(), Some("query_expr.any"));

    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let clauses = builder
        .create_vector::<searchkernel_ir::__flatbuffers::ForwardsUOffset<api::FilterExpr>>(&[]);
    let filter = api::FilterOr::create(
        &mut builder,
        &api::FilterOrArgs {
            clauses: Some(clauses),
        },
    );
    let filter = api::FilterExpr::create(
        &mut builder,
        &api::FilterExprArgs {
            expr_type: api::FilterExprKind::FilterOr,
            expr: Some(filter.as_union_value()),
        },
    );
    let query = builder.create_string("");
    let request = api::SearchRequest::create(
        &mut builder,
        &api::SearchRequestArgs {
            query: Some(query),
            filter: Some(filter),
            ..Default::default()
        },
    );
    builder.finish(request, None);
    let bytes = builder.finished_data().to_vec();
    let request = searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&bytes).unwrap();
    let (code, path) = result_code(context.search_to_ir(request));
    assert_eq!(code, "empty_boolean_filter");
    assert_eq!(path.as_deref(), Some("filter.any"));
}

#[test]
fn component_converters_cover_negative_validation_matrix() {
    let definition = rich_definition_bytes();
    let context = IndexContext::from_api(root_definition(&definition)).unwrap();

    // Search-field selection rejects unknown fields, duplicates, and bad weights.
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let field = builder.create_string("title");
    let weight = api::OptionalDouble::new(-1.0);
    let search_field = api::SearchField::create(
        &mut builder,
        &api::SearchFieldArgs {
            field: Some(field),
            weight: Some(&weight),
        },
    );
    let fields = builder.create_vector(&[search_field]);
    let selection = api::SearchFieldSelection::create(
        &mut builder,
        &api::SearchFieldSelectionArgs {
            fields: Some(fields),
        },
    );
    builder.finish(selection, None);
    let bytes = builder.finished_data().to_vec();
    let selection =
        searchkernel_ir::__flatbuffers::root::<api::SearchFieldSelection>(&bytes).unwrap();
    let (code, path) = result_code(context.search_fields_to_ir(selection));
    assert_eq!(code, "invalid_search_weight");
    assert_eq!(path.as_deref(), Some("search_fields.fields[0].weight"));

    // Projection rejects duplicates and highlighting distinguishes non-stored fields.
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let first = builder.create_string("title");
    let second = builder.create_string("title");
    let fields = builder.create_vector(&[first, second]);
    let projection = api::FieldProjection::create(
        &mut builder,
        &api::FieldProjectionArgs {
            fields: Some(fields),
        },
    );
    builder.finish(projection, None);
    let bytes = builder.finished_data().to_vec();
    let projection = searchkernel_ir::__flatbuffers::root::<api::FieldProjection>(&bytes).unwrap();
    let (code, path) = result_code(context.projection_to_ir(Some(projection)));
    assert_eq!(code, "duplicate_projection_field");
    assert_eq!(path.as_deref(), Some("projection.fields[1]"));

    let hidden_definition = custom_definition_bytes(
        "hidden",
        api::FieldKind::Text,
        api::FieldCardinality::Single,
        Some(false),
        Some(true),
        Some(false),
        Some(false),
        Some(false),
        None,
    );
    let hidden_context = IndexContext::from_api(root_definition(&hidden_definition)).unwrap();
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let field = builder.create_string("hidden");
    let fields = builder.create_vector(&[field]);
    let highlighting = api::HighlightRequest::create(
        &mut builder,
        &api::HighlightRequestArgs {
            fields: Some(fields),
        },
    );
    builder.finish(highlighting, None);
    let bytes = builder.finished_data().to_vec();
    let highlighting =
        searchkernel_ir::__flatbuffers::root::<api::HighlightRequest>(&bytes).unwrap();
    let (code, path) = result_code(hidden_context.highlighting_to_ir(Some(highlighting)));
    assert_eq!(code, "field_not_stored");
    assert_eq!(path.as_deref(), Some("highlighting.fields[0]"));

    // Sort and facet enums are validated independently of request parsing.
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let field = builder.create_string("price");
    let sort = api::SortExpr::create(
        &mut builder,
        &api::SortExprArgs {
            field: Some(field),
            direction: api::SortDirection(9),
            missing: None,
        },
    );
    builder.finish(sort, None);
    let bytes = builder.finished_data().to_vec();
    let sort = searchkernel_ir::__flatbuffers::root::<api::SortExpr>(&bytes).unwrap();
    let (code, path) = result_code(context.sort_to_ir(sort));
    assert_eq!(code, "invalid_filter");
    assert_eq!(path.as_deref(), Some("sort.direction"));

    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let field = builder.create_string("brand");
    let facet_sort = api::OptionalFacetSort::new(api::FacetSort(9));
    let facet = api::FacetRequest::create(
        &mut builder,
        &api::FacetRequestArgs {
            field: Some(field),
            limit: None,
            sort: Some(&facet_sort),
        },
    );
    builder.finish(facet, None);
    let bytes = builder.finished_data().to_vec();
    let facet = searchkernel_ir::__flatbuffers::root::<api::FacetRequest>(&bytes).unwrap();
    let (code, path) = result_code(context.facet_to_ir(facet));
    assert_eq!(code, "invalid_facet");
    assert_eq!(path.as_deref(), Some("facet.sort"));

    // Comparisons and ranges validate operator, value, field kind, and bounds.
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let field = builder.create_string("price");
    let value = int_value(&mut builder, 10);
    let comparison = api::FilterComparison::create(
        &mut builder,
        &api::FilterComparisonArgs {
            field: Some(field),
            op: api::ComparisonOp(9),
            value: Some(value),
        },
    );
    builder.finish(comparison, None);
    let bytes = builder.finished_data().to_vec();
    let comparison = searchkernel_ir::__flatbuffers::root::<api::FilterComparison>(&bytes).unwrap();
    let (code, path) = result_code(context.comparison_to_ir(comparison));
    assert_eq!(code, "invalid_filter");
    assert_eq!(path.as_deref(), Some("comparison.op"));

    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let field = builder.create_string("price");
    let range = api::FilterRange::create(
        &mut builder,
        &api::FilterRangeArgs {
            field: Some(field),
            lower: None,
            upper: None,
            include_lower: None,
            include_upper: None,
        },
    );
    builder.finish(range, None);
    let bytes = builder.finished_data().to_vec();
    let range = searchkernel_ir::__flatbuffers::root::<api::FilterRange>(&bytes).unwrap();
    let (code, path) = result_code(context.range_to_ir(range));
    assert_eq!(code, "invalid_range");
    assert_eq!(path.as_deref(), Some("range"));

    // Query terms require a searchable field and a selected value.
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let field = builder.create_string("brand");
    let value = string_value(&mut builder, "acme");
    let term = api::TermQuery::create(
        &mut builder,
        &api::TermQueryArgs {
            field: Some(field),
            value: Some(value),
        },
    );
    let expression = api::QueryExpr::create(
        &mut builder,
        &api::QueryExprArgs {
            expr_type: api::QueryExprKind::TermQuery,
            expr: Some(term.as_union_value()),
        },
    );
    builder.finish(expression, None);
    let bytes = builder.finished_data().to_vec();
    let expression = searchkernel_ir::__flatbuffers::root::<api::QueryExpr>(&bytes).unwrap();
    let (code, path) = result_code(context.query_to_ir("", Some(expression)));
    assert_eq!(code, "field_not_searchable");
    assert_eq!(path.as_deref(), Some("query_expr.term.field"));

    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let field = builder.create_string("title");
    let term = api::TermQuery::create(
        &mut builder,
        &api::TermQueryArgs {
            field: Some(field),
            value: None,
        },
    );
    let expression = api::QueryExpr::create(
        &mut builder,
        &api::QueryExprArgs {
            expr_type: api::QueryExprKind::TermQuery,
            expr: Some(term.as_union_value()),
        },
    );
    builder.finish(expression, None);
    let bytes = builder.finished_data().to_vec();
    let expression = searchkernel_ir::__flatbuffers::root::<api::QueryExpr>(&bytes).unwrap();
    let (code, path) = result_code(context.query_to_ir("", Some(expression)));
    assert_eq!(code, "missing_value");
    assert_eq!(path.as_deref(), Some("query_expr.term.value"));

    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let field = builder.create_string("title");
    let value = string_value(&mut builder, "x");
    let term = api::TermQuery::create(
        &mut builder,
        &api::TermQueryArgs {
            field: Some(field),
            value: Some(value),
        },
    );
    let expression = api::QueryExpr::create(
        &mut builder,
        &api::QueryExprArgs {
            expr_type: api::QueryExprKind::TermQuery,
            expr: Some(term.as_union_value()),
        },
    );
    builder.finish(expression, None);
    let bytes = builder.finished_data().to_vec();
    let expression = searchkernel_ir::__flatbuffers::root::<api::QueryExpr>(&bytes).unwrap();
    let (code, path) = result_code(context.query_to_ir("also text", Some(expression)));
    assert_eq!(code, "invalid_query");
    assert_eq!(path.as_deref(), Some("query"));
}
