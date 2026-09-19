//! Structural validation for typed query trees.

use super::{Query, Value, WeightedQueryItem, MAX_QUERY_DEPTH, MAX_QUERY_NODES};
use crate::{Error, Result};

pub(crate) fn validate_query(query: &Query) -> Result<()> {
    let mut nodes = 0;
    validate_query_at(query, 0, &mut nodes)
}

fn validate_query_at(query: &Query, depth: usize, nodes: &mut usize) -> Result<()> {
    if depth > MAX_QUERY_DEPTH {
        return Err(invalid_query(format!(
            "query nesting exceeds maximum depth {MAX_QUERY_DEPTH}"
        )));
    }
    *nodes = nodes
        .checked_add(1)
        .ok_or_else(|| invalid_query("query node count overflowed"))?;
    if *nodes > MAX_QUERY_NODES {
        return Err(invalid_query(format!(
            "query contains more than {MAX_QUERY_NODES} nodes"
        )));
    }
    match query {
        Query::True => {}
        Query::Term { field, text } | Query::Phrase { field, text } => {
            if field.is_empty() || text.is_empty() {
                return Err(invalid_query("text query field and text must not be empty"));
            }
        }
        Query::And(children) | Query::Or(children) | Query::WeakAnd { children, .. } => {
            if children.is_empty() {
                return Err(invalid_query(
                    "boolean query must contain at least one child",
                ));
            }
            if children.len() > MAX_QUERY_NODES {
                return Err(invalid_query(format!(
                    "boolean query contains more than {MAX_QUERY_NODES} children"
                )));
            }
            for child in children {
                validate_query_at(child, depth + 1, nodes)?;
            }
        }
        Query::Equiv(children) => {
            if children.len() < 2 {
                return Err(invalid_query("equiv query requires at least two children"));
            }
            if children.len() > MAX_QUERY_NODES {
                return Err(invalid_query(format!(
                    "equiv query contains more than {MAX_QUERY_NODES} children"
                )));
            }
            for child in children {
                validate_query_at(child, depth + 1, nodes)?;
            }
        }
        Query::Near {
            field,
            children,
            distance,
        }
        | Query::ONear {
            field,
            children,
            distance,
        } => {
            if field.is_empty() {
                return Err(invalid_query("proximity query field must not be empty"));
            }
            if *distance == 0 {
                return Err(invalid_query("proximity query distance must be positive"));
            }
            if children.len() < 2 {
                return Err(invalid_query(
                    "proximity query requires at least two children",
                ));
            }
            for child in children {
                validate_query_at(child, depth + 1, nodes)?;
            }
        }
        Query::Alternatives {
            field,
            alternatives,
        } => {
            if field.is_empty() {
                return Err(invalid_query("alternatives query field must not be empty"));
            }
            if alternatives.is_empty() {
                return Err(invalid_query(
                    "alternatives query requires at least one alternative",
                ));
            }
            for alternative in alternatives {
                if alternative.text.is_empty() {
                    return Err(invalid_query("alternative text must not be empty"));
                }
                if !alternative.exactness.is_finite()
                    || !(0.0..=1.0).contains(&alternative.exactness)
                {
                    return Err(invalid_query(
                        "alternative exactness must be finite and between zero and one",
                    ));
                }
            }
        }
        Query::Matches { field, pattern } => {
            if field.is_empty() || pattern.is_empty() {
                return Err(invalid_query(
                    "matches query field and pattern must not be empty",
                ));
            }
        }
        Query::Not(child) => validate_query_at(child, depth + 1, nodes)?,
        Query::Rank { matching, ranking } => {
            validate_query_at(matching, depth + 1, nodes)?;
            for child in ranking {
                validate_query_at(child, depth + 1, nodes)?;
            }
        }
        Query::Fuzzy {
            field,
            text,
            max_edit_distance,
            prefix_lock_length,
            ..
        } => {
            if field.is_empty() || text.is_empty() {
                return Err(invalid_query(
                    "fuzzy query field and text must not be empty",
                ));
            }
            if *max_edit_distance == 0 && *prefix_lock_length > 0 {
                return Err(invalid_query(
                    "fuzzy prefix lock length requires a positive edit distance",
                ));
            }
        }
        Query::WeightedSet { field, items }
        | Query::DotProduct { field, items }
        | Query::Wand { field, items } => {
            if field.is_empty() {
                return Err(invalid_query("weighted query field must not be empty"));
            }
            if items.is_empty() {
                return Err(invalid_query("weighted query requires at least one item"));
            }
            let first_kind = matches!(items.first(), Some(WeightedQueryItem::String { .. }));
            if items
                .iter()
                .any(|item| matches!(item, WeightedQueryItem::String { .. }) != first_kind)
            {
                return Err(invalid_query(
                    "weighted query cannot mix string and integer keys",
                ));
            }
        }
        Query::Labeled { query, score, .. } => {
            if !score.is_finite() {
                return Err(invalid_query("labeled query score must be finite"));
            }
            validate_query_at(query, depth + 1, nodes)?;
        }
        Query::Equals { field, .. } | Query::Exists { field } => {
            if field.is_empty() {
                return Err(invalid_query("numeric query field must not be empty"));
            }
        }
        Query::Range { field, .. } | Query::AttributeRange { field, .. } => {
            if field.is_empty() {
                return Err(invalid_query("range query field must not be empty"));
            }
        }
        Query::AttributeEquals { field, value } => {
            if field.is_empty() {
                return Err(invalid_query("attribute query field must not be empty"));
            }
            validate_value(value)?;
        }
        Query::NearestNeighbor {
            document_field,
            query_tensor,
            target_num_hits,
            ..
        } => {
            if document_field.is_empty() || query_tensor.is_empty() {
                return Err(invalid_query(
                    "nearest-neighbor field and query tensor must not be empty",
                ));
            }
            if *target_num_hits == 0 {
                return Err(invalid_query(
                    "nearest-neighbor target hits must be positive",
                ));
            }
        }
        Query::SameElement { field, children } => {
            if field.is_empty() {
                return Err(invalid_query("same-element query field must not be empty"));
            }
            if children.len() < 2 {
                return Err(invalid_query(
                    "same-element query requires at least two children",
                ));
            }
            for child in children {
                validate_query_at(child, depth + 1, nodes)?;
            }
        }
        Query::GeoLocation {
            field,
            latitude,
            longitude,
            radius,
        } => {
            if field.is_empty() || radius.is_empty() {
                return Err(invalid_query(
                    "geo-location field and radius must not be empty",
                ));
            }
            if !latitude.is_finite() || !longitude.is_finite() {
                return Err(invalid_query("geo-location coordinates must be finite"));
            }
        }
        Query::GeoBoundingBox {
            field,
            south,
            west,
            north,
            east,
        } => {
            if field.is_empty() {
                return Err(invalid_query("geo bounding-box field must not be empty"));
            }
            if [south, west, north, east]
                .iter()
                .any(|value| !value.is_finite())
            {
                return Err(invalid_query("geo bounding-box coordinates must be finite"));
            }
        }
        Query::Predicate {
            field,
            features,
            range_features,
        } => {
            if field.is_empty() {
                return Err(invalid_query("predicate query field must not be empty"));
            }
            if features.is_empty() && range_features.is_empty() {
                return Err(invalid_query(
                    "predicate query requires at least one feature",
                ));
            }
            if features.iter().any(|feature| feature.key.is_empty())
                || range_features.iter().any(|feature| feature.key.is_empty())
            {
                return Err(invalid_query("predicate feature keys must not be empty"));
            }
        }
    }
    Ok(())
}

fn validate_value(value: &Value) -> Result<()> {
    if let Value::Float(value) = value {
        if !value.is_finite() {
            return Err(invalid_query("attribute float value must be finite"));
        }
    }
    Ok(())
}

pub(crate) fn invalid_query(message: impl Into<String>) -> Error {
    Error::message(format!("invalid_query: {}", message.into()))
}
