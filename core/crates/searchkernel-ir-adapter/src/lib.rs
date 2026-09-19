//! Integration boundary between the portable Search Kernel IR and the
//! low-level Vespa engine wrapper.
//!
//! This crate is the only translation boundary between portable IR and the
//! Vespa-facing engine wrapper. It contains no fallback or in-memory engine.

mod backend;
mod schema;
mod search;

use searchkernel_ir::{DocumentIr, FieldValueIr, PortableError, Result, ScalarValueIr, Value};
use searchkernel_vespa_engine::{Document, FieldValue, ScalarValue};
use std::collections::BTreeMap;

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) use backend::MutationProbe;
pub use backend::VespaBackend;
pub use schema::{compile_index_definition, validate_index_definition};
#[cfg(test)]
pub(crate) use search::lower_query;
#[cfg(test)]
pub(crate) use search::{
    lower_comparison, lower_facet_groupings, lower_facets_into_request, lower_filter,
    lower_search_options, lower_search_query, ranking_profile, validate_search_request,
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
    }
}

fn native_error(error: searchkernel_vespa_engine::Error) -> PortableError {
    let message = error.to_string();
    match error.kind() {
        searchkernel_vespa_engine::ErrorKind::InvalidArgument => {
            PortableError::invalid("backend_request", None::<String>, message)
        }
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
        let path = match detail_code {
            "facet_key_type_not_supported" => Some("grouping"),
            _ => Some("sort"),
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
