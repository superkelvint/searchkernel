use searchkernel_ir::__flatbuffers::{root, FlatBufferBuilder};
use searchkernel_ir::{api, field_to_ir, IndexContext};

fn fixture() -> Vec<u8> {
    let mut builder = FlatBufferBuilder::new();
    let field_name = builder.create_string("title");
    let enabled = api::OptionalBool::new(true);
    let disabled = api::OptionalBool::new(false);
    let field = api::FieldDefinition::create(
        &mut builder,
        &api::FieldDefinitionArgs {
            name: Some(field_name),
            kind: api::FieldKind::Text,
            cardinality: api::FieldCardinality::Single,
            stored: Some(&enabled),
            searchable: Some(&enabled),
            filterable: Some(&disabled),
            sortable: Some(&disabled),
            facetable: Some(&disabled),
            analyzer: None,
        },
    );
    let brand_name = builder.create_string("brand");
    let brand = api::FieldDefinition::create(
        &mut builder,
        &api::FieldDefinitionArgs {
            name: Some(brand_name),
            kind: api::FieldKind::Keyword,
            cardinality: api::FieldCardinality::Single,
            stored: Some(&enabled),
            searchable: Some(&disabled),
            filterable: Some(&enabled),
            sortable: Some(&enabled),
            facetable: Some(&enabled),
            analyzer: None,
        },
    );
    let fields = builder.create_vector(&[field, brand]);
    let _ = fields;

    let string = builder.create_string("headphones");
    let string_value = api::StringValue::create(
        &mut builder,
        &api::StringValueArgs {
            value: Some(string),
        },
    );
    let value = api::Value::create(
        &mut builder,
        &api::ValueArgs {
            kind_type: api::ValueKind::StringValue,
            kind: Some(string_value.as_union_value()),
        },
    );
    let comparison = api::FilterComparison::create(
        &mut builder,
        &api::FilterComparisonArgs {
            field: Some(brand_name),
            op: api::ComparisonOp::Eq,
            value: Some(value),
        },
    );
    let filter = api::FilterExpr::create(
        &mut builder,
        &api::FilterExprArgs {
            expr_type: api::FilterExprKind::FilterComparison,
            expr: Some(comparison.as_union_value()),
        },
    );
    let facet = api::FacetRequest::create(
        &mut builder,
        &api::FacetRequestArgs {
            field: Some(brand_name),
            limit: None,
            sort: None,
        },
    );
    let term = api::TermQuery::create(
        &mut builder,
        &api::TermQueryArgs {
            field: Some(field_name),
            value: Some(value),
        },
    );
    let query_expr = api::QueryExpr::create(
        &mut builder,
        &api::QueryExprArgs {
            expr_type: api::QueryExprKind::TermQuery,
            expr: Some(term.as_union_value()),
        },
    );
    let facets = builder.create_vector(&[facet]);
    let search = api::SearchRequest::create(
        &mut builder,
        &api::SearchRequestArgs {
            query: None,
            filter: Some(filter),
            facets: Some(facets),
            query_expr: Some(query_expr),
            ..Default::default()
        },
    );
    builder.finish(search, None);
    builder.finished_data().to_vec()
}

#[test]
fn representative_converters_are_direct_core_calls() {
    let bytes = fixture();
    let search = root::<api::SearchRequest>(&bytes).unwrap();
    let definition_bytes = {
        // The search fixture is intentionally rooted at SearchRequest. Build a
        // separate schema buffer to exercise the component entry points too.
        let mut builder = FlatBufferBuilder::new();
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
        let brand_name = builder.create_string("brand");
        let brand = api::FieldDefinition::create(
            &mut builder,
            &api::FieldDefinitionArgs {
                name: Some(brand_name),
                kind: api::FieldKind::Keyword,
                cardinality: api::FieldCardinality::Single,
                stored: Some(&api::OptionalBool::new(true)),
                searchable: Some(&api::OptionalBool::new(false)),
                filterable: Some(&api::OptionalBool::new(true)),
                sortable: Some(&api::OptionalBool::new(true)),
                facetable: Some(&api::OptionalBool::new(true)),
                analyzer: None,
            },
        );
        let fields = builder.create_vector(&[field, brand]);
        let definition = api::IndexDefinition::create(
            &mut builder,
            &api::IndexDefinitionArgs {
                name: None,
                fields: Some(fields),
                default_search_fields: None,
            },
        );
        builder.finish(definition, None);
        builder.finished_data().to_vec()
    };
    let definition = root::<api::IndexDefinition>(&definition_bytes).unwrap();
    let field = definition.fields().unwrap().get(0);
    assert_eq!(field_to_ir(field).unwrap().name().as_str(), "title");

    let context = IndexContext::from_api(definition).unwrap();
    let query = context
        .query_to_ir("", Some(search.query_expr().unwrap()))
        .unwrap();
    assert!(matches!(query, searchkernel_ir::QueryIr::Term { .. }));
    let filter = search.filter().unwrap();
    let comparison = filter.expr_as_filter_comparison().unwrap();
    assert!(context
        .value_to_ir("brand", comparison.value().unwrap())
        .is_ok());
    assert!(context.filter_to_ir(filter).is_ok());
    assert!(context.facet_to_ir(search.facets().unwrap().get(0)).is_ok());
    assert!(context.search_to_ir(search).is_ok());
}
