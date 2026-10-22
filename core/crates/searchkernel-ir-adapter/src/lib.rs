//! Integration boundary between the portable Search Kernel IR and the
//! low-level Vespa engine wrapper.
//!
//! This crate is the only translation boundary between portable IR and the
//! Vespa-facing engine wrapper. It contains no fallback or in-memory engine.

// This optional native adapter is still compiled as a workspace member when
// default features are disabled; its implementation is only reachable from
// searchkernel's native feature in that configuration.
#![cfg_attr(not(feature = "native"), allow(dead_code, unused_imports))]

mod backend;
mod schema;
mod search;

use searchkernel_ir::{
    DocumentIr, FieldValueIr, PortableError, RequestTensorIr, RequestTensorPayloadIr, Result,
    ScalarValueIr, Value,
};
use searchkernel_vespa_engine::{Document, FieldValue, PositionValue, PredicateValue, ScalarValue};
use std::collections::BTreeMap;

#[cfg(all(test, feature = "native"))]
#[path = "tests/mod.rs"]
mod tests;

pub use backend::VespaBackend;
#[cfg(test)]
pub(crate) use backend::{portable_coverage, portable_errors_from_native_issues};
#[cfg(test)]
pub(crate) use backend::{MutationProbe, SearchGroupingProbe};
pub use schema::{
    compile_index_definition, rank_profile_change_supported, validate_index_definition,
};
#[cfg(test)]
pub(crate) use search::filter::{lower_comparison, lower_filter};
#[cfg(test)]
pub(crate) use search::lower_facet_groupings;
#[cfg(test)]
pub(crate) use search::lower_query;
#[cfg(test)]
pub(crate) use search::{
    lower_facets_into_request, lower_groupings, lower_search_options, lower_search_query,
    query_feature_value, ranking_profile, validate_search_request,
};

fn lower_document(document: &DocumentIr) -> Result<Document> {
    let mut result = Document::new(document.id().as_str());
    for (field, value) in document.fields() {
        let lowered = lower_field_value(value)?;
        result = result.field(field.as_str(), lowered.clone());
        if matches!(lowered, FieldValue::Scalar(ScalarValue::String(_))) {
            let analysis = searchkernel_vespa_engine::Linguistics.analyze_document(
                match value {
                    FieldValueIr::Single(ScalarValueIr::String(value)) => value,
                    _ => unreachable!("string annotation only follows a scalar string"),
                },
                searchkernel_vespa_engine::LinguisticsOptions::default(),
            );
            result = result.annotate_text(field.as_str(), analysis);
        }
    }
    Ok(result)
}

fn project_document(
    document: &Document,
    projection: &[searchkernel_ir::FieldName],
) -> Result<BTreeMap<String, Value>> {
    document
        .fields()
        .iter()
        .filter(|(name, _)| {
            projection
                .iter()
                .any(|field| field.as_str() == name.as_str())
        })
        .map(|(name, value)| value_to_ir(value).map(|value| (name.clone(), value)))
        .collect()
}

fn lower_field_value(value: &FieldValueIr) -> Result<FieldValue> {
    match value {
        FieldValueIr::Single(value) => Ok(FieldValue::Scalar(lower_scalar(value)?)),
        FieldValueIr::Position { latitude, longitude } => {
            let latitude = lower_position_coordinate(*latitude, "latitude")?;
            let longitude = lower_position_coordinate(*longitude, "longitude")?;
            Ok(FieldValue::Position(PositionValue::new(latitude, longitude)))
        },
        FieldValueIr::Predicate { features } => {
            if features.len() != 1 {
                return Err(PortableError::unsupported(
                    "predicate_feature_map_not_supported",
                    Some("document.value.features".to_owned()),
                    "the current native document boundary can preserve exactly one predicate feature key; use PredicateExpression for composed predicates",
                ));
            }
            Ok(FieldValue::Predicate(PredicateValue::new(features.clone())))
        }
        FieldValueIr::PredicateExpression { expression } => Ok(FieldValue::Predicate(
            PredicateValue::from_expression(native_predicate_expression(expression)?),
        )),
        FieldValueIr::Tensor(tensor) => match &tensor.payload {
            RequestTensorPayloadIr::Dense(values)
                if !tensor.type_spec.trim().is_empty()
                    && values.iter().all(|value| value.is_finite()) =>
            {
                Ok(FieldValue::Tensor(
                    searchkernel_vespa_engine::TensorValue::dense(
                        tensor.type_spec.clone(),
                        values.clone(),
                    ),
                ))
            }
            RequestTensorPayloadIr::Dense(_) => Err(PortableError::invalid(
                "invalid_tensor_value",
                Some("document.value".to_owned()),
                "dense tensor document values require a non-empty type spec and finite values",
            )),
            RequestTensorPayloadIr::Sparse(_) | RequestTensorPayloadIr::Mixed(_) => {
                Err(PortableError::unsupported(
                    "tensor_value_not_supported",
                    Some("document.value".to_owned()),
                    "sparse and mixed tensor document values are not supported by the current native document interface",
                ))
            }
        },
        FieldValueIr::Structured(values) => Ok(FieldValue::Array(
            searchkernel_vespa_engine::ArrayValue::Structs(
                values
                    .iter()
                    .map(|value| {
                        value
                            .iter()
                            .map(|(name, value)| {
                                lower_scalar(value).map(|value| (name.clone(), value))
                            })
                            .collect::<Result<BTreeMap<_, _>>>()
                    })
                    .collect::<Result<Vec<_>>>()?,
            ),
        )),
        FieldValueIr::Multi(values) => {
            let mut values = values.iter();
            let Some(first) = values.next() else {
                return Ok(FieldValue::Array(
                    searchkernel_vespa_engine::ArrayValue::Strings(Vec::new()),
                ));
            };
            let first = lower_scalar(first)?;
            match first {
                ScalarValue::String(first) => Ok(FieldValue::Array(
                    searchkernel_vespa_engine::ArrayValue::Strings(
                        std::iter::once(first)
                            .chain(
                                values
                                    .map(|value| match lower_scalar(value) {
                                        Ok(ScalarValue::String(value)) => Ok(value),
                                        Ok(_) => Err(unsupported("heterogeneous array")),
                                        Err(error) => Err(error),
                                    })
                                    .collect::<Result<Vec<_>>>()?,
                            )
                            .collect(),
                    ),
                )),
                ScalarValue::Integer(first) => Ok(FieldValue::Array(
                    searchkernel_vespa_engine::ArrayValue::Integers(
                        std::iter::once(first)
                            .chain(
                                values
                                    .map(|value| match lower_scalar(value) {
                                        Ok(ScalarValue::Integer(value)) => Ok(value),
                                        Ok(_) => Err(unsupported("heterogeneous array")),
                                        Err(error) => Err(error),
                                    })
                                    .collect::<Result<Vec<_>>>()?,
                            )
                            .collect(),
                    ),
                )),
                ScalarValue::Float(first) => Ok(FieldValue::Array(
                    searchkernel_vespa_engine::ArrayValue::Floats(
                        std::iter::once(first)
                            .chain(
                                values
                                    .map(|value| match lower_scalar(value) {
                                        Ok(ScalarValue::Float(value)) => Ok(value),
                                        Ok(_) => Err(unsupported("heterogeneous array")),
                                        Err(error) => Err(error),
                                    })
                                    .collect::<Result<Vec<_>>>()?,
                            )
                            .collect(),
                    ),
                )),
                ScalarValue::Boolean(first) => Ok(FieldValue::Array(
                    searchkernel_vespa_engine::ArrayValue::Booleans(
                        std::iter::once(first)
                            .chain(
                                values
                                    .map(|value| match lower_scalar(value) {
                                        Ok(ScalarValue::Boolean(value)) => Ok(value),
                                        Ok(_) => Err(unsupported("heterogeneous array")),
                                        Err(error) => Err(error),
                                    })
                                    .collect::<Result<Vec<_>>>()?,
                            )
                            .collect(),
                    ),
                )),
            }
        }
    }
}

fn validate_predicate_expression_numbers(value: &serde_json::Value) -> Result<()> {
    match value {
        serde_json::Value::Number(number) if number.as_i64().is_none() => {
            Err(PortableError::unsupported(
                "predicate_float_not_supported",
                Some("document.value.expression".to_owned()),
                "predicate expression numbers must be signed 64-bit integers",
            ))
        }
        serde_json::Value::Array(values) => {
            for value in values {
                validate_predicate_expression_numbers(value)?;
            }
            Ok(())
        }
        serde_json::Value::Object(values) => {
            for value in values.values() {
                validate_predicate_expression_numbers(value)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// Vespa normalizes predicate JSON while constructing the native value. The
/// native result decoder uses the root key order to preserve the distinction
/// between feature-form and expression-form predicates, so put the expression
/// discriminator first after canonical IR serialization has sorted object keys.
fn native_predicate_expression(expression: &str) -> Result<String> {
    let parsed = serde_json::from_str::<serde_json::Value>(expression).map_err(|error| {
        PortableError::invalid(
            "invalid_predicate_result",
            Some("document.value.expression"),
            format!("predicate expression is invalid JSON: {error}"),
        )
    })?;
    let Some(object) = parsed.as_object() else {
        return Err(PortableError::invalid(
            "invalid_predicate_result",
            Some("document.value.expression"),
            "predicate expression must be a JSON object",
        ));
    };
    validate_predicate_expression_numbers(&parsed)?;
    let Some(kind) = object.get("type") else {
        return Ok(expression.to_owned());
    };
    let mut remainder = object.clone();
    remainder.remove("type");
    let remainder =
        serde_json::to_string(&serde_json::Value::Object(remainder)).map_err(|error| {
            PortableError::invalid(
                "invalid_predicate_result",
                Some("document.value.expression"),
                format!("predicate expression could not be encoded: {error}"),
            )
        })?;
    let kind = serde_json::to_string(kind).map_err(|error| {
        PortableError::invalid(
            "invalid_predicate_result",
            Some("document.value.expression"),
            format!("predicate expression type could not be encoded: {error}"),
        )
    })?;
    let mut result = format!("{{\"type\":{kind}");
    if remainder.len() > 2 {
        result.push(',');
        result.push_str(&remainder[1..remainder.len() - 1]);
    }
    result.push('}');
    Ok(result)
}

fn lower_position_coordinate(value: f64, name: &str) -> Result<f64> {
    let scaled = value * 1_000_000.0;
    if !scaled.is_finite() || scaled.round() != scaled {
        return Err(PortableError::unsupported(
            "position_precision_not_supported",
            Some(format!("document.value.{name}")),
            "the current Vespa document representation preserves position coordinates only to microdegree precision",
        ));
    }
    Ok(value)
}

fn lower_scalar(value: &ScalarValueIr) -> Result<ScalarValue> {
    Ok(match value {
        ScalarValueIr::Bool(value) => ScalarValue::Boolean(*value),
        ScalarValueIr::Int64(value) => ScalarValue::Integer(*value),
        ScalarValueIr::Float64(value) => ScalarValue::Float(*value),
        ScalarValueIr::String(value) => ScalarValue::String(value.clone()),
    })
}

fn value_to_ir(value: &FieldValue) -> Result<Value> {
    Ok(match value {
        FieldValue::Scalar(ScalarValue::String(value)) => Value::String(value.clone()),
        FieldValue::Scalar(ScalarValue::Integer(value)) => Value::Int64(*value),
        FieldValue::Scalar(ScalarValue::Float(value)) => Value::Float64(*value),
        FieldValue::Scalar(ScalarValue::Boolean(value)) => Value::Bool(*value),
        FieldValue::Position(value) => Value::Object(
            [
                ("latitude".to_owned(), Value::Float64(value.latitude)),
                ("longitude".to_owned(), Value::Float64(value.longitude)),
            ]
            .into_iter()
            .collect(),
        ),
        FieldValue::Predicate(value) => predicate_value_to_ir(value)?,
        FieldValue::Array(searchkernel_vespa_engine::ArrayValue::Strings(values)) => {
            Value::List(values.iter().cloned().map(Value::String).collect())
        }
        FieldValue::Array(searchkernel_vespa_engine::ArrayValue::Integers(values)) => {
            Value::List(values.iter().copied().map(Value::Int64).collect())
        }
        FieldValue::Array(searchkernel_vespa_engine::ArrayValue::Floats(values)) => {
            Value::List(values.iter().copied().map(Value::Float64).collect())
        }
        FieldValue::Array(searchkernel_vespa_engine::ArrayValue::Booleans(values)) => {
            Value::List(values.iter().copied().map(Value::Bool).collect())
        }
        FieldValue::Array(searchkernel_vespa_engine::ArrayValue::Structs(values)) => Value::List(
            values
                .iter()
                .map(|value| {
                    Value::Object(
                        value
                            .iter()
                            .map(|(name, value)| (name.clone(), scalar_value_to_ir(value)))
                            .collect(),
                    )
                })
                .collect(),
        ),
        FieldValue::Tensor(tensor) => Value::Tensor(tensor_to_ir(tensor)),
    })
}

fn predicate_value_to_ir(value: &PredicateValue) -> Result<Value> {
    if let Some(expression) = &value.expression {
        let parsed = serde_json::from_str::<serde_json::Value>(expression).map_err(|error| {
            PortableError::invalid(
                "invalid_predicate_result",
                Some("document.value.expression"),
                format!("native predicate expression is invalid JSON: {error}"),
            )
        })?;
        return Ok(Value::Object(
            [(
                "expression".to_owned(),
                json_value_to_ir(parsed, "document.value.expression")?,
            )]
            .into_iter()
            .collect(),
        ));
    }

    Ok(Value::Object(
        [(
            "features".to_owned(),
            Value::Object(
                value
                    .features
                    .iter()
                    .map(|(key, value)| (key.clone(), Value::String(value.clone())))
                    .collect(),
            ),
        )]
        .into_iter()
        .collect(),
    ))
}

fn json_value_to_ir(value: serde_json::Value, path: &str) -> Result<Value> {
    Ok(match value {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(value) => Value::Bool(value),
        serde_json::Value::String(value) => Value::String(value),
        serde_json::Value::Array(values) => Value::List(
            values
                .into_iter()
                .enumerate()
                .map(|(index, value)| json_value_to_ir(value, &format!("{path}[{index}]")))
                .collect::<Result<Vec<_>>>()?,
        ),
        serde_json::Value::Object(values) => Value::Object(
            values
                .into_iter()
                .map(|(name, value)| {
                    json_value_to_ir(value, &format!("{path}.{name}")).map(|value| (name, value))
                })
                .collect::<Result<BTreeMap<_, _>>>()?,
        ),
        serde_json::Value::Number(value) => {
            if let Some(value) = value.as_i64() {
                Value::Int64(value)
            } else if let Some(value) = value.as_u64() {
                Value::UInt64(value)
            } else if let Some(value) = value.as_f64().filter(|value| value.is_finite()) {
                Value::Float64(value)
            } else {
                return Err(PortableError::invalid(
                    "invalid_predicate_result",
                    Some(path.to_owned()),
                    "predicate expression contains an unrepresentable number",
                ));
            }
        }
    })
}

fn scalar_value_to_ir(value: &ScalarValue) -> Value {
    match value {
        ScalarValue::String(value) => Value::String(value.clone()),
        ScalarValue::Integer(value) => Value::Int64(*value),
        ScalarValue::Float(value) => Value::Float64(*value),
        ScalarValue::Boolean(value) => Value::Bool(*value),
    }
}

fn tensor_to_ir(tensor: &searchkernel_vespa_engine::TensorValue) -> RequestTensorIr {
    match tensor {
        searchkernel_vespa_engine::TensorValue::Dense { type_spec, values } => RequestTensorIr {
            type_spec: type_spec.clone(),
            payload: RequestTensorPayloadIr::Dense(values.clone()),
        },
        searchkernel_vespa_engine::TensorValue::Sparse { type_spec, cells } => RequestTensorIr {
            type_spec: type_spec.clone(),
            payload: RequestTensorPayloadIr::Sparse(
                cells
                    .iter()
                    .map(|cell| searchkernel_ir::RequestTensorCellIr {
                        address: cell
                            .address
                            .iter()
                            .map(|entry| (entry.dimension.clone(), entry.label.clone()))
                            .collect(),
                        value: cell.value,
                    })
                    .collect(),
            ),
        },
        searchkernel_vespa_engine::TensorValue::Mixed { type_spec, blocks } => RequestTensorIr {
            type_spec: type_spec.clone(),
            payload: RequestTensorPayloadIr::Mixed(
                blocks
                    .iter()
                    .map(|block| searchkernel_ir::RequestTensorBlockIr {
                        address: block
                            .address
                            .iter()
                            .map(|entry| (entry.dimension.clone(), entry.label.clone()))
                            .collect(),
                        values: block.values.clone(),
                    })
                    .collect(),
            ),
        },
    }
}

fn native_error(error: searchkernel_vespa_engine::Error) -> PortableError {
    let message = error.to_string();
    match error.kind() {
        searchkernel_vespa_engine::ErrorKind::InvalidArgument => PortableError::invalid(
            error.detail_code().unwrap_or("backend_request"),
            None::<String>,
            message,
        ),
        searchkernel_vespa_engine::ErrorKind::ResourceExhausted => {
            PortableError::new(searchkernel_ir::ErrorCode::ResourceExhausted, message)
        }
        searchkernel_vespa_engine::ErrorKind::Unsupported => PortableError::unsupported(
            error
                .detail_code()
                .unwrap_or("native_capability_not_supported"),
            None::<String>,
            message,
        ),
        searchkernel_vespa_engine::ErrorKind::Type => {
            PortableError::invalid("backend_value_type", None::<String>, message)
        }
        searchkernel_vespa_engine::ErrorKind::Internal => {
            let mut portable = PortableError::new(searchkernel_ir::ErrorCode::Internal, message);
            if let Some(detail) = error.detail_code() {
                let detail_message = portable.message.clone();
                portable = portable.with_detail(detail, None::<String>, detail_message);
            }
            portable
        }
        searchkernel_vespa_engine::ErrorKind::DeadlineExceeded => {
            PortableError::new(searchkernel_ir::ErrorCode::DeadlineExceeded, message)
        }
    }
}

fn native_search_error(
    error: searchkernel_vespa_engine::Error,
    unsupported_code: &'static str,
) -> PortableError {
    if error.kind() == searchkernel_vespa_engine::ErrorKind::Unsupported {
        let detail_code = error.detail_code().unwrap_or(unsupported_code);
        let path = if detail_code == "facet_key_type_not_supported"
            || detail_code.starts_with("grouping_")
            || detail_code == "grouping_not_supported"
        {
            Some("grouping")
        } else {
            Some("sort")
        };
        return PortableError::unsupported(detail_code, path, error.to_string());
    }
    native_error(error)
}

fn analysis_options(
    language: Option<&searchkernel_ir::LanguageHint>,
) -> Result<searchkernel_vespa_engine::LinguisticsOptions> {
    match language.map(|value| value.as_str()) {
        None | Some("en") => Ok(searchkernel_vespa_engine::LinguisticsOptions::default()),
        Some(value) if value.starts_with("en-") => {
            Ok(searchkernel_vespa_engine::LinguisticsOptions::default())
        }
        Some(value) => Err(PortableError::unsupported(
            "language_not_supported",
            Some("language"),
            format!("language `{value}` is not supported by the current analyzer"),
        )),
    }
}

fn internal(message: impl Into<String>) -> PortableError {
    PortableError::new(searchkernel_ir::ErrorCode::Internal, message)
}

fn unsupported(message: impl Into<String>) -> PortableError {
    PortableError::unsupported("m_lowering", None::<String>, message)
}

#[cfg(test)]
mod predicate_expression_number_tests {
    use super::native_predicate_expression;
    use searchkernel_ir::ErrorCode;

    #[test]
    fn predicate_expression_float_numbers_fail_before_native_lowering() {
        let error = native_predicate_expression(
            r#"{"type":4,"key":"hobby","feature_set":["music"],"weight":1.23456789}"#,
        )
        .expect_err("floating predicate JSON must fail before native mutation");
        assert_eq!(error.code, ErrorCode::Unsupported);
        assert_eq!(error.details[0].code, "predicate_float_not_supported");
        assert_eq!(
            error.details[0].path.as_deref(),
            Some("document.value.expression")
        );
    }
}
