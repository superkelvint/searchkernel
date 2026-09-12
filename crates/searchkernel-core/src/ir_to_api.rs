//! Owned Search Kernel IR to portable FlatBuffers encoding.

use crate::error::{ErrorCode, PortableError};
use crate::model as ir;
use searchkernel_api::v1 as api;

/// Encode a portable error as a standalone FlatBuffers `Error` table.
pub fn error_to_api(error: &PortableError) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let message = builder.create_string(&error.message);
    let mut detail_offsets = Vec::with_capacity(error.details.len());
    for detail in &error.details {
        let code = builder.create_string(&detail.code);
        let path = detail.path.as_deref().map(|v| builder.create_string(v));
        let text = builder.create_string(&detail.message);
        let metadata = if detail.metadata.is_empty() {
            None
        } else {
            let mut entries = Vec::with_capacity(detail.metadata.len());
            for (key, value) in &detail.metadata {
                let key = builder.create_string(key);
                let value = builder.create_string(value);
                entries.push(api::StringEntry::create(
                    &mut builder,
                    &api::StringEntryArgs {
                        key: Some(key),
                        value: Some(value),
                    },
                ));
            }
            Some(builder.create_vector(&entries))
        };
        detail_offsets.push(api::ErrorDetail::create(
            &mut builder,
            &api::ErrorDetailArgs {
                code: Some(code),
                path,
                message: Some(text),
                metadata,
            },
        ));
    }
    let details = (!detail_offsets.is_empty()).then(|| builder.create_vector(&detail_offsets));
    let root = api::Error::create(
        &mut builder,
        &api::ErrorArgs {
            code: error_code(error.code),
            message: Some(message),
            details,
            retryable: error.retryable,
        },
    );
    builder.finish(root, None);
    builder.finished_data().to_vec()
}

fn error_code(code: ErrorCode) -> api::ErrorCode {
    match code {
        ErrorCode::InvalidArgument => api::ErrorCode::InvalidArgument,
        ErrorCode::NotFound => api::ErrorCode::NotFound,
        ErrorCode::AlreadyExists => api::ErrorCode::AlreadyExists,
        ErrorCode::FailedPrecondition => api::ErrorCode::FailedPrecondition,
        ErrorCode::Conflict => api::ErrorCode::Conflict,
        ErrorCode::Unsupported => api::ErrorCode::Unsupported,
        ErrorCode::ResourceExhausted => api::ErrorCode::ResourceExhausted,
        ErrorCode::Internal => api::ErrorCode::Internal,
        ErrorCode::Unavailable => api::ErrorCode::Unavailable,
    }
}

/// Encode the complete normalized request inside an `InspectQueryResult`.
pub fn inspect_query_result_to_api(request: &crate::model::SearchRequestIr) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let request = search_request_offset(&mut builder, request);
    let root = api::InspectQueryResult::create(
        &mut builder,
        &api::InspectQueryResultArgs {
            resolved_request: Some(request),
        },
    );
    builder.finish(root, None);
    builder.finished_data().to_vec()
}

/// Encode only the normalized `SearchRequest` table.
pub fn search_request_ir_to_api(request: &crate::model::SearchRequestIr) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let root = search_request_offset(&mut builder, request);
    builder.finish(root, None);
    builder.finished_data().to_vec()
}

/// Encode the effective index definition inside a `DescribeResult`.
pub fn describe_result_to_api(definition: &crate::model::IndexDefinitionIr) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let index = index_definition_offset(&mut builder, definition);
    let root = api::DescribeResult::create(
        &mut builder,
        &api::DescribeResultArgs { index: Some(index) },
    );
    builder.finish(root, None);
    builder.finished_data().to_vec()
}

/// Encode only the normalized `IndexDefinition` table.
pub fn index_definition_ir_to_api(definition: &crate::model::IndexDefinitionIr) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let root = index_definition_offset(&mut builder, definition);
    builder.finish(root, None);
    builder.finished_data().to_vec()
}

fn index_definition_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    definition: &crate::model::IndexDefinitionIr,
) -> flatbuffers::WIPOffset<api::IndexDefinition<'a>> {
    let name = definition
        .name
        .as_ref()
        .map(|v| builder.create_string(v.as_str()));
    let mut field_offsets = Vec::with_capacity(definition.fields.len());
    for field in &definition.fields {
        let field_name = builder.create_string(field.name.as_str());
        let (kind, cardinality) = match (field.kind, field.cardinality) {
            (crate::model::FieldKind::Text, crate::model::FieldCardinality::Single) => {
                (api::FieldKind::Text, api::FieldCardinality::Single)
            }
            (crate::model::FieldKind::Text, crate::model::FieldCardinality::Multi) => {
                (api::FieldKind::Text, api::FieldCardinality::Multi)
            }
            (crate::model::FieldKind::Keyword, crate::model::FieldCardinality::Single) => {
                (api::FieldKind::Keyword, api::FieldCardinality::Single)
            }
            (crate::model::FieldKind::Keyword, crate::model::FieldCardinality::Multi) => {
                (api::FieldKind::Keyword, api::FieldCardinality::Multi)
            }
            (crate::model::FieldKind::Int64, crate::model::FieldCardinality::Single) => {
                (api::FieldKind::Int64, api::FieldCardinality::Single)
            }
            (crate::model::FieldKind::Int64, crate::model::FieldCardinality::Multi) => {
                (api::FieldKind::Int64, api::FieldCardinality::Multi)
            }
            (crate::model::FieldKind::Float64, crate::model::FieldCardinality::Single) => {
                (api::FieldKind::Float64, api::FieldCardinality::Single)
            }
            (crate::model::FieldKind::Float64, crate::model::FieldCardinality::Multi) => {
                (api::FieldKind::Float64, api::FieldCardinality::Multi)
            }
            (crate::model::FieldKind::Bool, crate::model::FieldCardinality::Single) => {
                (api::FieldKind::Bool, api::FieldCardinality::Single)
            }
            (crate::model::FieldKind::Bool, crate::model::FieldCardinality::Multi) => {
                (api::FieldKind::Bool, api::FieldCardinality::Multi)
            }
        };
        let stored = api::OptionalBool::new(field.capabilities.stored);
        let searchable = api::OptionalBool::new(field.capabilities.searchable);
        let filterable = api::OptionalBool::new(field.capabilities.filterable);
        let sortable = api::OptionalBool::new(field.capabilities.sortable);
        let facetable = api::OptionalBool::new(field.capabilities.facetable);
        let analyzer = field.analyzer.map(|_| {
            let analyzer_name = builder.create_string("standard");
            api::Analyzer::create(
                builder,
                &api::AnalyzerArgs {
                    name: Some(analyzer_name),
                    version: 1,
                    options: None,
                },
            )
        });
        field_offsets.push(api::FieldDefinition::create(
            builder,
            &api::FieldDefinitionArgs {
                name: Some(field_name),
                kind,
                cardinality,
                stored: Some(&stored),
                searchable: Some(&searchable),
                filterable: Some(&filterable),
                sortable: Some(&sortable),
                facetable: Some(&facetable),
                analyzer,
            },
        ));
    }
    let fields = builder.create_vector(&field_offsets);
    let mut search_offsets = Vec::with_capacity(definition.default_search_fields.len());
    for search_field in &definition.default_search_fields {
        let field_name = builder.create_string(search_field.field.as_str());
        let weight = api::OptionalDouble::new(search_field.weight.get());
        search_offsets.push(api::SearchField::create(
            builder,
            &api::SearchFieldArgs {
                field: Some(field_name),
                weight: Some(&weight),
            },
        ));
    }
    let search_vector = builder.create_vector(&search_offsets);
    let selection = api::SearchFieldSelection::create(
        builder,
        &api::SearchFieldSelectionArgs {
            fields: Some(search_vector),
        },
    );
    api::IndexDefinition::create(
        builder,
        &api::IndexDefinitionArgs {
            name,
            fields: Some(fields),
            default_search_fields: Some(selection),
        },
    )
}

fn search_request_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    request: &ir::SearchRequestIr,
) -> flatbuffers::WIPOffset<api::SearchRequest<'a>> {
    let query = match &request.query {
        ir::QueryIr::Text(text) => Some(builder.create_string(text)),
        _ => Some(builder.create_string("")),
    };
    let query_expr = match request.query {
        ir::QueryIr::MatchAll | ir::QueryIr::Text(_) => None,
        _ => Some(query_expr_offset(builder, &request.query)),
    };

    let search_fields = search_field_selection_offset(builder, &request.search_fields);
    let filter = request
        .filter
        .as_ref()
        .map(|value| filter_offset(builder, value));
    let sorts = request
        .sort
        .iter()
        .map(|value| sort_offset(builder, value))
        .collect::<Vec<_>>();
    let sort = builder.create_vector(&sorts);
    let facets = request
        .facets
        .iter()
        .map(|value| facet_offset(builder, value))
        .collect::<Vec<_>>();
    let facets = builder.create_vector(&facets);
    let ranking = ranking_offset(builder, request.ranking);
    let limit = api::OptionalUInt32::new(request.limit);
    let offset = api::OptionalUInt32::new(request.offset);
    let projection = projection_offset(builder, &request.projection);
    let highlighting = match &request.highlighting {
        ir::HighlightingIr::Disabled => None,
        ir::HighlightingIr::Requested { fields } => Some(highlighting_offset(builder, fields)),
    };
    let language = request
        .language
        .as_ref()
        .map(|value| builder.create_string(value.as_str()));

    api::SearchRequest::create(
        builder,
        &api::SearchRequestArgs {
            query,
            search_fields: Some(search_fields),
            filter,
            sort: Some(sort),
            facets: Some(facets),
            ranking: Some(ranking),
            limit: Some(&limit),
            offset: Some(&offset),
            projection: Some(projection),
            highlighting,
            language,
            query_expr,
        },
    )
}

fn search_field_selection_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    fields: &[ir::SearchFieldIr],
) -> flatbuffers::WIPOffset<api::SearchFieldSelection<'a>> {
    let fields = fields
        .iter()
        .map(|field| {
            let name = builder.create_string(field.field.as_str());
            let weight = api::OptionalDouble::new(field.weight.get());
            api::SearchField::create(
                builder,
                &api::SearchFieldArgs {
                    field: Some(name),
                    weight: Some(&weight),
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

fn projection_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    projection: &ir::ProjectionIr,
) -> flatbuffers::WIPOffset<api::FieldProjection<'a>> {
    let fields = projection
        .fields
        .iter()
        .map(|field| builder.create_string(field.as_str()))
        .collect::<Vec<_>>();
    let fields = builder.create_vector(&fields);
    api::FieldProjection::create(
        builder,
        &api::FieldProjectionArgs {
            fields: Some(fields),
        },
    )
}

fn highlighting_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    fields: &[ir::FieldName],
) -> flatbuffers::WIPOffset<api::HighlightRequest<'a>> {
    let fields = fields
        .iter()
        .map(|field| builder.create_string(field.as_str()))
        .collect::<Vec<_>>();
    let fields = builder.create_vector(&fields);
    api::HighlightRequest::create(
        builder,
        &api::HighlightRequestArgs {
            fields: Some(fields),
        },
    )
}

fn query_expr_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    query: &ir::QueryIr,
) -> flatbuffers::WIPOffset<api::QueryExpr<'a>> {
    let (expr_type, expr) = match query {
        ir::QueryIr::Term { field, value } => {
            let field = builder.create_string(field.as_str());
            let value = scalar_value_offset(builder, value);
            let term = api::TermQuery::create(
                builder,
                &api::TermQueryArgs {
                    field: Some(field),
                    value: Some(value),
                },
            );
            (api::QueryExprKind::TermQuery, term.as_union_value())
        }
        ir::QueryIr::And(clauses) => {
            let clauses = clauses
                .iter()
                .map(|clause| query_expr_offset(builder, clause))
                .collect::<Vec<_>>();
            let clauses = builder.create_vector(&clauses);
            let value = api::QueryAnd::create(
                builder,
                &api::QueryAndArgs {
                    clauses: Some(clauses),
                },
            );
            (api::QueryExprKind::QueryAnd, value.as_union_value())
        }
        ir::QueryIr::Or(clauses) => {
            let clauses = clauses
                .iter()
                .map(|clause| query_expr_offset(builder, clause))
                .collect::<Vec<_>>();
            let clauses = builder.create_vector(&clauses);
            let value = api::QueryOr::create(
                builder,
                &api::QueryOrArgs {
                    clauses: Some(clauses),
                },
            );
            (api::QueryExprKind::QueryOr, value.as_union_value())
        }
        ir::QueryIr::Not(clause) => {
            let clause = query_expr_offset(builder, clause);
            let value = api::QueryNot::create(
                builder,
                &api::QueryNotArgs {
                    clause: Some(clause),
                },
            );
            (api::QueryExprKind::QueryNot, value.as_union_value())
        }
        ir::QueryIr::MatchAll | ir::QueryIr::Text(_) => unreachable!(),
    };
    api::QueryExpr::create(
        builder,
        &api::QueryExprArgs {
            expr_type,
            expr: Some(expr),
        },
    )
}

fn filter_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    filter: &ir::FilterIr,
) -> flatbuffers::WIPOffset<api::FilterExpr<'a>> {
    let (expr_type, expr) = match filter {
        ir::FilterIr::And(clauses) => {
            let clauses = clauses
                .iter()
                .map(|clause| filter_offset(builder, clause))
                .collect::<Vec<_>>();
            let clauses = builder.create_vector(&clauses);
            let value = api::FilterAnd::create(
                builder,
                &api::FilterAndArgs {
                    clauses: Some(clauses),
                },
            );
            (api::FilterExprKind::FilterAnd, value.as_union_value())
        }
        ir::FilterIr::Or(clauses) => {
            let clauses = clauses
                .iter()
                .map(|clause| filter_offset(builder, clause))
                .collect::<Vec<_>>();
            let clauses = builder.create_vector(&clauses);
            let value = api::FilterOr::create(
                builder,
                &api::FilterOrArgs {
                    clauses: Some(clauses),
                },
            );
            (api::FilterExprKind::FilterOr, value.as_union_value())
        }
        ir::FilterIr::Not(clause) => {
            let clause = filter_offset(builder, clause);
            let value = api::FilterNot::create(
                builder,
                &api::FilterNotArgs {
                    clause: Some(clause),
                },
            );
            (api::FilterExprKind::FilterNot, value.as_union_value())
        }
        ir::FilterIr::Comparison(comparison) => {
            let field = builder.create_string(comparison.field.as_str());
            let value = scalar_value_offset(builder, &comparison.value);
            let value = api::FilterComparison::create(
                builder,
                &api::FilterComparisonArgs {
                    field: Some(field),
                    op: comparison_op(comparison.op),
                    value: Some(value),
                },
            );
            (
                api::FilterExprKind::FilterComparison,
                value.as_union_value(),
            )
        }
        ir::FilterIr::Range(range) => {
            let field = builder.create_string(range.field.as_str());
            let lower = range
                .lower
                .as_ref()
                .map(|value| scalar_value_offset(builder, value));
            let upper = range
                .upper
                .as_ref()
                .map(|value| scalar_value_offset(builder, value));
            let include_lower = api::OptionalBool::new(range.include_lower);
            let include_upper = api::OptionalBool::new(range.include_upper);
            let value = api::FilterRange::create(
                builder,
                &api::FilterRangeArgs {
                    field: Some(field),
                    lower,
                    upper,
                    include_lower: Some(&include_lower),
                    include_upper: Some(&include_upper),
                },
            );
            (api::FilterExprKind::FilterRange, value.as_union_value())
        }
        ir::FilterIr::Exists { field } => {
            let field = builder.create_string(field.as_str());
            let value =
                api::FilterExists::create(builder, &api::FilterExistsArgs { field: Some(field) });
            (api::FilterExprKind::FilterExists, value.as_union_value())
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

fn scalar_value_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    value: &ir::ScalarValueIr,
) -> flatbuffers::WIPOffset<api::Value<'a>> {
    let (kind_type, kind) = match value {
        ir::ScalarValueIr::Bool(value) => (
            api::ValueKind::BoolValue,
            api::BoolValue::create(builder, &api::BoolValueArgs { value: *value }).as_union_value(),
        ),
        ir::ScalarValueIr::Int64(value) => (
            api::ValueKind::Int64Value,
            api::Int64Value::create(builder, &api::Int64ValueArgs { value: *value })
                .as_union_value(),
        ),
        ir::ScalarValueIr::Float64(value) => (
            api::ValueKind::Float64Value,
            api::Float64Value::create(builder, &api::Float64ValueArgs { value: *value })
                .as_union_value(),
        ),
        ir::ScalarValueIr::String(value) => {
            let value = builder.create_string(value);
            let value =
                api::StringValue::create(builder, &api::StringValueArgs { value: Some(value) });
            (api::ValueKind::StringValue, value.as_union_value())
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

fn comparison_op(op: ir::ComparisonOp) -> api::ComparisonOp {
    match op {
        ir::ComparisonOp::Eq => api::ComparisonOp::Eq,
        ir::ComparisonOp::Ne => api::ComparisonOp::Ne,
        ir::ComparisonOp::Lt => api::ComparisonOp::Lt,
        ir::ComparisonOp::Lte => api::ComparisonOp::Lte,
        ir::ComparisonOp::Gt => api::ComparisonOp::Gt,
        ir::ComparisonOp::Gte => api::ComparisonOp::Gte,
    }
}

fn sort_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    sort: &ir::SortIr,
) -> flatbuffers::WIPOffset<api::SortExpr<'a>> {
    let field = builder.create_string(sort.field.as_str());
    let missing = api::OptionalMissingPlacement::new(match sort.missing {
        ir::MissingPlacement::First => api::MissingPlacement::First,
        ir::MissingPlacement::Last => api::MissingPlacement::Last,
    });
    api::SortExpr::create(
        builder,
        &api::SortExprArgs {
            field: Some(field),
            direction: match sort.direction {
                ir::SortDirection::Asc => api::SortDirection::Asc,
                ir::SortDirection::Desc => api::SortDirection::Desc,
            },
            missing: Some(&missing),
        },
    )
}

fn facet_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    facet: &ir::FacetIr,
) -> flatbuffers::WIPOffset<api::FacetRequest<'a>> {
    let field = builder.create_string(facet.field.as_str());
    let limit = api::OptionalUInt32::new(facet.limit);
    let sort = api::OptionalFacetSort::new(match facet.sort {
        ir::FacetSort::CountDesc => api::FacetSort::CountDesc,
        ir::FacetSort::CountAsc => api::FacetSort::CountAsc,
        ir::FacetSort::ValueAsc => api::FacetSort::ValueAsc,
        ir::FacetSort::ValueDesc => api::FacetSort::ValueDesc,
    });
    api::FacetRequest::create(
        builder,
        &api::FacetRequestArgs {
            field: Some(field),
            limit: Some(&limit),
            sort: Some(&sort),
        },
    )
}

fn ranking_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    ranking: ir::RankingPolicyIr,
) -> flatbuffers::WIPOffset<api::RankingPolicy<'a>> {
    let (policy_type, policy) = match ranking {
        ir::RankingPolicyIr::DefaultRelevanceV1 => (
            api::RankingPolicyKind::DefaultRelevanceV1,
            api::DefaultRelevanceV1::create(builder, &api::DefaultRelevanceV1Args {})
                .as_union_value(),
        ),
        ir::RankingPolicyIr::Bm25 => (
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

pub fn insert_result_to_api(result: &ir::InsertResult) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let id = builder.create_string(&result.id);
    let root = api::InsertResult::create(&mut builder, &api::InsertResultArgs { id: Some(id) });
    builder.finish(root, None);
    builder.finished_data().to_vec()
}

pub fn upsert_result_to_api(result: &ir::UpsertResult) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let id = builder.create_string(&result.id);
    let root = api::UpsertResult::create(&mut builder, &api::UpsertResultArgs { id: Some(id) });
    builder.finish(root, None);
    builder.finished_data().to_vec()
}

pub fn patch_result_to_api(result: &ir::PatchResult) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let id = builder.create_string(&result.id);
    let root = api::PatchResult::create(&mut builder, &api::PatchResultArgs { id: Some(id) });
    builder.finish(root, None);
    builder.finished_data().to_vec()
}

pub fn delete_result_to_api(result: &ir::DeleteResult) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let id = builder.create_string(&result.id);
    let root = api::DeleteResult::create(
        &mut builder,
        &api::DeleteResultArgs {
            id: Some(id),
            deleted: result.deleted,
        },
    );
    builder.finish(root, None);
    builder.finished_data().to_vec()
}

pub fn get_result_to_api(result: &ir::GetResult) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let document = result
        .document
        .as_ref()
        .map(|document| document_offset(&mut builder, document));
    let root = api::GetResult::create(&mut builder, &api::GetResultArgs { document });
    builder.finish(root, None);
    builder.finished_data().to_vec()
}

pub fn search_result_to_api(result: &ir::SearchResult) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let hits = result
        .hits
        .iter()
        .map(|hit| hit_offset(&mut builder, hit))
        .collect::<Vec<_>>();
    let hits = builder.create_vector(&hits);
    let total_hits = api::TotalHits::create(
        &mut builder,
        &api::TotalHitsArgs {
            value: result.total_hits.value,
            relation: match result.total_hits.relation {
                ir::TotalHitsRelation::Exact => api::TotalHitsRelation::Exact,
                ir::TotalHitsRelation::LowerBound => api::TotalHitsRelation::LowerBound,
            },
        },
    );
    let facets = result
        .facets
        .iter()
        .map(|facet| facet_result_offset(&mut builder, facet))
        .collect::<Vec<_>>();
    let facets = builder.create_vector(&facets);
    let root = api::SearchResult::create(
        &mut builder,
        &api::SearchResultArgs {
            hits: Some(hits),
            total_hits: Some(total_hits),
            facets: Some(facets),
        },
    );
    builder.finish(root, None);
    builder.finished_data().to_vec()
}

pub fn analyze_result_to_api(result: &ir::AnalyzeResult) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let tokens = result
        .tokens
        .iter()
        .map(|token| {
            let surface = builder.create_string(&token.surface);
            let term = builder.create_string(&token.term);
            api::AnalyzedToken::create(
                &mut builder,
                &api::AnalyzedTokenArgs {
                    surface: Some(surface),
                    term: Some(term),
                    position: token.position,
                    start_byte: token.start_byte,
                    end_byte: token.end_byte,
                },
            )
        })
        .collect::<Vec<_>>();
    let tokens = builder.create_vector(&tokens);
    let root = api::AnalyzeResult::create(
        &mut builder,
        &api::AnalyzeResultArgs {
            tokens: Some(tokens),
        },
    );
    builder.finish(root, None);
    builder.finished_data().to_vec()
}

fn value_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    value: &ir::Value,
) -> flatbuffers::WIPOffset<api::Value<'a>> {
    let (kind_type, kind) = match value {
        ir::Value::Null => (
            api::ValueKind::NullValueValue,
            api::NullValueValue::create(
                builder,
                &api::NullValueValueArgs {
                    value: api::NullValue::Null,
                },
            )
            .as_union_value(),
        ),
        ir::Value::Bool(value) => (
            api::ValueKind::BoolValue,
            api::BoolValue::create(builder, &api::BoolValueArgs { value: *value }).as_union_value(),
        ),
        ir::Value::Int64(value) => (
            api::ValueKind::Int64Value,
            api::Int64Value::create(builder, &api::Int64ValueArgs { value: *value })
                .as_union_value(),
        ),
        ir::Value::UInt64(value) => (
            api::ValueKind::UInt64Value,
            api::UInt64Value::create(builder, &api::UInt64ValueArgs { value: *value })
                .as_union_value(),
        ),
        ir::Value::Float64(value) => (
            api::ValueKind::Float64Value,
            api::Float64Value::create(builder, &api::Float64ValueArgs { value: *value })
                .as_union_value(),
        ),
        ir::Value::String(value) => {
            let value = builder.create_string(value);
            let value =
                api::StringValue::create(builder, &api::StringValueArgs { value: Some(value) });
            (api::ValueKind::StringValue, value.as_union_value())
        }
        ir::Value::Bytes(value) => {
            let value = builder.create_vector(value);
            let value =
                api::BytesValue::create(builder, &api::BytesValueArgs { value: Some(value) });
            (api::ValueKind::BytesValue, value.as_union_value())
        }
        ir::Value::List(values) => {
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
        ir::Value::Object(fields) => {
            let fields = fields
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

fn document_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    document: &ir::DocumentResult,
) -> flatbuffers::WIPOffset<api::Document<'a>> {
    let id = builder.create_string(&document.id);
    let fields = document
        .fields
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
    let fields = builder.create_vector(&fields);
    api::Document::create(
        builder,
        &api::DocumentArgs {
            id: Some(id),
            fields: Some(fields),
        },
    )
}

fn hit_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    hit: &ir::Hit,
) -> flatbuffers::WIPOffset<api::Hit<'a>> {
    let id = builder.create_string(&hit.id);
    let score = hit.score.map(api::OptionalDouble::new);
    let fields = hit
        .fields
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
    let fields = builder.create_vector(&fields);
    let highlights = hit
        .highlights
        .iter()
        .map(|highlight| highlight_result_offset(builder, highlight))
        .collect::<Vec<_>>();
    let highlights = builder.create_vector(&highlights);
    api::Hit::create(
        builder,
        &api::HitArgs {
            id: Some(id),
            score: score.as_ref(),
            fields: Some(fields),
            highlights: Some(highlights),
        },
    )
}

fn facet_result_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    facet: &ir::FacetResult,
) -> flatbuffers::WIPOffset<api::FacetResult<'a>> {
    let field = builder.create_string(&facet.field);
    let values = facet
        .values
        .iter()
        .map(|value| {
            let value_offset = value_offset(builder, &value.value);
            api::FacetValue::create(
                builder,
                &api::FacetValueArgs {
                    value: Some(value_offset),
                    count: value.count,
                },
            )
        })
        .collect::<Vec<_>>();
    let values = builder.create_vector(&values);
    api::FacetResult::create(
        builder,
        &api::FacetResultArgs {
            field: Some(field),
            values: Some(values),
        },
    )
}

fn highlight_result_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    highlight: &ir::HighlightResult,
) -> flatbuffers::WIPOffset<api::HighlightResult<'a>> {
    let field = builder.create_string(&highlight.field);
    let fragments = highlight
        .fragments
        .iter()
        .map(|fragment| {
            let text = builder.create_string(&fragment.text);
            let matches = fragment
                .matches
                .iter()
                .map(|span| {
                    api::HighlightSpan::create(
                        builder,
                        &api::HighlightSpanArgs {
                            start_byte: span.start_byte,
                            end_byte: span.end_byte,
                        },
                    )
                })
                .collect::<Vec<_>>();
            let matches = builder.create_vector(&matches);
            api::HighlightFragment::create(
                builder,
                &api::HighlightFragmentArgs {
                    text: Some(text),
                    matches: Some(matches),
                },
            )
        })
        .collect::<Vec<_>>();
    let fragments = builder.create_vector(&fragments);
    api::HighlightResult::create(
        builder,
        &api::HighlightResultArgs {
            field: Some(field),
            fragments: Some(fragments),
        },
    )
}
