//! Lower typed query expressions into the Vespa query model.

use super::filter::{integer_query_value, lower_attribute_value};
use crate::unsupported;
use searchkernel_ir::{
    PortableError, QueryExprIr, QueryExprKindIr, QueryFieldRefIr, QueryIr, QueryRangeBoundIr,
    RequestValueIr, Result, ScalarValueIr,
};
use searchkernel_vespa_engine::{
    InQueryValue, NearestNeighborOptions, PredicateFeature, PredicateRangeFeature, Query,
    QueryAlternative, Value as VespaValue, WandOptions, WeightedQueryItem,
};

const DEFAULT_WEAK_AND_TARGET_HITS: u32 = 100;

fn lower_wand_options(
    annotations: &std::collections::BTreeMap<String, RequestValueIr>,
    path: &str,
) -> Result<WandOptions> {
    reject_unknown_annotations(
        annotations,
        &[
            "targetNumHits",
            "totalTargetHits",
            "scoreThreshold",
            "thresholdBoostFactor",
        ],
        path,
    )?;
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

fn lower_nearest_neighbor_options(
    annotations: &std::collections::BTreeMap<String, RequestValueIr>,
    path: &str,
) -> Result<NearestNeighborOptions> {
    reject_unknown_annotations(
        annotations,
        &[
            "targetNumHits",
            "totalTargetHits",
            "approximate",
            "distanceThreshold",
            "exploreAdditionalHits",
        ],
        path,
    )?;
    let target_num_hits = match (
        annotation_u32(annotations, "targetNumHits", path)?,
        annotation_u32(annotations, "totalTargetHits", path)?,
    ) {
        (Some(_), Some(_)) => {
            return Err(PortableError::invalid(
                "invalid_typed_query_annotation",
                Some(path),
                "nearest-neighbor may specify only one of targetNumHits and totalTargetHits",
            ));
        }
        (Some(value), None) | (None, Some(value)) => value,
        (None, None) => DEFAULT_WEAK_AND_TARGET_HITS,
    };
    Ok(NearestNeighborOptions {
        target_num_hits,
        approximate: annotation_bool(annotations, "approximate", path)?.unwrap_or(true),
        distance_threshold: annotation_f64(annotations, "distanceThreshold", path)?,
        explore_additional_hits: annotation_u32(annotations, "exploreAdditionalHits", path)?
            .unwrap_or(0),
    })
}

fn reject_unknown_annotations(
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

fn annotation_u32(
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

fn annotation_i64(
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

fn annotation_f64(
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

fn annotation_bool(
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

#[cfg(test)]
pub(crate) fn lower_query(
    query: &QueryIr,
    fields: &[searchkernel_ir::SearchFieldIr],
) -> Result<Query> {
    lower_query_with_language(
        query,
        fields,
        searchkernel_vespa_engine::LinguisticsOptions::default(),
    )
}

#[cfg(test)]
pub(crate) fn lower_query_with_language(
    query: &QueryIr,
    fields: &[searchkernel_ir::SearchFieldIr],
    language: searchkernel_vespa_engine::LinguisticsOptions,
) -> Result<Query> {
    lower_query_with_context(
        query,
        fields,
        language,
        None,
        None,
        &std::collections::BTreeMap::new(),
    )
}

pub(crate) fn lower_query_with_context(
    query: &QueryIr,
    fields: &[searchkernel_ir::SearchFieldIr],
    language: searchkernel_vespa_engine::LinguisticsOptions,
    default_index: Option<&str>,
    query_string: Option<&str>,
    rank_properties: &std::collections::BTreeMap<String, String>,
) -> Result<Query> {
    match query {
        QueryIr::MatchAll => Ok(Query::True),
        QueryIr::Text(text) => {
            let clauses = fields
                .iter()
                .map(|field| Query::Text {
                    field: field.field().to_string(),
                    text: text.clone(),
                    target_num_hits: DEFAULT_WEAK_AND_TARGET_HITS,
                })
                .collect::<Vec<_>>();
            match clauses.as_slice() {
                [] => Err(unsupported("text query has no searchable fields")),
                [query] => Ok(query.clone()),
                _ => Ok(Query::Or(clauses)),
            }
        }
        QueryIr::Term { field, value } => match value {
            ScalarValueIr::String(value) => Ok(Query::term(field.as_str(), value)),
            ScalarValueIr::Int64(value) => {
                Ok(Query::equals(field.as_str(), integer_query_value(*value)?))
            }
            _ => Err(unsupported("unsupported typed query value")),
        },
        QueryIr::And(children) => Ok(Query::And(
            children
                .iter()
                .map(|child| {
                    lower_query_with_context(
                        child,
                        fields,
                        language,
                        default_index,
                        query_string,
                        rank_properties,
                    )
                })
                .collect::<Result<Vec<_>>>()?,
        )),
        QueryIr::Or(children) => Ok(Query::Or(
            children
                .iter()
                .map(|child| {
                    lower_query_with_context(
                        child,
                        fields,
                        language,
                        default_index,
                        query_string,
                        rank_properties,
                    )
                })
                .collect::<Result<Vec<_>>>()?,
        )),
        QueryIr::Not(child) => Ok(Query::Not(Box::new(lower_query_with_context(
            child,
            fields,
            language,
            default_index,
            query_string,
            rank_properties,
        )?))),
        QueryIr::Yql { .. } => Err(PortableError::unsupported(
            "yql_not_supported",
            Some("query"),
            "the current native adapter does not expose a YQL execution path",
        )),
        QueryIr::Typed(expression) => lower_typed_query(
            expression,
            None,
            language,
            default_index,
            query_string,
            rank_properties,
            "query",
        ),
    }
}

fn lower_typed_query(
    expression: &QueryExprIr,
    field_context: Option<&QueryFieldRefIr>,
    language: searchkernel_vespa_engine::LinguisticsOptions,
    default_index: Option<&str>,
    query_string: Option<&str>,
    rank_properties: &std::collections::BTreeMap<String, String>,
    path: &str,
) -> Result<Query> {
    if !expression.annotations.is_empty()
        && !matches!(
            expression.kind,
            QueryExprKindIr::Wand { .. } | QueryExprKindIr::NearestNeighbor { .. }
        )
    {
        return Err(PortableError::unsupported(
            "typed_query_annotations_not_supported",
            Some(format!("{path}.annotations")),
            "the IR contains annotations without a resolved adapter lowering",
        ));
    }

    match &expression.kind {
        QueryExprKindIr::Term { value } => {
            let field = typed_query_field(field_context, path)?;
            Ok(Query::term(field, typed_term_text(value, path)?))
        }
        QueryExprKindIr::Contains { field, item } => {
            typed_query_field(Some(field), &format!("{path}.contains.field"))?;
            lower_typed_query(
                item,
                Some(field),
                language,
                default_index,
                query_string,
                rank_properties,
                &format!("{path}.contains.item"),
            )
        }
        QueryExprKindIr::And(children) => Ok(Query::And(
            children
                .iter()
                .enumerate()
                .map(|(index, child)| {
                    lower_typed_query(
                        child,
                        field_context,
                        language,
                        default_index,
                        query_string,
                        rank_properties,
                        &format!("{path}.and[{index}]"),
                    )
                })
                .collect::<Result<Vec<_>>>()?,
        )),
        QueryExprKindIr::Or(children) => Ok(Query::Or(
            children
                .iter()
                .enumerate()
                .map(|(index, child)| {
                    lower_typed_query(
                        child,
                        field_context,
                        language,
                        default_index,
                        query_string,
                        rank_properties,
                        &format!("{path}.or[{index}]"),
                    )
                })
                .collect::<Result<Vec<_>>>()?,
        )),
        QueryExprKindIr::Not(child) => Ok(Query::Not(Box::new(lower_typed_query(
            child,
            field_context,
            language,
            default_index,
            query_string,
            rank_properties,
            &format!("{path}.not.child"),
        )?))),
        QueryExprKindIr::True => Ok(Query::True),
        // The native ABI has no standalone false node. NOT(TRUE) is the
        // existing native boolean representation of a query matching nothing.
        QueryExprKindIr::False => Ok(Query::Not(Box::new(Query::True))),
        QueryExprKindIr::Comparison {
            field, op, value, ..
        } => lower_typed_comparison(field, *op, value, path),
        QueryExprKindIr::Range {
            field,
            lower,
            upper,
            include_lower,
            include_upper,
        } => lower_typed_range(
            field,
            lower,
            upper,
            include_lower.unwrap_or(true),
            include_upper.unwrap_or(true),
            path,
        ),
        QueryExprKindIr::In { field, values } => {
            if values.is_empty() {
                return Err(PortableError::invalid(
                    "empty_typed_query",
                    Some(format!("{path}.in.values")),
                    "in query requires at least one value",
                ));
            }
            let field = typed_query_field(Some(field), &format!("{path}.in.field"))?;
            let values = values
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    let value = lower_attribute_value(&typed_scalar(
                        value,
                        &format!("{path}.in.values[{index}]"),
                    )?)?;
                    match value {
                        VespaValue::String(value) => Ok(InQueryValue::String(value)),
                        VespaValue::Integer(value) => Ok(InQueryValue::Integer(value)),
                        _ => Err(unsupported(
                            "in query supports only string and integer values",
                        )),
                    }
                })
                .collect::<Result<Vec<_>>>()?;
            Ok(Query::In {
                field: field.to_owned(),
                values,
            })
        }
        QueryExprKindIr::Phrase(children) => {
            lower_typed_phrase(children, field_context, language, path)
        }
        QueryExprKindIr::Text { text } => {
            let field = typed_query_field(field_context, path)?;
            Ok(Query::Text {
                field: field.to_owned(),
                text: text.clone(),
                target_num_hits: DEFAULT_WEAK_AND_TARGET_HITS,
            })
        }
        QueryExprKindIr::Near { children, distance }
        | QueryExprKindIr::ONear { children, distance } => {
            let field = typed_query_field(field_context, path)?;
            let children = lower_typed_children(
                children,
                field_context,
                language,
                default_index,
                query_string,
                rank_properties,
                path,
                if matches!(&expression.kind, QueryExprKindIr::Near { .. }) {
                    "near"
                } else {
                    "onear"
                },
            )?;
            if children.len() < 2 {
                return Err(PortableError::invalid(
                    "empty_typed_query",
                    Some(path),
                    "proximity query requires at least two children",
                ));
            }
            let query = if matches!(&expression.kind, QueryExprKindIr::Near { .. }) {
                Query::Near {
                    field: field.to_owned(),
                    children,
                    distance: *distance,
                }
            } else {
                Query::ONear {
                    field: field.to_owned(),
                    children,
                    distance: *distance,
                }
            };
            Ok(query)
        }
        QueryExprKindIr::Equiv(children) => {
            typed_query_field(field_context, path)?;
            let children = lower_typed_children(
                children,
                field_context,
                language,
                default_index,
                query_string,
                rank_properties,
                path,
                "equiv",
            )?;
            if children.len() < 2 {
                return Err(PortableError::invalid(
                    "invalid_typed_query",
                    Some(path),
                    "equiv query requires at least two children",
                ));
            }
            Ok(Query::Equiv(children))
        }
        QueryExprKindIr::Fuzzy {
            value,
            max_edit_distance,
            prefix_length,
            prefix,
        } => {
            let field = typed_query_field(field_context, path)?;
            Ok(Query::Fuzzy {
                field: field.to_owned(),
                text: typed_term_text(value, &format!("{path}.fuzzy.value"))?,
                max_edit_distance: *max_edit_distance,
                prefix_lock_length: *prefix_length,
                prefix_match: *prefix,
            })
        }
        QueryExprKindIr::WeakAnd {
            children,
            target_hits,
        } => Ok(Query::WeakAnd {
            children: lower_typed_children(
                children,
                field_context,
                language,
                default_index,
                query_string,
                rank_properties,
                path,
                "weak_and",
            )?,
            target_num_hits: *target_hits,
        }),
        QueryExprKindIr::Rank { matching, ranking } => Ok(Query::Rank {
            matching: Box::new(lower_typed_query(
                matching,
                field_context,
                language,
                default_index,
                query_string,
                rank_properties,
                &format!("{path}.rank.match"),
            )?),
            ranking: lower_typed_children(
                ranking,
                field_context,
                language,
                default_index,
                query_string,
                rank_properties,
                path,
                "rank",
            )?,
        }),
        QueryExprKindIr::WeightedSet { field, items } => Ok(Query::WeightedSet {
            field: typed_query_field(Some(field), &format!("{path}.weighted_set.field"))?
                .to_owned(),
            items: lower_weighted_items(items, &format!("{path}.weighted_set.items"))?,
        }),
        QueryExprKindIr::Wand { field, items } => {
            let field = typed_query_field(Some(field), &format!("{path}.wand.field"))?.to_owned();
            let items = lower_weighted_items(items, &format!("{path}.wand.items"))?;
            let options = lower_wand_options(&expression.annotations, path)?;
            if options == WandOptions::default() {
                Ok(Query::Wand { field, items })
            } else {
                Ok(Query::WandWithOptions {
                    field,
                    items,
                    options,
                })
            }
        }
        QueryExprKindIr::Alternatives(values) => {
            let field = typed_query_field(field_context, path)?;
            let alternatives = values
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    let QueryExprKindIr::Term { value: term_value } = &value.query.kind else {
                        return Err(PortableError::unsupported(
                            "typed_query_alternative_not_supported",
                            Some(format!("{path}.alternatives[{index}].query")),
                            "alternatives currently require term children",
                        ));
                    };
                    if !value.query.annotations.is_empty() {
                        return Err(PortableError::unsupported(
                            "typed_query_annotations_not_supported",
                            Some(format!("{path}.alternatives[{index}].query.annotations")),
                            "alternative term annotations are not supported",
                        ));
                    }
                    Ok(QueryAlternative {
                        text: typed_term_text(
                            term_value,
                            &format!("{path}.alternatives[{index}].query"),
                        )?,
                        exactness: value.exactness.unwrap_or(1.0),
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            Ok(Query::Alternatives {
                field: field.to_owned(),
                alternatives,
            })
        }
        QueryExprKindIr::Matches { field, pattern } => Ok(Query::Matches {
            field: typed_query_field(Some(field), &format!("{path}.matches.field"))?.to_owned(),
            pattern: pattern.clone(),
        }),
        QueryExprKindIr::DotProduct { field, items } => Ok(Query::DotProduct {
            field: typed_query_field(Some(field), &format!("{path}.dot_product.field"))?.to_owned(),
            items: lower_weighted_items(items, &format!("{path}.dot_product.items"))?,
        }),
        QueryExprKindIr::Labeled {
            query,
            label,
            score,
        } => {
            let unique_id = label_unique_id(label, rank_properties, path)?;
            Ok(Query::Labeled {
                query: Box::new(lower_typed_query(
                    query,
                    field_context,
                    language,
                    default_index,
                    query_string,
                    rank_properties,
                    &format!("{path}.labeled.query"),
                )?),
                unique_id,
                score: *score,
            })
        }
        QueryExprKindIr::NonEmpty(_) => Err(PortableError::unsupported(
            "non_empty_not_supported",
            Some(path),
            "NonEmpty requires Vespa parser/null-marker semantics not established by this adapter",
        )),
        QueryExprKindIr::SameElement(children) => {
            let field = typed_query_field(field_context, path)?;
            let children = lower_typed_children(
                children,
                field_context,
                language,
                default_index,
                query_string,
                rank_properties,
                path,
                "same_element",
            )?;
            if children.len() < 2 {
                return Err(PortableError::invalid(
                    "invalid_typed_query",
                    Some(path),
                    "same-element query requires at least two children",
                ));
            }
            Ok(Query::SameElement {
                field: field.to_owned(),
                children,
            })
        }
        QueryExprKindIr::GeoLocation {
            field,
            latitude,
            longitude,
            radius,
        } => Ok(Query::GeoLocation {
            field: typed_query_field(Some(field), &format!("{path}.geo_location.field"))?
                .to_owned(),
            latitude: *latitude,
            longitude: *longitude,
            radius: radius.clone(),
        }),
        QueryExprKindIr::GeoBoundingBox {
            field,
            south,
            west,
            north,
            east,
        } => Ok(Query::GeoBoundingBox {
            field: typed_query_field(Some(field), &format!("{path}.geo_bounding_box.field"))?
                .to_owned(),
            south: *south,
            west: *west,
            north: *north,
            east: *east,
        }),
        QueryExprKindIr::NearestNeighbor {
            document_field,
            query_tensor,
        } => {
            let document_field = typed_query_field(
                Some(document_field),
                &format!("{path}.nearest_neighbor.document_field"),
            )?
            .to_owned();
            let options = lower_nearest_neighbor_options(&expression.annotations, path)?;
            if options == NearestNeighborOptions::default() {
                Ok(Query::NearestNeighbor {
                    document_field,
                    query_tensor: query_tensor.clone(),
                    target_num_hits: options.target_num_hits,
                    approximate: options.approximate,
                })
            } else {
                Ok(Query::NearestNeighborWithOptions {
                    document_field,
                    query_tensor: query_tensor.clone(),
                    options,
                })
            }
        }
        QueryExprKindIr::Predicate {
            field,
            features,
            range_features,
        } => {
            let field = typed_query_field(Some(field), &format!("{path}.predicate.field"))?;
            let features =
                lower_predicate_features(features, &format!("{path}.predicate.features"))?;
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
        QueryExprKindIr::UserInput { .. } | QueryExprKindIr::UserQuery { .. } => {
            Err(PortableError::unsupported(
                "user_query_parser_not_supported",
                Some(path),
                "userInput/userQuery parsing is not approximated by a WeakAnd lowering",
            ))
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn lower_typed_children(
    children: &[QueryExprIr],
    field_context: Option<&QueryFieldRefIr>,
    language: searchkernel_vespa_engine::LinguisticsOptions,
    default_index: Option<&str>,
    query_string: Option<&str>,
    rank_properties: &std::collections::BTreeMap<String, String>,
    path: &str,
    operator: &str,
) -> Result<Vec<Query>> {
    children
        .iter()
        .enumerate()
        .map(|(index, child)| {
            lower_typed_query(
                child,
                field_context,
                language,
                default_index,
                query_string,
                rank_properties,
                &format!("{path}.{operator}[{index}]"),
            )
        })
        .collect()
}

fn label_unique_id(
    label: &str,
    rank_properties: &std::collections::BTreeMap<String, String>,
    path: &str,
) -> Result<i32> {
    if label.is_empty() {
        return Err(PortableError::invalid(
            "invalid_typed_query",
            Some(format!("{path}.labeled.label")),
            "labeled query label must not be empty",
        ));
    }
    let property = format!("vespa.label.{label}.id");
    let Some(value) = rank_properties.get(&property) else {
        return Err(PortableError::invalid(
            "labeled_query_label_id_missing",
            Some(format!("{path}.labeled.label")),
            format!("rank property {property} is required for labeled queries"),
        ));
    };
    let value = value.parse::<i32>().map_err(|_| {
        PortableError::invalid(
            "labeled_query_label_id_invalid",
            Some(format!("{path}.labeled.label")),
            format!("rank property {property} must be a signed 32-bit integer"),
        )
    })?;
    if value < 0 {
        return Err(PortableError::invalid(
            "labeled_query_label_id_invalid",
            Some(format!("{path}.labeled.label")),
            format!("rank property {property} must not be negative"),
        ));
    }
    Ok(value)
}

fn lower_weighted_items(
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

fn typed_query_field<'a>(field: Option<&'a QueryFieldRefIr>, path: &str) -> Result<&'a str> {
    let Some(field) = field else {
        return Err(PortableError::invalid(
            "missing_field_context",
            Some(path),
            "term-like typed query requires a containing field",
        ));
    };
    if field.element_index.is_some() {
        return Err(PortableError::unsupported(
            "typed_query_field_element_not_supported",
            Some(path),
            "collection element query paths are outside the Typed Query A execution subset",
        ));
    }
    Ok(field.path.as_str())
}

const ALL_PREDICATE_SUB_QUERIES: u64 = u64::MAX;

fn lower_predicate_features(
    values: &std::collections::BTreeMap<String, RequestValueIr>,
    path: &str,
) -> Result<Vec<PredicateFeature>> {
    let mut lowered = Vec::new();
    for (key, value) in values {
        lower_predicate_feature_value(key, value, ALL_PREDICATE_SUB_QUERIES, &mut lowered, path)?;
    }
    Ok(lowered)
}

fn lower_predicate_feature_value(
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

fn lower_predicate_range_features(
    values: &std::collections::BTreeMap<String, RequestValueIr>,
    path: &str,
) -> Result<Vec<PredicateRangeFeature>> {
    let mut lowered = Vec::new();
    for (key, value) in values {
        lower_predicate_range_value(key, value, ALL_PREDICATE_SUB_QUERIES, &mut lowered, path)?;
    }
    Ok(lowered)
}

fn lower_predicate_range_value(
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

fn parse_predicate_bitmap(value: &str, path: &str) -> Result<u64> {
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

fn typed_term_text(value: &RequestValueIr, path: &str) -> Result<String> {
    match value {
        RequestValueIr::String(value) if !value.is_empty() => Ok(value.clone()),
        _ => Err(PortableError::unsupported(
            "typed_query_value_not_supported",
            Some(format!("{path}.term.value")),
            "typed term values must be non-empty strings",
        )),
    }
}

fn typed_scalar(value: &RequestValueIr, path: &str) -> Result<ScalarValueIr> {
    match value {
        RequestValueIr::Bool(value) => Ok(ScalarValueIr::Bool(*value)),
        RequestValueIr::Int64(value) => Ok(ScalarValueIr::Int64(*value)),
        RequestValueIr::Float64(value) if value.is_finite() => Ok(ScalarValueIr::Float64(*value)),
        RequestValueIr::String(value) => Ok(ScalarValueIr::String(value.clone())),
        RequestValueIr::UInt64(value) => i64::try_from(*value)
            .map(ScalarValueIr::Int64)
            .map_err(|_| {
                PortableError::unsupported(
                    "typed_query_value_not_supported",
                    Some(path),
                    "unsigned query values above INT64_MAX are not representable by the native query model",
                )
            }),
        RequestValueIr::Float64(_) | RequestValueIr::NonFiniteFloat(_) => {
            Err(PortableError::invalid(
                "invalid_query_value",
                Some(path),
                "typed query numeric values must be finite",
            ))
        }
        _ => Err(PortableError::unsupported(
            "typed_query_value_not_supported",
            Some(path),
            "this typed query value is not representable by the native query model",
        )),
    }
}

fn lower_typed_comparison(
    field: &QueryFieldRefIr,
    op: searchkernel_ir::ComparisonOp,
    value: &RequestValueIr,
    path: &str,
) -> Result<Query> {
    let field = typed_query_field(Some(field), &format!("{path}.comparison.field"))?;
    let value = lower_attribute_value(&typed_scalar(value, &format!("{path}.comparison.value"))?)?;
    match op {
        searchkernel_ir::ComparisonOp::Eq => Ok(Query::AttributeEquals {
            field: field.to_owned(),
            value,
        }),
        searchkernel_ir::ComparisonOp::Ne => Ok(Query::Not(Box::new(Query::AttributeEquals {
            field: field.to_owned(),
            value,
        }))),
        searchkernel_ir::ComparisonOp::Lt
        | searchkernel_ir::ComparisonOp::Lte
        | searchkernel_ir::ComparisonOp::Gt
        | searchkernel_ir::ComparisonOp::Gte => {
            let is_lower = matches!(
                op,
                searchkernel_ir::ComparisonOp::Gt | searchkernel_ir::ComparisonOp::Gte
            );
            let included = matches!(
                op,
                searchkernel_ir::ComparisonOp::Lte | searchkernel_ir::ComparisonOp::Gte
            );
            if !matches!(value, VespaValue::Integer(_) | VespaValue::Float(_)) {
                return Err(PortableError::unsupported(
                    "typed_query_ordered_value_not_supported",
                    Some(format!("{path}.comparison.value")),
                    "ordered typed comparisons require numeric values",
                ));
            }
            let bound = if included {
                searchkernel_vespa_engine::AttributeBound::Included(value)
            } else {
                searchkernel_vespa_engine::AttributeBound::Excluded(value)
            };
            let (lower, upper) = if is_lower {
                (bound, searchkernel_vespa_engine::AttributeBound::Unbounded)
            } else {
                (searchkernel_vespa_engine::AttributeBound::Unbounded, bound)
            };
            Ok(Query::AttributeRange {
                field: field.to_owned(),
                lower,
                upper,
            })
        }
    }
}

fn lower_typed_range(
    field: &QueryFieldRefIr,
    lower: &QueryRangeBoundIr,
    upper: &QueryRangeBoundIr,
    include_lower: bool,
    include_upper: bool,
    path: &str,
) -> Result<Query> {
    let field = typed_query_field(Some(field), &format!("{path}.range.field"))?;
    let lower = typed_range_bound(lower, true, include_lower, &format!("{path}.range.lower"))?;
    let upper = typed_range_bound(upper, false, include_upper, &format!("{path}.range.upper"))?;
    Ok(Query::AttributeRange {
        field: field.to_owned(),
        lower,
        upper,
    })
}

fn typed_range_bound(
    bound: &QueryRangeBoundIr,
    lower: bool,
    included: bool,
    path: &str,
) -> Result<searchkernel_vespa_engine::AttributeBound> {
    match bound {
        QueryRangeBoundIr::Unbounded => Ok(searchkernel_vespa_engine::AttributeBound::Unbounded),
        QueryRangeBoundIr::NegativeInfinity if lower => {
            Ok(searchkernel_vespa_engine::AttributeBound::Unbounded)
        }
        QueryRangeBoundIr::PositiveInfinity if !lower => {
            Ok(searchkernel_vespa_engine::AttributeBound::Unbounded)
        }
        QueryRangeBoundIr::NegativeInfinity | QueryRangeBoundIr::PositiveInfinity => {
            Err(PortableError::unsupported(
                "typed_query_range_bound_not_supported",
                Some(path),
                "an infinity bound must be on the corresponding outer side of a range",
            ))
        }
        QueryRangeBoundIr::Finite(value) => {
            let value = lower_attribute_value(&typed_scalar(value, path)?)?;
            if !matches!(value, VespaValue::Integer(_) | VespaValue::Float(_)) {
                return Err(PortableError::unsupported(
                    "typed_query_range_value_not_supported",
                    Some(path),
                    "typed ranges currently require numeric values",
                ));
            }
            Ok(if included {
                searchkernel_vespa_engine::AttributeBound::Included(value)
            } else {
                searchkernel_vespa_engine::AttributeBound::Excluded(value)
            })
        }
    }
}

fn lower_typed_phrase(
    children: &[QueryExprIr],
    field_context: Option<&QueryFieldRefIr>,
    language: searchkernel_vespa_engine::LinguisticsOptions,
    path: &str,
) -> Result<Query> {
    let field = typed_query_field(field_context, path)?;
    let mut terms = Vec::new();
    for (index, child) in children.iter().enumerate() {
        collect_typed_phrase_terms(
            child,
            field_context,
            &mut terms,
            &format!("{path}.phrase[{index}]"),
        )?;
    }
    if terms.is_empty() {
        return Err(PortableError::invalid(
            "empty_typed_query",
            Some(format!("{path}.phrase")),
            "phrase query requires at least one term",
        ));
    }
    let text = terms.join(" ");
    let analyzed = searchkernel_vespa_engine::Linguistics
        .analyze_query(&text, language)
        .terms
        .len();
    if analyzed == 0 {
        return Err(PortableError::invalid(
            "empty_typed_query",
            Some(format!("{path}.phrase")),
            "phrase query must analyze to at least one term",
        ));
    }
    Ok(Query::phrase(field, text))
}

fn collect_typed_phrase_terms(
    expression: &QueryExprIr,
    field_context: Option<&QueryFieldRefIr>,
    terms: &mut Vec<String>,
    path: &str,
) -> Result<()> {
    if !expression.annotations.is_empty() {
        return Err(PortableError::unsupported(
            "typed_query_annotations_not_supported",
            Some(format!("{path}.annotations")),
            "typed query annotations are outside the Typed Query A execution subset",
        ));
    }
    match &expression.kind {
        QueryExprKindIr::Term { value } => terms.push(typed_term_text(value, path)?),
        QueryExprKindIr::Contains { field, item } => {
            typed_query_field(Some(field), &format!("{path}.contains.field"))?;
            if field_context.is_some_and(|context| context.path != field.path) {
                return Err(PortableError::unsupported(
                    "typed_query_phrase_field_mismatch",
                    Some(path),
                    "all phrase terms must target the same field",
                ));
            }
            collect_typed_phrase_terms(item, Some(field), terms, &format!("{path}.contains.item"))?;
        }
        QueryExprKindIr::Phrase(children) => {
            for (index, child) in children.iter().enumerate() {
                collect_typed_phrase_terms(
                    child,
                    field_context,
                    terms,
                    &format!("{path}.phrase[{index}]"),
                )?;
            }
        }
        _ => {
            return Err(PortableError::unsupported(
                "typed_query_phrase_child_not_supported",
                Some(path),
                "phrase children must be term, contains, or phrase nodes",
            ));
        }
    }
    Ok(())
}
