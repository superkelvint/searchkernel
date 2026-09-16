//! Private encoding of product values into the portable FlatBuffers contract.

use crate::analyze::{AnalyzeBuilder, AnalyzeTarget};
use crate::filter::{pending_value, ComparisonOp, Filter, FilterNode};
use crate::patch::{Patch, PatchOperation};
use crate::schema::{Analyzer, Field, FieldKind, IndexDefinition, SearchField};
use crate::search::{
    Facet, FacetSort, MissingPlacement, Ranking, SearchBuilder, Sort, SortDirection,
};
use crate::{Error, Result};
use searchkernel_ir::{__flatbuffers as flatbuffers, api};
use serde_json::Value;

pub(crate) fn definition(definition: &IndexDefinition) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let root = index_definition(&mut builder, definition);
    builder.finish(root, None);
    builder.finished_data().to_vec()
}

pub(crate) fn insert(id: &str, value: &Value) -> Result<Vec<u8>> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let document = document(&mut builder, id, value)?;
    let root = api::InsertRequest::create(
        &mut builder,
        &api::InsertRequestArgs {
            document: Some(document),
        },
    );
    builder.finish(root, None);
    Ok(builder.finished_data().to_vec())
}

pub(crate) fn upsert(id: &str, value: &Value) -> Result<Vec<u8>> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let document = document(&mut builder, id, value)?;
    let root = api::UpsertRequest::create(
        &mut builder,
        &api::UpsertRequestArgs {
            document: Some(document),
        },
    );
    builder.finish(root, None);
    Ok(builder.finished_data().to_vec())
}

pub(crate) fn patch(id: &str, patch: &Patch) -> Result<Vec<u8>> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let id = builder.create_string(id);
    let operations = patch
        .operations
        .iter()
        .map(|operation| patch_operation(&mut builder, operation))
        .collect::<Result<Vec<_>>>()?;
    let operations = builder.create_vector(&operations);
    let root = api::PatchRequest::create(
        &mut builder,
        &api::PatchRequestArgs {
            id: Some(id),
            operations: Some(operations),
        },
    );
    builder.finish(root, None);
    Ok(builder.finished_data().to_vec())
}

pub(crate) fn delete(id: &str) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let id = builder.create_string(id);
    let root = api::DeleteRequest::create(&mut builder, &api::DeleteRequestArgs { id: Some(id) });
    builder.finish(root, None);
    builder.finished_data().to_vec()
}

pub(crate) fn get(id: &str, selected_fields: Option<&[String]>) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let id = builder.create_string(id);
    let projection = selected_fields.map(|fields| projection(&mut builder, fields));
    let root = api::GetRequest::create(
        &mut builder,
        &api::GetRequestArgs {
            id: Some(id),
            projection,
        },
    );
    builder.finish(root, None);
    builder.finished_data().to_vec()
}

pub(crate) fn analyze(request: &AnalyzeBuilder<'_>) -> Result<Vec<u8>> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let (target_type, target) = match request.target.as_ref() {
        Some(AnalyzeTarget::Analyzer(analyzer)) => (
            api::AnalyzeTarget::Analyzer,
            analyzer_value(&mut builder, analyzer).as_union_value(),
        ),
        Some(AnalyzeTarget::Field) => {
            let field = request.field_name.as_deref().ok_or_else(|| {
                Error::search(
                    crate::ErrorCode::InvalidArgument,
                    "analyze field target is missing a field name",
                )
            })?;
            let field = builder.create_string(field);
            let target = api::AnalyzeFieldTarget::create(
                &mut builder,
                &api::AnalyzeFieldTargetArgs { field: Some(field) },
            );
            (
                api::AnalyzeTarget::AnalyzeFieldTarget,
                target.as_union_value(),
            )
        }
        None => {
            return Err(Error::search(
                crate::ErrorCode::InvalidArgument,
                "analyze requires an analyzer or field target",
            ))
        }
    };
    let text = builder.create_string(&request.text);
    let language = request
        .language
        .as_deref()
        .map(|value| builder.create_string(value));
    let root = api::AnalyzeRequest::create(
        &mut builder,
        &api::AnalyzeRequestArgs {
            target_type,
            target: Some(target),
            text: Some(text),
            language,
        },
    );
    builder.finish(root, None);
    Ok(builder.finished_data().to_vec())
}

pub(crate) fn search(request: &SearchBuilder) -> Result<Vec<u8>> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let query = builder.create_string(&request.query);
    let search_fields = request
        .search_fields
        .as_ref()
        .map(|fields| search_field_selection(&mut builder, fields));
    let filter = match request.filters.as_slice() {
        [] => None,
        [filter] => Some(filter_expr(&mut builder, filter)?),
        filters => Some(filter_expr(
            &mut builder,
            &Filter::all(filters.iter().cloned()),
        )?),
    };
    let sorts = request
        .sorts
        .iter()
        .map(|sort| sort_expr(&mut builder, sort))
        .collect::<Vec<_>>();
    let sorts = builder.create_vector(&sorts);
    let facets = request
        .facets
        .iter()
        .map(|facet| facet_request(&mut builder, facet))
        .collect::<Vec<_>>();
    let facets = builder.create_vector(&facets);
    let ranking = request
        .ranking
        .as_ref()
        .map(|ranking| ranking_policy(&mut builder, ranking));
    let limit = request.limit.map(api::OptionalUInt32::new);
    let offset = request.offset.map(api::OptionalUInt32::new);
    let projection = request
        .projection
        .as_ref()
        .map(|fields| projection(&mut builder, fields));
    let highlighting = request
        .highlighting
        .as_ref()
        .map(|fields| highlighting(&mut builder, fields));
    let language = request
        .language
        .as_deref()
        .map(|value| builder.create_string(value));
    let root = api::SearchRequest::create(
        &mut builder,
        &api::SearchRequestArgs {
            query: Some(query),
            search_fields,
            filter,
            sort: Some(sorts),
            facets: Some(facets),
            ranking,
            limit: limit.as_ref(),
            offset: offset.as_ref(),
            projection,
            highlighting,
            language,
            query_expr: None,
        },
    );
    builder.finish(root, None);
    Ok(builder.finished_data().to_vec())
}

fn index_definition<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    definition: &IndexDefinition,
) -> flatbuffers::WIPOffset<api::IndexDefinition<'a>> {
    let name = definition
        .name
        .as_deref()
        .map(|value| builder.create_string(value));
    let fields = definition
        .fields
        .iter()
        .map(|field| field_definition(builder, field))
        .collect::<Vec<_>>();
    let fields = builder.create_vector(&fields);
    let default_search_fields = definition
        .default_search_fields
        .as_ref()
        .map(|fields| search_field_selection(builder, fields));
    api::IndexDefinition::create(
        builder,
        &api::IndexDefinitionArgs {
            name,
            fields: Some(fields),
            default_search_fields,
        },
    )
}

fn field_definition<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    field: &Field,
) -> flatbuffers::WIPOffset<api::FieldDefinition<'a>> {
    let name = builder.create_string(&field.name);
    let analyzer = field
        .analyzer
        .as_ref()
        .map(|analyzer| analyzer_value(builder, analyzer));
    let stored = field.stored.map(api::OptionalBool::new);
    let searchable = field.searchable.map(api::OptionalBool::new);
    let filterable = field.filterable.map(api::OptionalBool::new);
    let sortable = field.sortable.map(api::OptionalBool::new);
    let facetable = field.facetable.map(api::OptionalBool::new);
    api::FieldDefinition::create(
        builder,
        &api::FieldDefinitionArgs {
            name: Some(name),
            kind: field_kind(field.kind),
            cardinality: if field.multi {
                api::FieldCardinality::Multi
            } else {
                api::FieldCardinality::Unspecified
            },
            stored: stored.as_ref(),
            searchable: searchable.as_ref(),
            filterable: filterable.as_ref(),
            sortable: sortable.as_ref(),
            facetable: facetable.as_ref(),
            analyzer,
        },
    )
}

fn analyzer_value<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    analyzer: &Analyzer,
) -> flatbuffers::WIPOffset<api::Analyzer<'a>> {
    let name = builder.create_string(&analyzer.name);
    let options = (!analyzer.options.is_empty()).then(|| {
        let entries = analyzer
            .options
            .iter()
            .map(|(key, value)| {
                let key = builder.create_string(key);
                let value = value_offset(builder, value);
                api::ValueEntry::create(
                    builder,
                    &api::ValueEntryArgs {
                        key: Some(key),
                        value: Some(value),
                    },
                )
            })
            .collect::<Vec<_>>();
        builder.create_vector(&entries)
    });
    api::Analyzer::create(
        builder,
        &api::AnalyzerArgs {
            name: Some(name),
            version: analyzer.version,
            options,
        },
    )
}

fn search_field_selection<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    fields: &[SearchField],
) -> flatbuffers::WIPOffset<api::SearchFieldSelection<'a>> {
    let fields = fields
        .iter()
        .map(|field| {
            let name = builder.create_string(&field.field);
            let weight = field.weight.map(api::OptionalDouble::new);
            api::SearchField::create(
                builder,
                &api::SearchFieldArgs {
                    field: Some(name),
                    weight: weight.as_ref(),
                },
            )
        })
        .collect::<Vec<_>>();
    let fields = builder.create_vector(&fields);
    api::SearchFieldSelection::create(
        builder,
        &api::SearchFieldSelectionArgs {
            fields: Some(fields),
        },
    )
}

fn document<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    id: &str,
    value: &Value,
) -> Result<flatbuffers::WIPOffset<api::Document<'a>>> {
    let Value::Object(fields) = value else {
        return Err(Error::Serialization(
            "document must serialize as an object".to_owned(),
        ));
    };
    let id = builder.create_string(id);
    let fields = fields
        .iter()
        .map(|(name, value)| {
            let key = builder.create_string(name);
            let value = value_offset(builder, value);
            api::ValueEntry::create(
                builder,
                &api::ValueEntryArgs {
                    key: Some(key),
                    value: Some(value),
                },
            )
        })
        .collect::<Vec<_>>();
    let fields = builder.create_vector(&fields);
    Ok(api::Document::create(
        builder,
        &api::DocumentArgs {
            id: Some(id),
            fields: Some(fields),
        },
    ))
}

fn patch_operation<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    operation: &PatchOperation,
) -> Result<flatbuffers::WIPOffset<api::PatchOperation<'a>>> {
    let (operation_type, operation) = match operation {
        PatchOperation::Set { field, value } => {
            let field = builder.create_string(field);
            let value = value_offset(builder, pending_value(value).map_err(Error::Serialization)?);
            let operation = api::SetField::create(
                builder,
                &api::SetFieldArgs {
                    field: Some(field),
                    value: Some(value),
                },
            );
            (
                api::PatchOperationKind::SetField,
                operation.as_union_value(),
            )
        }
        PatchOperation::Unset { field } => {
            let field = builder.create_string(field);
            let operation =
                api::UnsetField::create(builder, &api::UnsetFieldArgs { field: Some(field) });
            (
                api::PatchOperationKind::UnsetField,
                operation.as_union_value(),
            )
        }
    };
    Ok(api::PatchOperation::create(
        builder,
        &api::PatchOperationArgs {
            operation_type,
            operation: Some(operation),
        },
    ))
}

fn value_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    value: &Value,
) -> flatbuffers::WIPOffset<api::Value<'a>> {
    let (kind_type, kind) = match value {
        Value::Null => (
            api::ValueKind::NullValueValue,
            api::NullValueValue::create(
                builder,
                &api::NullValueValueArgs {
                    value: api::NullValue::Null,
                },
            )
            .as_union_value(),
        ),
        Value::Bool(value) => (
            api::ValueKind::BoolValue,
            api::BoolValue::create(builder, &api::BoolValueArgs { value: *value }).as_union_value(),
        ),
        Value::Number(value) if value.as_i64().is_some() => {
            let value = api::Int64Value::create(
                builder,
                &api::Int64ValueArgs {
                    value: value.as_i64().unwrap_or_default(),
                },
            );
            (api::ValueKind::Int64Value, value.as_union_value())
        }
        Value::Number(value) if value.as_u64().is_some() => {
            let value = api::UInt64Value::create(
                builder,
                &api::UInt64ValueArgs {
                    value: value.as_u64().unwrap_or_default(),
                },
            );
            (api::ValueKind::UInt64Value, value.as_union_value())
        }
        Value::Number(value) => {
            let value = api::Float64Value::create(
                builder,
                &api::Float64ValueArgs {
                    value: value.as_f64().unwrap_or_default(),
                },
            );
            (api::ValueKind::Float64Value, value.as_union_value())
        }
        Value::String(value) => {
            let value = builder.create_string(value);
            let value =
                api::StringValue::create(builder, &api::StringValueArgs { value: Some(value) });
            (api::ValueKind::StringValue, value.as_union_value())
        }
        Value::Array(values) => {
            let values = values
                .iter()
                .map(|value| value_offset(builder, value))
                .collect::<Vec<_>>();
            let values = builder.create_vector(&values);
            let value = api::ValueList::create(
                builder,
                &api::ValueListArgs {
                    values: Some(values),
                },
            );
            (api::ValueKind::ValueList, value.as_union_value())
        }
        Value::Object(fields) => {
            let fields = fields
                .iter()
                .map(|(name, value)| {
                    let key = builder.create_string(name);
                    let value = value_offset(builder, value);
                    api::ValueEntry::create(
                        builder,
                        &api::ValueEntryArgs {
                            key: Some(key),
                            value: Some(value),
                        },
                    )
                })
                .collect::<Vec<_>>();
            let fields = builder.create_vector(&fields);
            let value = api::ValueObject::create(
                builder,
                &api::ValueObjectArgs {
                    fields: Some(fields),
                },
            );
            (api::ValueKind::ValueObject, value.as_union_value())
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

fn filter_expr<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    filter: &Filter,
) -> Result<flatbuffers::WIPOffset<api::FilterExpr<'a>>> {
    let (expr_type, expr) = match filter {
        Filter(FilterNode::All(filters)) => {
            let values = filters
                .iter()
                .map(|filter| filter_expr(builder, filter))
                .collect::<Result<Vec<_>>>()?;
            let values = builder.create_vector(&values);
            let value = api::FilterAnd::create(
                builder,
                &api::FilterAndArgs {
                    clauses: Some(values),
                },
            );
            (api::FilterExprKind::FilterAnd, value.as_union_value())
        }
        Filter(FilterNode::Any(filters)) => {
            let values = filters
                .iter()
                .map(|filter| filter_expr(builder, filter))
                .collect::<Result<Vec<_>>>()?;
            let values = builder.create_vector(&values);
            let value = api::FilterOr::create(
                builder,
                &api::FilterOrArgs {
                    clauses: Some(values),
                },
            );
            (api::FilterExprKind::FilterOr, value.as_union_value())
        }
        Filter(FilterNode::Not(filter)) => {
            let clause = filter_expr(builder, filter)?;
            let value = api::FilterNot::create(
                builder,
                &api::FilterNotArgs {
                    clause: Some(clause),
                },
            );
            (api::FilterExprKind::FilterNot, value.as_union_value())
        }
        Filter(FilterNode::Comparison { field, op, value }) => {
            let field = builder.create_string(field);
            let value = value_offset(builder, pending_value(value).map_err(Error::Serialization)?);
            let value = api::FilterComparison::create(
                builder,
                &api::FilterComparisonArgs {
                    field: Some(field),
                    op: comparison_op(*op),
                    value: Some(value),
                },
            );
            (
                api::FilterExprKind::FilterComparison,
                value.as_union_value(),
            )
        }
        Filter(FilterNode::Range { field, bounds }) => {
            let field = builder.create_string(field);
            let lower = bounds
                .lower
                .as_ref()
                .map(|value| pending_value(value).map(|value| value_offset(builder, value)))
                .transpose()
                .map_err(Error::Serialization)?;
            let upper = bounds
                .upper
                .as_ref()
                .map(|value| pending_value(value).map(|value| value_offset(builder, value)))
                .transpose()
                .map_err(Error::Serialization)?;
            let include_lower = bounds.include_lower.map(api::OptionalBool::new);
            let include_upper = bounds.include_upper.map(api::OptionalBool::new);
            let value = api::FilterRange::create(
                builder,
                &api::FilterRangeArgs {
                    field: Some(field),
                    lower,
                    upper,
                    include_lower: include_lower.as_ref(),
                    include_upper: include_upper.as_ref(),
                },
            );
            (api::FilterExprKind::FilterRange, value.as_union_value())
        }
        Filter(FilterNode::Exists { field }) => {
            let field = builder.create_string(field);
            let value =
                api::FilterExists::create(builder, &api::FilterExistsArgs { field: Some(field) });
            (api::FilterExprKind::FilterExists, value.as_union_value())
        }
    };
    Ok(api::FilterExpr::create(
        builder,
        &api::FilterExprArgs {
            expr_type,
            expr: Some(expr),
        },
    ))
}

fn projection<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    fields: &[String],
) -> flatbuffers::WIPOffset<api::FieldProjection<'a>> {
    let fields = fields
        .iter()
        .map(|field| builder.create_string(field))
        .collect::<Vec<_>>();
    let fields = builder.create_vector(&fields);
    api::FieldProjection::create(
        builder,
        &api::FieldProjectionArgs {
            fields: Some(fields),
        },
    )
}

fn highlighting<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    fields: &[String],
) -> flatbuffers::WIPOffset<api::HighlightRequest<'a>> {
    let fields = fields
        .iter()
        .map(|field| builder.create_string(field))
        .collect::<Vec<_>>();
    let fields = builder.create_vector(&fields);
    api::HighlightRequest::create(
        builder,
        &api::HighlightRequestArgs {
            fields: Some(fields),
        },
    )
}

fn sort_expr<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    sort: &Sort,
) -> flatbuffers::WIPOffset<api::SortExpr<'a>> {
    let field = builder.create_string(&sort.field);
    let missing = sort.missing.map(|missing| {
        api::OptionalMissingPlacement::new(match missing {
            MissingPlacement::First => api::MissingPlacement::First,
            MissingPlacement::Last => api::MissingPlacement::Last,
        })
    });
    api::SortExpr::create(
        builder,
        &api::SortExprArgs {
            field: Some(field),
            direction: match sort.direction {
                SortDirection::Asc => api::SortDirection::Asc,
                SortDirection::Desc => api::SortDirection::Desc,
            },
            missing: missing.as_ref(),
        },
    )
}

fn facet_request<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    facet: &Facet,
) -> flatbuffers::WIPOffset<api::FacetRequest<'a>> {
    let field = builder.create_string(&facet.field);
    let limit = facet.limit.map(api::OptionalUInt32::new);
    let sort = facet.sort.map(|sort| {
        api::OptionalFacetSort::new(match sort {
            FacetSort::CountDesc => api::FacetSort::CountDesc,
            FacetSort::CountAsc => api::FacetSort::CountAsc,
            FacetSort::ValueAsc => api::FacetSort::ValueAsc,
            FacetSort::ValueDesc => api::FacetSort::ValueDesc,
        })
    });
    api::FacetRequest::create(
        builder,
        &api::FacetRequestArgs {
            field: Some(field),
            limit: limit.as_ref(),
            sort: sort.as_ref(),
        },
    )
}

fn ranking_policy<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    ranking: &Ranking,
) -> flatbuffers::WIPOffset<api::RankingPolicy<'a>> {
    let (policy_type, policy) = match ranking.kind {
        super::search::RankingKind::DefaultRelevanceV1 => (
            api::RankingPolicyKind::DefaultRelevanceV1,
            api::DefaultRelevanceV1::create(builder, &api::DefaultRelevanceV1Args {})
                .as_union_value(),
        ),
        super::search::RankingKind::Bm25 => (
            api::RankingPolicyKind::Bm25Ranking,
            api::Bm25Ranking::create(builder, &api::Bm25RankingArgs {}).as_union_value(),
        ),
    };
    api::RankingPolicy::create(
        builder,
        &api::RankingPolicyArgs {
            policy_type,
            policy: Some(policy),
        },
    )
}

fn field_kind(kind: FieldKind) -> api::FieldKind {
    match kind {
        FieldKind::Text => api::FieldKind::Text,
        FieldKind::Keyword => api::FieldKind::Keyword,
        FieldKind::Int64 => api::FieldKind::Int64,
        FieldKind::Float64 => api::FieldKind::Float64,
        FieldKind::Bool => api::FieldKind::Bool,
    }
}

fn comparison_op(op: ComparisonOp) -> api::ComparisonOp {
    match op {
        ComparisonOp::Eq => api::ComparisonOp::Eq,
        ComparisonOp::Ne => api::ComparisonOp::Ne,
        ComparisonOp::Lt => api::ComparisonOp::Lt,
        ComparisonOp::Lte => api::ComparisonOp::Lte,
        ComparisonOp::Gt => api::ComparisonOp::Gt,
        ComparisonOp::Gte => api::ComparisonOp::Gte,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::Index;
    use crate::schema::{Field, IndexDefinition};
    use crate::{field, Filter, Range};
    use searchkernel_testkit::RecordingBackend;
    use std::sync::Arc;

    struct FailingSerialize;

    impl serde::Serialize for FailingSerialize {
        fn serialize<S>(&self, _serializer: S) -> std::result::Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            Err(serde::ser::Error::custom(
                "intentional serialization failure",
            ))
        }
    }

    fn test_index() -> Index {
        Index::from_backend(
            IndexDefinition::new()
                .field(Field::text("title"))
                .field(Field::keyword("brand"))
                .field(Field::float64("price")),
            Arc::new(RecordingBackend::new()),
        )
        .expect("test index")
    }

    #[test]
    fn search_encoder_populates_all_optional_sections() {
        let index = test_index();
        let request = index
            .search("headphones")
            .search_fields([SearchField::new("title").weight(2.0)])
            .filter(Filter::any([
                field("brand").eq("Acme"),
                field("price").range(Range::new().lower(10).include_upper(false)),
            ]))
            .sort(Sort::desc("price").missing_last())
            .facet(Facet::new("brand").limit(5).sort(FacetSort::ValueAsc))
            .ranking(Ranking::bm25())
            .limit(10)
            .offset(2)
            .projection(["title"])
            .highlight(["title"])
            .language("en-us");
        let bytes = search(&request).expect("search request encoding");
        let encoded = searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&bytes)
            .expect("encoded search request");

        assert_eq!(encoded.query(), Some("headphones"));
        assert_eq!(encoded.search_fields().unwrap().fields().unwrap().len(), 1);
        assert_eq!(
            encoded.filter().unwrap().expr_type(),
            api::FilterExprKind::FilterOr
        );
        assert_eq!(encoded.sort().unwrap().len(), 1);
        assert_eq!(encoded.facets().unwrap().len(), 1);
        assert!(encoded.ranking().is_some());
        assert_eq!(encoded.limit().unwrap().value(), 10);
        assert_eq!(encoded.offset().unwrap().value(), 2);
        assert_eq!(encoded.projection().unwrap().fields().unwrap().len(), 1);
        assert_eq!(encoded.highlighting().unwrap().fields().unwrap().len(), 1);
        assert_eq!(encoded.language(), Some("en-us"));
    }

    #[test]
    fn write_and_analysis_encoders_cover_success_and_validation_errors() {
        let index = test_index();
        let patch_value = Patch::new().set("title", "new").unset("brand");
        let patch_bytes = patch("doc-1", &patch_value).expect("patch request encoding");
        let encoded_patch = searchkernel_ir::__flatbuffers::root::<api::PatchRequest>(&patch_bytes)
            .expect("encoded patch request");
        assert_eq!(encoded_patch.id(), Some("doc-1"));
        assert_eq!(encoded_patch.operations().unwrap().len(), 2);

        let analyze_request = AnalyzeBuilder::new(&index, "running")
            .analyzer(Analyzer::standard_v1())
            .language("en");
        let analyze_bytes = analyze(&analyze_request).expect("analyze request encoding");
        let encoded_analyze =
            searchkernel_ir::__flatbuffers::root::<api::AnalyzeRequest>(&analyze_bytes)
                .expect("encoded analyze request");
        assert_eq!(encoded_analyze.text(), Some("running"));
        assert_eq!(encoded_analyze.language(), Some("en"));
        assert_eq!(encoded_analyze.target_type(), api::AnalyzeTarget::Analyzer);

        let missing_target = AnalyzeBuilder::new(&index, "running");
        assert!(matches!(
            super::analyze(&missing_target),
            Err(Error::Search {
                code: crate::ErrorCode::InvalidArgument,
                ..
            })
        ));
        assert!(matches!(
            insert("doc-1", &Value::Array(vec![])),
            Err(Error::Serialization(message)) if message.contains("object")
        ));
        let invalid_patch = Patch::new().set("ratio", FailingSerialize);
        assert!(matches!(
            super::patch("doc-1", &invalid_patch),
            Err(Error::Serialization(_))
        ));
    }
}
