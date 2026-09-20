//! Typed-query annotation parsing.

use searchkernel_ir::{PortableError, RequestValueIr, Result};

pub(super) fn reject_unknown_annotations(
    annotations: &std::collections::BTreeMap<String, RequestValueIr>,
    supported: &[&str],
    path: &str,
) -> Result<()> {
    if let Some(name) = annotations
        .keys()
        .find(|name| !supported.iter().any(|candidate| candidate == name))
    {
        return Err(PortableError::unsupported(
            "typed_query_annotations_not_supported",
            Some(format!("{path}.annotations.{name}")),
            "this typed query node has no native lowering for the annotation",
        ));
    }
    Ok(())
}

pub(super) fn annotation_u32(
    annotations: &std::collections::BTreeMap<String, RequestValueIr>,
    name: &str,
    path: &str,
) -> Result<Option<u32>> {
    let Some(value) = annotations.get(name) else {
        return Ok(None);
    };
    let value = match value {
        RequestValueIr::UInt64(value) => u32::try_from(*value).ok(),
        RequestValueIr::Int64(value) if *value >= 0 => u32::try_from(*value).ok(),
        _ => None,
    };
    value.filter(|value| *value > 0).map(Some).ok_or_else(|| {
        PortableError::invalid(
            "invalid_typed_query_annotation",
            Some(format!("{path}.annotations.{name}")),
            format!("{name} must be a positive integer"),
        )
    })
}

pub(super) fn annotation_nonnegative_u32(
    annotations: &std::collections::BTreeMap<String, RequestValueIr>,
    name: &str,
    path: &str,
) -> Result<Option<u32>> {
    let Some(value) = annotations.get(name) else {
        return Ok(None);
    };
    let value = match value {
        RequestValueIr::UInt64(value) => u32::try_from(*value).ok(),
        RequestValueIr::Int64(value) if *value >= 0 => u32::try_from(*value).ok(),
        _ => None,
    };
    value.map(Some).ok_or_else(|| {
        PortableError::invalid(
            "invalid_typed_query_annotation",
            Some(format!("{path}.annotations.{name}")),
            format!("{name} must be a non-negative integer"),
        )
    })
}

pub(super) fn annotation_i64(
    annotations: &std::collections::BTreeMap<String, RequestValueIr>,
    name: &str,
    path: &str,
) -> Result<Option<i64>> {
    let Some(value) = annotations.get(name) else {
        return Ok(None);
    };
    let value = match value {
        RequestValueIr::Int64(value) => Some(*value),
        RequestValueIr::UInt64(value) => i64::try_from(*value).ok(),
        _ => None,
    };
    value.map(Some).ok_or_else(|| {
        PortableError::invalid(
            "invalid_typed_query_annotation",
            Some(format!("{path}.annotations.{name}")),
            format!("{name} must be an integer"),
        )
    })
}

pub(super) fn annotation_f64(
    annotations: &std::collections::BTreeMap<String, RequestValueIr>,
    name: &str,
    path: &str,
) -> Result<Option<f64>> {
    let Some(value) = annotations.get(name) else {
        return Ok(None);
    };
    let value = match value {
        RequestValueIr::Float64(value) if value.is_finite() => Some(*value),
        RequestValueIr::Int64(value) => Some(*value as f64),
        RequestValueIr::UInt64(value) => Some(*value as f64),
        _ => None,
    };
    value.map(Some).ok_or_else(|| {
        PortableError::invalid(
            "invalid_typed_query_annotation",
            Some(format!("{path}.annotations.{name}")),
            format!("{name} must be a finite number"),
        )
    })
}

pub(super) fn annotation_bool(
    annotations: &std::collections::BTreeMap<String, RequestValueIr>,
    name: &str,
    path: &str,
) -> Result<Option<bool>> {
    let Some(value) = annotations.get(name) else {
        return Ok(None);
    };
    match value {
        RequestValueIr::Bool(value) => Ok(Some(*value)),
        _ => Err(PortableError::invalid(
            "invalid_typed_query_annotation",
            Some(format!("{path}.annotations.{name}")),
            format!("{name} must be a boolean"),
        )),
    }
}

pub(super) fn annotation_u32_list(
    annotations: &std::collections::BTreeMap<String, RequestValueIr>,
    name: &str,
    path: &str,
) -> Result<Vec<u32>> {
    let Some(RequestValueIr::List(values)) = annotations.get(name) else {
        if annotations.contains_key(name) {
            return Err(PortableError::invalid(
                "invalid_typed_query_annotation",
                Some(format!("{path}.annotations.{name}")),
                format!("{name} must be a list of non-negative integers"),
            ));
        }
        return Ok(Vec::new());
    };
    values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let value = match value {
                RequestValueIr::UInt64(value) => *value,
                RequestValueIr::Int64(value) if *value >= 0 => *value as u64,
                _ => {
                    return Err(PortableError::invalid(
                        "invalid_typed_query_annotation",
                        Some(format!("{path}.annotations.{name}[{index}]")),
                        format!("{name} must contain only non-negative integers"),
                    ));
                }
            };
            u32::try_from(value).map_err(|_| {
                PortableError::invalid(
                    "invalid_typed_query_annotation",
                    Some(format!("{path}.annotations.{name}[{index}]")),
                    format!("{name} values must fit in an unsigned 32-bit integer"),
                )
            })
        })
        .collect()
}
