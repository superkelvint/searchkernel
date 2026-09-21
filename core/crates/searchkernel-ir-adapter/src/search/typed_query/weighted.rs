//! Weighted query lowering.

use super::super::filter::integer_query_value;
use super::annotations::{annotation_f64, annotation_i64, annotation_u32};
use super::DEFAULT_WEAK_AND_TARGET_HITS;
use searchkernel_ir::{PortableError, RequestValueIr, Result};
use searchkernel_vespa_engine::{WandOptions, WeightedQueryItem};

pub(super) fn lower_wand_options(
    annotations: &std::collections::BTreeMap<String, RequestValueIr>,
    path: &str,
) -> Result<WandOptions> {
    let target_num_hits = match (
        annotation_u32(annotations, "targetNumHits", path)?,
        annotation_u32(annotations, "totalTargetHits", path)?,
    ) {
        (Some(_), Some(_)) => {
            return Err(PortableError::invalid(
                "invalid_typed_query_annotation",
                Some(path),
                "WAND may specify only one of targetNumHits and totalTargetHits",
            ));
        }
        (Some(value), None) | (None, Some(value)) => value,
        (None, None) => DEFAULT_WEAK_AND_TARGET_HITS,
    };
    Ok(WandOptions {
        target_num_hits,
        score_threshold: annotation_i64(annotations, "scoreThreshold", path)?.unwrap_or(0),
        threshold_boost_factor: annotation_f64(annotations, "thresholdBoostFactor", path)?
            .unwrap_or(1.0),
    })
}

pub(super) fn lower_weighted_items(
    items: &[searchkernel_ir::WeightedQueryItemIr],
    path: &str,
) -> Result<Vec<WeightedQueryItem>> {
    if items.is_empty() {
        return Err(PortableError::invalid(
            "empty_typed_query",
            Some(path),
            "weighted query requires at least one item",
        ));
    }
    let mut lowered = Vec::with_capacity(items.len());
    let mut string_keys = None;
    for (index, item) in items.iter().enumerate() {
        let item_path = format!("{path}[{index}]");
        let (is_string, lowered_item) = match &item.key {
            RequestValueIr::String(key) if !key.is_empty() => (
                true,
                WeightedQueryItem::String {
                    key: key.clone(),
                    weight: item.weight,
                },
            ),
            RequestValueIr::Int64(key) => (
                false,
                WeightedQueryItem::Integer {
                    key: integer_query_value(*key)?,
                    weight: item.weight,
                },
            ),
            RequestValueIr::UInt64(key) => {
                let key = i64::try_from(*key).map_err(|_| {
                    PortableError::unsupported(
                        "typed_query_weighted_key_not_supported",
                        Some(format!("{item_path}.key")),
                        "unsigned weighted query keys above INT64_MAX are not representable by Vespa",
                    )
                })?;
                (
                    false,
                    WeightedQueryItem::Integer {
                        key: integer_query_value(key)?,
                        weight: item.weight,
                    },
                )
            }
            RequestValueIr::String(_) => {
                return Err(PortableError::invalid(
                    "invalid_typed_query",
                    Some(format!("{item_path}.key")),
                    "weighted query string keys must not be empty",
                ));
            }
            _ => {
                return Err(PortableError::unsupported(
                    "typed_query_weighted_key_not_supported",
                    Some(format!("{item_path}.key")),
                    "weighted query keys must be strings or signed integer values",
                ));
            }
        };
        if string_keys.is_some_and(|previous| previous != is_string) {
            return Err(PortableError::unsupported(
                "typed_query_weighted_key_not_supported",
                Some(format!("{item_path}.key")),
                "weighted query cannot mix string and integer keys",
            ));
        }
        string_keys = Some(is_string);
        lowered.push(lowered_item);
    }
    Ok(lowered)
}
