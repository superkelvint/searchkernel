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
use std::time::Duration;

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
            condition: None,
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
    let root = api::DeleteRequest::create(
        &mut builder,
        &api::DeleteRequestArgs {
            id: Some(id),
            condition: None,
        },
    );
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

pub(crate) fn analyze(request: &AnalyzeBuilder) -> Result<Vec<u8>> {
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
    let query_text = builder.create_string(&request.query);
    let query_value = api::SearchKernelTextQuery::create(
        &mut builder,
        &api::SearchKernelTextQueryArgs {
            text: Some(query_text),
        },
    );
    let query = api::QuerySource::create(
        &mut builder,
        &api::QuerySourceArgs {
            source_type: api::QuerySourceKind::SearchKernelTextQuery,
            source: Some(query_value.as_union_value()),
        },
    );
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
    let filter = filter.map(|filter| {
        let filters = builder.create_vector(&[filter]);
        api::FilterRequest::create(
            &mut builder,
            &api::FilterRequestArgs {
                structured: Some(filters),
                sources: None,
                parameters: None,
            },
        )
    });
    let sorts = request
        .sorts
        .iter()
        .map(|sort| sort_expr(&mut builder, sort))
        .collect::<Vec<_>>();
    let sorts = builder.create_vector(&sorts);
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
    let timeout = request
        .timeout
        .map(timeout_millis)
        .transpose()?
        .map(api::OptionalUInt64::new);
    let grouping = request
        .facets
        .iter()
        .map(|facet| facet_grouping(&mut builder, facet))
        .collect::<Vec<_>>();
    let grouping = (!grouping.is_empty()).then(|| builder.create_vector(&grouping));
    let query_model = request.language.as_deref().map(|language| {
        let language = builder.create_string(language);
        api::QueryModel::create(
            &mut builder,
            &api::QueryModelArgs {
                language: Some(language),
                ..Default::default()
            },
        )
    });
    let presentation = match (projection, highlighting) {
        (None, None) => None,
        (fields, highlighting) => Some(api::PresentationRequest::create(
            &mut builder,
            &api::PresentationRequestArgs {
                summary: None,
                fields,
                highlighting,
                format: None,
                bolding: None,
                timing: None,
            },
        )),
    };
    let root = api::SearchRequest::create(
        &mut builder,
        &api::SearchRequestArgs {
            query: Some(query),
            search_fields,
            filters: filter,
            sort: Some(sorts),
            ranking,
            limit: limit.as_ref(),
            offset: offset.as_ref(),
            timeout_ms: timeout.as_ref(),
            grouping,
            inputs: None,
            query_model,
            presentation,
            properties: None,
            query_profile: None,
            search_chain: None,
        },
    );
    builder.finish(root, None);
    Ok(builder.finished_data().to_vec())
}

/// The product's legacy facet builder is a convenience over V2 grouping, not
/// a second aggregation request format.  Its normalized shape is:
/// `all(group(attribute(field)), each(max(limit), order(...), output(count)))`.
fn facet_grouping<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    facet: &Facet,
) -> flatbuffers::WIPOffset<api::GroupingRequest<'a>> {
    let field = builder.create_string(&facet.field);
    let attribute = api::GroupingAttributeExpression::create(
        builder,
        &api::GroupingAttributeExpressionArgs { field: Some(field) },
    );
    let attribute = api::GroupingExpression::create(
        builder,
        &api::GroupingExpressionArgs {
            expr_type: api::GroupingExpressionKind::GroupingAttributeExpression,
            expr: Some(attribute.as_union_value()),
        },
    );
    let group = api::GroupingGroupOperation::create(
        builder,
        &api::GroupingGroupOperationArgs {
            expression: Some(attribute),
        },
    );
    let group = api::GroupingOperation::create(
        builder,
        &api::GroupingOperationArgs {
            operation_type: api::GroupingOperationKind::GroupingGroupOperation,
            operation: Some(group.as_union_value()),
        },
    );
    let count = api::CountAggregation::create(builder, &api::CountAggregationArgs {});
    let count = api::GroupingAggregationExpression::create(
        builder,
        &api::GroupingAggregationExpressionArgs {
            aggregation_type: api::AggregationKind::CountAggregation,
            aggregation: Some(count.as_union_value()),
        },
    );
    let count = api::GroupingExpression::create(
        builder,
        &api::GroupingExpressionArgs {
            expr_type: api::GroupingExpressionKind::GroupingAggregationExpression,
            expr: Some(count.as_union_value()),
        },
    );
    let output = api::GroupingOutput::create(
        builder,
        &api::GroupingOutputArgs {
            label: None,
            expression: Some(count),
        },
    );
    let outputs = builder.create_vector(&[output]);
    let output = api::GroupingOutputOperation::create(
        builder,
        &api::GroupingOutputOperationArgs {
            outputs: Some(outputs),
        },
    );
    let output = api::GroupingOperation::create(
        builder,
        &api::GroupingOperationArgs {
            operation_type: api::GroupingOperationKind::GroupingOutputOperation,
            operation: Some(output.as_union_value()),
        },
    );
    let max = api::FiniteGroupingMax::create(
        builder,
        &api::FiniteGroupingMaxArgs {
            value: facet.limit.unwrap_or(10),
        },
    );
    let max = api::GroupingMaxOperation::create(
        builder,
        &api::GroupingMaxOperationArgs {
            value_type: api::GroupingMaxKind::FiniteGroupingMax,
            value: Some(max.as_union_value()),
        },
    );
    let max = api::GroupingOperation::create(
        builder,
        &api::GroupingOperationArgs {
            operation_type: api::GroupingOperationKind::GroupingMaxOperation,
            operation: Some(max.as_union_value()),
        },
    );
    // The explicit order operation makes all four facade sort modes canonical.
    let order_expression = match facet.sort.unwrap_or(FacetSort::CountDesc) {
        FacetSort::CountDesc | FacetSort::CountAsc => count,
        FacetSort::ValueAsc | FacetSort::ValueDesc => attribute,
    };
    let order_expression = match facet.sort.unwrap_or(FacetSort::CountDesc) {
        FacetSort::CountDesc | FacetSort::ValueDesc => {
            let negative = api::GroupingUnaryExpression::create(
                builder,
                &api::GroupingUnaryExpressionArgs {
                    op: api::GroupingUnaryOperator::Negative,
                    expression: Some(order_expression),
                },
            );
            api::GroupingExpression::create(
                builder,
                &api::GroupingExpressionArgs {
                    expr_type: api::GroupingExpressionKind::GroupingUnaryExpression,
                    expr: Some(negative.as_union_value()),
                },
            )
        }
        FacetSort::CountAsc | FacetSort::ValueAsc => order_expression,
    };
    let order_expression = api::GroupingOrderExpr::create(
        builder,
        &api::GroupingOrderExprArgs {
            expression: Some(order_expression),
        },
    );
    let order_expressions = builder.create_vector(&[order_expression]);
    let order = api::GroupingOrderOperation::create(
        builder,
        &api::GroupingOrderOperationArgs {
            expressions: Some(order_expressions),
        },
    );
    let order = api::GroupingOperation::create(
        builder,
        &api::GroupingOperationArgs {
            operation_type: api::GroupingOperationKind::GroupingOrderOperation,
            operation: Some(order.as_union_value()),
        },
    );
    let each_operations = builder.create_vector(&[max, order, output]);
    let each = api::GroupingEachOperation::create(
        builder,
        &api::GroupingEachOperationArgs {
            operations: Some(each_operations),
            label: None,
        },
    );
    let each = api::GroupingOperation::create(
        builder,
        &api::GroupingOperationArgs {
            operation_type: api::GroupingOperationKind::GroupingEachOperation,
            operation: Some(each.as_union_value()),
        },
    );
    let operations = builder.create_vector(&[group, each]);
    let root = api::GroupingAllOperation::create(
        builder,
        &api::GroupingAllOperationArgs {
            operations: Some(operations),
            label: None,
        },
    );
    let structured = api::StructuredGroupingSource::create(
        builder,
        &api::StructuredGroupingSourceArgs { root: Some(root) },
    );
    api::GroupingRequest::create(
        builder,
        &api::GroupingRequestArgs {
            source_type: api::GroupingSourceKind::StructuredGroupingSource,
            source: Some(structured.as_union_value()),
            continuations: None,
        },
    )
}

fn timeout_millis(timeout: Duration) -> Result<u64> {
    let millis = u64::try_from(timeout.as_millis()).map_err(|_| {
        Error::search(
            crate::ErrorCode::InvalidArgument,
            "search timeout is too large to encode",
        )
    })?;
    if !timeout.is_zero() && millis == 0 {
        return Err(Error::search(
            crate::ErrorCode::InvalidArgument,
            "search timeout must be at least one millisecond",
        ));
    }
    Ok(millis)
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
            properties: None,
            ..Default::default()
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
    let data_type = data_type(builder, field.kind, field.multi);
    let matching = matches!(field.kind, FieldKind::Text | FieldKind::Keyword).then(|| {
        api::MatchingConfig::create(
            builder,
            &api::MatchingConfigArgs {
                mode: match field.kind {
                    FieldKind::Text => api::MatchingMode::Text,
                    FieldKind::Keyword => api::MatchingMode::Exact,
                    _ => api::MatchingMode::Unspecified,
                },
                options: None,
            },
        )
    });
    api::FieldDefinition::create(
        builder,
        &api::FieldDefinitionArgs {
            name: Some(name),
            data_type: Some(data_type),
            stored: stored.as_ref(),
            searchable: searchable.as_ref(),
            filterable: filterable.as_ref(),
            sortable: sortable.as_ref(),
            facetable: facetable.as_ref(),
            analyzer,
            matching,
            index: None,
            attribute: None,
            summary: None,
            indexing_script: None,
            properties: None,
            ..Default::default()
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

fn range_bound<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    value: Option<&Value>,
) -> flatbuffers::WIPOffset<api::RangeBound<'a>> {
    let (bound_type, bound) = match value {
        Some(value) => {
            let value = value_offset(builder, value);
            let bound = api::FiniteRangeBound::create(
                builder,
                &api::FiniteRangeBoundArgs { value: Some(value) },
            );
            (
                api::RangeBoundKind::FiniteRangeBound,
                bound.as_union_value(),
            )
        }
        None => {
            let bound = api::UnboundedRangeBound::create(builder, &api::UnboundedRangeBoundArgs {});
            (
                api::RangeBoundKind::UnboundedRangeBound,
                bound.as_union_value(),
            )
        }
    };
    api::RangeBound::create(
        builder,
        &api::RangeBoundArgs {
            bound_type,
            bound: Some(bound),
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
            let field = query_field_ref(builder, field);
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
            let field = query_field_ref(builder, field);
            let lower = bounds
                .lower
                .as_ref()
                .map(|value| pending_value(value).map(|value| range_bound(builder, Some(value))))
                .transpose()
                .map_err(Error::Serialization)?;
            let upper = bounds
                .upper
                .as_ref()
                .map(|value| pending_value(value).map(|value| range_bound(builder, Some(value))))
                .transpose()
                .map_err(Error::Serialization)?;
            let lower = lower.unwrap_or_else(|| range_bound(builder, None));
            let upper = upper.unwrap_or_else(|| range_bound(builder, None));
            let include_lower = bounds.include_lower.map(api::OptionalBool::new);
            let include_upper = bounds.include_upper.map(api::OptionalBool::new);
            let value = api::FilterRange::create(
                builder,
                &api::FilterRangeArgs {
                    field: Some(field),
                    lower: Some(lower),
                    upper: Some(upper),
                    include_lower: include_lower.as_ref(),
                    include_upper: include_upper.as_ref(),
                },
            );
            (api::FilterExprKind::FilterRange, value.as_union_value())
        }
        Filter(FilterNode::Exists { field }) => {
            let field = query_field_ref(builder, field);
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
    let field =
        api::FieldSortTarget::create(builder, &api::FieldSortTargetArgs { field: Some(field) });
    let target = api::SortTarget::create(
        builder,
        &api::SortTargetArgs {
            target_type: api::SortTargetKind::FieldSortTarget,
            target: Some(field.as_union_value()),
        },
    );
    let missing = sort.missing.map(|missing| missing_policy(builder, missing));
    api::SortExpr::create(
        builder,
        &api::SortExprArgs {
            target: Some(target),
            direction: match sort.direction {
                SortDirection::Asc => api::SortDirection::Asc,
                SortDirection::Desc => api::SortDirection::Desc,
            },
            missing,
            annotations: None,
        },
    )
}

fn query_field_ref<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    path: &str,
) -> flatbuffers::WIPOffset<api::QueryFieldRef<'a>> {
    let path = builder.create_string(path);
    api::QueryFieldRef::create(
        builder,
        &api::QueryFieldRefArgs {
            path: Some(path),
            element_index: None,
        },
    )
}

fn missing_policy<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    missing: MissingPlacement,
) -> flatbuffers::WIPOffset<api::MissingPolicy<'a>> {
    let (policy_type, policy) = match missing {
        MissingPlacement::First => {
            let policy = api::MissingFirst::create(builder, &api::MissingFirstArgs {});
            (
                api::MissingPolicyKind::MissingFirst,
                policy.as_union_value(),
            )
        }
        MissingPlacement::Last => {
            let policy = api::MissingLast::create(builder, &api::MissingLastArgs {});
            (api::MissingPolicyKind::MissingLast, policy.as_union_value())
        }
    };
    api::MissingPolicy::create(
        builder,
        &api::MissingPolicyArgs {
            policy_type,
            policy: Some(policy),
        },
    )
}

fn ranking_policy<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    ranking: &Ranking,
) -> flatbuffers::WIPOffset<api::RankingRequest<'a>> {
    let profile = builder.create_string(match ranking.kind {
        super::search::RankingKind::DefaultRelevanceV1 => "default",
        super::search::RankingKind::Bm25 => "bm25",
    });
    api::RankingRequest::create(
        builder,
        &api::RankingRequestArgs {
            profile: Some(profile),
            properties: None,
        },
    )
}

fn data_type<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    kind: FieldKind,
    multi: bool,
) -> flatbuffers::WIPOffset<api::DataType<'a>> {
    let primitive = api::PrimitiveDataType::create(
        builder,
        &api::PrimitiveDataTypeArgs {
            kind: match kind {
                FieldKind::Text | FieldKind::Keyword => api::PrimitiveDataTypeKind::String,
                FieldKind::Int64 => api::PrimitiveDataTypeKind::Int64,
                FieldKind::Float64 => api::PrimitiveDataTypeKind::Float64,
                FieldKind::Bool => api::PrimitiveDataTypeKind::Bool,
            },
        },
    );
    let primitive = api::DataType::create(
        builder,
        &api::DataTypeArgs {
            kind_type: api::DataTypeKind::PrimitiveDataType,
            kind: Some(primitive.as_union_value()),
        },
    );
    if !multi {
        return primitive;
    }
    let array = api::ArrayDataType::create(
        builder,
        &api::ArrayDataTypeArgs {
            element_type: Some(primitive),
        },
    );
    api::DataType::create(
        builder,
        &api::DataTypeArgs {
            kind_type: api::DataTypeKind::ArrayDataType,
            kind: Some(array.as_union_value()),
        },
    )
}

/*
fn field_kind(kind: FieldKind) -> api::FieldKind {
    match kind {
        FieldKind::Text => api::FieldKind::Text,
        FieldKind::Keyword => api::FieldKind::Keyword,
        FieldKind::Int64 => api::FieldKind::Int64,
        FieldKind::Float64 => api::FieldKind::Float64,
        FieldKind::Bool => api::FieldKind::Bool,
    }
}
*/

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
    use crate::index::{Index, IndexRuntime};
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

    fn test_index() -> (IndexRuntime, Index) {
        let runtime = IndexRuntime::from_backend(
            IndexDefinition::new()
                .field(Field::text("title"))
                .field(Field::keyword("brand"))
                .field(Field::float64("price")),
            Arc::new(RecordingBackend::new()),
        )
        .expect("test index");
        let index = runtime.index();
        (runtime, index)
    }

    #[test]
    fn search_encoder_populates_all_optional_sections() {
        let (_runtime, index) = test_index();
        let request = index
            .search("headphones")
            .search_fields([SearchField::new("title").weight(2.0)])
            .filter(Filter::any([
                field("brand").eq("Acme"),
                field("price").range(Range::new().lower(10).include_upper(false)),
            ]))
            .sort(Sort::desc("price").missing_last())
            .ranking(Ranking::bm25())
            .limit(10)
            .offset(2)
            .projection(["title"])
            .highlight(["title"])
            .language("en-us")
            .timeout(Duration::from_millis(200));
        let bytes = search(&request).expect("search request encoding");
        let encoded = searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&bytes)
            .expect("encoded search request");

        assert_eq!(
            encoded
                .query()
                .and_then(|query| query.source_as_search_kernel_text_query())
                .and_then(|query| query.text()),
            Some("headphones")
        );
        assert_eq!(encoded.search_fields().unwrap().fields().unwrap().len(), 1);
        assert_eq!(
            encoded
                .filters()
                .unwrap()
                .structured()
                .unwrap()
                .get(0)
                .expr_type(),
            api::FilterExprKind::FilterOr
        );
        assert_eq!(encoded.sort().unwrap().len(), 1);
        assert!(encoded.ranking().is_some());
        assert_eq!(encoded.limit().unwrap().value(), 10);
        assert_eq!(encoded.offset().unwrap().value(), 2);
        assert_eq!(
            encoded
                .presentation()
                .unwrap()
                .fields()
                .unwrap()
                .fields()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            encoded
                .presentation()
                .unwrap()
                .highlighting()
                .unwrap()
                .fields()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(encoded.query_model().unwrap().language(), Some("en-us"));
        assert_eq!(encoded.timeout_ms().unwrap().value(), 200);
    }

    #[test]
    fn search_encoder_rejects_sub_millisecond_timeout() {
        let (_runtime, index) = test_index();
        let request = index.search("headphones").timeout(Duration::from_nanos(1));

        assert!(matches!(
            search(&request),
            Err(Error::Search {
                code: crate::ErrorCode::InvalidArgument,
                ..
            })
        ));
    }

    #[test]
    fn write_and_analysis_encoders_cover_success_and_validation_errors() {
        let (_runtime, index) = test_index();
        let patch_value = Patch::new().set("title", "new").unset("brand");
        let patch_bytes = patch("doc-1", &patch_value).expect("patch request encoding");
        let encoded_patch = searchkernel_ir::__flatbuffers::root::<api::PatchRequest>(&patch_bytes)
            .expect("encoded patch request");
        assert_eq!(encoded_patch.id(), Some("doc-1"));
        assert_eq!(encoded_patch.operations().unwrap().len(), 2);

        let analyze_request = AnalyzeBuilder::new(index.clone(), "running")
            .analyzer(Analyzer::standard_v1())
            .language("en");
        let analyze_bytes = analyze(&analyze_request).expect("analyze request encoding");
        let encoded_analyze =
            searchkernel_ir::__flatbuffers::root::<api::AnalyzeRequest>(&analyze_bytes)
                .expect("encoded analyze request");
        assert_eq!(encoded_analyze.text(), Some("running"));
        assert_eq!(encoded_analyze.language(), Some("en"));
        assert_eq!(encoded_analyze.target_type(), api::AnalyzeTarget::Analyzer);

        let missing_target = AnalyzeBuilder::new(index, "running");
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
