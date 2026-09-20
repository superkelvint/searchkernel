//! Predicate query lowering.

use super::typed_query_field;
use searchkernel_ir::{PortableError, QueryFieldRefIr, RequestValueIr, Result};
use searchkernel_vespa_engine::{PredicateFeature, PredicateRangeFeature, Query};

const ALL_PREDICATE_SUB_QUERIES: u64 = u64::MAX;

pub(super) fn lower_predicate_features(
    values: &std::collections::BTreeMap<String, RequestValueIr>,
    path: &str,
) -> Result<Vec<PredicateFeature>> {
    let mut lowered = Vec::new();
    for (key, value) in values {
        lower_predicate_feature_value(key, value, ALL_PREDICATE_SUB_QUERIES, &mut lowered, path)?;
    }
    Ok(lowered)
}

pub(super) fn lower_predicate_feature_value(
    key: &str,
    value: &RequestValueIr,
    mask: u64,
    output: &mut Vec<PredicateFeature>,
    path: &str,
) -> Result<()> {
    match value {
        RequestValueIr::String(value) if !key.is_empty() && !value.is_empty() => {
            output.push(PredicateFeature {
                key: key.to_owned(),
                value: value.clone(),
                sub_query_bitmask: mask,
            });
            Ok(())
        }
        RequestValueIr::List(values) => {
            for (index, value) in values.iter().enumerate() {
                lower_predicate_feature_value(
                    key,
                    value,
                    mask,
                    output,
                    &format!("{path}.{key}[{index}]"),
                )?;
            }
            Ok(())
        }
        RequestValueIr::Object(values) => {
            let bitmap = parse_predicate_bitmap(key, path)?;
            for (nested_key, nested_value) in values {
                lower_predicate_feature_value(nested_key, nested_value, bitmap, output, path)?;
            }
            Ok(())
        }
        RequestValueIr::String(_) => Err(PortableError::invalid(
            "invalid_predicate_value",
            Some(format!("{path}.{key}")),
            "predicate feature keys and values must not be empty",
        )),
        _ => Err(PortableError::unsupported(
            "predicate_feature_value_not_supported",
            Some(format!("{path}.{key}")),
            "predicate features support non-empty strings, string lists, or bitmap-keyed objects",
        )),
    }
}

pub(super) fn lower_predicate_range_features(
    values: &std::collections::BTreeMap<String, RequestValueIr>,
    path: &str,
) -> Result<Vec<PredicateRangeFeature>> {
    let mut lowered = Vec::new();
    for (key, value) in values {
        lower_predicate_range_value(key, value, ALL_PREDICATE_SUB_QUERIES, &mut lowered, path)?;
    }
    Ok(lowered)
}

pub(super) fn lower_predicate_range_value(
    key: &str,
    value: &RequestValueIr,
    mask: u64,
    output: &mut Vec<PredicateRangeFeature>,
    path: &str,
) -> Result<()> {
    match value {
        RequestValueIr::UInt64(value) => {
            if key.is_empty() {
                return Err(PortableError::invalid(
                    "invalid_predicate_value",
                    Some(format!("{path}.{key}")),
                    "predicate range feature keys must not be empty",
                ));
            }
            output.push(PredicateRangeFeature {
                key: key.to_owned(),
                value: *value,
                sub_query_bitmask: mask,
            });
            Ok(())
        }
        RequestValueIr::Int64(value) if *value >= 0 => {
            if key.is_empty() {
                return Err(PortableError::invalid(
                    "invalid_predicate_value",
                    Some(format!("{path}.{key}")),
                    "predicate range feature keys must not be empty",
                ));
            }
            output.push(PredicateRangeFeature {
                key: key.to_owned(),
                value: *value as u64,
                sub_query_bitmask: mask,
            });
            Ok(())
        }
        RequestValueIr::List(values) => {
            for (index, value) in values.iter().enumerate() {
                lower_predicate_range_value(
                    key,
                    value,
                    mask,
                    output,
                    &format!("{path}.{key}[{index}]"),
                )?;
            }
            Ok(())
        }
        RequestValueIr::Object(values) => {
            let bitmap = parse_predicate_bitmap(key, path)?;
            for (nested_key, nested_value) in values {
                lower_predicate_range_value(nested_key, nested_value, bitmap, output, path)?;
            }
            Ok(())
        }
        RequestValueIr::Int64(_) => Err(PortableError::unsupported(
            "predicate_range_value_not_supported",
            Some(format!("{path}.{key}")),
            "predicate range features require non-negative integer values",
        )),
        _ => Err(PortableError::unsupported(
            "predicate_range_value_not_supported",
            Some(format!("{path}.{key}")),
            "predicate range features support unsigned or non-negative integer values, integer lists, or bitmap-keyed objects",
        )),
    }
}

pub(super) fn parse_predicate_bitmap(value: &str, path: &str) -> Result<u64> {
    let (digits, radix) = value
        .strip_prefix("0x")
        .map_or((value, 10), |value| (value, 16));
    u64::from_str_radix(digits, radix).map_err(|_| {
        PortableError::invalid(
            "invalid_predicate_sub_query_bitmap",
            Some(format!("{path}.{value}")),
            "predicate bitmap object keys must be decimal or 0x-prefixed hexadecimal uint64 values",
        )
    })
}

pub(super) fn lower_predicate(
    field: &QueryFieldRefIr,
    features: &std::collections::BTreeMap<String, RequestValueIr>,
    range_features: &std::collections::BTreeMap<String, RequestValueIr>,
    path: &str,
) -> Result<Query> {
    let field = typed_query_field(Some(field), &format!("{path}.predicate.field"))?;
    let features = lower_predicate_features(features, &format!("{path}.predicate.features"))?;
    let range_features = lower_predicate_range_features(
        range_features,
        &format!("{path}.predicate.range_features"),
    )?;
    if features.is_empty() && range_features.is_empty() {
        return Err(PortableError::invalid(
            "empty_typed_query",
            Some(path),
            "predicate query requires at least one feature",
        ));
    }
    Ok(Query::Predicate {
        field: field.to_owned(),
        features,
        range_features,
    })
}
