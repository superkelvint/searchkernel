use crate::{
    error::{ErrorCode, PortableError},
    model as ir,
};
use searchkernel_protocol::wire::v1 as api;
use std::{collections::BTreeMap, result::Result};

fn invalid_response(path: impl Into<String>, message: impl Into<String>) -> PortableError {
    let path = path.into();
    let message = message.into();
    PortableError::new(ErrorCode::Internal, message.clone()).with_detail(
        "invalid_remote_response",
        Some(path),
        message,
    )
}

fn present_string(value: Option<&str>, path: &str) -> Result<String, PortableError> {
    value
        .map(ToOwned::to_owned)
        .ok_or_else(|| invalid_response(path, "required string is missing"))
}

fn non_empty_string(value: Option<&str>, path: &str) -> Result<String, PortableError> {
    let value = present_string(value, path)?;
    if value.is_empty() {
        return Err(invalid_response(path, "required string must not be empty"));
    }
    Ok(value)
}

fn result_value_from_api(value: api::Value<'_>, path: &str) -> Result<ir::Value, PortableError> {
    match value.kind_type() {
        api::ValueKind::NullValueValue => {
            let value = value
                .kind_as_null_value_value()
                .ok_or_else(|| invalid_response(path, "null value payload is missing"))?;
            if value.value() != api::NullValue::Null {
                return Err(invalid_response(path, "unknown null value discriminator"));
            }
            Ok(ir::Value::Null)
        }
        api::ValueKind::BoolValue => value
            .kind_as_bool_value()
            .map(|value| ir::Value::Bool(value.value()))
            .ok_or_else(|| invalid_response(path, "boolean value payload is missing")),
        api::ValueKind::Int64Value => value
            .kind_as_int_64_value()
            .map(|value| ir::Value::Int64(value.value()))
            .ok_or_else(|| invalid_response(path, "integer value payload is missing")),
        api::ValueKind::UInt64Value => value
            .kind_as_uint_64_value()
            .map(|value| ir::Value::UInt64(value.value()))
            .ok_or_else(|| invalid_response(path, "unsigned integer value payload is missing")),
        api::ValueKind::Float64Value => {
            let value = value
                .kind_as_float_64_value()
                .ok_or_else(|| invalid_response(path, "float value payload is missing"))?
                .value();
            if !value.is_finite() {
                return Err(invalid_response(
                    path,
                    "Float64Value payload must be finite; non-finite values require NonFiniteFloatValue",
                ));
            }
            Ok(ir::Value::Float64(value))
        }
        api::ValueKind::NonFiniteFloatValue => {
            let value = value
                .kind_as_non_finite_float_value()
                .ok_or_else(|| invalid_response(path, "non-finite float payload is missing"))?;
            let value = match value.kind() {
                api::NonFiniteFloatKind::NaN => f64::NAN,
                api::NonFiniteFloatKind::PositiveInfinity => f64::INFINITY,
                api::NonFiniteFloatKind::NegativeInfinity => f64::NEG_INFINITY,
                _ => return Err(invalid_response(path, "unknown non-finite float kind")),
            };
            Ok(ir::Value::Float64(value))
        }
        api::ValueKind::StringValue => value
            .kind_as_string_value()
            .and_then(|value| value.value())
            .map(|value| ir::Value::String(value.to_owned()))
            .ok_or_else(|| invalid_response(path, "string value payload is missing")),
        api::ValueKind::BytesValue => value
            .kind_as_bytes_value()
            .and_then(|value| value.value())
            .map(|value| ir::Value::Bytes(value.iter().collect()))
            .ok_or_else(|| invalid_response(path, "bytes value payload is missing")),
        api::ValueKind::ValueList => {
            let values = value
                .kind_as_value_list()
                .ok_or_else(|| invalid_response(path, "list value payload is missing"))?
                .values()
                .ok_or_else(|| invalid_response(path, "list values vector is missing"))?
                .iter()
                .enumerate()
                .map(|(index, value)| result_value_from_api(value, &format!("{path}[{index}]")))
                .collect::<Result<Vec<_>, PortableError>>()?;
            Ok(ir::Value::List(values))
        }
        api::ValueKind::ValueObject => {
            let values = value
                .kind_as_value_object()
                .ok_or_else(|| invalid_response(path, "object value payload is missing"))?
                .fields();
            Ok(ir::Value::Object(value_entries_from_api(values, path)?))
        }
        api::ValueKind::TensorValue => {
            let tensor = value
                .kind_as_tensor_value()
                .ok_or_else(|| invalid_response(path, "tensor value payload is missing"))?;
            crate::api_to_ir::value::request_tensor_to_ir(tensor, path.to_owned())
                .map(ir::Value::Tensor)
                .map_err(|error| {
                    invalid_response(
                        path,
                        format!("invalid tensor response payload: {}", error.message),
                    )
                })
        }
        _ => Err(invalid_response(
            path,
            "value does not select a supported kind",
        )),
    }
}

fn value_entries_from_api(
    entries: Option<flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<api::ValueEntry<'_>>>>,
    path: &str,
) -> Result<BTreeMap<String, ir::Value>, PortableError> {
    let mut output = BTreeMap::new();
    if let Some(entries) = entries {
        for (index, entry) in entries.iter().enumerate() {
            let key = present_string(Some(entry.key()), &format!("{path}[{index}].key"))?;
            let value = entry.value().ok_or_else(|| {
                invalid_response(format!("{path}[{index}].value"), "value is missing")
            })?;
            if output
                .insert(
                    key.clone(),
                    result_value_from_api(value, &format!("{path}.{key}"))?,
                )
                .is_some()
            {
                return Err(invalid_response(
                    format!("{path}[{index}].key"),
                    format!("duplicate value entry {key}"),
                ));
            }
        }
    }
    Ok(output)
}

fn string_entries_from_api(
    entries: Option<flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<api::StringEntry<'_>>>>,
    path: &str,
) -> Result<BTreeMap<String, String>, PortableError> {
    let mut output = BTreeMap::new();
    if let Some(entries) = entries {
        for (index, entry) in entries.iter().enumerate() {
            let key = present_string(Some(entry.key()), &format!("{path}[{index}].key"))?;
            let value = present_string(entry.value(), &format!("{path}[{index}].value"))?;
            if output.insert(key.clone(), value).is_some() {
                return Err(invalid_response(
                    format!("{path}[{index}].key"),
                    format!("duplicate string entry {key}"),
                ));
            }
        }
    }
    Ok(output)
}

fn document_from_api(
    document: api::Document<'_>,
    path: &str,
) -> Result<ir::DocumentResult, PortableError> {
    Ok(ir::DocumentResult {
        id: non_empty_string(document.id(), &format!("{path}.id"))?,
        fields: value_entries_from_api(document.fields(), &format!("{path}.fields"))?,
    })
}

pub fn insert_result_from_api(
    result: api::InsertResult<'_>,
) -> Result<ir::InsertResult, PortableError> {
    Ok(ir::InsertResult {
        id: non_empty_string(result.id(), "insert.id")?,
    })
}

pub fn upsert_result_from_api(
    result: api::UpsertResult<'_>,
) -> Result<ir::UpsertResult, PortableError> {
    Ok(ir::UpsertResult {
        id: non_empty_string(result.id(), "upsert.id")?,
    })
}

pub fn patch_result_from_api(
    result: api::PatchResult<'_>,
) -> Result<ir::PatchResult, PortableError> {
    Ok(ir::PatchResult {
        id: non_empty_string(result.id(), "patch.id")?,
    })
}

pub fn update_result_from_api(
    result: api::UpdateResult<'_>,
) -> Result<ir::UpdateResult, PortableError> {
    Ok(ir::UpdateResult {
        id: non_empty_string(result.id(), "update.id")?,
    })
}

pub fn delete_result_from_api(
    result: api::DeleteResult<'_>,
) -> Result<ir::DeleteResult, PortableError> {
    Ok(ir::DeleteResult {
        id: non_empty_string(result.id(), "delete.id")?,
        deleted: result.deleted(),
    })
}

pub fn get_result_from_api(result: api::GetResult<'_>) -> Result<ir::GetResult, PortableError> {
    Ok(ir::GetResult {
        document: result
            .document()
            .map(|document| document_from_api(document, "get.document"))
            .transpose()?,
    })
}

fn relevance_from_api(relevance: api::Relevance<'_>, path: &str) -> Result<f64, PortableError> {
    match relevance.value_type() {
        api::RelevanceKind::FiniteRelevance => relevance
            .value_as_finite_relevance()
            .map(|value| value.value())
            .filter(|value| value.is_finite())
            .ok_or_else(|| {
                invalid_response(path, "FiniteRelevance payload must contain a finite value")
            }),
        api::RelevanceKind::PositiveInfinityRelevance => relevance
            .value_as_positive_infinity_relevance()
            .map(|_| f64::INFINITY)
            .ok_or_else(|| {
                invalid_response(path, "positive-infinity relevance payload is missing")
            }),
        api::RelevanceKind::NegativeInfinityRelevance => relevance
            .value_as_negative_infinity_relevance()
            .map(|_| f64::NEG_INFINITY)
            .ok_or_else(|| {
                invalid_response(path, "negative-infinity relevance payload is missing")
            }),
        _ => Err(invalid_response(path, "relevance does not select a kind")),
    }
}

fn highlights_from_api(
    values: Option<flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<api::HighlightResult<'_>>>>,
    path: &str,
) -> Result<Vec<ir::HighlightResult>, PortableError> {
    values
        .map(|values| {
            values
                .iter()
                .enumerate()
                .map(|(index, highlight)| {
                    let hpath = format!("{path}[{index}]");
                    let fragments = highlight
                        .fragments()
                        .map(|fragments| {
                            fragments
                                .iter()
                                .enumerate()
                                .map(|(fragment_index, fragment)| {
                                    let fpath = format!("{hpath}.fragments[{fragment_index}]");
                                    let text = present_string(
                                        fragment.text(),
                                        &format!("{fpath}.text"),
                                    )?;
                                    let matches = fragment
                                        .matches()
                                        .map(|matches| {
                                            matches
                                                .iter()
                                                .enumerate()
                                                .map(|(span_index, span)| {
                                                    let start = span.start_byte();
                                                    let end = span.end_byte();
                                                    if start > end || usize::try_from(end).map_or(true, |end| end > text.len()) {
                                                        return Err(invalid_response(
                                                            format!("{fpath}.matches[{span_index}]"),
                                                            "highlight span is outside its fragment",
                                                        ));
                                                    }
                                                    Ok(ir::HighlightSpan {
                                                        start_byte: start,
                                                        end_byte: end,
                                                    })
                                                })
                                                .collect::<Result<Vec<_>, PortableError>>()
                                        })
                                        .transpose()?
                                        .unwrap_or_default();
                                    Ok(ir::HighlightFragment { text, matches })
                                })
                                .collect()
                        })
                        .transpose()?
                        .unwrap_or_default();
                    Ok(ir::HighlightResult {
                        field: present_string(highlight.field(), &format!("{hpath}.field"))?,
                        fragments,
                    })
                })
                .collect()
        })
        .transpose()
        .map(Option::unwrap_or_default)
}

fn strings_from_api(
    values: Option<flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<&str>>>,
) -> Vec<String> {
    values
        .map(|values| values.iter().map(ToOwned::to_owned).collect())
        .unwrap_or_default()
}

fn result_node_from_api(
    node: api::ResultNode<'_>,
    path: &str,
) -> Result<ir::ResultNode, PortableError> {
    match node.node_type() {
        api::ResultNodeKind::Hit => {
            let hit = node
                .node_as_hit()
                .ok_or_else(|| invalid_response(path, "hit result payload is missing"))?;
            let sort_values = hit
                .sort_values()
                .map(|values| {
                    values
                        .iter()
                        .enumerate()
                        .map(|(index, value)| {
                            result_value_from_api(value, &format!("{path}.sort_values[{index}]"))
                        })
                        .collect()
                })
                .transpose()?
                .unwrap_or_default();
            Ok(ir::ResultNode::Hit(ir::Hit {
                id: non_empty_string(hit.id(), &format!("{path}.id"))?,
                score: hit
                    .relevance()
                    .map(|value| relevance_from_api(value, &format!("{path}.relevance")))
                    .transpose()?,
                fields: value_entries_from_api(hit.fields(), &format!("{path}.fields"))?,
                highlights: highlights_from_api(hit.highlights(), &format!("{path}.highlights"))?,
                source: hit.source().map(ToOwned::to_owned),
                types: strings_from_api(hit.types()),
                match_features: value_entries_from_api(
                    hit.match_features(),
                    &format!("{path}.match_features"),
                )?,
                summary_features: value_entries_from_api(
                    hit.summary_features(),
                    &format!("{path}.summary_features"),
                )?,
                rank_features: value_entries_from_api(
                    hit.rank_features(),
                    &format!("{path}.rank_features"),
                )?,
                sort_values,
            }))
        }
        api::ResultNodeKind::ResultGroup => {
            let group = node
                .node_as_result_group()
                .ok_or_else(|| invalid_response(path, "group result payload is missing"))?;
            let children = group
                .children()
                .map(|children| {
                    children
                        .iter()
                        .enumerate()
                        .map(|(index, child)| {
                            result_node_from_api(child, &format!("{path}.children[{index}]"))
                        })
                        .collect()
                })
                .transpose()?
                .unwrap_or_default();
            let limits = group
                .limits()
                .map(|limits| {
                    Ok(ir::ResultLimits {
                        from: limits
                            .from()
                            .map(|value| {
                                result_value_from_api(value, &format!("{path}.limits.from"))
                            })
                            .transpose()?,
                        to: limits
                            .to()
                            .map(|value| result_value_from_api(value, &format!("{path}.limits.to")))
                            .transpose()?,
                    })
                })
                .transpose()?;
            Ok(ir::ResultNode::Group(ir::ResultGroup {
                id: group.id().map(ToOwned::to_owned),
                label: group.label().map(ToOwned::to_owned),
                value: group
                    .value()
                    .map(|value| result_value_from_api(value, &format!("{path}.value")))
                    .transpose()?,
                fields: value_entries_from_api(group.fields(), &format!("{path}.fields"))?,
                children,
                continuations: string_entries_from_api(
                    group.continuations(),
                    &format!("{path}.continuations"),
                )?,
                source: group.source().map(ToOwned::to_owned),
                types: strings_from_api(group.types()),
                score: group
                    .relevance()
                    .map(|value| relevance_from_api(value, &format!("{path}.relevance")))
                    .transpose()?,
                limits,
                metadata: value_entries_from_api(group.metadata(), &format!("{path}.metadata"))?,
            }))
        }
        _ => Err(invalid_response(
            path,
            "result node does not select a supported kind",
        )),
    }
}

fn optional_finite(
    value: Option<&api::OptionalDouble>,
    path: &str,
) -> Result<Option<f64>, PortableError> {
    value
        .map(|value| value.value())
        .map(|value| {
            value.is_finite().then_some(value).ok_or_else(|| {
                invalid_response(path, "floating-point response value must be finite")
            })
        })
        .transpose()
}

fn coverage_from_api(
    coverage: api::Coverage<'_>,
    path: &str,
) -> Result<ir::Coverage, PortableError> {
    let degraded = coverage
        .degraded()
        .map(|degraded| {
            Ok(ir::CoverageDegraded {
                match_phase: degraded.match_phase().map(|value| value.value()),
                timeout: degraded.timeout().map(|value| value.value()),
                adaptive_timeout: degraded.adaptive_timeout().map(|value| value.value()),
                ann_timeout: degraded.ann_timeout().map(|value| value.value()),
                non_ideal_state: degraded.non_ideal_state().map(|value| value.value()),
                metadata: value_entries_from_api(
                    degraded.metadata(),
                    &format!("{path}.degraded.metadata"),
                )?,
            })
        })
        .transpose()?;
    Ok(ir::Coverage {
        coverage_percent: optional_finite(
            coverage.coverage_percent(),
            &format!("{path}.coverage_percent"),
        )?,
        documents: coverage.documents().map(|value| value.value()),
        full: coverage.full().map(|value| value.value()),
        nodes: coverage.nodes().map(|value| value.value()),
        results: coverage.results().map(|value| value.value()),
        results_full: coverage.results_full().map(|value| value.value()),
        degraded,
        metadata: value_entries_from_api(coverage.metadata(), &format!("{path}.metadata"))?,
    })
}

fn timing_from_api(
    timing: api::SearchTiming<'_>,
    path: &str,
) -> Result<ir::SearchTiming, PortableError> {
    Ok(ir::SearchTiming {
        query_time_seconds: optional_finite(
            timing.query_time_seconds(),
            &format!("{path}.query_time_seconds"),
        )?,
        search_time_seconds: optional_finite(
            timing.search_time_seconds(),
            &format!("{path}.search_time_seconds"),
        )?,
        summary_fetch_time_seconds: optional_finite(
            timing.summary_fetch_time_seconds(),
            &format!("{path}.summary_fetch_time_seconds"),
        )?,
        metadata: value_entries_from_api(timing.metadata(), &format!("{path}.metadata"))?,
    })
}

pub fn search_result_from_api(
    result: api::SearchResult<'_>,
) -> Result<ir::SearchResult, PortableError> {
    let root = result
        .root()
        .ok_or_else(|| invalid_response("search.root", "search result root is missing"))?;
    let total_hits = result
        .total_hits()
        .ok_or_else(|| invalid_response("search.total_hits", "total-hit information is missing"))?;
    let relation = match total_hits.relation() {
        api::TotalHitsRelation::Unspecified => ir::TotalHitsRelation::Unspecified,
        api::TotalHitsRelation::Exact => ir::TotalHitsRelation::Exact,
        api::TotalHitsRelation::LowerBound => ir::TotalHitsRelation::LowerBound,
        api::TotalHitsRelation::Approximate => ir::TotalHitsRelation::Approximate,
        api::TotalHitsRelation::Unknown => ir::TotalHitsRelation::Unknown,
        _ => {
            return Err(invalid_response(
                "search.total_hits.relation",
                "unknown total-hit relation",
            ))
        }
    };
    let errors = result
        .errors()
        .map(|errors| {
            errors
                .iter()
                .enumerate()
                .map(|(index, error)| {
                    Ok(ir::SearchResultError {
                        code: error.code(),
                        summary: error.summary().map(ToOwned::to_owned),
                        message: error.message().map(ToOwned::to_owned),
                        source: error.source().map(ToOwned::to_owned),
                        stack_trace: error.stack_trace().map(ToOwned::to_owned),
                        transient: error.transient().map(|value| value.value()),
                        metadata: value_entries_from_api(
                            error.metadata(),
                            &format!("search.errors[{index}].metadata"),
                        )?,
                    })
                })
                .collect()
        })
        .transpose()?
        .unwrap_or_default();
    let decoded = ir::SearchResult {
        root: result_node_from_api(root, "search.root")?,
        total_hits: ir::TotalHits {
            value: total_hits.value(),
            relation,
        },
        coverage: result
            .coverage()
            .map(|coverage| coverage_from_api(coverage, "search.coverage"))
            .transpose()?,
        timing: result
            .timing()
            .map(|timing| timing_from_api(timing, "search.timing"))
            .transpose()?,
        errors,
        trace: result
            .trace()
            .map(|trace| result_value_from_api(trace, "search.trace"))
            .transpose()?,
        metadata: value_entries_from_api(result.metadata(), "search.metadata")?,
    };
    crate::ir_to_api::validate_search_result(&decoded).map_err(|error| {
        invalid_response(
            "search",
            format!(
                "decoded remote search result violates canonical result invariants: {}",
                error.message
            ),
        )
    })?;
    Ok(decoded)
}

pub fn analyze_result_from_api(
    result: api::AnalyzeResult<'_>,
) -> Result<ir::AnalyzeResult, PortableError> {
    let tokens = result
        .tokens()
        .map(|tokens| {
            tokens
                .iter()
                .enumerate()
                .map(|(index, token)| {
                    let surface = present_string(
                        token.surface(),
                        &format!("analyze.tokens[{index}].surface"),
                    )?;
                    let start_byte = token.start_byte();
                    let end_byte = token.end_byte();
                    if start_byte > end_byte {
                        return Err(invalid_response(
                            format!("analyze.tokens[{index}]"),
                            "analyzed token byte range is reversed",
                        ));
                    }
                    Ok(ir::AnalyzedToken {
                        surface,
                        term: present_string(
                            token.term(),
                            &format!("analyze.tokens[{index}].term"),
                        )?,
                        position: token.position(),
                        start_byte,
                        end_byte,
                    })
                })
                .collect()
        })
        .transpose()?
        .unwrap_or_default();
    Ok(ir::AnalyzeResult { tokens })
}

pub fn portable_error_from_api(error: api::Error<'_>) -> Result<PortableError, PortableError> {
    let code = match error.code() {
        api::ErrorCode::InvalidArgument => ErrorCode::InvalidArgument,
        api::ErrorCode::NotFound => ErrorCode::NotFound,
        api::ErrorCode::AlreadyExists => ErrorCode::AlreadyExists,
        api::ErrorCode::FailedPrecondition => ErrorCode::FailedPrecondition,
        api::ErrorCode::Conflict => ErrorCode::Conflict,
        api::ErrorCode::Unsupported => ErrorCode::Unsupported,
        api::ErrorCode::ResourceExhausted => ErrorCode::ResourceExhausted,
        api::ErrorCode::Internal => ErrorCode::Internal,
        api::ErrorCode::Unavailable => ErrorCode::Unavailable,
        api::ErrorCode::DeadlineExceeded => ErrorCode::DeadlineExceeded,
        _ => {
            return Err(invalid_response(
                "error.code",
                "portable error code is unspecified or unknown",
            ))
        }
    };
    let message = present_string(error.message(), "error.message")?;
    let mut details = Vec::new();
    if let Some(api_details) = error.details() {
        for (index, detail) in api_details.iter().enumerate() {
            let path = format!("error.details[{index}]");
            details.push(crate::ErrorDetail {
                code: present_string(detail.code(), &format!("{path}.code"))?,
                path: detail.path().map(ToOwned::to_owned),
                message: present_string(detail.message(), &format!("{path}.message"))?,
                metadata: string_entries_from_api(detail.metadata(), &format!("{path}.metadata"))?,
            });
        }
    }
    Ok(PortableError {
        code,
        message,
        details,
        retryable: error.retryable(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn present_empty_strings_are_preserved() {
        assert_eq!(
            present_string(Some(""), "result.text").expect("present empty string"),
            ""
        );
        assert!(present_string(None, "result.text").is_err());
    }

    #[test]
    fn finite_value_discriminator_rejects_non_finite_payload() {
        let mut builder = flatbuffers::FlatBufferBuilder::new();
        let payload =
            api::Float64Value::create(&mut builder, &api::Float64ValueArgs { value: f64::NAN });
        let value = api::Value::create(
            &mut builder,
            &api::ValueArgs {
                kind_type: api::ValueKind::Float64Value,
                kind: Some(payload.as_union_value()),
            },
        );
        builder.finish(value, None);
        let value = flatbuffers::root::<api::Value<'_>>(builder.finished_data())
            .expect("test value FlatBuffer");

        let error = result_value_from_api(value, "result.value")
            .expect_err("non-finite Float64Value must be rejected");
        assert_eq!(error.code, ErrorCode::Internal);
        assert_eq!(error.details[0].code, "invalid_remote_response");
    }

    #[test]
    fn finite_relevance_discriminator_rejects_infinity() {
        let mut builder = flatbuffers::FlatBufferBuilder::new();
        let payload = api::FiniteRelevance::create(
            &mut builder,
            &api::FiniteRelevanceArgs {
                value: f64::INFINITY,
            },
        );
        let relevance = api::Relevance::create(
            &mut builder,
            &api::RelevanceArgs {
                value_type: api::RelevanceKind::FiniteRelevance,
                value: Some(payload.as_union_value()),
            },
        );
        builder.finish(relevance, None);
        let relevance = flatbuffers::root::<api::Relevance<'_>>(builder.finished_data())
            .expect("test relevance FlatBuffer");

        let error = relevance_from_api(relevance, "result.relevance")
            .expect_err("infinite FiniteRelevance must be rejected");
        assert_eq!(error.code, ErrorCode::Internal);
        assert_eq!(error.details[0].code, "invalid_remote_response");
    }

    #[test]
    fn coverage_rejects_non_finite_optional_double() {
        let mut builder = flatbuffers::FlatBufferBuilder::new();
        let coverage_percent = api::OptionalDouble::new(f64::INFINITY);
        let coverage = api::Coverage::create(
            &mut builder,
            &api::CoverageArgs {
                coverage_percent: Some(&coverage_percent),
                ..Default::default()
            },
        );
        builder.finish(coverage, None);
        let coverage = flatbuffers::root::<api::Coverage<'_>>(builder.finished_data())
            .expect("test coverage FlatBuffer");

        let error = coverage_from_api(coverage, "search.coverage")
            .expect_err("non-finite coverage must be rejected");
        assert_eq!(error.code, ErrorCode::Internal);
        assert_eq!(error.details[0].code, "invalid_remote_response");
    }

    #[test]
    fn malformed_tensor_response_stays_in_response_error_domain() {
        let mut builder = flatbuffers::FlatBufferBuilder::new();
        let dense_values = builder.create_vector(&[f64::NAN]);
        let dense = api::DenseTensorValues::create(
            &mut builder,
            &api::DenseTensorValuesArgs {
                values: Some(dense_values),
            },
        );
        let type_spec = builder.create_string("tensor<float>(x[1])");
        let tensor = api::TensorValue::create(
            &mut builder,
            &api::TensorValueArgs {
                type_spec: Some(type_spec),
                payload_type: api::TensorPayloadKind::DenseTensorValues,
                payload: Some(dense.as_union_value()),
            },
        );
        let value = api::Value::create(
            &mut builder,
            &api::ValueArgs {
                kind_type: api::ValueKind::TensorValue,
                kind: Some(tensor.as_union_value()),
            },
        );
        builder.finish(value, None);
        let value = flatbuffers::root::<api::Value<'_>>(builder.finished_data())
            .expect("test tensor value FlatBuffer");

        let error = result_value_from_api(value, "result.tensor")
            .expect_err("malformed tensor response must be rejected");
        assert_eq!(error.code, ErrorCode::Internal);
        assert_eq!(error.details[0].code, "invalid_remote_response");
    }

    #[test]
    fn empty_operation_result_identity_is_rejected() {
        let mut builder = flatbuffers::FlatBufferBuilder::new();
        let id = builder.create_string("");
        let result =
            api::InsertResult::create(&mut builder, &api::InsertResultArgs { id: Some(id) });
        builder.finish(result, None);
        let result = flatbuffers::root::<api::InsertResult<'_>>(builder.finished_data())
            .expect("test insert result FlatBuffer");

        let error = insert_result_from_api(result)
            .expect_err("empty remote document identity must be rejected");
        assert_eq!(error.code, ErrorCode::Internal);
        assert_eq!(error.details[0].code, "invalid_remote_response");
    }

    #[test]
    fn highlight_text_may_be_empty_but_spans_must_be_in_bounds() {
        let mut builder = flatbuffers::FlatBufferBuilder::new();
        let empty = builder.create_string("");
        let span = api::HighlightSpan::create(
            &mut builder,
            &api::HighlightSpanArgs {
                start_byte: 0,
                end_byte: 1,
            },
        );
        let spans = builder.create_vector(&[span]);
        let fragment = api::HighlightFragment::create(
            &mut builder,
            &api::HighlightFragmentArgs {
                text: Some(empty),
                matches: Some(spans),
            },
        );
        let fragments = builder.create_vector(&[fragment]);
        let field = builder.create_string("title");
        let highlight = api::HighlightResult::create(
            &mut builder,
            &api::HighlightResultArgs {
                field: Some(field),
                fragments: Some(fragments),
            },
        );
        let highlights = builder.create_vector(&[highlight]);
        builder.finish(highlights, None);

        let highlights = flatbuffers::root::<
            flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<api::HighlightResult<'_>>>,
        >(builder.finished_data())
        .expect("test highlights FlatBuffer");
        let error = highlights_from_api(Some(highlights), "hit.highlights")
            .expect_err("out-of-bounds highlight span must be rejected");
        assert_eq!(error.code, ErrorCode::Internal);
        assert_eq!(error.details[0].code, "invalid_remote_response");
    }
}
