use super::*;
use crate::wire::v1::schema as api;
use crate::{
    AnalyzerIr, DataTypeIr, FieldCapabilities, FieldIr, FieldName, IndexContext,
    PrimitiveDataTypeIr,
};
use std::collections::BTreeSet;

fn schema() -> IndexDefinitionIr {
    let field = |name, kind, filterable| {
        let data_type = match kind {
            FieldKind::Text | FieldKind::Keyword => {
                DataTypeIr::Primitive(PrimitiveDataTypeIr::String)
            }
            FieldKind::Int64 => DataTypeIr::Primitive(PrimitiveDataTypeIr::Int64),
            FieldKind::Float64 => DataTypeIr::Primitive(PrimitiveDataTypeIr::Float64),
            FieldKind::Bool => DataTypeIr::Primitive(PrimitiveDataTypeIr::Bool),
        };
        let mut field = FieldIr::new(
            FieldName::parse(name).unwrap(),
            data_type,
            (kind == FieldKind::Text).then(|| AnalyzerIr {
                name: "standard".to_owned(),
                version: 1,
                options: std::collections::BTreeMap::new(),
            }),
            FieldCapabilities::new(false, false, filterable, false, false),
        );
        if kind == FieldKind::Keyword {
            field.matching = Some(crate::MatchingConfigIr {
                mode: crate::MatchingModeIr::Exact,
                options: std::collections::BTreeMap::new(),
            });
        }
        field
    };
    IndexDefinitionIr::new(
        None,
        vec![
            field("brand", FieldKind::Keyword, true),
            field("price", FieldKind::Int64, true),
            field("rating", FieldKind::Float64, true),
            field("in_stock", FieldKind::Bool, true),
            field("title", FieldKind::Text, true),
            field("hidden", FieldKind::Keyword, false),
        ],
        Vec::new(),
    )
    .unwrap()
}

fn parameters(values: impl IntoIterator<Item = (&'static str, Value)>) -> FilterParameters {
    values
        .into_iter()
        .map(|(name, value)| (name.to_owned(), value))
        .collect()
}

fn comparison(field: &str, op: ComparisonOp, value: ScalarValueIr) -> FilterIr {
    FilterIr::Comparison(ComparisonIr::new(
        FieldName::parse(field).unwrap(),
        op,
        value,
    ))
}

fn structured_context() -> IndexContext {
    let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
    let filterable = api::OptionalBool::new(true);
    let fields = [
        structured_field(
            &mut builder,
            "brand",
            api::PrimitiveDataTypeKind::String,
            &filterable,
        ),
        structured_field(
            &mut builder,
            "price",
            api::PrimitiveDataTypeKind::Int64,
            &filterable,
        ),
        structured_field(
            &mut builder,
            "rating",
            api::PrimitiveDataTypeKind::Float64,
            &filterable,
        ),
        structured_field(
            &mut builder,
            "in_stock",
            api::PrimitiveDataTypeKind::Bool,
            &filterable,
        ),
    ];
    let fields = builder.create_vector(&fields);
    let definition = api::IndexDefinition::create(
        &mut builder,
        &api::IndexDefinitionArgs {
            fields: Some(fields),
            ..Default::default()
        },
    );
    builder.finish(definition, None);
    let definition = crate::__flatbuffers::root::<api::IndexDefinition>(builder.finished_data())
        .expect("valid structured test schema");
    IndexContext::from_api(definition).expect("structured test schema converts")
}

fn structured_field<'bldr, 'args, A: crate::__flatbuffers::Allocator + 'bldr>(
    builder: &'args mut crate::__flatbuffers::FlatBufferBuilder<'bldr, A>,
    name: &str,
    kind: api::PrimitiveDataTypeKind,
    filterable: &api::OptionalBool,
) -> crate::__flatbuffers::WIPOffset<api::FieldDefinition<'bldr>>
where
    'bldr: 'args,
{
    let name = builder.create_string(name);
    let primitive = api::PrimitiveDataType::create(builder, &api::PrimitiveDataTypeArgs { kind });
    let data_type = api::DataType::create(
        builder,
        &api::DataTypeArgs {
            kind_type: api::DataTypeKind::PrimitiveDataType,
            kind: Some(primitive.as_union_value()),
        },
    );
    let matching = (kind == api::PrimitiveDataTypeKind::String).then(|| {
        api::MatchingConfig::create(
            builder,
            &api::MatchingConfigArgs {
                mode: api::MatchingMode::Exact,
                ..Default::default()
            },
        )
    });
    api::FieldDefinition::create(
        builder,
        &api::FieldDefinitionArgs {
            name: Some(name),
            data_type: Some(data_type),
            filterable: Some(filterable),
            matching,
            ..Default::default()
        },
    )
}

enum StructuredValue {
    String(&'static str),
    Int64(i64),
    Float64(f64),
    Bool(bool),
}

enum StructuredFilter {
    Compare {
        field: &'static str,
        op: api::ComparisonOp,
        value: StructuredValue,
    },
    In {
        field: &'static str,
        values: Vec<StructuredValue>,
    },
    And(Vec<StructuredFilter>),
    Or(Vec<StructuredFilter>),
    Not(Box<StructuredFilter>),
}

fn structured_value<'bldr, 'args, A: crate::__flatbuffers::Allocator + 'bldr>(
    builder: &'args mut crate::__flatbuffers::FlatBufferBuilder<'bldr, A>,
    value: StructuredValue,
) -> crate::__flatbuffers::WIPOffset<api::Value<'bldr>>
where
    'bldr: 'args,
{
    let (kind_type, kind) = match value {
        StructuredValue::String(value) => {
            let value = builder.create_string(value);
            let value =
                api::StringValue::create(builder, &api::StringValueArgs { value: Some(value) });
            (api::ValueKind::StringValue, value.as_union_value())
        }
        StructuredValue::Int64(value) => {
            let value = api::Int64Value::create(builder, &api::Int64ValueArgs { value });
            (api::ValueKind::Int64Value, value.as_union_value())
        }
        StructuredValue::Float64(value) => {
            let value = api::Float64Value::create(builder, &api::Float64ValueArgs { value });
            (api::ValueKind::Float64Value, value.as_union_value())
        }
        StructuredValue::Bool(value) => {
            let value = api::BoolValue::create(builder, &api::BoolValueArgs { value });
            (api::ValueKind::BoolValue, value.as_union_value())
        }
    };
    api::Value::create(
        builder,
        &api::ValueArgs {
            kind_type,
            kind: Some(kind),
        },
    )
}

fn structured_filter<'bldr, 'args, A: crate::__flatbuffers::Allocator + 'bldr>(
    builder: &'args mut crate::__flatbuffers::FlatBufferBuilder<'bldr, A>,
    filter: StructuredFilter,
) -> crate::__flatbuffers::WIPOffset<api::FilterExpr<'bldr>>
where
    'bldr: 'args,
{
    let (expr_type, expr) = match filter {
        StructuredFilter::Compare { field, op, value } => {
            let field = field_ref(builder, field);
            let value = structured_value(builder, value);
            let comparison = api::FilterComparison::create(
                builder,
                &api::FilterComparisonArgs {
                    field: Some(field),
                    op,
                    value: Some(value),
                },
            );
            (
                api::FilterExprKind::FilterComparison,
                comparison.as_union_value(),
            )
        }
        StructuredFilter::In { field, values } => {
            let field = field_ref(builder, field);
            let values = values
                .into_iter()
                .map(|value| structured_value(builder, value))
                .collect::<Vec<_>>();
            let values = builder.create_vector(&values);
            let input = api::FilterIn::create(
                builder,
                &api::FilterInArgs {
                    field: Some(field),
                    values: Some(values),
                },
            );
            (api::FilterExprKind::FilterIn, input.as_union_value())
        }
        StructuredFilter::And(filters) => {
            let clauses = filters
                .into_iter()
                .map(|filter| structured_filter(builder, filter))
                .collect::<Vec<_>>();
            let clauses = builder.create_vector(&clauses);
            let input = api::FilterAnd::create(
                builder,
                &api::FilterAndArgs {
                    clauses: Some(clauses),
                },
            );
            (api::FilterExprKind::FilterAnd, input.as_union_value())
        }
        StructuredFilter::Or(filters) => {
            let clauses = filters
                .into_iter()
                .map(|filter| structured_filter(builder, filter))
                .collect::<Vec<_>>();
            let clauses = builder.create_vector(&clauses);
            let input = api::FilterOr::create(
                builder,
                &api::FilterOrArgs {
                    clauses: Some(clauses),
                },
            );
            (api::FilterExprKind::FilterOr, input.as_union_value())
        }
        StructuredFilter::Not(filter) => {
            let clause = structured_filter(builder, *filter);
            let input = api::FilterNot::create(
                builder,
                &api::FilterNotArgs {
                    clause: Some(clause),
                },
            );
            (api::FilterExprKind::FilterNot, input.as_union_value())
        }
    };
    api::FilterExpr::create(
        builder,
        &api::FilterExprArgs {
            expr_type,
            expr: Some(expr),
        },
    )
}

fn field_ref<'bldr, A: crate::__flatbuffers::Allocator + 'bldr>(
    builder: &mut crate::__flatbuffers::FlatBufferBuilder<'bldr, A>,
    value: &str,
) -> crate::__flatbuffers::WIPOffset<api::QueryFieldRef<'bldr>> {
    let path = builder.create_string(value);
    api::QueryFieldRef::create(
        builder,
        &api::QueryFieldRefArgs {
            path: Some(path),
            element_index: None,
        },
    )
}

fn resolve_structured(context: &IndexContext, filter: StructuredFilter) -> FilterIr {
    let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
    let expression = structured_filter(&mut builder, filter);
    builder.finish(expression, None);
    let expression = crate::__flatbuffers::root::<api::FilterExpr>(builder.finished_data())
        .expect("valid structured filter");
    context
        .filter_to_ir(expression)
        .expect("structured filter converts")
}

fn structured_string_filter(context: &IndexContext) -> FilterIr {
    let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
    let field = field_ref(&mut builder, "brand");
    let text = builder.create_string("Acme");
    let string =
        api::StringValue::create(&mut builder, &api::StringValueArgs { value: Some(text) });
    let scalar = api::Value::create(
        &mut builder,
        &api::ValueArgs {
            kind_type: api::ValueKind::StringValue,
            kind: Some(string.as_union_value()),
        },
    );
    let comparison = api::FilterComparison::create(
        &mut builder,
        &api::FilterComparisonArgs {
            field: Some(field),
            op: api::ComparisonOp::Eq,
            value: Some(scalar),
        },
    );
    let expression = api::FilterExpr::create(
        &mut builder,
        &api::FilterExprArgs {
            expr_type: api::FilterExprKind::FilterComparison,
            expr: Some(comparison.as_union_value()),
        },
    );
    builder.finish(expression, None);
    let expression = crate::__flatbuffers::root::<api::FilterExpr>(builder.finished_data())
        .expect("valid structured comparison");
    context
        .filter_to_ir(expression)
        .expect("comparison converts")
}

fn structured_in_filter(context: &IndexContext) -> FilterIr {
    let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
    let field = field_ref(&mut builder, "brand");
    let acme_text = builder.create_string("Acme");
    let acme_string = api::StringValue::create(
        &mut builder,
        &api::StringValueArgs {
            value: Some(acme_text),
        },
    );
    let acme = api::Value::create(
        &mut builder,
        &api::ValueArgs {
            kind_type: api::ValueKind::StringValue,
            kind: Some(acme_string.as_union_value()),
        },
    );
    let trailco_text = builder.create_string("TrailCo");
    let trailco_string = api::StringValue::create(
        &mut builder,
        &api::StringValueArgs {
            value: Some(trailco_text),
        },
    );
    let trailco = api::Value::create(
        &mut builder,
        &api::ValueArgs {
            kind_type: api::ValueKind::StringValue,
            kind: Some(trailco_string.as_union_value()),
        },
    );
    let values = builder.create_vector(&[acme, trailco]);
    let input = api::FilterIn::create(
        &mut builder,
        &api::FilterInArgs {
            field: Some(field),
            values: Some(values),
        },
    );
    let expression = api::FilterExpr::create(
        &mut builder,
        &api::FilterExprArgs {
            expr_type: api::FilterExprKind::FilterIn,
            expr: Some(input.as_union_value()),
        },
    );
    builder.finish(expression, None);
    let expression = crate::__flatbuffers::root::<api::FilterExpr>(builder.finished_data())
        .expect("valid structured IN");
    context.filter_to_ir(expression).expect("IN converts")
}

fn assert_detail(source: &str, params: &FilterParameters, code: &str) {
    let error = bind_filter_expression(source, &schema(), params).unwrap_err();
    assert_eq!(error.code, crate::ErrorCode::InvalidArgument);
    assert_eq!(error.details[0].code, code, "{error:?}");
    assert!(error.details[0].metadata.contains_key("start_offset"));
    assert!(error.details[0].metadata.contains_key("end_offset"));
}

#[test]
fn textual_filters_match_the_structured_ir() {
    assert_eq!(
        bind_filter_expression("brand = 'Acme'", &schema(), &FilterParameters::new()).unwrap(),
        comparison(
            "brand",
            ComparisonOp::Eq,
            ScalarValueIr::String("Acme".to_owned())
        )
    );
    assert_eq!(
        bind_filter_expression(
            "price <= 120 AND in_stock = true",
            &schema(),
            &FilterParameters::new(),
        )
        .unwrap(),
        FilterIr::And(vec![
            comparison("price", ComparisonOp::Lte, ScalarValueIr::Int64(120)),
            comparison("in_stock", ComparisonOp::Eq, ScalarValueIr::Bool(true)),
        ])
    );
    assert_eq!(
        bind_filter_expression(
            "rating > 4.5 OR NOT (brand = 'Acme')",
            &schema(),
            &FilterParameters::new(),
        )
        .unwrap(),
        FilterIr::Or(vec![
            comparison("rating", ComparisonOp::Gt, ScalarValueIr::Float64(4.5)),
            FilterIr::Not(Box::new(comparison(
                "brand",
                ComparisonOp::Eq,
                ScalarValueIr::String("Acme".to_owned()),
            ))),
        ])
    );
    assert_eq!(
        bind_filter_expression(
            "brand IN ('Acme', 'TrailCo')",
            &schema(),
            &FilterParameters::new(),
        )
        .unwrap(),
        FilterIr::In {
            field: FieldName::parse("brand").unwrap(),
            values: vec![
                ScalarValueIr::String("Acme".to_owned()),
                ScalarValueIr::String("TrailCo".to_owned()),
            ],
        }
    );
}

#[test]
fn textual_filters_equal_the_existing_structured_conversion() {
    let context = structured_context();
    assert_eq!(
        bind_filter_expression(
            "brand = 'Acme'",
            context.definition(),
            &FilterParameters::new(),
        )
        .unwrap(),
        structured_string_filter(&context)
    );
    assert_eq!(
        bind_filter_expression(
            "brand IN ('Acme', 'TrailCo')",
            context.definition(),
            &FilterParameters::new(),
        )
        .unwrap(),
        structured_in_filter(&context)
    );
}

#[test]
fn all_textual_filter_operators_equal_structured_conversion() {
    let context = structured_context();
    let cases = [
        (
            "price = 4",
            StructuredFilter::Compare {
                field: "price",
                op: api::ComparisonOp::Eq,
                value: StructuredValue::Int64(4),
            },
        ),
        (
            "price < 4",
            StructuredFilter::Compare {
                field: "price",
                op: api::ComparisonOp::Lt,
                value: StructuredValue::Int64(4),
            },
        ),
        (
            "price <= 4",
            StructuredFilter::Compare {
                field: "price",
                op: api::ComparisonOp::Lte,
                value: StructuredValue::Int64(4),
            },
        ),
        (
            "price > 4",
            StructuredFilter::Compare {
                field: "price",
                op: api::ComparisonOp::Gt,
                value: StructuredValue::Int64(4),
            },
        ),
        (
            "price >= 4",
            StructuredFilter::Compare {
                field: "price",
                op: api::ComparisonOp::Gte,
                value: StructuredValue::Int64(4),
            },
        ),
        (
            "rating = 4.5",
            StructuredFilter::Compare {
                field: "rating",
                op: api::ComparisonOp::Eq,
                value: StructuredValue::Float64(4.5),
            },
        ),
        (
            "rating < 4.5",
            StructuredFilter::Compare {
                field: "rating",
                op: api::ComparisonOp::Lt,
                value: StructuredValue::Float64(4.5),
            },
        ),
        (
            "rating <= 4.5",
            StructuredFilter::Compare {
                field: "rating",
                op: api::ComparisonOp::Lte,
                value: StructuredValue::Float64(4.5),
            },
        ),
        (
            "rating > 4.5",
            StructuredFilter::Compare {
                field: "rating",
                op: api::ComparisonOp::Gt,
                value: StructuredValue::Float64(4.5),
            },
        ),
        (
            "rating >= 4.5",
            StructuredFilter::Compare {
                field: "rating",
                op: api::ComparisonOp::Gte,
                value: StructuredValue::Float64(4.5),
            },
        ),
        (
            "in_stock = true",
            StructuredFilter::Compare {
                field: "in_stock",
                op: api::ComparisonOp::Eq,
                value: StructuredValue::Bool(true),
            },
        ),
        (
            "brand IN ('Acme', 'TrailCo')",
            StructuredFilter::In {
                field: "brand",
                values: vec![
                    StructuredValue::String("Acme"),
                    StructuredValue::String("TrailCo"),
                ],
            },
        ),
        (
            "price IN (1, 2, 3)",
            StructuredFilter::In {
                field: "price",
                values: vec![
                    StructuredValue::Int64(1),
                    StructuredValue::Int64(2),
                    StructuredValue::Int64(3),
                ],
            },
        ),
        (
            "price <= 120 AND in_stock = true",
            StructuredFilter::And(vec![
                StructuredFilter::Compare {
                    field: "price",
                    op: api::ComparisonOp::Lte,
                    value: StructuredValue::Int64(120),
                },
                StructuredFilter::Compare {
                    field: "in_stock",
                    op: api::ComparisonOp::Eq,
                    value: StructuredValue::Bool(true),
                },
            ]),
        ),
        (
            "brand = 'Acme' OR brand = 'TrailCo'",
            StructuredFilter::Or(vec![
                StructuredFilter::Compare {
                    field: "brand",
                    op: api::ComparisonOp::Eq,
                    value: StructuredValue::String("Acme"),
                },
                StructuredFilter::Compare {
                    field: "brand",
                    op: api::ComparisonOp::Eq,
                    value: StructuredValue::String("TrailCo"),
                },
            ]),
        ),
        (
            "NOT (price > 10 AND (in_stock = false OR brand = 'Acme'))",
            StructuredFilter::Not(Box::new(StructuredFilter::And(vec![
                StructuredFilter::Compare {
                    field: "price",
                    op: api::ComparisonOp::Gt,
                    value: StructuredValue::Int64(10),
                },
                StructuredFilter::Or(vec![
                    StructuredFilter::Compare {
                        field: "in_stock",
                        op: api::ComparisonOp::Eq,
                        value: StructuredValue::Bool(false),
                    },
                    StructuredFilter::Compare {
                        field: "brand",
                        op: api::ComparisonOp::Eq,
                        value: StructuredValue::String("Acme"),
                    },
                ]),
            ]))),
        ),
    ];
    for (source, structured) in cases {
        assert_eq!(
            bind_filter_expression(source, context.definition(), &FilterParameters::new()).unwrap(),
            resolve_structured(&context, structured),
            "source: {source}"
        );
    }
}

#[test]
fn typed_parameters_and_references_are_preserved() {
    let params = parameters([
        ("max", Value::Int64(120)),
        ("brand", Value::String("Acme".to_owned())),
        (
            "brands",
            Value::List(vec![
                Value::String("Acme".to_owned()),
                Value::String("TrailCo".to_owned()),
            ]),
        ),
    ]);
    let bound = bind_filter_expression_with_references(
        "price <= :max AND brand = :brand AND brand IN (:brands)",
        &schema(),
        &params,
    )
    .unwrap();
    assert_eq!(
        bound.referenced_parameters(),
        &BTreeSet::from(["brand".to_owned(), "brands".to_owned(), "max".to_owned()])
    );
    assert!(matches!(bound.expression(), FilterIr::And(clauses) if clauses.len() == 3));
}

#[test]
fn parsed_syntax_ast_binds_without_reparsing() {
    let parsed = searchkernel_filter::parse_filter("price <= :max").unwrap();
    let bound = super::bind_parsed_filter(
        &parsed,
        &schema(),
        &parameters([("max", Value::Int64(120))]),
    )
    .unwrap();
    assert_eq!(
        bound.expression(),
        &comparison("price", ComparisonOp::Lte, ScalarValueIr::Int64(120))
    );
}

#[test]
fn binder_rejects_invalid_fields_operators_and_types() {
    let empty = FilterParameters::new();
    assert_detail("missing = 1", &empty, "unknown_field");
    assert_detail("hidden = 'x'", &empty, "field_not_filterable");
    assert_detail("title = 'x'", &empty, "unsupported_operator");
    assert_detail("brand < 'x'", &empty, "unsupported_operator");
    assert_detail("in_stock < true", &empty, "unsupported_operator");
    assert_detail("price <= true", &empty, "type_mismatch");
    assert_detail("price <= 4.5", &empty, "type_mismatch");
    assert_detail("rating IN (1, 2)", &empty, "unsupported_operator");
    assert_detail("price = 9223372036854775808", &empty, "invalid_literal");
    assert_detail("rating = 1e309", &empty, "non_finite_number");
}

#[test]
fn binder_rejects_invalid_parameter_use() {
    assert_detail(
        "price <= :missing",
        &FilterParameters::new(),
        "unknown_parameter",
    );
    assert_detail(
        "price <= :max",
        &parameters([("max", Value::String("120".to_owned()))]),
        "type_mismatch",
    );
    assert_detail(
        "price <= :max",
        &parameters([("max", Value::List(vec![Value::Int64(120)]))]),
        "type_mismatch",
    );
    assert_detail(
        "brand IN (:brands)",
        &parameters([("brands", Value::String("Acme".to_owned()))]),
        "type_mismatch",
    );
    assert_detail(
        "brand IN (:brands)",
        &parameters([("brands", Value::List(vec![Value::Int64(1)]))]),
        "type_mismatch",
    );
    assert_detail(
        "brand IN (:brands)",
        &parameters([("brands", Value::List(Vec::new()))]),
        "empty_in",
    );

    let error = bind_filter_expression(
        "price <= :max",
        &schema(),
        &parameters([("max", Value::Int64(120)), ("unused", Value::Int64(4))]),
    )
    .unwrap_err();
    assert_eq!(error.details[0].code, "unused_parameter");
}

#[test]
fn in_list_parameter_enforces_expanded_value_limit() {
    let at_limit = Value::List(
        (0..searchkernel_filter::limits::MAX_IN_VALUES)
            .map(|i| Value::String(format!("brand-{i}")))
            .collect(),
    );
    let bound = bind_filter_expression(
        "brand IN (:brands)",
        &schema(),
        &parameters([("brands", at_limit)]),
    )
    .expect("IN list parameter at the canonical limit must bind");
    assert!(
        matches!(bound, FilterIr::In { values, .. } if values.len() == searchkernel_filter::limits::MAX_IN_VALUES)
    );

    let over_limit = Value::List(
        (0..=searchkernel_filter::limits::MAX_IN_VALUES)
            .map(|i| Value::String(format!("brand-{i}")))
            .collect(),
    );
    let error = bind_filter_expression(
        "brand IN (:brands)",
        &schema(),
        &parameters([("brands", over_limit)]),
    )
    .unwrap_err();
    assert_eq!(error.code, crate::ErrorCode::InvalidArgument);
    assert_eq!(error.details[0].code, "expression_too_complex");
    assert_eq!(
        error.details[0].metadata.get("limit").map(String::as_str),
        Some("InValues")
    );
}

#[test]
fn numeric_parameters_are_normalized_to_the_field_type() {
    assert_eq!(
        bind_filter_expression(
            "rating <= :max",
            &schema(),
            &parameters([("max", Value::Int64(120))]),
        )
        .unwrap(),
        comparison("rating", ComparisonOp::Lte, ScalarValueIr::Float64(120.0))
    );
    assert_eq!(
        bind_filter_expression(
            "price = :value",
            &schema(),
            &parameters([("value", Value::UInt64(120))]),
        )
        .unwrap(),
        comparison("price", ComparisonOp::Eq, ScalarValueIr::Int64(120))
    );
    assert_eq!(
        bind_filter_expression(
            "rating = :value",
            &schema(),
            &parameters([("value", Value::UInt64(120))]),
        )
        .unwrap(),
        comparison("rating", ComparisonOp::Eq, ScalarValueIr::Float64(120.0))
    );
}

#[test]
fn parser_errors_become_portable_errors() {
    assert_detail("brand =", &FilterParameters::new(), "syntax_error");
    assert_detail("brand IN ()", &FilterParameters::new(), "empty_in");
}
