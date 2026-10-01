use super::*;

pub fn insert_result_to_api(result: &ir::InsertResult) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let root = insert_result_offset(&mut builder, result);
    builder.finish(root, None);
    builder.finished_data().to_vec()
}

pub fn insert_result_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    result: &ir::InsertResult,
) -> flatbuffers::WIPOffset<api::InsertResult<'a>> {
    let id = builder.create_string(&result.id);
    api::InsertResult::create(builder, &api::InsertResultArgs { id: Some(id) })
}

pub fn upsert_result_to_api(result: &ir::UpsertResult) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let root = upsert_result_offset(&mut builder, result);
    builder.finish(root, None);
    builder.finished_data().to_vec()
}

pub fn upsert_result_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    result: &ir::UpsertResult,
) -> flatbuffers::WIPOffset<api::UpsertResult<'a>> {
    let id = builder.create_string(&result.id);
    api::UpsertResult::create(builder, &api::UpsertResultArgs { id: Some(id) })
}

pub fn patch_result_to_api(result: &ir::PatchResult) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let root = patch_result_offset(&mut builder, result);
    builder.finish(root, None);
    builder.finished_data().to_vec()
}

pub fn update_result_to_api(result: &ir::UpdateResult) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let offset = update_result_offset(&mut builder, result);
    builder.finish(offset, None);
    builder.finished_data().to_vec()
}

pub fn update_result_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    result: &ir::UpdateResult,
) -> flatbuffers::WIPOffset<api::UpdateResult<'a>> {
    let id = builder.create_string(&result.id);
    api::UpdateResult::create(builder, &api::UpdateResultArgs { id: Some(id) })
}

pub fn patch_result_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    result: &ir::PatchResult,
) -> flatbuffers::WIPOffset<api::PatchResult<'a>> {
    let id = builder.create_string(&result.id);
    api::PatchResult::create(builder, &api::PatchResultArgs { id: Some(id) })
}

pub fn delete_result_to_api(result: &ir::DeleteResult) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let root = delete_result_offset(&mut builder, result);
    builder.finish(root, None);
    builder.finished_data().to_vec()
}

pub fn delete_result_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    result: &ir::DeleteResult,
) -> flatbuffers::WIPOffset<api::DeleteResult<'a>> {
    let id = builder.create_string(&result.id);
    api::DeleteResult::create(
        builder,
        &api::DeleteResultArgs {
            id: Some(id),
            deleted: result.deleted,
        },
    )
}

pub fn get_result_to_api(result: &ir::GetResult) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let root = get_result_offset(&mut builder, result);
    builder.finish(root, None);
    builder.finished_data().to_vec()
}

pub fn get_result_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    result: &ir::GetResult,
) -> flatbuffers::WIPOffset<api::GetResult<'a>> {
    let document = result
        .document
        .as_ref()
        .map(|document| document_offset(builder, document));
    api::GetResult::create(builder, &api::GetResultArgs { document })
}

pub fn search_result_to_api(result: &ir::SearchResult) -> Result<Vec<u8>, PortableError> {
    validate_search_result(result)?;
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let root = search_result_offset_unchecked(&mut builder, result);
    builder.finish(root, None);
    Ok(builder.finished_data().to_vec())
}

pub fn search_result_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    result: &ir::SearchResult,
) -> Result<flatbuffers::WIPOffset<api::SearchResult<'a>>, PortableError> {
    validate_search_result(result)?;
    Ok(search_result_offset_unchecked(builder, result))
}

pub(super) fn search_result_offset_unchecked<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    result: &ir::SearchResult,
) -> flatbuffers::WIPOffset<api::SearchResult<'a>> {
    let root = result_node_offset(builder, &result.root);
    let total_hits = api::TotalHits::create(
        builder,
        &api::TotalHitsArgs {
            value: result.total_hits.value,
            relation: match result.total_hits.relation {
                ir::TotalHitsRelation::Unspecified => api::TotalHitsRelation::Unspecified,
                ir::TotalHitsRelation::Exact => api::TotalHitsRelation::Exact,
                ir::TotalHitsRelation::LowerBound => api::TotalHitsRelation::LowerBound,
                ir::TotalHitsRelation::Approximate => api::TotalHitsRelation::Approximate,
                ir::TotalHitsRelation::Unknown => api::TotalHitsRelation::Unknown,
            },
        },
    );
    let coverage = result
        .coverage
        .as_ref()
        .map(|coverage| coverage_offset(builder, coverage));
    let timing = result
        .timing
        .as_ref()
        .map(|timing| search_timing_offset(builder, timing));
    let errors = (!result.errors.is_empty()).then(|| {
        let errors = result
            .errors
            .iter()
            .map(|error| search_result_error_offset(builder, error))
            .collect::<Vec<_>>();
        builder.create_vector(&errors)
    });
    let trace = result
        .trace
        .as_ref()
        .map(|trace| value_offset(builder, trace));
    let metadata = value_entries_offset(builder, &result.metadata);
    api::SearchResult::create(
        builder,
        &api::SearchResultArgs {
            root: Some(root),
            total_hits: Some(total_hits),
            coverage,
            timing,
            errors,
            trace,
            metadata,
        },
    )
}

/// Validate backend-owned result values before they enter the canonical wire
/// response. The schema can represent non-finite scalar values explicitly,
/// but relevance, coverage/timing doubles, and tensor cells have narrower
/// contracts.
pub fn validate_search_result(result: &ir::SearchResult) -> Result<(), PortableError> {
    validate_result_node(&result.root, "root")?;
    if let Some(coverage) = &result.coverage {
        validate_finite(
            coverage.coverage_percent,
            "coverage.coverage_percent",
            "coverage percentage must be finite",
        )?;
        if let Some(degraded) = &coverage.degraded {
            validate_value_map(&degraded.metadata, "coverage.degraded.metadata")?;
        }
        validate_value_map(&coverage.metadata, "coverage.metadata")?;
    }
    if let Some(timing) = &result.timing {
        validate_finite(
            timing.query_time_seconds,
            "timing.query_time_seconds",
            "query time must be finite",
        )?;
        validate_finite(
            timing.search_time_seconds,
            "timing.search_time_seconds",
            "search time must be finite",
        )?;
        validate_finite(
            timing.summary_fetch_time_seconds,
            "timing.summary_fetch_time_seconds",
            "summary fetch time must be finite",
        )?;
        validate_value_map(&timing.metadata, "timing.metadata")?;
    }
    validate_value_map(&result.metadata, "metadata")?;
    if let Some(trace) = &result.trace {
        validate_value(trace, "trace")?;
    }
    for (index, error) in result.errors.iter().enumerate() {
        validate_value_map(&error.metadata, &format!("errors[{index}].metadata"))?;
    }
    Ok(())
}

pub(super) fn validate_result_node(node: &ir::ResultNode, path: &str) -> Result<(), PortableError> {
    match node {
        ir::ResultNode::Hit(hit) => {
            validate_relevance(hit.score, &format!("{path}.relevance"))?;
            validate_value_map(&hit.fields, &format!("{path}.fields"))?;
            validate_value_map(&hit.match_features, &format!("{path}.match_features"))?;
            validate_value_map(&hit.summary_features, &format!("{path}.summary_features"))?;
            validate_value_map(&hit.rank_features, &format!("{path}.rank_features"))?;
            for (index, value) in hit.sort_values.iter().enumerate() {
                validate_value(value, &format!("{path}.sort_values[{index}]"))?;
            }
        }
        ir::ResultNode::Group(group) => {
            validate_relevance(group.score, &format!("{path}.relevance"))?;
            validate_value_option(&group.value, &format!("{path}.value"))?;
            validate_value_map(&group.fields, &format!("{path}.fields"))?;
            for (index, child) in group.children.iter().enumerate() {
                validate_result_node(child, &format!("{path}.children[{index}]"))?;
            }
            if let Some(limits) = &group.limits {
                validate_value_option(&limits.from, &format!("{path}.limits.from"))?;
                validate_value_option(&limits.to, &format!("{path}.limits.to"))?;
            }
            validate_value_map(&group.metadata, &format!("{path}.metadata"))?;
        }
    }
    Ok(())
}

pub(super) fn validate_value_map(
    values: &std::collections::BTreeMap<String, ir::Value>,
    path: &str,
) -> Result<(), PortableError> {
    for (key, value) in values {
        validate_value(value, &format!("{path}.{key}"))?;
    }
    Ok(())
}

pub(super) fn validate_value_option(
    value: &Option<ir::Value>,
    path: &str,
) -> Result<(), PortableError> {
    if let Some(value) = value {
        validate_value(value, path)?;
    }
    Ok(())
}

pub(super) fn validate_value(value: &ir::Value, path: &str) -> Result<(), PortableError> {
    match value {
        ir::Value::List(values) => {
            for (index, value) in values.iter().enumerate() {
                validate_value(value, &format!("{path}[{index}]"))?;
            }
        }
        ir::Value::Object(values) => validate_value_map(values, path)?,
        ir::Value::Tensor(tensor) => {
            let finite = match &tensor.payload {
                ir::RequestTensorPayloadIr::Dense(values) => {
                    values.iter().all(|value| value.is_finite())
                }
                ir::RequestTensorPayloadIr::Sparse(cells) => {
                    cells.iter().all(|cell| cell.value.is_finite())
                }
                ir::RequestTensorPayloadIr::Mixed(blocks) => blocks
                    .iter()
                    .all(|block| block.values.iter().all(|value| value.is_finite())),
            };
            if !finite {
                return Err(PortableError::invalid(
                    "non_finite_tensor_cell",
                    Some(format!("{path}.payload")),
                    "tensor cell values must be finite",
                ));
            }
        }
        _ => {}
    }
    Ok(())
}

pub(super) fn validate_relevance(score: Option<f64>, path: &str) -> Result<(), PortableError> {
    if score.is_some_and(f64::is_nan) {
        return Err(PortableError::invalid(
            "non_finite_relevance",
            Some(path),
            "relevance must be finite or infinity",
        ));
    }
    Ok(())
}

pub(super) fn validate_finite(
    value: Option<f64>,
    path: &str,
    message: &str,
) -> Result<(), PortableError> {
    if value.is_some_and(|value| !value.is_finite()) {
        return Err(PortableError::invalid(
            "non_finite_number",
            Some(path),
            message,
        ));
    }
    Ok(())
}

pub fn analyze_result_to_api(result: &ir::AnalyzeResult) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let root = analyze_result_offset(&mut builder, result);
    builder.finish(root, None);
    builder.finished_data().to_vec()
}

pub fn analyze_result_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    result: &ir::AnalyzeResult,
) -> flatbuffers::WIPOffset<api::AnalyzeResult<'a>> {
    let tokens = result
        .tokens
        .iter()
        .map(|token| {
            let surface = builder.create_string(&token.surface);
            let term = builder.create_string(&token.term);
            api::AnalyzedToken::create(
                builder,
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
    api::AnalyzeResult::create(
        builder,
        &api::AnalyzeResultArgs {
            tokens: Some(tokens),
        },
    )
}

pub(super) fn value_offset<'a>(
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
        ir::Value::Float64(value) if value.is_finite() => (
            api::ValueKind::Float64Value,
            api::Float64Value::create(builder, &api::Float64ValueArgs { value: *value })
                .as_union_value(),
        ),
        ir::Value::Float64(value) => {
            let kind = if value.is_nan() {
                api::NonFiniteFloatKind::NaN
            } else if value.is_sign_positive() {
                api::NonFiniteFloatKind::PositiveInfinity
            } else {
                api::NonFiniteFloatKind::NegativeInfinity
            };
            (
                api::ValueKind::NonFiniteFloatValue,
                api::NonFiniteFloatValue::create(builder, &api::NonFiniteFloatValueArgs { kind })
                    .as_union_value(),
            )
        }
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
        ir::Value::Tensor(value) => (
            api::ValueKind::TensorValue,
            request_tensor_offset(builder, value).as_union_value(),
        ),
    };
    api::Value::create(
        builder,
        &api::ValueArgs {
            kind_type,
            kind: Some(kind),
        },
    )
}

pub(super) fn document_offset<'a>(
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

pub(super) fn value_entries_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    values: &std::collections::BTreeMap<String, ir::Value>,
) -> Option<
    flatbuffers::WIPOffset<
        flatbuffers::Vector<'a, flatbuffers::ForwardsUOffset<api::ValueEntry<'a>>>,
    >,
> {
    (!values.is_empty()).then(|| {
        let entries = values
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
    })
}

pub(super) fn relevance_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    score: f64,
) -> flatbuffers::WIPOffset<api::Relevance<'a>> {
    let (value_type, value) = if score.is_infinite() && score.is_sign_positive() {
        (
            api::RelevanceKind::PositiveInfinityRelevance,
            api::PositiveInfinityRelevance::create(builder, &api::PositiveInfinityRelevanceArgs {})
                .as_union_value(),
        )
    } else if score.is_infinite() {
        (
            api::RelevanceKind::NegativeInfinityRelevance,
            api::NegativeInfinityRelevance::create(builder, &api::NegativeInfinityRelevanceArgs {})
                .as_union_value(),
        )
    } else {
        (
            api::RelevanceKind::FiniteRelevance,
            api::FiniteRelevance::create(builder, &api::FiniteRelevanceArgs { value: score })
                .as_union_value(),
        )
    };
    api::Relevance::create(
        builder,
        &api::RelevanceArgs {
            value_type,
            value: Some(value),
        },
    )
}

pub(super) fn result_node_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    node: &ir::ResultNode,
) -> flatbuffers::WIPOffset<api::ResultNode<'a>> {
    let (node_type, node) = match node {
        ir::ResultNode::Hit(hit) => (
            api::ResultNodeKind::Hit,
            hit_offset(builder, hit).as_union_value(),
        ),
        ir::ResultNode::Group(group) => (
            api::ResultNodeKind::ResultGroup,
            result_group_offset(builder, group).as_union_value(),
        ),
    };
    api::ResultNode::create(
        builder,
        &api::ResultNodeArgs {
            node_type,
            node: Some(node),
        },
    )
}

pub(super) fn result_group_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    group: &ir::ResultGroup,
) -> flatbuffers::WIPOffset<api::ResultGroup<'a>> {
    let id = group.id.as_ref().map(|id| builder.create_string(id));
    let label = group
        .label
        .as_ref()
        .map(|label| builder.create_string(label));
    let value = group
        .value
        .as_ref()
        .map(|value| value_offset(builder, value));
    let fields = value_entries_offset(builder, &group.fields);
    let children = (!group.children.is_empty()).then(|| {
        let children = group
            .children
            .iter()
            .map(|child| result_node_offset(builder, child))
            .collect::<Vec<_>>();
        builder.create_vector(&children)
    });
    let continuations = string_entry_offsets(builder, &group.continuations);
    let source = group
        .source
        .as_ref()
        .map(|source| builder.create_string(source));
    let types = string_vector_offset(builder, &group.types);
    let relevance = group.score.map(|score| relevance_offset(builder, score));
    let limits = group.limits.as_ref().map(|limits| {
        let from = limits
            .from
            .as_ref()
            .map(|value| value_offset(builder, value));
        let to = limits.to.as_ref().map(|value| value_offset(builder, value));
        api::ResultLimits::create(builder, &api::ResultLimitsArgs { from, to })
    });
    let metadata = value_entries_offset(builder, &group.metadata);
    api::ResultGroup::create(
        builder,
        &api::ResultGroupArgs {
            id,
            label,
            value,
            fields,
            children,
            continuations,
            source,
            types,
            relevance,
            limits,
            metadata,
        },
    )
}

pub(super) fn coverage_degraded_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    degraded: &ir::CoverageDegraded,
) -> flatbuffers::WIPOffset<api::CoverageDegraded<'a>> {
    let match_phase = degraded.match_phase.map(api::OptionalBool::new);
    let timeout = degraded.timeout.map(api::OptionalBool::new);
    let adaptive_timeout = degraded.adaptive_timeout.map(api::OptionalBool::new);
    let ann_timeout = degraded.ann_timeout.map(api::OptionalBool::new);
    let non_ideal_state = degraded.non_ideal_state.map(api::OptionalBool::new);
    let metadata = value_entries_offset(builder, &degraded.metadata);
    api::CoverageDegraded::create(
        builder,
        &api::CoverageDegradedArgs {
            match_phase: match_phase.as_ref(),
            timeout: timeout.as_ref(),
            adaptive_timeout: adaptive_timeout.as_ref(),
            ann_timeout: ann_timeout.as_ref(),
            non_ideal_state: non_ideal_state.as_ref(),
            metadata,
        },
    )
}

pub(super) fn coverage_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    coverage: &ir::Coverage,
) -> flatbuffers::WIPOffset<api::Coverage<'a>> {
    let coverage_percent = coverage.coverage_percent.map(api::OptionalDouble::new);
    let documents = coverage.documents.map(api::OptionalUInt64::new);
    let full = coverage.full.map(api::OptionalBool::new);
    let nodes = coverage.nodes.map(api::OptionalUInt32::new);
    let results = coverage.results.map(api::OptionalUInt64::new);
    let results_full = coverage.results_full.map(api::OptionalUInt64::new);
    let degraded = coverage
        .degraded
        .as_ref()
        .map(|degraded| coverage_degraded_offset(builder, degraded));
    let metadata = value_entries_offset(builder, &coverage.metadata);
    api::Coverage::create(
        builder,
        &api::CoverageArgs {
            coverage_percent: coverage_percent.as_ref(),
            documents: documents.as_ref(),
            full: full.as_ref(),
            nodes: nodes.as_ref(),
            results: results.as_ref(),
            results_full: results_full.as_ref(),
            degraded,
            metadata,
        },
    )
}

pub(super) fn search_timing_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    timing: &ir::SearchTiming,
) -> flatbuffers::WIPOffset<api::SearchTiming<'a>> {
    let query_time_seconds = timing.query_time_seconds.map(api::OptionalDouble::new);
    let search_time_seconds = timing.search_time_seconds.map(api::OptionalDouble::new);
    let summary_fetch_time_seconds = timing
        .summary_fetch_time_seconds
        .map(api::OptionalDouble::new);
    let metadata = value_entries_offset(builder, &timing.metadata);
    api::SearchTiming::create(
        builder,
        &api::SearchTimingArgs {
            query_time_seconds: query_time_seconds.as_ref(),
            search_time_seconds: search_time_seconds.as_ref(),
            summary_fetch_time_seconds: summary_fetch_time_seconds.as_ref(),
            metadata,
        },
    )
}

pub(super) fn search_result_error_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    error: &ir::SearchResultError,
) -> flatbuffers::WIPOffset<api::SearchResultError<'a>> {
    let summary = error
        .summary
        .as_ref()
        .map(|summary| builder.create_string(summary));
    let message = error
        .message
        .as_ref()
        .map(|message| builder.create_string(message));
    let source = error
        .source
        .as_ref()
        .map(|source| builder.create_string(source));
    let stack_trace = error
        .stack_trace
        .as_ref()
        .map(|stack_trace| builder.create_string(stack_trace));
    let transient = error.transient.map(api::OptionalBool::new);
    let metadata = value_entries_offset(builder, &error.metadata);
    api::SearchResultError::create(
        builder,
        &api::SearchResultErrorArgs {
            code: error.code,
            summary,
            message,
            source,
            stack_trace,
            transient: transient.as_ref(),
            metadata,
        },
    )
}

pub(super) fn hit_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    hit: &ir::Hit,
) -> flatbuffers::WIPOffset<api::Hit<'a>> {
    let id = builder.create_string(&hit.id);
    let relevance = hit.score.map(|score| relevance_offset(builder, score));
    let fields = value_entries_offset(builder, &hit.fields);
    let highlights = hit
        .highlights
        .iter()
        .map(|highlight| highlight_result_offset(builder, highlight))
        .collect::<Vec<_>>();
    let highlights = (!highlights.is_empty()).then(|| builder.create_vector(&highlights));
    let source = hit
        .source
        .as_ref()
        .map(|source| builder.create_string(source));
    let types = string_vector_offset(builder, &hit.types);
    let match_features = value_entries_offset(builder, &hit.match_features);
    let summary_features = value_entries_offset(builder, &hit.summary_features);
    let rank_features = value_entries_offset(builder, &hit.rank_features);
    let sort_values = (!hit.sort_values.is_empty()).then(|| {
        let values = hit
            .sort_values
            .iter()
            .map(|value| value_offset(builder, value))
            .collect::<Vec<_>>();
        builder.create_vector(&values)
    });
    api::Hit::create(
        builder,
        &api::HitArgs {
            id: Some(id),
            relevance,
            fields,
            highlights,
            source,
            types,
            match_features,
            summary_features,
            rank_features,
            sort_values,
        },
    )
}

pub(super) fn highlight_result_offset<'a>(
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
