//! Lower typed query expressions into the Vespa query model.

mod annotations;
mod geo;
mod nearest_neighbor;
mod predicate;
mod text;
mod weighted;

use super::filter::{lower_attribute_value, lower_in_value};
use crate::unsupported;
use searchkernel_ir::{
    PortableError, QueryExprIr, QueryExprKindIr, QueryFieldRefIr, QueryIr, QueryRangeBoundIr,
    RequestValueIr, Result, ScalarValueIr,
};
use searchkernel_vespa_engine::{
    Query, QueryAlternative, SearchRequest, Value as VespaValue, WandOptions,
};

use annotations::annotation_u32_list;
use geo::{lower_geo_bounding_box, lower_geo_location};
use nearest_neighbor::lower_nearest_neighbor;
use predicate::lower_predicate;
use text::{lower_typed_phrase, lower_user_query, typed_term_text};
use weighted::{lower_wand_options, lower_weighted_items};

const DEFAULT_WEAK_AND_TARGET_HITS: u32 = 100;

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
        QueryIr::Yql { source, .. } => SearchRequest::from_yql(source)
            .map(|request| request.query)
            .map_err(|error| {
                PortableError::invalid("invalid_yql", Some("query"), error.to_string())
            }),
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
    match &expression.kind {
        QueryExprKindIr::Term { value } => {
            let field = typed_query_field(field_context, path)?;
            Ok(Query::term(field, typed_term_text(value, path)?))
        }
        QueryExprKindIr::Contains { field, item } => {
            let field =
                qualify_query_field(field, field_context, &format!("{path}.contains.field"))?;
            lower_typed_query(
                item,
                Some(&field),
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
                    lower_in_value(&typed_scalar(
                        value,
                        &format!("{path}.in.values[{index}]"),
                    )?)
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
        QueryExprKindIr::Matches { field, pattern } => {
            let field =
                qualify_query_field(field, field_context, &format!("{path}.matches.field"))?;
            Ok(Query::Matches {
                field: typed_query_field(Some(&field), &format!("{path}.matches.field"))?
                    .to_owned(),
                pattern: pattern.clone(),
            })
        }
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
        QueryExprKindIr::NonEmpty(query) => {
            let lowered = lower_typed_query(
                query,
                field_context,
                language,
                default_index,
                query_string,
                rank_properties,
                &format!("{path}.non_empty.query"),
            )?;
            let is_empty = matches!(
                &lowered,
                Query::And(children) | Query::Or(children) if children.is_empty()
            ) || matches!(&lowered, Query::Not(child) if child.as_ref() == &Query::True);
            if is_empty {
                return Err(PortableError::invalid(
                    "empty_typed_query",
                    Some(path),
                    "NonEmpty requires a non-empty child query",
                ));
            }
            Ok(lowered)
        }
        QueryExprKindIr::SameElement(children) => {
            let element_filter =
                annotation_u32_list(&expression.annotations, "elementFilter", path)?;
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
                element_filter,
            })
        }

        QueryExprKindIr::GeoLocation {
            field,
            latitude,
            longitude,
            radius,
        } => lower_geo_location(field, *latitude, *longitude, radius, path),
        QueryExprKindIr::GeoBoundingBox {
            field,
            south,
            west,
            north,
            east,
        } => lower_geo_bounding_box(field, *south, *west, *north, *east, path),
        QueryExprKindIr::NearestNeighbor {
            document_field,
            query_tensor,
        } => lower_nearest_neighbor(document_field, query_tensor, &expression.annotations, path),
        QueryExprKindIr::Predicate {
            field,
            features,
            range_features,
        } => lower_predicate(field, features, range_features, path),

        QueryExprKindIr::UserInput {
            value,
            default_index: query_default_index,
            target_hits,
        } => lower_user_query(
            value,
            field_context,
            query_default_index.as_deref().or(default_index),
            *target_hits,
            path,
        ),
        QueryExprKindIr::UserQuery {
            default_index: query_default_index,
            target_hits,
        } => {
            let value = query_string.ok_or_else(|| {
                PortableError::invalid(
                    "missing_query_string",
                    Some(path),
                    "userQuery requires QueryModel.query_string",
                )
            })?;
            lower_user_query(
                value,
                field_context,
                query_default_index.as_deref().or(default_index),
                *target_hits,
                path,
            )
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

pub(super) fn typed_query_field<'a>(
    field: Option<&'a QueryFieldRefIr>,
    path: &str,
) -> Result<&'a str> {
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

fn qualify_query_field(
    field: &QueryFieldRefIr,
    field_context: Option<&QueryFieldRefIr>,
    path: &str,
) -> Result<QueryFieldRefIr> {
    if field.path.as_str().contains('.')
        || field_context.is_none()
        || field_context.is_some_and(|context| context.path == field.path)
    {
        typed_query_field(Some(field), path)?;
        return Ok(field.clone());
    }
    let context = field_context.expect("checked above");
    let path = searchkernel_ir::FieldName::parse_query_path(format!(
        "{}.{}",
        context.path.as_str(),
        field.path.as_str()
    ))?;
    let qualified = QueryFieldRefIr {
        path: path.clone(),
        element_index: field.element_index,
    };
    typed_query_field(Some(&qualified), qualified.path.as_str())?;
    Ok(qualified)
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
