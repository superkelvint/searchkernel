use super::*;
use crate::model::*;
use searchkernel_protocol::v2 as api;
use std::collections::BTreeMap;

fn context() -> IndexContext {
    let field = |name, kind| {
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
            None,
            FieldCapabilities::new(false, false, true, false, false),
        );
        if kind == FieldKind::Keyword {
            field.matching = Some(MatchingConfigIr {
                mode: MatchingModeIr::Exact,
                options: BTreeMap::new(),
            });
        }
        field
    };
    let definition = IndexDefinitionIr::new(
        None,
        vec![
            field("brand", FieldKind::Keyword),
            field("price", FieldKind::Int64),
            field("rating", FieldKind::Float64),
        ],
        Vec::new(),
        RankingPolicyIr::DefaultRelevanceV1,
        PrefixPolicy::Disabled,
        TypoPolicy::Disabled,
    )
    .unwrap();
    let document_fields = definition
        .fields()
        .iter()
        .enumerate()
        .map(|(index, field)| (field.name().as_str().to_owned(), index))
        .collect();
    let query_fields = definition
        .fields()
        .iter()
        .enumerate()
        .map(|(index, field)| {
            (
                field.name().as_str().to_owned(),
                QueryFieldRef::Document(index),
            )
        })
        .collect();
    IndexContext {
        definition,
        document_fields,
        query_fields,
    }
}

#[test]
fn rust_resolvers_match_flatbuffer_adapters_at_the_ir_boundary() {
    let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
    let id = builder.create_string("doc-7");
    let condition = builder.create_string("tenant = 'acme'");
    let delete = api::DeleteRequest::create(
        &mut builder,
        &api::DeleteRequestArgs {
            id: Some(id),
            condition: Some(condition),
        },
    );
    builder.finish(delete, None);
    let delete = crate::__flatbuffers::root::<api::DeleteRequest>(builder.finished_data())
        .expect("valid delete request");
    assert_eq!(
        resolve_delete("doc-7", Some("tenant = 'acme'")).unwrap(),
        context().delete_to_ir(delete).unwrap()
    );

    let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
    let id = builder.create_string("doc-7");
    let get = api::GetRequest::create(
        &mut builder,
        &api::GetRequestArgs {
            id: Some(id),
            projection: None,
        },
    );
    builder.finish(get, None);
    let get = crate::__flatbuffers::root::<api::GetRequest>(builder.finished_data())
        .expect("valid get request");
    assert_eq!(
        resolve_get(
            &context(),
            GetInput {
                id: "doc-7".to_owned(),
                projection: None,
            },
        )
        .unwrap(),
        context().get_to_ir(get).unwrap()
    );

    let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
    let name = builder.create_string("standard");
    let analyzer = api::Analyzer::create(
        &mut builder,
        &api::AnalyzerArgs {
            name: Some(name),
            version: 1,
            options: None,
        },
    );
    let text = builder.create_string("The quick brown fox");
    let analyze = api::AnalyzeRequest::create(
        &mut builder,
        &api::AnalyzeRequestArgs {
            target_type: api::AnalyzeTarget::Analyzer,
            target: Some(analyzer.as_union_value()),
            text: Some(text),
            language: None,
        },
    );
    builder.finish(analyze, None);
    let analyze = crate::__flatbuffers::root::<api::AnalyzeRequest>(builder.finished_data())
        .expect("valid analyze request");
    assert_eq!(
        resolve_analyze(
            &context(),
            AnalyzeInput {
                target: Some(AnalyzeTargetInput::Analyzer(AnalyzerInput {
                    name: "standard".to_owned(),
                    version: 1,
                    options: BTreeMap::new(),
                })),
                text: "The quick brown fox".to_owned(),
                language: None,
            },
        )
        .unwrap(),
        context().analyze_to_ir(analyze).unwrap()
    );
}

#[test]
fn schema_only_fields_are_query_visible_but_not_feedable() {
    let mut definition = IndexDefinitionIr::new(
        None,
        vec![FieldIr::new(
            FieldName::parse("title").unwrap(),
            DataTypeIr::Primitive(PrimitiveDataTypeIr::String),
            None,
            FieldCapabilities::new(true, true, false, false, false),
        )],
        Vec::new(),
        RankingPolicyIr::DefaultRelevanceV1,
        PrefixPolicy::Disabled,
        TypoPolicy::Disabled,
    )
    .unwrap();
    definition.set_schema_fields(vec![FieldIr::new(
        FieldName::parse("derived").unwrap(),
        DataTypeIr::Primitive(PrimitiveDataTypeIr::Int64),
        None,
        FieldCapabilities::new(true, true, true, true, false),
    )]);
    let bytes = crate::index_definition_ir_to_api(&definition);
    let context =
        IndexContext::from_api(crate::__flatbuffers::root::<api::IndexDefinition>(&bytes).unwrap())
            .unwrap();

    let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
    let name = builder.create_string("derived");
    let weight = api::OptionalDouble::new(1.0);
    let search_field = api::SearchField::create(
        &mut builder,
        &api::SearchFieldArgs {
            field: Some(name),
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
    let selection =
        crate::__flatbuffers::root::<api::SearchFieldSelection>(builder.finished_data()).unwrap();
    assert_eq!(context.search_fields_to_ir(selection).unwrap().len(), 1);

    let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
    let value = api::Int64Value::create(&mut builder, &api::Int64ValueArgs { value: 3 });
    let value = api::Value::create(
        &mut builder,
        &api::ValueArgs {
            kind_type: api::ValueKind::Int64Value,
            kind: Some(value.as_union_value()),
        },
    );
    builder.finish(value, None);
    let value = crate::__flatbuffers::root::<api::Value>(builder.finished_data()).unwrap();
    let error = context.value_to_ir("derived", value).unwrap_err();
    assert_eq!(error.details[0].code, "unknown_field");
}

#[test]
fn imported_aliases_are_preserved_but_explicitly_not_query_bound() {
    let mut definition = IndexDefinitionIr::new(
        None,
        vec![FieldIr::new(
            FieldName::parse("parent").unwrap(),
            DataTypeIr::Reference {
                document_type: "parent".to_owned(),
            },
            None,
            FieldCapabilities::new(false, false, false, false, false),
        )],
        Vec::new(),
        RankingPolicyIr::DefaultRelevanceV1,
        PrefixPolicy::Disabled,
        TypoPolicy::Disabled,
    )
    .unwrap();
    definition.set_imported_fields(vec![ImportedFieldDefinitionIr {
        reference_field: FieldName::parse("parent").unwrap(),
        target_field: "name".to_owned(),
        alias: FieldName::parse("parent_name").unwrap(),
        properties: BTreeMap::new(),
    }]);
    let bytes = crate::index_definition_ir_to_api(&definition);
    let context =
        IndexContext::from_api(crate::__flatbuffers::root::<api::IndexDefinition>(&bytes).unwrap())
            .unwrap();
    let error = crate::bind_filter_expression(
        "parent_name = 'Acme'",
        context.definition(),
        &BTreeMap::new(),
    )
    .unwrap_err();
    assert_eq!(error.details[0].code, "imported_field_query_not_supported");

    let mut invalid = IndexDefinitionIr::new(
        None,
        vec![FieldIr::new(
            FieldName::parse("title").unwrap(),
            DataTypeIr::Primitive(PrimitiveDataTypeIr::String),
            None,
            FieldCapabilities::new(true, true, false, false, false),
        )],
        Vec::new(),
        RankingPolicyIr::DefaultRelevanceV1,
        PrefixPolicy::Disabled,
        TypoPolicy::Disabled,
    )
    .unwrap();
    invalid.set_schema_fields(vec![FieldIr::new(
        FieldName::parse("derived_ref").unwrap(),
        DataTypeIr::Reference {
            document_type: "parent".to_owned(),
        },
        None,
        FieldCapabilities::new(false, false, false, false, false),
    )]);
    invalid.set_imported_fields(vec![ImportedFieldDefinitionIr {
        reference_field: FieldName::parse("derived_ref").unwrap(),
        target_field: "name".to_owned(),
        alias: FieldName::parse("derived_name").unwrap(),
        properties: BTreeMap::new(),
    }]);
    let bytes = crate::index_definition_ir_to_api(&invalid);
    let error = crate::index_definition_to_ir(
        crate::__flatbuffers::root::<api::IndexDefinition>(&bytes).unwrap(),
    )
    .unwrap_err();
    assert_eq!(error.details[0].code, "unknown_field");
}

#[test]
fn repeated_source_errors_identify_clause_and_local_offsets() {
    let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
    let first = builder.create_string("brand = 'Acme'");
    let second = builder.create_string("price <= ");
    let first = api::FilterClause::create(
        &mut builder,
        &api::FilterClauseArgs {
            source: Some(first),
            ..Default::default()
        },
    );
    let second = api::FilterClause::create(
        &mut builder,
        &api::FilterClauseArgs {
            source: Some(second),
            ..Default::default()
        },
    );
    let textual = builder.create_vector(&[first, second]);
    let request = api::FilterRequest::create(
        &mut builder,
        &api::FilterRequestArgs {
            textual: Some(textual),
            ..Default::default()
        },
    );
    builder.finish(request, None);
    let request = crate::__flatbuffers::root::<api::FilterRequest>(builder.finished_data())
        .expect("valid filter request");

    let error = context().filters_to_ir(Some(request)).unwrap_err();
    let detail = &error.details[0];
    assert_eq!(detail.code, "syntax_error");
    assert_eq!(detail.metadata.get("clause_index"), Some(&"1".to_owned()));
    assert!(detail.metadata.contains_key("start_offset"));
    assert!(detail.metadata.contains_key("end_offset"));
    assert!(!detail.metadata.contains_key("span_start"));
    assert!(!detail.metadata.contains_key("span_end"));
}

#[test]
fn textual_bindings_are_local_to_each_clause() {
    let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();

    let price_key = builder.create_string("value");
    let price_value = api::Int64Value::create(&mut builder, &api::Int64ValueArgs { value: 120 });
    let price_value = api::Value::create(
        &mut builder,
        &api::ValueArgs {
            kind_type: api::ValueKind::Int64Value,
            kind: Some(price_value.as_union_value()),
        },
    );
    let price_binding = api::ValueEntry::create(
        &mut builder,
        &api::ValueEntryArgs {
            key: Some(price_key),
            value: Some(price_value),
        },
    );
    let price_bindings = builder.create_vector(&[price_binding]);
    let price_source = builder.create_string("price <= :value");
    let price_clause = api::FilterClause::create(
        &mut builder,
        &api::FilterClauseArgs {
            source: Some(price_source),
            bindings: Some(price_bindings),
        },
    );

    let rating_key = builder.create_string("value");
    let rating_value = api::Int64Value::create(&mut builder, &api::Int64ValueArgs { value: 4 });
    let rating_value = api::Value::create(
        &mut builder,
        &api::ValueArgs {
            kind_type: api::ValueKind::Int64Value,
            kind: Some(rating_value.as_union_value()),
        },
    );
    let rating_binding = api::ValueEntry::create(
        &mut builder,
        &api::ValueEntryArgs {
            key: Some(rating_key),
            value: Some(rating_value),
        },
    );
    let rating_bindings = builder.create_vector(&[rating_binding]);
    let rating_source = builder.create_string("rating >= :value");
    let rating_clause = api::FilterClause::create(
        &mut builder,
        &api::FilterClauseArgs {
            source: Some(rating_source),
            bindings: Some(rating_bindings),
        },
    );

    let textual = builder.create_vector(&[price_clause, rating_clause]);
    let request = api::FilterRequest::create(
        &mut builder,
        &api::FilterRequestArgs {
            textual: Some(textual),
            ..Default::default()
        },
    );
    builder.finish(request, None);
    let request = crate::__flatbuffers::root::<api::FilterRequest>(builder.finished_data())
        .expect("valid filter request");

    let filter = context()
        .filters_to_ir(Some(request))
        .expect("clause-local bindings resolve independently")
        .expect("textual filters produce a filter");
    assert!(matches!(filter, FilterIr::And(clauses) if clauses.len() == 2));
}

#[test]
fn binding_from_one_clause_cannot_satisfy_another_clause() {
    let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
    let key = builder.create_string("value");
    let value = api::Int64Value::create(&mut builder, &api::Int64ValueArgs { value: 120 });
    let value = api::Value::create(
        &mut builder,
        &api::ValueArgs {
            kind_type: api::ValueKind::Int64Value,
            kind: Some(value.as_union_value()),
        },
    );
    let binding = api::ValueEntry::create(
        &mut builder,
        &api::ValueEntryArgs {
            key: Some(key),
            value: Some(value),
        },
    );
    let bindings = builder.create_vector(&[binding]);
    let first_source = builder.create_string("price <= :value");
    let first = api::FilterClause::create(
        &mut builder,
        &api::FilterClauseArgs {
            source: Some(first_source),
            bindings: Some(bindings),
        },
    );
    let second_source = builder.create_string("rating >= :value");
    let second = api::FilterClause::create(
        &mut builder,
        &api::FilterClauseArgs {
            source: Some(second_source),
            ..Default::default()
        },
    );
    let textual = builder.create_vector(&[first, second]);
    let request = api::FilterRequest::create(
        &mut builder,
        &api::FilterRequestArgs {
            textual: Some(textual),
            ..Default::default()
        },
    );
    builder.finish(request, None);
    let request = crate::__flatbuffers::root::<api::FilterRequest>(builder.finished_data())
        .expect("valid filter request");

    let error = context().filters_to_ir(Some(request)).unwrap_err();
    assert_eq!(error.details[0].code, "unknown_parameter");
    assert_eq!(
        error.details[0].metadata.get("clause_index"),
        Some(&"1".to_owned())
    );
}

#[test]
fn duplicate_bindings_in_one_clause_are_rejected() {
    let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
    let key_a = builder.create_string("value");
    let key_b = builder.create_string("value");
    let value_a = api::Int64Value::create(&mut builder, &api::Int64ValueArgs { value: 120 });
    let value_b = api::Int64Value::create(&mut builder, &api::Int64ValueArgs { value: 121 });
    let value_a = api::Value::create(
        &mut builder,
        &api::ValueArgs {
            kind_type: api::ValueKind::Int64Value,
            kind: Some(value_a.as_union_value()),
        },
    );
    let value_b = api::Value::create(
        &mut builder,
        &api::ValueArgs {
            kind_type: api::ValueKind::Int64Value,
            kind: Some(value_b.as_union_value()),
        },
    );
    let binding_a = api::ValueEntry::create(
        &mut builder,
        &api::ValueEntryArgs {
            key: Some(key_a),
            value: Some(value_a),
        },
    );
    let binding_b = api::ValueEntry::create(
        &mut builder,
        &api::ValueEntryArgs {
            key: Some(key_b),
            value: Some(value_b),
        },
    );
    let bindings = builder.create_vector(&[binding_a, binding_b]);
    let source = builder.create_string("price <= :value");
    let clause = api::FilterClause::create(
        &mut builder,
        &api::FilterClauseArgs {
            source: Some(source),
            bindings: Some(bindings),
        },
    );
    let textual = builder.create_vector(&[clause]);
    let request = api::FilterRequest::create(
        &mut builder,
        &api::FilterRequestArgs {
            textual: Some(textual),
            ..Default::default()
        },
    );
    builder.finish(request, None);
    let request = crate::__flatbuffers::root::<api::FilterRequest>(builder.finished_data())
        .expect("valid filter request");

    let error = context().filters_to_ir(Some(request)).unwrap_err();
    assert_eq!(error.details[0].code, "duplicate_parameter");
}

#[test]
fn legacy_global_textual_filter_fields_are_rejected() {
    let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
    let source = builder.create_string("price <= :value");
    let sources = builder.create_vector(&[source]);
    let request = api::FilterRequest::create(
        &mut builder,
        &api::FilterRequestArgs {
            sources: Some(sources),
            ..Default::default()
        },
    );
    builder.finish(request, None);
    let request = crate::__flatbuffers::root::<api::FilterRequest>(builder.finished_data())
        .expect("valid legacy filter request");

    let error = context().filters_to_ir(Some(request)).unwrap_err();
    assert_eq!(error.details[0].code, "invalid_filter");
}

#[test]
fn v2_query_variant_malformed_payloads_are_rejected_explicitly() {
    fn bare_expression(kind: api::QueryExprKind) -> Vec<u8> {
        let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
        macro_rules! empty_payload {
            ($table:ident, $args:ident) => {
                api::$table::create(&mut builder, &api::$args::default()).as_union_value()
            };
        }
        let payload = match kind {
            api::QueryExprKind::TermQuery => empty_payload!(TermQuery, TermQueryArgs),
            api::QueryExprKind::QueryContains => {
                empty_payload!(QueryContains, QueryContainsArgs)
            }
            api::QueryExprKind::QueryAnd => empty_payload!(QueryAnd, QueryAndArgs),
            api::QueryExprKind::QueryOr => empty_payload!(QueryOr, QueryOrArgs),
            api::QueryExprKind::QueryNot => empty_payload!(QueryNot, QueryNotArgs),
            api::QueryExprKind::QueryTrue => empty_payload!(QueryTrue, QueryTrueArgs),
            api::QueryExprKind::QueryFalse => empty_payload!(QueryFalse, QueryFalseArgs),
            api::QueryExprKind::QueryComparison => {
                empty_payload!(QueryComparison, QueryComparisonArgs)
            }
            api::QueryExprKind::QueryRange => empty_payload!(QueryRange, QueryRangeArgs),
            api::QueryExprKind::QueryIn => empty_payload!(QueryIn, QueryInArgs),
            api::QueryExprKind::QueryPhrase => empty_payload!(QueryPhrase, QueryPhraseArgs),
            api::QueryExprKind::QueryNear => empty_payload!(QueryNear, QueryNearArgs),
            api::QueryExprKind::QueryONear => empty_payload!(QueryONear, QueryONearArgs),
            api::QueryExprKind::QuerySameElement => {
                empty_payload!(QuerySameElement, QuerySameElementArgs)
            }
            api::QueryExprKind::QueryEquiv => empty_payload!(QueryEquiv, QueryEquivArgs),
            api::QueryExprKind::QueryAlternatives => {
                empty_payload!(QueryAlternatives, QueryAlternativesArgs)
            }
            api::QueryExprKind::QueryFuzzy => empty_payload!(QueryFuzzy, QueryFuzzyArgs),
            api::QueryExprKind::QueryMatches => empty_payload!(QueryMatches, QueryMatchesArgs),
            api::QueryExprKind::QueryText => empty_payload!(QueryText, QueryTextArgs),
            api::QueryExprKind::QueryUserInput => {
                empty_payload!(QueryUserInput, QueryUserInputArgs)
            }
            api::QueryExprKind::QueryUserQuery => {
                empty_payload!(QueryUserQuery, QueryUserQueryArgs)
            }
            api::QueryExprKind::QueryDotProduct => {
                empty_payload!(QueryDotProduct, QueryDotProductArgs)
            }
            api::QueryExprKind::QueryWeightedSet => {
                empty_payload!(QueryWeightedSet, QueryWeightedSetArgs)
            }
            api::QueryExprKind::QueryWand => empty_payload!(QueryWand, QueryWandArgs),
            api::QueryExprKind::QueryWeakAnd => empty_payload!(QueryWeakAnd, QueryWeakAndArgs),
            api::QueryExprKind::QueryRank => empty_payload!(QueryRank, QueryRankArgs),
            api::QueryExprKind::QueryLabeled => empty_payload!(QueryLabeled, QueryLabeledArgs),
            api::QueryExprKind::QueryNonEmpty => {
                empty_payload!(QueryNonEmpty, QueryNonEmptyArgs)
            }
            api::QueryExprKind::QueryGeoLocation => {
                empty_payload!(QueryGeoLocation, QueryGeoLocationArgs)
            }
            api::QueryExprKind::QueryGeoBoundingBox => {
                empty_payload!(QueryGeoBoundingBox, QueryGeoBoundingBoxArgs)
            }
            api::QueryExprKind::QueryNearestNeighbor => {
                empty_payload!(QueryNearestNeighbor, QueryNearestNeighborArgs)
            }
            api::QueryExprKind::QueryPredicate => {
                empty_payload!(QueryPredicate, QueryPredicateArgs)
            }
            _ => unreachable!("unknown QueryExprKind in test fixture"),
        };
        let expression = api::QueryExpr::create(
            &mut builder,
            &api::QueryExprArgs {
                expr_type: kind,
                expr: Some(payload),
                annotations: None,
            },
        );
        builder.finish(expression, None);
        builder.finished_data().to_vec()
    }

    fn contains_expression() -> Vec<u8> {
        let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
        let field_name = builder.create_string("brand");
        let field = api::QueryFieldRef::create(
            &mut builder,
            &api::QueryFieldRefArgs {
                path: Some(field_name),
                element_index: None,
            },
        );
        let term_value = builder.create_string("Acme");
        let term_value = api::StringValue::create(
            &mut builder,
            &api::StringValueArgs {
                value: Some(term_value),
            },
        );
        let term_value = api::Value::create(
            &mut builder,
            &api::ValueArgs {
                kind_type: api::ValueKind::StringValue,
                kind: Some(term_value.as_union_value()),
            },
        );
        let term = api::TermQuery::create(
            &mut builder,
            &api::TermQueryArgs {
                value: Some(term_value),
            },
        );
        let item = api::QueryExpr::create(
            &mut builder,
            &api::QueryExprArgs {
                expr_type: api::QueryExprKind::TermQuery,
                expr: Some(term.as_union_value()),
                annotations: None,
            },
        );
        let contains = api::QueryContains::create(
            &mut builder,
            &api::QueryContainsArgs {
                field: Some(field),
                item: Some(item),
            },
        );
        let expression = api::QueryExpr::create(
            &mut builder,
            &api::QueryExprArgs {
                expr_type: api::QueryExprKind::QueryContains,
                expr: Some(contains.as_union_value()),
                annotations: None,
            },
        );
        builder.finish(expression, None);
        builder.finished_data().to_vec()
    }

    fn boolean_expression(kind: api::QueryExprKind) -> Vec<u8> {
        let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
        let child = {
            let field_name = builder.create_string("brand");
            let field = api::QueryFieldRef::create(
                &mut builder,
                &api::QueryFieldRefArgs {
                    path: Some(field_name),
                    element_index: None,
                },
            );
            let value = builder.create_string("Acme");
            let value = api::StringValue::create(
                &mut builder,
                &api::StringValueArgs { value: Some(value) },
            );
            let value = api::Value::create(
                &mut builder,
                &api::ValueArgs {
                    kind_type: api::ValueKind::StringValue,
                    kind: Some(value.as_union_value()),
                },
            );
            let term =
                api::TermQuery::create(&mut builder, &api::TermQueryArgs { value: Some(value) });
            let item = api::QueryExpr::create(
                &mut builder,
                &api::QueryExprArgs {
                    expr_type: api::QueryExprKind::TermQuery,
                    expr: Some(term.as_union_value()),
                    annotations: None,
                },
            );
            let contains = api::QueryContains::create(
                &mut builder,
                &api::QueryContainsArgs {
                    field: Some(field),
                    item: Some(item),
                },
            );
            api::QueryExpr::create(
                &mut builder,
                &api::QueryExprArgs {
                    expr_type: api::QueryExprKind::QueryContains,
                    expr: Some(contains.as_union_value()),
                    annotations: None,
                },
            )
        };
        let clauses = builder.create_vector(&[child]);
        let expression = match kind {
            api::QueryExprKind::QueryAnd => {
                let value = api::QueryAnd::create(
                    &mut builder,
                    &api::QueryAndArgs {
                        clauses: Some(clauses),
                    },
                );
                api::QueryExpr::create(
                    &mut builder,
                    &api::QueryExprArgs {
                        expr_type: kind,
                        expr: Some(value.as_union_value()),
                        annotations: None,
                    },
                )
            }
            api::QueryExprKind::QueryOr => {
                let value = api::QueryOr::create(
                    &mut builder,
                    &api::QueryOrArgs {
                        clauses: Some(clauses),
                    },
                );
                api::QueryExpr::create(
                    &mut builder,
                    &api::QueryExprArgs {
                        expr_type: kind,
                        expr: Some(value.as_union_value()),
                        annotations: None,
                    },
                )
            }
            api::QueryExprKind::QueryNot => {
                let value = api::QueryNot::create(
                    &mut builder,
                    &api::QueryNotArgs {
                        clause: Some(child),
                    },
                );
                api::QueryExpr::create(
                    &mut builder,
                    &api::QueryExprArgs {
                        expr_type: kind,
                        expr: Some(value.as_union_value()),
                        annotations: None,
                    },
                )
            }
            _ => unreachable!(),
        };
        builder.finish(expression, None);
        builder.finished_data().to_vec()
    }

    let mut query_context = context();
    query_context.definition.fields[0].capabilities.searchable = true;
    let supported = [
        ("QueryContains", contains_expression()),
        ("QueryAnd", boolean_expression(api::QueryExprKind::QueryAnd)),
        ("QueryOr", boolean_expression(api::QueryExprKind::QueryOr)),
        ("QueryNot", boolean_expression(api::QueryExprKind::QueryNot)),
    ];
    for (name, bytes) in &supported {
        let expression = crate::__flatbuffers::root::<api::QueryExpr>(bytes).unwrap();
        assert!(
            query_context.query_expr_to_ir(expression, "query").is_ok(),
            "{name} should be in the characterized supported subset"
        );
    }

    let unsupported = [
        ("TermQuery", api::QueryExprKind::TermQuery),
        ("QueryTrue", api::QueryExprKind::QueryTrue),
        ("QueryFalse", api::QueryExprKind::QueryFalse),
        ("QueryComparison", api::QueryExprKind::QueryComparison),
        ("QueryRange", api::QueryExprKind::QueryRange),
        ("QueryIn", api::QueryExprKind::QueryIn),
        ("QueryPhrase", api::QueryExprKind::QueryPhrase),
        ("QueryNear", api::QueryExprKind::QueryNear),
        ("QueryONear", api::QueryExprKind::QueryONear),
        ("QuerySameElement", api::QueryExprKind::QuerySameElement),
        ("QueryEquiv", api::QueryExprKind::QueryEquiv),
        ("QueryAlternatives", api::QueryExprKind::QueryAlternatives),
        ("QueryFuzzy", api::QueryExprKind::QueryFuzzy),
        ("QueryMatches", api::QueryExprKind::QueryMatches),
        ("QueryText", api::QueryExprKind::QueryText),
        ("QueryUserInput", api::QueryExprKind::QueryUserInput),
        ("QueryUserQuery", api::QueryExprKind::QueryUserQuery),
        ("QueryDotProduct", api::QueryExprKind::QueryDotProduct),
        ("QueryWeightedSet", api::QueryExprKind::QueryWeightedSet),
        ("QueryWand", api::QueryExprKind::QueryWand),
        ("QueryWeakAnd", api::QueryExprKind::QueryWeakAnd),
        ("QueryRank", api::QueryExprKind::QueryRank),
        ("QueryLabeled", api::QueryExprKind::QueryLabeled),
        ("QueryNonEmpty", api::QueryExprKind::QueryNonEmpty),
        ("QueryGeoLocation", api::QueryExprKind::QueryGeoLocation),
        (
            "QueryGeoBoundingBox",
            api::QueryExprKind::QueryGeoBoundingBox,
        ),
        (
            "QueryNearestNeighbor",
            api::QueryExprKind::QueryNearestNeighbor,
        ),
        ("QueryPredicate", api::QueryExprKind::QueryPredicate),
    ];
    assert_eq!(supported.len() + unsupported.len(), 32);
    for (name, kind) in unsupported {
        let bytes = bare_expression(kind);
        let expression = crate::__flatbuffers::root::<api::QueryExpr>(&bytes).unwrap();
        if let Err(error) = query_context.query_expr_to_ir(expression, "query") {
            assert!(
                error
                    .details
                    .iter()
                    .all(|detail| detail.code != "unsupported_query_expression"),
                "{name} must be rejected by validation, not an unsupported-variant fallback"
            );
        }
    }
}

#[test]
fn every_v2_query_variant_round_trips_through_typed_ir() {
    fn field(name: &str) -> QueryFieldRefIr {
        QueryFieldRefIr {
            path: FieldName::parse(name).unwrap(),
            element_index: Some(2),
        }
    }
    fn leaf() -> QueryExprIr {
        QueryExprIr {
            kind: QueryExprKindIr::True,
            annotations: BTreeMap::new(),
        }
    }
    fn term() -> QueryExprIr {
        QueryExprIr {
            kind: QueryExprKindIr::Term {
                value: RequestValueIr::String("Acme".to_owned()),
            },
            annotations: BTreeMap::new(),
        }
    }
    fn annotated(kind: QueryExprKindIr) -> QueryIr {
        let mut annotations = BTreeMap::new();
        annotations.insert("label".to_owned(), RequestValueIr::String("v2".to_owned()));
        QueryIr::Typed(Box::new(QueryExprIr { kind, annotations }))
    }
    fn request(query: QueryIr) -> SearchRequestIr {
        let mut request = SearchRequestIr::new(
            query,
            Vec::new(),
            None,
            Vec::new(),
            RankingPolicyIr::DefaultRelevanceV1,
            10,
            0,
            ProjectionIr::new(Vec::new()),
            HighlightingIr::Disabled,
            None,
            PrefixPolicy::Disabled,
            TypoPolicy::Disabled,
        );
        request.inputs.insert(
            "q".to_owned(),
            QueryInputIr::Literal(RequestValueIr::String("tensor".to_owned())),
        );
        request
    }

    let cases = vec![
        (
            "TermQuery",
            QueryExprKindIr::Term {
                value: RequestValueIr::String("x".to_owned()),
            },
        ),
        (
            "QueryContains",
            QueryExprKindIr::Contains {
                field: field("brand"),
                item: Box::new(term()),
            },
        ),
        ("QueryAnd", QueryExprKindIr::And(vec![leaf()])),
        ("QueryOr", QueryExprKindIr::Or(vec![leaf()])),
        ("QueryNot", QueryExprKindIr::Not(Box::new(leaf()))),
        ("QueryTrue", QueryExprKindIr::True),
        ("QueryFalse", QueryExprKindIr::False),
        (
            "QueryComparison",
            QueryExprKindIr::Comparison {
                field: field("price"),
                op: ComparisonOp::Gt,
                value: RequestValueIr::Int64(3),
            },
        ),
        (
            "QueryRange",
            QueryExprKindIr::Range {
                field: field("price"),
                lower: QueryRangeBoundIr::Finite(RequestValueIr::Int64(1)),
                upper: QueryRangeBoundIr::PositiveInfinity,
                include_lower: Some(true),
                include_upper: Some(false),
            },
        ),
        (
            "QueryIn",
            QueryExprKindIr::In {
                field: field("brand"),
                values: vec![RequestValueIr::String("Acme".to_owned())],
            },
        ),
        ("QueryPhrase", QueryExprKindIr::Phrase(vec![term()])),
        ("QueryNear", QueryExprKindIr::Near(vec![term()])),
        ("QueryONear", QueryExprKindIr::ONear(vec![term()])),
        (
            "QuerySameElement",
            QueryExprKindIr::SameElement(vec![term()]),
        ),
        ("QueryEquiv", QueryExprKindIr::Equiv(vec![term()])),
        (
            "QueryAlternatives",
            QueryExprKindIr::Alternatives(vec![QueryAlternativeIr {
                query: term(),
                exactness: Some(0.75),
            }]),
        ),
        (
            "QueryFuzzy",
            QueryExprKindIr::Fuzzy {
                value: RequestValueIr::String("acm".to_owned()),
            },
        ),
        (
            "QueryMatches",
            QueryExprKindIr::Matches {
                field: field("brand"),
                pattern: "A.*".to_owned(),
            },
        ),
        (
            "QueryText",
            QueryExprKindIr::Text {
                text: "Acme".to_owned(),
            },
        ),
        (
            "QueryUserInput",
            QueryExprKindIr::UserInput {
                value: "query".to_owned(),
            },
        ),
        ("QueryUserQuery", QueryExprKindIr::UserQuery),
        (
            "QueryDotProduct",
            QueryExprKindIr::DotProduct {
                field: field("price"),
                items: vec![WeightedQueryItemIr {
                    key: RequestValueIr::String("x".to_owned()),
                    weight: 2,
                }],
            },
        ),
        (
            "QueryWeightedSet",
            QueryExprKindIr::WeightedSet {
                field: field("brand"),
                items: vec![WeightedQueryItemIr {
                    key: RequestValueIr::String("x".to_owned()),
                    weight: 2,
                }],
            },
        ),
        (
            "QueryWand",
            QueryExprKindIr::Wand {
                field: field("brand"),
                items: vec![WeightedQueryItemIr {
                    key: RequestValueIr::String("x".to_owned()),
                    weight: 2,
                }],
            },
        ),
        ("QueryWeakAnd", QueryExprKindIr::WeakAnd(vec![term()])),
        (
            "QueryRank",
            QueryExprKindIr::Rank {
                matching: Box::new(term()),
                ranking: vec![leaf()],
            },
        ),
        (
            "QueryLabeled",
            QueryExprKindIr::Labeled {
                query: Box::new(term()),
                label: "my_label".to_owned(),
                score: 1.25,
            },
        ),
        ("QueryNonEmpty", QueryExprKindIr::NonEmpty(Box::new(term()))),
        (
            "QueryGeoLocation",
            QueryExprKindIr::GeoLocation {
                field: field("brand"),
                latitude: 59.9,
                longitude: 10.7,
                radius: "10 km".to_owned(),
            },
        ),
        (
            "QueryGeoBoundingBox",
            QueryExprKindIr::GeoBoundingBox {
                field: field("brand"),
                south: 59.0,
                west: 10.0,
                north: 60.0,
                east: 11.0,
            },
        ),
        (
            "QueryNearestNeighbor",
            QueryExprKindIr::NearestNeighbor {
                document_field: field("brand"),
                query_tensor: "q".to_owned(),
            },
        ),
        (
            "QueryPredicate",
            QueryExprKindIr::Predicate {
                field: field("brand"),
                features: BTreeMap::from([(
                    String::from("hobby"),
                    RequestValueIr::String("ski".to_owned()),
                )]),
                range_features: BTreeMap::new(),
            },
        ),
    ];
    assert_eq!(cases.len(), 32);

    let mut query_context = context();
    query_context.definition.fields[0].capabilities.searchable = true;
    for (name, kind) in cases {
        let expected = annotated(kind);
        let expected = if matches!(
            name,
            "TermQuery"
                | "QueryPhrase"
                | "QueryNear"
                | "QueryONear"
                | "QuerySameElement"
                | "QueryEquiv"
                | "QueryAlternatives"
                | "QueryFuzzy"
                | "QueryText"
                | "QueryUserInput"
                | "QueryWeakAnd"
                | "QueryRank"
                | "QueryLabeled"
                | "QueryNonEmpty"
        ) {
            let QueryIr::Typed(inner) = expected else {
                unreachable!()
            };
            QueryIr::Typed(Box::new(QueryExprIr {
                kind: QueryExprKindIr::Contains {
                    field: field("brand"),
                    item: inner,
                },
                annotations: BTreeMap::new(),
            }))
        } else {
            expected
        };
        let bytes = crate::search_request_ir_to_api(&request(expected.clone()));
        let api_request = crate::__flatbuffers::root::<api::SearchRequest>(&bytes).unwrap();
        let actual = query_context.search_to_ir(api_request).unwrap();
        assert_eq!(
            actual.query(),
            &expected,
            "{name} lost typed query semantics"
        );
    }
}

#[test]
fn query_user_input_accepts_annotation_or_query_model_default_index() {
    fn request_bytes(annotation: bool, query_model: bool) -> Vec<u8> {
        let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
        let input = builder.create_string("free form input");
        let input = api::QueryUserInput::create(
            &mut builder,
            &api::QueryUserInputArgs { value: Some(input) },
        );
        let annotations = if annotation {
            let key = builder.create_string("defaultIndex");
            let value = builder.create_string("title");
            let value = api::StringValue::create(
                &mut builder,
                &api::StringValueArgs { value: Some(value) },
            );
            let value = api::Value::create(
                &mut builder,
                &api::ValueArgs {
                    kind_type: api::ValueKind::StringValue,
                    kind: Some(value.as_union_value()),
                },
            );
            let entry = api::ValueEntry::create(
                &mut builder,
                &api::ValueEntryArgs {
                    key: Some(key),
                    value: Some(value),
                },
            );
            Some(builder.create_vector(&[entry]))
        } else {
            None
        };
        let expression = api::QueryExpr::create(
            &mut builder,
            &api::QueryExprArgs {
                expr_type: api::QueryExprKind::QueryUserInput,
                expr: Some(input.as_union_value()),
                annotations,
            },
        );
        let typed = api::TypedQuery::create(
            &mut builder,
            &api::TypedQueryArgs {
                query: Some(expression),
            },
        );
        let source = api::QuerySource::create(
            &mut builder,
            &api::QuerySourceArgs {
                source_type: api::QuerySourceKind::TypedQuery,
                source: Some(typed.as_union_value()),
            },
        );
        let query_model = if query_model {
            let default_index = builder.create_string("title");
            Some(api::QueryModel::create(
                &mut builder,
                &api::QueryModelArgs {
                    default_index: Some(default_index),
                    ..Default::default()
                },
            ))
        } else {
            None
        };
        let request = api::SearchRequest::create(
            &mut builder,
            &api::SearchRequestArgs {
                query: Some(source),
                query_model,
                ..Default::default()
            },
        );
        builder.finish(request, None);
        builder.finished_data().to_vec()
    }

    for (name, bytes) in [
        ("query expression annotation", request_bytes(true, false)),
        ("query model", request_bytes(false, true)),
    ] {
        let request = crate::__flatbuffers::root::<api::SearchRequest>(&bytes).unwrap();
        let converted = context().search_to_ir(request);
        assert!(
            converted.is_ok(),
            "{name} should provide user input context"
        );
    }

    let bytes = request_bytes(false, false);
    let request = crate::__flatbuffers::root::<api::SearchRequest>(&bytes).unwrap();
    let error = context().search_to_ir(request).unwrap_err();
    assert_eq!(error.details[0].code, "missing_field_context");
}

#[test]
fn sort_missing_as_is_not_rejected_or_dropped() {
    let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
    let field = builder.create_string("price");
    let field = api::FieldSortTarget::create(
        &mut builder,
        &api::FieldSortTargetArgs { field: Some(field) },
    );
    let target = api::SortTarget::create(
        &mut builder,
        &api::SortTargetArgs {
            target_type: api::SortTargetKind::FieldSortTarget,
            target: Some(field.as_union_value()),
        },
    );
    let value = api::Int64Value::create(&mut builder, &api::Int64ValueArgs { value: -1 });
    let value = api::Value::create(
        &mut builder,
        &api::ValueArgs {
            kind_type: api::ValueKind::Int64Value,
            kind: Some(value.as_union_value()),
        },
    );
    let missing = api::MissingAs::create(&mut builder, &api::MissingAsArgs { value: Some(value) });
    let missing = api::MissingPolicy::create(
        &mut builder,
        &api::MissingPolicyArgs {
            policy_type: api::MissingPolicyKind::MissingAs,
            policy: Some(missing.as_union_value()),
        },
    );
    let key = builder.create_string("locale");
    let annotation_value = builder.create_string("en-US");
    let annotation_value = api::StringValue::create(
        &mut builder,
        &api::StringValueArgs {
            value: Some(annotation_value),
        },
    );
    let annotation_value = api::Value::create(
        &mut builder,
        &api::ValueArgs {
            kind_type: api::ValueKind::StringValue,
            kind: Some(annotation_value.as_union_value()),
        },
    );
    let annotation = api::ValueEntry::create(
        &mut builder,
        &api::ValueEntryArgs {
            key: Some(key),
            value: Some(annotation_value),
        },
    );
    let annotations = builder.create_vector(&[annotation]);
    let sort = api::SortExpr::create(
        &mut builder,
        &api::SortExprArgs {
            target: Some(target),
            direction: api::SortDirection::Asc,
            missing: Some(missing),
            annotations: Some(annotations),
        },
    );
    builder.finish(sort, None);
    let sort = crate::__flatbuffers::root::<api::SortExpr>(builder.finished_data()).unwrap();

    let mut query_context = context();
    query_context.definition.fields[1].capabilities.sortable = true;
    let converted = query_context
        .sort_to_ir(sort)
        .expect("MissingAs and sort annotations must have an IR representation");
    assert_eq!(
        converted.missing(),
        MissingPlacement::As(RequestValueIr::Int64(-1))
    );
    assert_eq!(
        converted.annotations().get("locale"),
        Some(&RequestValueIr::String("en-US".to_owned()))
    );

    let request = SearchRequestIr::new(
        QueryIr::MatchAll,
        Vec::new(),
        None,
        vec![converted],
        RankingPolicyIr::DefaultRelevanceV1,
        10,
        0,
        ProjectionIr::new(Vec::new()),
        HighlightingIr::Disabled,
        None,
        PrefixPolicy::Disabled,
        TypoPolicy::Disabled,
    );
    let bytes = crate::search_request_ir_to_api(&request);
    let encoded = crate::__flatbuffers::root::<api::SearchRequest>(&bytes).unwrap();
    let encoded_sort = encoded.sort().unwrap().get(0);
    assert_eq!(
        encoded_sort
            .missing()
            .and_then(|value| value.policy_as_missing_as())
            .and_then(|value| value.value())
            .and_then(|value| value.kind_as_int_64_value())
            .map(|value| value.value()),
        Some(-1)
    );
    assert_eq!(encoded_sort.annotations().unwrap().get(0).key(), "locale");
}

#[test]
fn positive_infinity_filter_lower_bound_is_not_treated_as_unbounded() {
    let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
    let field = builder.create_string("price");
    let field = api::QueryFieldRef::create(
        &mut builder,
        &api::QueryFieldRefArgs {
            path: Some(field),
            element_index: None,
        },
    );
    let lower =
        api::PositiveInfinityBound::create(&mut builder, &api::PositiveInfinityBoundArgs {});
    let lower = api::RangeBound::create(
        &mut builder,
        &api::RangeBoundArgs {
            bound_type: api::RangeBoundKind::PositiveInfinityBound,
            bound: Some(lower.as_union_value()),
        },
    );
    let upper_value = api::Int64Value::create(&mut builder, &api::Int64ValueArgs { value: 10 });
    let upper_value = api::Value::create(
        &mut builder,
        &api::ValueArgs {
            kind_type: api::ValueKind::Int64Value,
            kind: Some(upper_value.as_union_value()),
        },
    );
    let upper = api::FiniteRangeBound::create(
        &mut builder,
        &api::FiniteRangeBoundArgs {
            value: Some(upper_value),
        },
    );
    let upper = api::RangeBound::create(
        &mut builder,
        &api::RangeBoundArgs {
            bound_type: api::RangeBoundKind::FiniteRangeBound,
            bound: Some(upper.as_union_value()),
        },
    );
    let range = api::FilterRange::create(
        &mut builder,
        &api::FilterRangeArgs {
            field: Some(field),
            lower: Some(lower),
            upper: Some(upper),
            ..Default::default()
        },
    );
    let expression = api::FilterExpr::create(
        &mut builder,
        &api::FilterExprArgs {
            expr_type: api::FilterExprKind::FilterRange,
            expr: Some(range.as_union_value()),
        },
    );
    builder.finish(expression, None);
    let expression =
        crate::__flatbuffers::root::<api::FilterExpr>(builder.finished_data()).unwrap();

    assert!(
        context().filter_to_ir(expression).is_err(),
        "a lower +infinity bound must not collapse into an absent lower bound"
    );
}
