use super::*;
use crate::model::*;
use searchkernel_protocol::wire::v1 as api;
use std::collections::BTreeMap;

/// Canonical default for near and ordered-near query distance.
pub const DEFAULT_PROXIMITY_DISTANCE: u32 = 2;
/// Canonical default for weak-and, WAND, and nearest-neighbor target hits.
pub const DEFAULT_WEAK_AND_TARGET_HITS: u32 = 100;
/// Canonical default for fuzzy maximum edit distance.
pub const DEFAULT_FUZZY_MAX_EDIT_DISTANCE: u32 = 2;
/// Canonical default for fuzzy prefix length.
pub const DEFAULT_FUZZY_PREFIX_LENGTH: u32 = 0;
/// Canonical default for fuzzy prefix matching.
pub const DEFAULT_FUZZY_PREFIX: bool = false;
/// Canonical default for WAND score threshold.
pub const DEFAULT_WAND_SCORE_THRESHOLD: i64 = 0;
/// Canonical default for WAND threshold boost factor.
pub const DEFAULT_WAND_THRESHOLD_BOOST_FACTOR: f64 = 1.0;
/// Canonical default for nearest-neighbor approximate mode.
pub const DEFAULT_NEAREST_APPROXIMATE: bool = true;
/// Canonical default for nearest-neighbor additional explored hits.
pub const DEFAULT_NEAREST_EXPLORE_ADDITIONAL_HITS: u32 = 0;

fn reject_unknown_typed_annotations(
    annotations: &BTreeMap<String, RequestValueIr>,
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
            "this typed query node has no portable meaning for the annotation",
        ));
    }
    Ok(())
}

fn resolve_typed_query_annotations(
    kind: &mut QueryExprKindIr,
    annotations: &mut BTreeMap<String, RequestValueIr>,
    path: &str,
) -> Result<()> {
    match kind {
        QueryExprKindIr::Near { distance, .. } | QueryExprKindIr::ONear { distance, .. } => {
            reject_unknown_typed_annotations(annotations, &["distance"], path)?;
            *distance = annotation_u32(annotations, "distance", path, 1)?
                .unwrap_or(DEFAULT_PROXIMITY_DISTANCE);
        }
        QueryExprKindIr::WeakAnd { target_hits, .. } => {
            reject_unknown_typed_annotations(annotations, &["totalTargetHits"], path)?;
            *target_hits = annotation_u32(annotations, "totalTargetHits", path, 1)?
                .unwrap_or(DEFAULT_WEAK_AND_TARGET_HITS);
        }
        QueryExprKindIr::Fuzzy {
            max_edit_distance,
            prefix_length,
            prefix,
            ..
        } => {
            reject_unknown_typed_annotations(
                annotations,
                &["maxEditDistance", "prefixLength", "prefix"],
                path,
            )?;
            *max_edit_distance = annotation_u32(annotations, "maxEditDistance", path, 0)?
                .unwrap_or(DEFAULT_FUZZY_MAX_EDIT_DISTANCE);
            *prefix_length = annotation_u32(annotations, "prefixLength", path, 0)?
                .unwrap_or(DEFAULT_FUZZY_PREFIX_LENGTH);
            *prefix = annotation_bool(annotations, "prefix", path)?.unwrap_or(DEFAULT_FUZZY_PREFIX);
            if *max_edit_distance == 0 && *prefix_length > 0 {
                return Err(invalid(
                    "invalid_typed_query_annotation",
                    Some(format!("{path}.annotations.prefixLength")),
                    "prefixLength requires a positive maxEditDistance",
                ));
            }
        }
        QueryExprKindIr::UserInput {
            default_index,
            target_hits,
            ..
        }
        | QueryExprKindIr::UserQuery {
            default_index,
            target_hits,
        } => {
            reject_unknown_typed_annotations(annotations, &["defaultIndex"], path)?;
            *default_index = annotation_string(annotations, "defaultIndex", path)?;
            *target_hits = DEFAULT_WEAK_AND_TARGET_HITS;
        }
        QueryExprKindIr::Wand { .. } => {
            reject_unknown_typed_annotations(
                annotations,
                &[
                    "targetNumHits",
                    "totalTargetHits",
                    "scoreThreshold",
                    "thresholdBoostFactor",
                ],
                path,
            )?;
            let target_num_hits = resolve_target_hits(annotations, path, "WAND")?
                .unwrap_or(DEFAULT_WEAK_AND_TARGET_HITS);
            let score_threshold = annotation_i64(annotations, "scoreThreshold", path)?
                .unwrap_or(DEFAULT_WAND_SCORE_THRESHOLD);
            let threshold_boost_factor = annotation_f64(annotations, "thresholdBoostFactor", path)?
                .unwrap_or(DEFAULT_WAND_THRESHOLD_BOOST_FACTOR);
            if threshold_boost_factor <= 0.0 {
                return Err(invalid(
                    "invalid_typed_query_annotation",
                    Some(format!("{path}.annotations.thresholdBoostFactor")),
                    "thresholdBoostFactor must be greater than zero",
                ));
            }
            annotations.insert(
                "targetNumHits".to_owned(),
                RequestValueIr::UInt64(u64::from(target_num_hits)),
            );
            annotations.insert(
                "scoreThreshold".to_owned(),
                RequestValueIr::Int64(score_threshold),
            );
            annotations.insert(
                "thresholdBoostFactor".to_owned(),
                RequestValueIr::Float64(threshold_boost_factor),
            );
        }
        QueryExprKindIr::NearestNeighbor { .. } => {
            reject_unknown_typed_annotations(
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
            let target_num_hits = resolve_target_hits(annotations, path, "nearest-neighbor")?
                .unwrap_or(DEFAULT_WEAK_AND_TARGET_HITS);
            let approximate = annotation_bool(annotations, "approximate", path)?
                .unwrap_or(DEFAULT_NEAREST_APPROXIMATE);
            let distance_threshold = annotation_f64(annotations, "distanceThreshold", path)?;
            if distance_threshold.is_some_and(|value| value < 0.0) {
                return Err(invalid(
                    "invalid_typed_query_annotation",
                    Some(format!("{path}.annotations.distanceThreshold")),
                    "distanceThreshold must be non-negative",
                ));
            }
            let explore_additional_hits =
                annotation_nonnegative_u32(annotations, "exploreAdditionalHits", path)?
                    .unwrap_or(DEFAULT_NEAREST_EXPLORE_ADDITIONAL_HITS);
            annotations.insert(
                "targetNumHits".to_owned(),
                RequestValueIr::UInt64(u64::from(target_num_hits)),
            );
            annotations.insert("approximate".to_owned(), RequestValueIr::Bool(approximate));
            annotations.insert(
                "exploreAdditionalHits".to_owned(),
                RequestValueIr::UInt64(u64::from(explore_additional_hits)),
            );
            if let Some(distance_threshold) = distance_threshold {
                annotations.insert(
                    "distanceThreshold".to_owned(),
                    RequestValueIr::Float64(distance_threshold),
                );
            }
        }
        QueryExprKindIr::SameElement(_) => {
            reject_unknown_typed_annotations(annotations, &["elementFilter"], path)?;
            if let Some(element_filter) = annotation_u32_list(annotations, "elementFilter", path)? {
                annotations.insert(
                    "elementFilter".to_owned(),
                    RequestValueIr::List(
                        element_filter
                            .into_iter()
                            .map(u64::from)
                            .map(RequestValueIr::UInt64)
                            .collect(),
                    ),
                );
            }
        }
        _ => reject_unknown_typed_annotations(annotations, &[], path)?,
    }
    Ok(())
}

fn resolve_target_hits(
    annotations: &mut BTreeMap<String, RequestValueIr>,
    path: &str,
    query_name: &str,
) -> Result<Option<u32>> {
    let target_num_hits = annotation_u32(annotations, "targetNumHits", path, 1)?;
    let total_target_hits = annotation_u32(annotations, "totalTargetHits", path, 1)?;
    if target_num_hits.is_some() && total_target_hits.is_some() {
        return Err(invalid(
            "invalid_typed_query_annotation",
            Some(path.to_owned()),
            format!("{query_name} may specify only one of targetNumHits and totalTargetHits"),
        ));
    }
    Ok(target_num_hits.or(total_target_hits))
}

fn annotation_i64(
    annotations: &mut BTreeMap<String, RequestValueIr>,
    name: &str,
    path: &str,
) -> Result<Option<i64>> {
    let Some(value) = annotations.remove(name) else {
        return Ok(None);
    };
    match value {
        RequestValueIr::Int64(value) => Ok(Some(value)),
        RequestValueIr::UInt64(value) => i64::try_from(value).map(Some).map_err(|_| {
            invalid(
                "invalid_typed_query_annotation",
                Some(format!("{path}.annotations.{name}")),
                format!("{name} must be an integer in the supported range"),
            )
        }),
        _ => Err(invalid(
            "invalid_typed_query_annotation",
            Some(format!("{path}.annotations.{name}")),
            format!("{name} must be an integer"),
        )),
    }
}

fn annotation_f64(
    annotations: &mut BTreeMap<String, RequestValueIr>,
    name: &str,
    path: &str,
) -> Result<Option<f64>> {
    let Some(value) = annotations.remove(name) else {
        return Ok(None);
    };
    let value = match value {
        RequestValueIr::Float64(value) if value.is_finite() => Some(value),
        RequestValueIr::Int64(value) => Some(value as f64),
        RequestValueIr::UInt64(value) => Some(value as f64),
        _ => None,
    };
    value.map(Some).ok_or_else(|| {
        invalid(
            "invalid_typed_query_annotation",
            Some(format!("{path}.annotations.{name}")),
            format!("{name} must be a finite number"),
        )
    })
}

fn annotation_nonnegative_u32(
    annotations: &mut BTreeMap<String, RequestValueIr>,
    name: &str,
    path: &str,
) -> Result<Option<u32>> {
    let Some(value) = annotations.remove(name) else {
        return Ok(None);
    };
    let value = match value {
        RequestValueIr::UInt64(value) => u32::try_from(value).ok(),
        RequestValueIr::Int64(value) if value >= 0 => u32::try_from(value).ok(),
        _ => None,
    };
    value.map(Some).ok_or_else(|| {
        invalid(
            "invalid_typed_query_annotation",
            Some(format!("{path}.annotations.{name}")),
            format!("{name} must be a non-negative integer in the supported range"),
        )
    })
}

fn annotation_u32_list(
    annotations: &mut BTreeMap<String, RequestValueIr>,
    name: &str,
    path: &str,
) -> Result<Option<Vec<u32>>> {
    let Some(value) = annotations.remove(name) else {
        return Ok(None);
    };
    let RequestValueIr::List(values) = value else {
        return Err(invalid(
            "invalid_typed_query_annotation",
            Some(format!("{path}.annotations.{name}")),
            format!("{name} must be a list of non-negative integers"),
        ));
    };
    values
        .into_iter()
        .enumerate()
        .map(|(index, value)| {
            let value = match value {
                RequestValueIr::UInt64(value) => value,
                RequestValueIr::Int64(value) if value >= 0 => value as u64,
                _ => {
                    return Err(invalid(
                        "invalid_typed_query_annotation",
                        Some(format!("{path}.annotations.{name}[{index}]")),
                        format!("{name} must contain only non-negative integers"),
                    ));
                }
            };
            u32::try_from(value).map_err(|_| {
                invalid(
                    "invalid_typed_query_annotation",
                    Some(format!("{path}.annotations.{name}[{index}]")),
                    format!("{name} values must fit in an unsigned 32-bit integer"),
                )
            })
        })
        .collect::<Result<Vec<_>>>()
        .map(Some)
}

fn annotation_u32(
    annotations: &mut BTreeMap<String, RequestValueIr>,
    name: &str,
    path: &str,
    minimum: u32,
) -> Result<Option<u32>> {
    let Some(value) = annotations.remove(name) else {
        return Ok(None);
    };
    let value = match value {
        RequestValueIr::Int64(value) if value >= i64::from(minimum) => u32::try_from(value).ok(),
        RequestValueIr::UInt64(value) if value >= u64::from(minimum) => u32::try_from(value).ok(),
        _ => None,
    };
    value
        .ok_or_else(|| {
            invalid(
                "invalid_typed_query_annotation",
                Some(format!("{path}.annotations.{name}")),
                format!("{name} must be an integer in the supported range"),
            )
        })
        .map(Some)
}

fn annotation_bool(
    annotations: &mut BTreeMap<String, RequestValueIr>,
    name: &str,
    path: &str,
) -> Result<Option<bool>> {
    let Some(value) = annotations.remove(name) else {
        return Ok(None);
    };
    match value {
        RequestValueIr::Bool(value) => Ok(Some(value)),
        _ => Err(invalid(
            "invalid_typed_query_annotation",
            Some(format!("{path}.annotations.{name}")),
            format!("{name} must be a boolean"),
        )),
    }
}

fn annotation_string(
    annotations: &mut BTreeMap<String, RequestValueIr>,
    name: &str,
    path: &str,
) -> Result<Option<String>> {
    let Some(value) = annotations.remove(name) else {
        return Ok(None);
    };
    match value {
        RequestValueIr::String(value) if !value.is_empty() => Ok(Some(value)),
        RequestValueIr::String(_) => Err(invalid(
            "invalid_typed_query_annotation",
            Some(format!("{path}.annotations.{name}")),
            format!("{name} must not be empty"),
        )),
        _ => Err(invalid(
            "invalid_typed_query_annotation",
            Some(format!("{path}.annotations.{name}")),
            format!("{name} must be a string"),
        )),
    }
}

impl IndexContext {
    pub(crate) fn typed_query_to_ir(
        &self,
        expression: api::QueryExpr<'_>,
        path: &str,
        default_index: Option<&str>,
    ) -> Result<QueryIr> {
        let mut query = self.typed_query_expr_to_ir(expression, path)?;
        normalize_typed_query(&mut query, path)?;
        self.validate_typed_query(&query, None, default_index, path)?;
        Ok(QueryIr::Typed(Box::new(query)))
    }

    pub(crate) fn validate_typed_query(
        &self,
        query: &QueryExprIr,
        field_context: Option<&QueryFieldRefIr>,
        default_index: Option<&str>,
        path: &str,
    ) -> Result<()> {
        let require_field_context = |kind: &str| {
            if field_context.is_none() {
                Err(invalid(
                    "missing_field_context",
                    Some(path.to_owned()),
                    format!("{kind} query requires a containing field context"),
                ))
            } else {
                Ok(())
            }
        };
        let child_path = |suffix: &str| format!("{path}.{suffix}");
        let validate_children = |children: &[QueryExprIr], name: &str| {
            if children.is_empty() {
                return Err(invalid(
                    "empty_query_children",
                    Some(child_path(name)),
                    "query operation requires at least one child",
                ));
            }
            for (index, child) in children.iter().enumerate() {
                self.validate_typed_query(
                    child,
                    field_context,
                    default_index,
                    &format!("{path}.{name}[{index}]"),
                )?;
            }
            Ok(())
        };
        let validate_proximity_children = |children: &[QueryExprIr], name: &str| {
            if children.is_empty() {
                return Err(invalid(
                    "empty_query_children",
                    Some(child_path(name)),
                    "phrase/proximity operation requires at least one child",
                ));
            }
            for (index, child) in children.iter().enumerate() {
                if !matches!(
                    child.kind,
                    QueryExprKindIr::Term { .. }
                        | QueryExprKindIr::Contains { .. }
                        | QueryExprKindIr::Phrase(_)
                        | QueryExprKindIr::Near { .. }
                        | QueryExprKindIr::ONear { .. }
                        | QueryExprKindIr::SameElement(_)
                        | QueryExprKindIr::Equiv(_)
                        | QueryExprKindIr::Alternatives(_)
                        | QueryExprKindIr::Fuzzy { .. }
                        | QueryExprKindIr::Matches { .. }
                        | QueryExprKindIr::Text { .. }
                        | QueryExprKindIr::UserInput { .. }
                        | QueryExprKindIr::UserQuery { .. }
                ) {
                    return Err(invalid(
                        "invalid_query_child",
                        Some(format!("{path}.{name}[{index}]")),
                        "phrase/proximity children must be query terms or other term-like nodes",
                    ));
                }
                self.validate_typed_query(
                    child,
                    field_context,
                    default_index,
                    &format!("{path}.{name}[{index}]"),
                )?;
            }
            Ok(())
        };

        match &query.kind {
            QueryExprKindIr::Term { value } => {
                let field = field_context.ok_or_else(|| {
                    invalid(
                        "missing_field_context",
                        Some(path.to_owned()),
                        "term-like query requires a containing field context",
                    )
                })?;
                self.validate_searchable_query_field(field, path)?;
                Self::validate_typed_term_value(value, &format!("{path}.term.value"))
            }
            QueryExprKindIr::Fuzzy { value, .. } => {
                let field = field_context.ok_or_else(|| {
                    invalid(
                        "missing_field_context",
                        Some(path.to_owned()),
                        "term-like query requires a containing field context",
                    )
                })?;
                self.validate_fuzzy_query_field(field, path)?;
                Self::validate_typed_term_value(value, &format!("{path}.fuzzy.value"))
            }
            QueryExprKindIr::Text { .. } => {
                let field = field_context.ok_or_else(|| {
                    invalid(
                        "missing_field_context",
                        Some(path.to_owned()),
                        "term-like query requires a containing field context",
                    )
                })?;
                self.validate_searchable_query_field(field, path)
            }
            QueryExprKindIr::UserInput {
                default_index: query_default_index,
                ..
            } => {
                if field_context.is_some()
                    || query_default_index.is_some()
                    || default_index.is_some()
                {
                    Ok(())
                } else {
                    require_field_context("user input")
                }
            }
            QueryExprKindIr::Contains { field, item } => {
                let field = if matches!(item.kind, QueryExprKindIr::SameElement(_))
                    && self.is_struct_array_field(field)
                {
                    field.clone()
                } else if matches!(item.kind, QueryExprKindIr::Fuzzy { .. }) {
                    self.resolve_fuzzy_query_field(
                        field,
                        field_context,
                        &child_path("contains.field"),
                    )?
                } else {
                    self.resolve_searchable_query_field(
                        field,
                        field_context,
                        &child_path("contains.field"),
                    )?
                };
                self.validate_typed_query(
                    item,
                    Some(&field),
                    default_index,
                    &child_path("contains.item"),
                )
            }
            QueryExprKindIr::And(children) | QueryExprKindIr::Or(children) => validate_children(
                children,
                match &query.kind {
                    QueryExprKindIr::And(_) => "and",
                    QueryExprKindIr::Or(_) => "or",
                    _ => unreachable!(),
                },
            ),
            QueryExprKindIr::WeakAnd {
                children,
                target_hits,
            } => {
                if *target_hits == 0 {
                    return Err(invalid(
                        "invalid_typed_query_annotation",
                        Some(format!("{path}.annotations.totalTargetHits")),
                        "totalTargetHits must be at least one",
                    ));
                }
                validate_children(children, "weak_and")
            }
            QueryExprKindIr::Not(child) | QueryExprKindIr::NonEmpty(child) => {
                self.validate_typed_query(child, field_context, default_index, &child_path("child"))
            }
            QueryExprKindIr::True | QueryExprKindIr::False | QueryExprKindIr::UserQuery { .. } => {
                Ok(())
            }
            QueryExprKindIr::Comparison { field, op, value } => {
                let schema_field = self.validate_filter_query_field(
                    field,
                    &child_path("comparison.field"),
                    "comparison",
                )?;
                if matches!(
                    op,
                    ComparisonOp::Lt | ComparisonOp::Lte | ComparisonOp::Gt | ComparisonOp::Gte
                ) && !matches!(schema_field.kind(), FieldKind::Int64 | FieldKind::Float64)
                {
                    return Err(invalid(
                        "invalid_filter",
                        Some(child_path("comparison.op")),
                        "ordered comparison requires a numeric field",
                    ));
                }
                self.validate_typed_query_scalar(
                    value,
                    schema_field.kind(),
                    &child_path("comparison.value"),
                )
            }
            QueryExprKindIr::Range {
                field,
                lower,
                upper,
                include_lower,
                include_upper,
            } => {
                let schema_field =
                    self.validate_filter_query_field(field, &child_path("range.field"), "range")?;
                if !matches!(schema_field.kind(), FieldKind::Int64 | FieldKind::Float64) {
                    return Err(invalid(
                        "invalid_filter",
                        Some(path.to_owned()),
                        "range requires an Int64 or Float64 field",
                    ));
                }
                self.validate_typed_query_range_bound(
                    lower,
                    schema_field.kind(),
                    &child_path("range.lower"),
                )?;
                self.validate_typed_query_range_bound(
                    upper,
                    schema_field.kind(),
                    &child_path("range.upper"),
                )?;
                if matches!(lower, QueryRangeBoundIr::PositiveInfinity) {
                    return Err(invalid(
                        "invalid_range",
                        Some(child_path("range.lower")),
                        "positive infinity is only valid as an upper bound",
                    ));
                }
                if matches!(upper, QueryRangeBoundIr::NegativeInfinity) {
                    return Err(invalid(
                        "invalid_range",
                        Some(child_path("range.upper")),
                        "negative infinity is only valid as a lower bound",
                    ));
                }
                if matches!(lower, QueryRangeBoundIr::Unbounded)
                    && matches!(upper, QueryRangeBoundIr::Unbounded)
                {
                    return Err(invalid(
                        "invalid_range",
                        Some(path.to_owned()),
                        "range requires at least one bound",
                    ));
                }
                if matches!(lower, QueryRangeBoundIr::Unbounded) && *include_lower == Some(true) {
                    return Err(invalid(
                        "invalid_range",
                        Some(child_path("range.include_lower")),
                        "lower inclusion requires a lower bound",
                    ));
                }
                if matches!(upper, QueryRangeBoundIr::Unbounded) && *include_upper == Some(true) {
                    return Err(invalid(
                        "invalid_range",
                        Some(child_path("range.include_upper")),
                        "upper inclusion requires an upper bound",
                    ));
                }
                Ok(())
            }
            QueryExprKindIr::In { field, values } => {
                let schema_field =
                    self.validate_filter_query_field(field, &child_path("in.field"), "IN")?;
                if !matches!(schema_field.kind(), FieldKind::Keyword | FieldKind::Int64) {
                    return Err(invalid(
                        "invalid_filter",
                        Some(path.to_owned()),
                        "IN requires a Keyword or Int64 field",
                    ));
                }
                if values.is_empty() {
                    return Err(invalid(
                        "empty_in",
                        Some(path.to_owned()),
                        "IN requires at least one value",
                    ));
                }
                for (index, value) in values.iter().enumerate() {
                    self.validate_typed_query_scalar(
                        value,
                        schema_field.kind(),
                        &format!("{path}.in.values[{index}]"),
                    )?;
                }
                Ok(())
            }
            QueryExprKindIr::Matches { field, .. } => self
                .resolve_matches_query_field(field, field_context, &child_path("matches.field"))
                .map(|_| ()),
            QueryExprKindIr::DotProduct { field, items }
            | QueryExprKindIr::WeightedSet { field, items }
            | QueryExprKindIr::Wand { field, items } => {
                let schema_field = self.validate_query_field_reference(field, path)?;
                if !schema_field.capabilities().searchable()
                    && !schema_field.capabilities().filterable()
                {
                    return Err(invalid(
                        "field_not_searchable",
                        Some(path.to_owned()),
                        "weighted query requires an indexed or attribute field",
                    ));
                }
                if !matches!(
                    schema_field.data_type().scalar_primitive(),
                    Some(PrimitiveDataTypeIr::String | PrimitiveDataTypeIr::Int64)
                ) {
                    return Err(invalid(
                        "type_mismatch",
                        Some(path.to_owned()),
                        "weighted query requires a string or integer field",
                    ));
                }
                if items.is_empty() {
                    return Err(invalid(
                        "empty_weighted_query",
                        Some(path.to_owned()),
                        "weighted query requires at least one item",
                    ));
                }
                for (index, item) in items.iter().enumerate() {
                    self.validate_weighted_query_item(
                        item,
                        schema_field.kind(),
                        &format!("{path}.items[{index}].key"),
                    )?;
                }
                Ok(())
            }
            QueryExprKindIr::GeoLocation {
                field,
                latitude,
                longitude,
                radius,
            } => {
                if !latitude.is_finite() || !longitude.is_finite() {
                    return Err(invalid(
                        "non_finite_number",
                        Some(format!("{path}.geo_location")),
                        "geo coordinates must be finite",
                    ));
                }
                if !(-90.0..=90.0).contains(latitude) {
                    return Err(invalid(
                        "invalid_geo_coordinate",
                        Some(format!("{path}.geo_location.latitude")),
                        "latitude must be between -90 and 90",
                    ));
                }
                if !(-180.0..=180.0).contains(longitude) {
                    return Err(invalid(
                        "invalid_geo_coordinate",
                        Some(format!("{path}.geo_location.longitude")),
                        "longitude must be between -180 and 180",
                    ));
                }
                validate_geo_radius(radius, &format!("{path}.geo_location.radius"))?;
                let schema_field = self.validate_query_field_reference(field, path)?;
                if !matches!(schema_field.data_type(), DataTypeIr::Position) {
                    return Err(invalid(
                        "type_mismatch",
                        Some(path.to_owned()),
                        "geo queries require a position field",
                    ));
                }
                Ok(())
            }
            QueryExprKindIr::GeoBoundingBox {
                field,
                south,
                west,
                north,
                east,
            } => {
                if [*south, *west, *north, *east]
                    .iter()
                    .any(|value| !value.is_finite())
                {
                    return Err(invalid(
                        "non_finite_number",
                        Some(format!("{path}.geo_bounding_box")),
                        "geo coordinates must be finite",
                    ));
                }
                if !(-90.0..=90.0).contains(south) || !(-90.0..=90.0).contains(north) {
                    return Err(invalid(
                        "invalid_geo_coordinate",
                        Some(format!("{path}.geo_bounding_box")),
                        "south and north must be between -90 and 90",
                    ));
                }
                if !(-180.0..=180.0).contains(west) || !(-180.0..=180.0).contains(east) {
                    return Err(invalid(
                        "invalid_geo_coordinate",
                        Some(format!("{path}.geo_bounding_box")),
                        "west and east must be between -180 and 180",
                    ));
                }
                if south > north {
                    return Err(invalid(
                        "invalid_geo_bounding_box",
                        Some(format!("{path}.geo_bounding_box")),
                        "south must not be greater than north",
                    ));
                }
                if west > east {
                    return Err(invalid(
                        "invalid_geo_bounding_box",
                        Some(format!("{path}.geo_bounding_box")),
                        "west must not be greater than east",
                    ));
                }
                let schema_field = self.validate_query_field_reference(field, path)?;
                if !matches!(schema_field.data_type(), DataTypeIr::Position) {
                    return Err(invalid(
                        "type_mismatch",
                        Some(path.to_owned()),
                        "geo queries require a position field",
                    ));
                }
                Ok(())
            }
            QueryExprKindIr::NearestNeighbor {
                document_field,
                query_tensor,
            } => {
                if query_tensor.is_empty() {
                    return Err(invalid(
                        "missing_value",
                        Some(format!("{path}.nearest_neighbor.query_tensor")),
                        "nearest-neighbor query tensor name must not be empty",
                    ));
                }
                let schema_field = self.validate_query_field_reference(document_field, path)?;
                if !matches!(schema_field.data_type(), DataTypeIr::Tensor { .. }) {
                    return Err(invalid(
                        "type_mismatch",
                        Some(path.to_owned()),
                        "nearest-neighbor queries require a tensor field",
                    ));
                }
                Ok(())
            }
            QueryExprKindIr::Phrase(children)
            | QueryExprKindIr::Near { children, .. }
            | QueryExprKindIr::ONear { children, .. }
            | QueryExprKindIr::SameElement(children) => {
                require_field_context("phrase/proximity")?;
                if matches!(&query.kind, QueryExprKindIr::SameElement(_)) && children.len() < 2 {
                    return Err(invalid(
                        "invalid_query_children",
                        Some(child_path("same_element")),
                        "sameElement requires at least two children",
                    ));
                }
                validate_proximity_children(
                    children,
                    match &query.kind {
                        QueryExprKindIr::Phrase(_) => "phrase",
                        QueryExprKindIr::Near { .. } => "near",
                        QueryExprKindIr::ONear { .. } => "onear",
                        _ => "same_element",
                    },
                )
            }
            QueryExprKindIr::Equiv(children) => {
                require_field_context("phrase/proximity")?;
                if children.len() < 2 {
                    return Err(invalid(
                        "invalid_query_children",
                        Some(child_path("equiv")),
                        "equiv requires at least two children",
                    ));
                }
                validate_proximity_children(children, "equiv")
            }
            QueryExprKindIr::Alternatives(values) => {
                require_field_context("alternatives")?;
                if values.is_empty() {
                    return Err(invalid(
                        "empty_query_children",
                        Some(child_path("alternatives")),
                        "alternatives requires at least one query",
                    ));
                }
                for (index, value) in values.iter().enumerate() {
                    if value
                        .exactness
                        .is_some_and(|exactness| !(0.0..=1.0).contains(&exactness))
                    {
                        return Err(invalid(
                            "invalid_exactness",
                            Some(format!("{path}.alternatives[{index}].exactness")),
                            "alternative exactness must be between zero and one",
                        ));
                    }
                    self.validate_typed_query(
                        &value.query,
                        field_context,
                        default_index,
                        &format!("{path}.alternatives[{index}].query"),
                    )?;
                }
                Ok(())
            }
            QueryExprKindIr::Rank { matching, ranking } => {
                self.validate_typed_query(
                    matching,
                    field_context,
                    default_index,
                    &child_path("rank.match"),
                )?;
                validate_children(ranking, "rank.ranking")
            }
            QueryExprKindIr::Labeled {
                query,
                label,
                score,
            } => {
                if label.is_empty() {
                    return Err(invalid(
                        "missing_label",
                        Some(child_path("labeled.label")),
                        "labeled query requires a non-empty label",
                    ));
                }
                if !score.is_finite() {
                    return Err(invalid(
                        "non_finite_number",
                        Some(format!("{path}.labeled.score")),
                        "labeled score must be finite",
                    ));
                }
                self.validate_typed_query(
                    query,
                    field_context,
                    default_index,
                    &child_path("labeled.query"),
                )
            }
            QueryExprKindIr::Predicate {
                field,
                features,
                range_features,
                ..
            } => {
                let schema_field = self.validate_query_field_reference(field, path)?;
                if !matches!(schema_field.data_type(), DataTypeIr::Predicate) {
                    return Err(invalid(
                        "type_mismatch",
                        Some(path.to_owned()),
                        "predicate queries require a predicate field",
                    ));
                }
                let has_feature =
                    validate_predicate_features(features, &child_path("predicate.features"))?;
                let has_range_feature = validate_predicate_range_features(
                    range_features,
                    &child_path("predicate.range_features"),
                )?;
                if !has_feature && !has_range_feature {
                    return Err(invalid(
                        "empty_typed_query",
                        Some(path.to_owned()),
                        "predicate query requires a feature or range feature",
                    ));
                }
                Ok(())
            }
        }
    }

    fn validate_searchable_query_field(&self, field: &QueryFieldRefIr, path: &str) -> Result<()> {
        let schema_field = self.query_field(field.path.as_str(), format!("{path}.path"))?;
        if !schema_field.capabilities().searchable()
            && !self.struct_member_is_searchable(field.path.as_str())
        {
            return Err(invalid(
                "field_not_searchable",
                Some(path.to_owned()),
                "query term requires a searchable field",
            ));
        }
        Ok(())
    }

    fn struct_member_is_searchable(&self, path: &str) -> bool {
        let mut parts = path.split('.');
        let Some(root) = parts.next() else {
            return false;
        };
        if parts.clone().next().is_none() {
            return false;
        }
        let Ok(field) = self.query_field(root, format!("query.{root}")) else {
            return false;
        };
        let mut data_type = field.data_type().clone();
        for part in parts {
            if let crate::model::DataTypeIr::Array(element) = data_type {
                data_type = *element;
            }
            if let crate::model::DataTypeIr::StructReference { name } = &data_type {
                let Some(structure) = self
                    .definition
                    .structs()
                    .iter()
                    .find(|value| value.name == *name)
                else {
                    return false;
                };
                data_type = crate::model::DataTypeIr::Struct {
                    fields: structure.fields.clone(),
                };
            }
            let crate::model::DataTypeIr::Struct { fields } = data_type else {
                return false;
            };
            let Some(member) = fields.into_iter().find(|field| field.name == part) else {
                return false;
            };
            data_type = member.data_type;
        }
        matches!(
            data_type,
            crate::model::DataTypeIr::Primitive(crate::model::PrimitiveDataTypeIr::String)
        )
    }

    fn is_struct_array_field(&self, field: &QueryFieldRefIr) -> bool {
        self.query_field(field.path.as_str(), "query.contains.field".to_owned())
            .is_ok_and(|field| {
                matches!(
                    field.data_type(),
                    crate::model::DataTypeIr::Array(element)
                        if matches!(
                            element.as_ref(),
                            crate::model::DataTypeIr::Struct { .. }
                                | crate::model::DataTypeIr::StructReference { .. }
                        )
                )
            })
    }

    fn validate_typed_term_value(value: &RequestValueIr, path: &str) -> Result<()> {
        match value {
            RequestValueIr::String(value) if !value.is_empty() => Ok(()),
            _ => Err(PortableError::unsupported(
                "typed_query_value_not_supported",
                Some(path.to_owned()),
                "typed term values must be non-empty strings",
            )),
        }
    }

    fn validate_fuzzy_query_field(&self, field: &QueryFieldRefIr, path: &str) -> Result<()> {
        let schema_field = self.query_field(field.path.as_str(), format!("{path}.path"))?;
        let string_attribute =
            schema_field.kind() == FieldKind::Keyword && schema_field.capabilities().filterable();
        if !schema_field.capabilities().searchable() && !string_attribute {
            return Err(invalid(
                "field_not_searchable",
                Some(path.to_owned()),
                "fuzzy query requires a searchable text field or filterable keyword field",
            ));
        }
        Ok(())
    }

    fn validate_matches_query_field(&self, field: &QueryFieldRefIr, path: &str) -> Result<()> {
        let schema_field = self.query_field(field.path.as_str(), format!("{path}.path"))?;
        let attribute_string =
            schema_field.kind() == FieldKind::Keyword && schema_field.capabilities().filterable();
        if !schema_field.capabilities().searchable() && !attribute_string {
            return Err(invalid(
                "field_not_searchable",
                Some(path.to_owned()),
                "matches query requires a searchable text field or filterable keyword field",
            ));
        }
        Ok(())
    }

    fn resolve_matches_query_field(
        &self,
        field: &QueryFieldRefIr,
        field_context: Option<&QueryFieldRefIr>,
        path: &str,
    ) -> Result<QueryFieldRefIr> {
        if let Some(context) = field_context {
            if !field.path.as_str().contains('.') && context.path != field.path {
                let qualified = FieldName::parse_query_path(format!(
                    "{}.{}",
                    context.path.as_str(),
                    field.path.as_str()
                ))?;
                let qualified_ref = QueryFieldRefIr {
                    path: qualified,
                    element_index: field.element_index,
                };
                self.validate_matches_query_field(&qualified_ref, path)?;
                return Ok(qualified_ref);
            }
        }
        self.validate_matches_query_field(field, path)?;
        Ok(field.clone())
    }

    pub(crate) fn resolve_searchable_query_field(
        &self,
        field: &QueryFieldRefIr,
        field_context: Option<&QueryFieldRefIr>,
        path: &str,
    ) -> Result<QueryFieldRefIr> {
        if let Some(context) = field_context {
            if !field.path.as_str().contains('.') && context.path != field.path {
                let qualified = FieldName::parse_query_path(format!(
                    "{}.{}",
                    context.path.as_str(),
                    field.path.as_str()
                ))?;
                let qualified_ref = QueryFieldRefIr {
                    path: qualified,
                    element_index: field.element_index,
                };
                self.validate_searchable_query_field(&qualified_ref, path)?;
                return Ok(qualified_ref);
            }
        }
        if self
            .query_field(field.path.as_str(), format!("{path}.path"))
            .is_ok()
            && self
                .query_field(field.path.as_str(), format!("{path}.path"))?
                .capabilities()
                .searchable()
        {
            return Ok(field.clone());
        }
        let Some(context) = field_context else {
            return self
                .validate_searchable_query_field(field, path)
                .map(|()| field.clone());
        };
        if field.path.as_str().contains('.') {
            return self
                .validate_searchable_query_field(field, path)
                .map(|()| field.clone());
        }
        let qualified = FieldName::parse_query_path(format!(
            "{}.{}",
            context.path.as_str(),
            field.path.as_str()
        ))?;
        let qualified_ref = QueryFieldRefIr {
            path: qualified,
            element_index: field.element_index,
        };
        self.validate_searchable_query_field(&qualified_ref, path)?;
        Ok(qualified_ref)
    }

    fn resolve_fuzzy_query_field(
        &self,
        field: &QueryFieldRefIr,
        field_context: Option<&QueryFieldRefIr>,
        path: &str,
    ) -> Result<QueryFieldRefIr> {
        if let Some(context) = field_context {
            if !field.path.as_str().contains('.') && context.path != field.path {
                let qualified = FieldName::parse_query_path(format!(
                    "{}.{}",
                    context.path.as_str(),
                    field.path.as_str()
                ))?;
                let qualified_ref = QueryFieldRefIr {
                    path: qualified,
                    element_index: field.element_index,
                };
                self.validate_fuzzy_query_field(&qualified_ref, path)?;
                return Ok(qualified_ref);
            }
        }
        if let Ok(schema_field) = self.query_field(field.path.as_str(), format!("{path}.path")) {
            if schema_field.capabilities().searchable()
                || (schema_field.kind() == FieldKind::Keyword
                    && schema_field.capabilities().filterable())
            {
                return Ok(field.clone());
            }
        }
        let Some(context) = field_context else {
            self.validate_fuzzy_query_field(field, path)?;
            return Ok(field.clone());
        };
        if field.path.as_str().contains('.') {
            self.validate_fuzzy_query_field(field, path)?;
            return Ok(field.clone());
        }
        let qualified = FieldName::parse_query_path(format!(
            "{}.{}",
            context.path.as_str(),
            field.path.as_str()
        ))?;
        let qualified_ref = QueryFieldRefIr {
            path: qualified,
            element_index: field.element_index,
        };
        self.validate_fuzzy_query_field(&qualified_ref, path)?;
        Ok(qualified_ref)
    }

    fn validate_filter_query_field<'a>(
        &'a self,
        field: &QueryFieldRefIr,
        path: &str,
        operation: &str,
    ) -> Result<&'a FieldIr> {
        let schema_field = self.query_field(field.path.as_str(), format!("{path}.path"))?;
        if !schema_field.capabilities().filterable() {
            return Err(invalid(
                "field_not_filterable",
                Some(path.to_owned()),
                format!("{operation} requires a filterable field"),
            ));
        }
        Ok(schema_field)
    }

    fn validate_typed_query_scalar(
        &self,
        value: &RequestValueIr,
        kind: FieldKind,
        path: &str,
    ) -> Result<()> {
        let valid = match (value, kind) {
            (RequestValueIr::String(_), FieldKind::Text | FieldKind::Keyword)
            | (RequestValueIr::Int64(_), FieldKind::Int64)
            | (RequestValueIr::Bool(_), FieldKind::Bool) => true,
            (RequestValueIr::Float64(value), FieldKind::Float64) => value.is_finite(),
            _ => false,
        };
        if !valid {
            return Err(invalid(
                "type_mismatch",
                Some(path.to_owned()),
                "query value type does not match field kind",
            ));
        }
        Ok(())
    }

    fn validate_typed_query_range_bound(
        &self,
        bound: &QueryRangeBoundIr,
        kind: FieldKind,
        path: &str,
    ) -> Result<()> {
        if let QueryRangeBoundIr::Finite(value) = bound {
            self.validate_typed_query_scalar(value, kind, path)?;
        }
        Ok(())
    }

    fn validate_weighted_query_item(
        &self,
        item: &WeightedQueryItemIr,
        kind: FieldKind,
        path: &str,
    ) -> Result<()> {
        let valid = match (&item.key, kind) {
            (RequestValueIr::String(value), FieldKind::Text | FieldKind::Keyword) => {
                !value.is_empty()
            }
            (RequestValueIr::Int64(_), FieldKind::Int64) => true,
            _ => false,
        };
        if !valid {
            if matches!(&item.key, RequestValueIr::String(value) if value.is_empty()) {
                return Err(invalid(
                    "invalid_typed_query",
                    Some(path.to_owned()),
                    "weighted query string keys must not be empty",
                ));
            }
            return Err(invalid(
                "type_mismatch",
                Some(path.to_owned()),
                "weighted query key type does not match the field kind",
            ));
        }
        Ok(())
    }

    pub(crate) fn typed_query_expr_to_ir(
        &self,
        expression: api::QueryExpr<'_>,
        path: &str,
    ) -> Result<QueryExprIr> {
        let annotations =
            request_values_to_map(expression.annotations(), &format!("{path}.annotations"))?;
        let kind = match expression.expr_type() {
            api::QueryExprKind::TermQuery => {
                let value = expression
                    .expr_as_term_query()
                    .and_then(|query| query.value())
                    .ok_or_else(|| {
                        invalid(
                            "missing_value",
                            Some(format!("{path}.term.value")),
                            "term query requires a value",
                        )
                    })?;
                QueryExprKindIr::Term {
                    value: RequestValueIr::from_api(value, format!("{path}.term.value"))?,
                }
            }
            api::QueryExprKind::QueryContains => {
                let query = expression.expr_as_query_contains().ok_or_else(|| {
                    invalid("invalid_query", Some(path), "contains query is missing")
                })?;
                let field =
                    self.query_field_ref_to_ir(query.field(), &format!("{path}.contains.field"))?;
                let item = query.item().ok_or_else(|| {
                    invalid(
                        "missing_query_item",
                        Some(format!("{path}.contains.item")),
                        "query contains requires an item",
                    )
                })?;
                QueryExprKindIr::Contains {
                    field,
                    item: Box::new(
                        self.typed_query_expr_to_ir(item, &format!("{path}.contains.item"))?,
                    ),
                }
            }
            api::QueryExprKind::QueryAnd => QueryExprKindIr::And(
                self.typed_query_children(
                    expression
                        .expr_as_query_and()
                        .and_then(|value| value.clauses()),
                    path,
                    "and",
                )?,
            ),
            api::QueryExprKind::QueryOr => QueryExprKindIr::Or(
                self.typed_query_children(
                    expression
                        .expr_as_query_or()
                        .and_then(|value| value.clauses()),
                    path,
                    "or",
                )?,
            ),
            api::QueryExprKind::QueryNot => {
                let query = expression
                    .expr_as_query_not()
                    .and_then(|value| value.clause())
                    .ok_or_else(|| {
                        invalid(
                            "missing_query",
                            Some(format!("{path}.not.clause")),
                            "not query requires a clause",
                        )
                    })?;
                QueryExprKindIr::Not(Box::new(
                    self.typed_query_expr_to_ir(query, &format!("{path}.not.clause"))?,
                ))
            }
            api::QueryExprKind::QueryTrue => QueryExprKindIr::True,
            api::QueryExprKind::QueryFalse => QueryExprKindIr::False,
            api::QueryExprKind::QueryComparison => self.comparison_query_to_ir(expression, path)?,
            api::QueryExprKind::QueryRange => self.range_query_to_ir(expression, path)?,
            api::QueryExprKind::QueryIn => {
                let query = expression
                    .expr_as_query_in()
                    .ok_or_else(|| invalid("invalid_query", Some(path), "in query is missing"))?;
                QueryExprKindIr::In {
                    field: self
                        .query_field_ref_to_ir(query.field(), &format!("{path}.in.field"))?,
                    values: query
                        .values()
                        .map(|values| {
                            values
                                .iter()
                                .enumerate()
                                .map(|(index, value)| {
                                    RequestValueIr::from_api(
                                        value,
                                        format!("{path}.in.values[{index}]"),
                                    )
                                })
                                .collect::<Result<Vec<_>>>()
                        })
                        .transpose()?
                        .unwrap_or_default(),
                }
            }
            api::QueryExprKind::QueryPhrase => QueryExprKindIr::Phrase(
                self.typed_query_children(
                    expression
                        .expr_as_query_phrase()
                        .and_then(|value| value.terms()),
                    path,
                    "phrase",
                )?,
            ),
            api::QueryExprKind::QueryNear => QueryExprKindIr::Near {
                children: self.typed_query_children(
                    expression
                        .expr_as_query_near()
                        .and_then(|value| value.terms()),
                    path,
                    "near",
                )?,
                distance: DEFAULT_PROXIMITY_DISTANCE,
            },
            api::QueryExprKind::QueryONear => QueryExprKindIr::ONear {
                children: self.typed_query_children(
                    expression
                        .expr_as_query_onear()
                        .and_then(|value| value.terms()),
                    path,
                    "onear",
                )?,
                distance: DEFAULT_PROXIMITY_DISTANCE,
            },
            api::QueryExprKind::QuerySameElement => QueryExprKindIr::SameElement(
                self.typed_query_children(
                    expression
                        .expr_as_query_same_element()
                        .and_then(|value| value.clauses()),
                    path,
                    "same_element",
                )?,
            ),
            api::QueryExprKind::QueryEquiv => QueryExprKindIr::Equiv(
                self.typed_query_children(
                    expression
                        .expr_as_query_equiv()
                        .and_then(|value| value.clauses()),
                    path,
                    "equiv",
                )?,
            ),
            api::QueryExprKind::QueryAlternatives => {
                self.alternatives_query_to_ir(expression, path)?
            }
            api::QueryExprKind::QueryFuzzy => {
                let value = expression
                    .expr_as_query_fuzzy()
                    .and_then(|query| query.value())
                    .ok_or_else(|| {
                        invalid(
                            "missing_value",
                            Some(format!("{path}.fuzzy.value")),
                            "fuzzy query requires a value",
                        )
                    })?;
                QueryExprKindIr::Fuzzy {
                    value: RequestValueIr::from_api(value, format!("{path}.fuzzy.value"))?,
                    max_edit_distance: DEFAULT_FUZZY_MAX_EDIT_DISTANCE,
                    prefix_length: DEFAULT_FUZZY_PREFIX_LENGTH,
                    prefix: DEFAULT_FUZZY_PREFIX,
                }
            }
            api::QueryExprKind::QueryMatches => {
                let query = expression.expr_as_query_matches().ok_or_else(|| {
                    invalid("invalid_query", Some(path), "matches query is missing")
                })?;
                QueryExprKindIr::Matches {
                    field: self
                        .query_field_ref_to_ir(query.field(), &format!("{path}.matches.field"))?,
                    pattern: query
                        .pattern()
                        .ok_or_else(|| {
                            invalid(
                                "missing_value",
                                Some(format!("{path}.matches.pattern")),
                                "matches query requires a pattern",
                            )
                        })?
                        .to_owned(),
                }
            }
            api::QueryExprKind::QueryText => QueryExprKindIr::Text {
                text: expression
                    .expr_as_query_text()
                    .and_then(|query| query.text())
                    .ok_or_else(|| {
                        invalid(
                            "missing_value",
                            Some(format!("{path}.text.text")),
                            "text query requires text",
                        )
                    })?
                    .to_owned(),
            },
            api::QueryExprKind::QueryUserInput => QueryExprKindIr::UserInput {
                value: expression
                    .expr_as_query_user_input()
                    .and_then(|query| query.value())
                    .ok_or_else(|| {
                        invalid(
                            "missing_value",
                            Some(format!("{path}.user_input.value")),
                            "user input query requires a value",
                        )
                    })?
                    .to_owned(),
                default_index: None,
                target_hits: DEFAULT_WEAK_AND_TARGET_HITS,
            },
            api::QueryExprKind::QueryUserQuery => QueryExprKindIr::UserQuery {
                default_index: None,
                target_hits: DEFAULT_WEAK_AND_TARGET_HITS,
            },
            api::QueryExprKind::QueryDotProduct
            | api::QueryExprKind::QueryWeightedSet
            | api::QueryExprKind::QueryWand => self.weighted_query_to_ir(expression, path)?,
            api::QueryExprKind::QueryWeakAnd => QueryExprKindIr::WeakAnd {
                children: self.typed_query_children(
                    expression
                        .expr_as_query_weak_and()
                        .and_then(|value| value.clauses()),
                    path,
                    "weak_and",
                )?,
                target_hits: DEFAULT_WEAK_AND_TARGET_HITS,
            },
            api::QueryExprKind::QueryRank => {
                let query = expression
                    .expr_as_query_rank()
                    .ok_or_else(|| invalid("invalid_query", Some(path), "rank query is missing"))?;
                QueryExprKindIr::Rank {
                    matching: Box::new(self.typed_query_expr_to_ir(
                        query.match_().ok_or_else(|| {
                            invalid(
                                "missing_query",
                                Some(format!("{path}.rank.match")),
                                "rank query requires a match",
                            )
                        })?,
                        &format!("{path}.rank.match"),
                    )?),
                    ranking: self.typed_query_children(query.rank(), path, "rank")?,
                }
            }
            api::QueryExprKind::QueryLabeled => {
                let query = expression.expr_as_query_labeled().ok_or_else(|| {
                    invalid("invalid_query", Some(path), "labeled query is missing")
                })?;
                let score = query.score();
                QueryExprKindIr::Labeled {
                    query: Box::new(self.typed_query_expr_to_ir(
                        query.query().ok_or_else(|| {
                            invalid(
                                "missing_query",
                                Some(format!("{path}.labeled.query")),
                                "labeled query requires a query",
                            )
                        })?,
                        &format!("{path}.labeled.query"),
                    )?),
                    label: query.label().unwrap_or_default().to_owned(),
                    score,
                }
            }
            api::QueryExprKind::QueryNonEmpty => {
                let query = expression.expr_as_query_non_empty().ok_or_else(|| {
                    invalid("invalid_query", Some(path), "non-empty query is missing")
                })?;
                QueryExprKindIr::NonEmpty(Box::new(self.typed_query_expr_to_ir(
                    query.query().ok_or_else(|| {
                        invalid(
                            "missing_query",
                            Some(format!("{path}.non_empty.query")),
                            "non-empty query requires a query",
                        )
                    })?,
                    &format!("{path}.non_empty.query"),
                )?))
            }
            api::QueryExprKind::QueryGeoLocation | api::QueryExprKind::QueryGeoBoundingBox => {
                self.geo_query_to_ir(expression, path)?
            }
            api::QueryExprKind::QueryNearestNeighbor => {
                self.nearest_neighbor_to_ir(expression, path)?
            }
            api::QueryExprKind::QueryPredicate => self.predicate_query_to_ir(expression, path)?,
            _ => {
                return Err(invalid(
                    "invalid_query",
                    Some(path),
                    "query expression must select an operation",
                ))
            }
        };
        Ok(QueryExprIr { kind, annotations })
    }

    pub(crate) fn comparison_query_to_ir(
        &self,
        expression: api::QueryExpr<'_>,
        path: &str,
    ) -> Result<QueryExprKindIr> {
        let query = expression
            .expr_as_query_comparison()
            .ok_or_else(|| invalid("invalid_query", Some(path), "comparison query is missing"))?;
        Ok(QueryExprKindIr::Comparison {
            field: self
                .query_field_ref_to_ir(query.field(), &format!("{path}.comparison.field"))?,
            op: query_comparison_op(query.op().0, &format!("{path}.comparison.op"))?,
            value: RequestValueIr::from_api(
                query.value().ok_or_else(|| {
                    invalid(
                        "missing_value",
                        Some(format!("{path}.comparison.value")),
                        "comparison requires a value",
                    )
                })?,
                format!("{path}.comparison.value"),
            )?,
        })
    }

    pub(crate) fn range_query_to_ir(
        &self,
        expression: api::QueryExpr<'_>,
        path: &str,
    ) -> Result<QueryExprKindIr> {
        let query = expression
            .expr_as_query_range()
            .ok_or_else(|| invalid("invalid_query", Some(path), "range query is missing"))?;
        Ok(QueryExprKindIr::Range {
            field: self.query_field_ref_to_ir(query.field(), &format!("{path}.range.field"))?,
            lower: query_range_bound_to_ir(query.lower(), &format!("{path}.range.lower"))?,
            upper: query_range_bound_to_ir(query.upper(), &format!("{path}.range.upper"))?,
            include_lower: query.include_lower().map(|value| value.value()),
            include_upper: query.include_upper().map(|value| value.value()),
        })
    }

    pub(crate) fn alternatives_query_to_ir(
        &self,
        expression: api::QueryExpr<'_>,
        path: &str,
    ) -> Result<QueryExprKindIr> {
        let values = expression
            .expr_as_query_alternatives()
            .and_then(|value| value.alternatives())
            .map(|values| {
                values
                    .iter()
                    .enumerate()
                    .map(|(index, value)| {
                        let query = value.query().ok_or_else(|| {
                            invalid(
                                "missing_query",
                                Some(format!("{path}.alternatives[{index}].query")),
                                "alternative requires a query",
                            )
                        })?;
                        let exactness = value.exactness().map(|value| value.value());
                        Ok(QueryAlternativeIr {
                            query: self.typed_query_expr_to_ir(
                                query,
                                &format!("{path}.alternatives[{index}].query"),
                            )?,
                            exactness,
                        })
                    })
                    .collect::<Result<Vec<_>>>()
            })
            .transpose()?
            .unwrap_or_default();
        Ok(QueryExprKindIr::Alternatives(values))
    }

    pub(crate) fn weighted_query_to_ir(
        &self,
        expression: api::QueryExpr<'_>,
        path: &str,
    ) -> Result<QueryExprKindIr> {
        let (field, items) = match expression.expr_type() {
            api::QueryExprKind::QueryDotProduct => {
                let query = expression.expr_as_query_dot_product().ok_or_else(|| {
                    invalid("invalid_query", Some(path), "dot product query is missing")
                })?;
                (query.field(), query.items())
            }
            api::QueryExprKind::QueryWeightedSet => {
                let query = expression.expr_as_query_weighted_set().ok_or_else(|| {
                    invalid("invalid_query", Some(path), "weighted set query is missing")
                })?;
                (query.field(), query.items())
            }
            api::QueryExprKind::QueryWand => {
                let query = expression
                    .expr_as_query_wand()
                    .ok_or_else(|| invalid("invalid_query", Some(path), "wand query is missing"))?;
                (query.field(), query.items())
            }
            _ => {
                return Err(invalid(
                    "invalid_query",
                    Some(path),
                    "weighted query must select an operation",
                ));
            }
        };
        let items = weighted_query_items_to_ir(items, path)?;
        let field = self.query_field_ref_to_ir(field, &format!("{path}.field"))?;
        Ok(match expression.expr_type() {
            api::QueryExprKind::QueryDotProduct => QueryExprKindIr::DotProduct { field, items },
            api::QueryExprKind::QueryWeightedSet => QueryExprKindIr::WeightedSet { field, items },
            api::QueryExprKind::QueryWand => QueryExprKindIr::Wand { field, items },
            _ => {
                return Err(invalid(
                    "invalid_query",
                    Some(path),
                    "weighted query must select an operation",
                ));
            }
        })
    }

    pub(crate) fn geo_query_to_ir(
        &self,
        expression: api::QueryExpr<'_>,
        path: &str,
    ) -> Result<QueryExprKindIr> {
        match expression.expr_type() {
            api::QueryExprKind::QueryGeoLocation => {
                let query = expression.expr_as_query_geo_location().ok_or_else(|| {
                    invalid("invalid_query", Some(path), "geo location query is missing")
                })?;
                Ok(QueryExprKindIr::GeoLocation {
                    field: self.query_field_ref_to_ir(
                        query.field(),
                        &format!("{path}.geo_location.field"),
                    )?,
                    latitude: query.latitude(),
                    longitude: query.longitude(),
                    radius: query
                        .radius()
                        .ok_or_else(|| {
                            invalid(
                                "missing_value",
                                Some(format!("{path}.geo_location.radius")),
                                "geo location query requires a radius",
                            )
                        })?
                        .to_owned(),
                })
            }
            api::QueryExprKind::QueryGeoBoundingBox => {
                let query = expression.expr_as_query_geo_bounding_box().ok_or_else(|| {
                    invalid(
                        "invalid_query",
                        Some(path),
                        "geo bounding box query is missing",
                    )
                })?;
                Ok(QueryExprKindIr::GeoBoundingBox {
                    field: self.query_field_ref_to_ir(
                        query.field(),
                        &format!("{path}.geo_bounding_box.field"),
                    )?,
                    south: query.south(),
                    west: query.west(),
                    north: query.north(),
                    east: query.east(),
                })
            }
            _ => Err(invalid(
                "invalid_query",
                Some(path),
                "geo query must select an operation",
            )),
        }
    }

    pub(crate) fn nearest_neighbor_to_ir(
        &self,
        expression: api::QueryExpr<'_>,
        path: &str,
    ) -> Result<QueryExprKindIr> {
        let query = expression.expr_as_query_nearest_neighbor().ok_or_else(|| {
            invalid(
                "invalid_query",
                Some(path),
                "nearest neighbor query is missing",
            )
        })?;
        Ok(QueryExprKindIr::NearestNeighbor {
            document_field: self.query_field_ref_to_ir(
                query.document_field(),
                &format!("{path}.nearest_neighbor.document_field"),
            )?,
            query_tensor: query
                .query_tensor()
                .ok_or_else(|| {
                    invalid(
                        "missing_value",
                        Some(format!("{path}.nearest_neighbor.query_tensor")),
                        "nearest-neighbor query requires a query tensor name",
                    )
                })?
                .to_owned(),
        })
    }

    pub(crate) fn predicate_query_to_ir(
        &self,
        expression: api::QueryExpr<'_>,
        path: &str,
    ) -> Result<QueryExprKindIr> {
        let query = expression
            .expr_as_query_predicate()
            .ok_or_else(|| invalid("invalid_query", Some(path), "predicate query is missing"))?;
        Ok(QueryExprKindIr::Predicate {
            field: self.query_field_ref_to_ir(query.field(), &format!("{path}.predicate.field"))?,
            features: query_value_object_to_ir(
                query.features(),
                &format!("{path}.predicate.features"),
            )?,
            range_features: query_value_object_to_ir(
                query.range_features(),
                &format!("{path}.predicate.range_features"),
            )?,
        })
    }

    pub(crate) fn query_field_ref_to_ir(
        &self,
        value: Option<api::QueryFieldRef<'_>>,
        path: &str,
    ) -> Result<QueryFieldRefIr> {
        let value = value
            .ok_or_else(|| invalid("missing_query_field", Some(path), "query field is required"))?;
        let field_path = value.path().unwrap_or_default();
        if field_path.is_empty() {
            return Err(invalid(
                "missing_query_field",
                Some(format!("{path}.path")),
                "query field path is required",
            ));
        }
        Ok(QueryFieldRefIr {
            path: FieldName::parse_query_path(field_path.to_owned())?,
            element_index: value.element_index().map(|value| value.value()),
        })
    }

    fn validate_query_field_reference(
        &self,
        field: &QueryFieldRefIr,
        path: &str,
    ) -> Result<&FieldIr> {
        self.query_field(field.path.as_str(), format!("{path}.field.path"))
    }

    pub(crate) fn typed_query_children(
        &self,
        children: Option<flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<api::QueryExpr<'_>>>>,
        parent_path: &str,
        operation_name: &str,
    ) -> Result<Vec<QueryExprIr>> {
        children
            .map(|children| {
                children
                    .iter()
                    .enumerate()
                    .map(|(index, child)| {
                        self.typed_query_expr_to_ir(
                            child,
                            &format!("{parent_path}.{operation_name}.clauses[{index}]"),
                        )
                    })
                    .collect()
            })
            .transpose()
            .map(|value| value.unwrap_or_default())
    }
}

/// Apply typed-query defaults and annotation validation after either ingress
/// has produced the structural query tree.
pub(crate) fn normalize_typed_query(query: &mut QueryExprIr, path: &str) -> Result<()> {
    resolve_typed_query_annotations(&mut query.kind, &mut query.annotations, path)?;
    match &mut query.kind {
        QueryExprKindIr::Contains { item, .. } => {
            normalize_typed_query(item, &format!("{path}.contains.item"))
        }
        QueryExprKindIr::And(children) => normalize_typed_query_children(children, path, "and"),
        QueryExprKindIr::Or(children) => normalize_typed_query_children(children, path, "or"),
        QueryExprKindIr::Not(child) => normalize_typed_query(child, &format!("{path}.not.clause")),
        QueryExprKindIr::Phrase(children) => {
            normalize_typed_query_children(children, path, "phrase")
        }
        QueryExprKindIr::Near { children, .. } => {
            normalize_typed_query_children(children, path, "near")
        }
        QueryExprKindIr::ONear { children, .. } => {
            normalize_typed_query_children(children, path, "onear")
        }
        QueryExprKindIr::SameElement(children) => {
            normalize_typed_query_children(children, path, "same_element")
        }
        QueryExprKindIr::Equiv(children) => normalize_typed_query_children(children, path, "equiv"),
        QueryExprKindIr::Alternatives(values) => {
            for (index, value) in values.iter_mut().enumerate() {
                normalize_typed_query(
                    &mut value.query,
                    &format!("{path}.alternatives[{index}].query"),
                )?;
            }
            Ok(())
        }
        QueryExprKindIr::WeakAnd { children, .. } => {
            normalize_typed_query_children(children, path, "weak_and")
        }
        QueryExprKindIr::Rank { matching, ranking } => {
            normalize_typed_query(matching, &format!("{path}.rank.match"))?;
            normalize_typed_query_children(ranking, path, "rank")
        }
        QueryExprKindIr::Labeled { query, .. } => {
            normalize_typed_query(query, &format!("{path}.labeled.query"))
        }
        QueryExprKindIr::NonEmpty(child) => {
            normalize_typed_query(child, &format!("{path}.non_empty.query"))
        }
        _ => Ok(()),
    }
}

fn normalize_typed_query_children(
    children: &mut [QueryExprIr],
    path: &str,
    operation: &str,
) -> Result<()> {
    for (index, child) in children.iter_mut().enumerate() {
        normalize_typed_query(child, &format!("{path}.{operation}.clauses[{index}]"))?;
    }
    Ok(())
}

fn validate_geo_radius(radius: &str, path: &str) -> Result<()> {
    let radius = radius.trim();
    if radius.is_empty() {
        return Err(invalid(
            "invalid_geo_radius",
            Some(path.to_owned()),
            "geo radius must not be empty",
        ));
    }
    let (number, multiplier) = if let Some(number) = radius.strip_suffix("km") {
        (number, 1.0 / 111.1950802335329)
    } else if let Some(number) = radius.strip_suffix('m') {
        (number, 1.0 / 111_195.0802335329)
    } else {
        (radius, 1.0)
    };
    let amount = number.trim().parse::<f64>().ok();
    let Some(amount) = amount else {
        return Err(invalid(
            "invalid_geo_radius",
            Some(path.to_owned()),
            "geo radius must be a positive number with optional m or km suffix",
        ));
    };
    let microdegrees = amount * multiplier * 1_000_000.0;
    if !amount.is_finite()
        || amount <= 0.0
        || !microdegrees.is_finite()
        || microdegrees > f64::from(u32::MAX)
    {
        return Err(invalid(
            "invalid_geo_radius",
            Some(path.to_owned()),
            "geo radius must be a positive finite value within the supported range",
        ));
    }
    Ok(())
}

fn validate_predicate_features(
    values: &BTreeMap<String, RequestValueIr>,
    path: &str,
) -> Result<bool> {
    let mut has_feature = false;
    for (key, value) in values {
        has_feature |= validate_predicate_feature_value(key, value, path)?;
    }
    Ok(has_feature)
}

fn validate_predicate_feature_value(key: &str, value: &RequestValueIr, path: &str) -> Result<bool> {
    match value {
        RequestValueIr::String(value) if !key.is_empty() && !value.is_empty() => Ok(true),
        RequestValueIr::List(values) => {
            values
                .iter()
                .enumerate()
                .try_fold(false, |has_feature, (index, value)| {
                    Ok(has_feature
                        | validate_predicate_feature_value(
                            key,
                            value,
                            &format!("{path}.{key}[{index}]"),
                        )?)
                })
        }
        RequestValueIr::Object(values) => {
            parse_predicate_bitmap(key, path)?;
            values
                .iter()
                .try_fold(false, |has_feature, (nested_key, nested_value)| {
                    Ok(has_feature
                        | validate_predicate_feature_value(nested_key, nested_value, path)?)
                })
        }
        RequestValueIr::String(_) => Err(invalid(
            "invalid_predicate_value",
            Some(format!("{path}.{key}")),
            "predicate feature keys and values must not be empty",
        )),
        _ => Err(invalid(
            "invalid_predicate_value",
            Some(format!("{path}.{key}")),
            "predicate features support non-empty strings, string lists, or bitmap-keyed objects",
        )),
    }
}

fn validate_predicate_range_features(
    values: &BTreeMap<String, RequestValueIr>,
    path: &str,
) -> Result<bool> {
    let mut has_feature = false;
    for (key, value) in values {
        has_feature |= validate_predicate_range_value(key, value, path)?;
    }
    Ok(has_feature)
}

fn validate_predicate_range_value(key: &str, value: &RequestValueIr, path: &str) -> Result<bool> {
    match value {
        RequestValueIr::UInt64(_) => {
            if key.is_empty() {
                return Err(invalid(
                    "invalid_predicate_value",
                    Some(format!("{path}.{key}")),
                    "predicate range feature keys must not be empty",
                ));
            }
            Ok(true)
        }
        RequestValueIr::Int64(value) if *value >= 0 => {
            if key.is_empty() {
                return Err(invalid(
                    "invalid_predicate_value",
                    Some(format!("{path}.{key}")),
                    "predicate range feature keys must not be empty",
                ));
            }
            Ok(true)
        }
        RequestValueIr::List(values) => {
            values
                .iter()
                .enumerate()
                .try_fold(false, |has_feature, (index, value)| {
                    Ok(has_feature
                        | validate_predicate_range_value(
                            key,
                            value,
                            &format!("{path}.{key}[{index}]"),
                        )?)
                })
        }
        RequestValueIr::Object(values) => {
            parse_predicate_bitmap(key, path)?;
            values.iter().try_fold(false, |has_feature, (nested_key, nested_value)| {
                Ok(has_feature
                    | validate_predicate_range_value(nested_key, nested_value, path)?)
            })
        }
        _ => Err(invalid(
            "invalid_predicate_value",
            Some(format!("{path}.{key}")),
            "predicate range features require unsigned or non-negative integer values, integer lists, or bitmap-keyed objects",
        )),
    }
}

fn parse_predicate_bitmap(value: &str, path: &str) -> Result<u64> {
    let (digits, radix) = value
        .strip_prefix("0x")
        .map_or((value, 10), |value| (value, 16));
    u64::from_str_radix(digits, radix).map_err(|_| {
        invalid(
            "invalid_predicate_sub_query_bitmap",
            Some(format!("{path}.{value}")),
            "predicate bitmap object keys must be decimal or 0x-prefixed hexadecimal uint64 values",
        )
    })
}

pub(crate) fn query_comparison_op(value: u8, path: &str) -> Result<ComparisonOp> {
    match value {
        1 => Ok(ComparisonOp::Eq),
        2 => Ok(ComparisonOp::Ne),
        3 => Ok(ComparisonOp::Lt),
        4 => Ok(ComparisonOp::Lte),
        5 => Ok(ComparisonOp::Gt),
        6 => Ok(ComparisonOp::Gte),
        _ => Err(invalid(
            "invalid_query",
            Some(path),
            "comparison operator is unspecified or unknown",
        )),
    }
}

pub(crate) fn query_range_bound_to_ir(
    value: Option<api::RangeBound<'_>>,
    path: &str,
) -> Result<QueryRangeBoundIr> {
    let value = value
        .ok_or_else(|| invalid("missing_range_bound", Some(path), "range bound is required"))?;
    match value.bound_type() {
        api::RangeBoundKind::FiniteRangeBound => {
            Ok(QueryRangeBoundIr::Finite(RequestValueIr::from_api(
                value
                    .bound_as_finite_range_bound()
                    .and_then(|bound| bound.value())
                    .ok_or_else(|| {
                        invalid(
                            "missing_value",
                            Some(path),
                            "finite range bound requires a value",
                        )
                    })?,
                format!("{path}.value"),
            )?))
        }
        api::RangeBoundKind::NegativeInfinityBound => Ok(QueryRangeBoundIr::NegativeInfinity),
        api::RangeBoundKind::PositiveInfinityBound => Ok(QueryRangeBoundIr::PositiveInfinity),
        api::RangeBoundKind::UnboundedRangeBound => Ok(QueryRangeBoundIr::Unbounded),
        _ => Err(invalid(
            "invalid_query",
            Some(path),
            "range bound must select a kind",
        )),
    }
}
pub(crate) fn weighted_query_items_to_ir(
    values: Option<
        flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<api::WeightedQueryItem<'_>>>,
    >,
    path: &str,
) -> Result<Vec<WeightedQueryItemIr>> {
    values
        .map(|values| {
            values
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    Ok(WeightedQueryItemIr {
                        key: RequestValueIr::from_api(
                            value.key().ok_or_else(|| {
                                invalid(
                                    "missing_value",
                                    Some(format!("{path}.items[{index}].key")),
                                    "weighted query item requires a key",
                                )
                            })?,
                            format!("{path}.items[{index}].key"),
                        )?,
                        weight: value.weight(),
                    })
                })
                .collect()
        })
        .transpose()
        .map(|value| value.unwrap_or_default())
}

pub(crate) fn query_value_object_to_ir(
    value: Option<api::ValueObject<'_>>,
    path: &str,
) -> Result<BTreeMap<String, RequestValueIr>> {
    let mut output = BTreeMap::new();
    if let Some(value) = value {
        if let Some(fields) = value.fields() {
            for (index, entry) in fields.iter().enumerate() {
                let key = entry.key().to_owned();
                let value = RequestValueIr::from_api(
                    entry.value().ok_or_else(|| {
                        invalid(
                            "missing_value",
                            Some(format!("{path}.fields[{index}].value")),
                            "predicate feature value is required",
                        )
                    })?,
                    format!("{path}.fields[{index}].value"),
                )?;
                if output.insert(key.clone(), value).is_some() {
                    return Err(invalid(
                        "duplicate_property",
                        Some(format!("{path}.fields[{index}].key")),
                        format!("predicate feature {key} occurs more than once"),
                    ));
                }
            }
        }
    }
    Ok(output)
}
