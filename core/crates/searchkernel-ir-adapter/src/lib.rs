//! Integration boundary between the portable Search Kernel IR and the
//! low-level Vespa engine wrapper.
//!
//! This crate is the only translation boundary between portable IR and the
//! Vespa-facing engine wrapper. It contains no fallback or in-memory engine.

mod backend;
mod schema;
mod search;

use searchkernel_ir::{
    DocumentIr, FieldValueIr, PortableError, RequestTensorIr, RequestTensorPayloadIr, Result,
    ScalarValueIr, Value,
};
use searchkernel_vespa_engine::{Document, FieldValue, PositionValue, PredicateValue, ScalarValue};
use std::collections::BTreeMap;

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) use backend::MutationProbe;
pub use backend::VespaBackend;
#[cfg(test)]
pub(crate) use backend::{portable_coverage, portable_errors_from_native_issues};
pub use schema::{compile_index_definition, validate_index_definition};
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
) -> BTreeMap<String, Value> {
    document
        .fields()
        .iter()
        .filter(|(name, _)| {
            projection
                .iter()
                .any(|field| field.as_str() == name.as_str())
        })
        .map(|(name, value)| (name.clone(), value_to_ir(value)))
        .collect()
}

fn lower_field_value(value: &FieldValueIr) -> Result<FieldValue> {
    match value {
        FieldValueIr::Single(value) => Ok(FieldValue::Scalar(lower_scalar(value)?)),
        FieldValueIr::Position { latitude, longitude } => Ok(FieldValue::Position(
            PositionValue::new(*latitude, *longitude),
        )),
        FieldValueIr::Predicate { features } => Ok(FieldValue::Predicate(PredicateValue::new(
            features.clone(),
        ))),
        FieldValueIr::PredicateExpression { expression } => Ok(FieldValue::Predicate(
            PredicateValue::from_expression(expression.clone()),
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

fn lower_scalar(value: &ScalarValueIr) -> Result<ScalarValue> {
    Ok(match value {
        ScalarValueIr::Bool(value) => ScalarValue::Boolean(*value),
        ScalarValueIr::Int64(value) => ScalarValue::Integer(*value),
        ScalarValueIr::Float64(value) => ScalarValue::Float(*value),
        ScalarValueIr::String(value) => ScalarValue::String(value.clone()),
    })
}

fn value_to_ir(value: &FieldValue) -> Value {
    match value {
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
        FieldValue::Predicate(value) => Value::Object(
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
        ),
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
    }
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
            PortableError::new(searchkernel_ir::ErrorCode::Internal, message)
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
