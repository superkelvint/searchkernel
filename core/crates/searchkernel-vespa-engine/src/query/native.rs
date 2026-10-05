//! Native query lowering and field validation.

use super::{
    invalid_query, AttributeBound, InQueryValue, NearestNeighborOptions, PredicateFeature,
    PredicateRangeFeature, Query, QueryAlternative, RangeBound, Value, WandOptions,
    WeightedQueryItem,
};
use crate::error::{check, cstring};
use crate::linguistics::{Linguistics, LinguisticsOptions};
use crate::native::{
    length_u32, nearest_neighbor_options, text_query_terms, validate_term_query, NativeError,
    NativeQuery, NativeQueryTerms,
};
use crate::{Error, Result};
use searchkernel_vespa_bindings as sys;

pub(crate) fn lower_query(
    native: &NativeQuery,
    query: &Query,
    language: LinguisticsOptions,
) -> Result<(), Error> {
    match query {
        Query::True => {
            let mut error = NativeError::new();
            let status = unsafe {
                sys::sk_query_add_true(native.0.as_ptr(), error.as_mut_ptr(), error.capacity())
            };
            check(status, &error)
        }
        Query::Text {
            field,
            text,
            target_num_hits,
        } => {
            let analysis = Linguistics.analyze_query(text, language);
            let terms = text_query_terms(&analysis)?;
            if terms.term_lengths.is_empty() {
                return Err(invalid_query("text must analyze to at least one term"));
            }
            add_native_weak_and(native, terms.term_lengths.len(), *target_num_hits)?;
            let mut offset = 0usize;
            for term_length in terms.term_lengths.iter().copied() {
                let end = offset
                    + usize::try_from(term_length).map_err(|_| {
                        Error::message("text query term length cannot be represented")
                    })?;
                let single = NativeQueryTerms {
                    term_bytes: terms.term_bytes[offset..end].to_vec(),
                    term_lengths: vec![term_length],
                };
                add_native_text(native, sys::SK_QUERY_TEXT_TERM, field, &single)?;
                offset = end;
            }
            Ok(())
        }
        Query::Term { field, text } => {
            let analysis = Linguistics.analyze_query(text, language);
            validate_term_query(&analysis)
                .map_err(|error| invalid_query(format!("term analysis failed: {error}")))?;
            let terms = text_query_terms(&analysis)?;
            add_native_text(native, sys::SK_QUERY_TEXT_TERM, field, &terms)
        }
        Query::Phrase { field, text } => {
            let analysis = Linguistics.analyze_query(text, language);
            if analysis.terms.is_empty() {
                return Err(invalid_query("phrase must analyze to at least one term"));
            }
            let terms = text_query_terms(&analysis)?;
            add_native_text(native, sys::SK_QUERY_TEXT_PHRASE, field, &terms)
        }
        Query::And(children) => {
            add_native_operator(native, sys::SK_QUERY_NODE_AND, children.len())?;
            for child in children {
                lower_query(native, child, language)?;
            }
            Ok(())
        }
        Query::Or(children) => {
            add_native_operator(native, sys::SK_QUERY_NODE_OR, children.len())?;
            for child in children {
                lower_query(native, child, language)?;
            }
            Ok(())
        }
        Query::Not(child) => {
            add_native_operator(native, sys::SK_QUERY_NODE_NOT, 1)?;
            lower_query(native, child, language)
        }
        Query::Near {
            field,
            children,
            distance,
        } => {
            add_native_proximity(native, false, field, children.len(), *distance)?;
            for child in children {
                lower_query(native, child, language)?;
            }
            Ok(())
        }
        Query::ONear {
            field,
            children,
            distance,
        } => {
            add_native_proximity(native, true, field, children.len(), *distance)?;
            for child in children {
                lower_query(native, child, language)?;
            }
            Ok(())
        }
        Query::Equiv(children) => {
            add_native_equiv(native, children.len())?;
            for child in children {
                lower_query(native, child, language)?;
            }
            Ok(())
        }
        Query::Alternatives {
            field,
            alternatives,
        } => {
            add_native_alternatives(native, field, alternatives)?;
            Ok(())
        }
        Query::Matches { field, pattern } => add_native_matches(native, field, pattern),
        Query::WeakAnd {
            children,
            target_num_hits,
        } => {
            add_native_weak_and(native, children.len(), *target_num_hits)?;
            for child in children {
                lower_query(native, child, language)?;
            }
            Ok(())
        }
        Query::Rank { matching, ranking } => {
            add_native_rank(native, 1usize.saturating_add(ranking.len()))?;
            lower_query(native, matching, language)?;
            for child in ranking {
                lower_query(native, child, language)?;
            }
            Ok(())
        }
        Query::Fuzzy {
            field,
            text,
            max_edit_distance,
            prefix_lock_length,
            prefix_match,
        } => add_native_fuzzy(
            native,
            field,
            text,
            *max_edit_distance,
            *prefix_lock_length,
            *prefix_match,
        ),
        Query::WeightedSet { field, items } => add_native_weighted(native, 0, field, items),
        Query::DotProduct { field, items } => add_native_weighted(native, 2, field, items),
        Query::Wand { field, items } => add_native_weighted(native, 1, field, items),
        Query::WandWithOptions {
            field,
            items,
            options,
        } => add_native_weighted_with_options(native, field, items, *options),
        Query::In { field, values } => add_native_in(native, field, values),
        Query::Labeled {
            query,
            unique_id,
            score,
        } => {
            add_native_labeled(native, *unique_id, *score)?;
            lower_query(native, query, language)
        }
        Query::Equals { field, value } => {
            let field = cstring(field)?;
            let mut error = NativeError::new();
            let status = unsafe {
                sys::sk_query_add_equals(
                    native.0.as_ptr(),
                    field.as_ptr(),
                    *value,
                    error.as_mut_ptr(),
                    error.capacity(),
                )
            };
            check(status, &error)
        }
        Query::Range {
            field,
            lower,
            upper,
        } => {
            let field = cstring(field)?;
            let (lower_kind, lower_value) = native_bound(lower);
            let (upper_kind, upper_value) = native_bound(upper);
            let mut error = NativeError::new();
            let status = unsafe {
                sys::sk_query_add_range(
                    native.0.as_ptr(),
                    field.as_ptr(),
                    lower_kind,
                    lower_value,
                    upper_kind,
                    upper_value,
                    error.as_mut_ptr(),
                    error.capacity(),
                )
            };
            check(status, &error)
        }
        Query::Exists { field } => {
            let field = cstring(field)?;
            let mut error = NativeError::new();
            let status = unsafe {
                sys::sk_query_add_range(
                    native.0.as_ptr(),
                    field.as_ptr(),
                    sys::SK_QUERY_BOUND_UNBOUNDED,
                    0,
                    sys::SK_QUERY_BOUND_UNBOUNDED,
                    0,
                    error.as_mut_ptr(),
                    error.capacity(),
                )
            };
            check(status, &error)
        }
        Query::AttributeEquals { field, value } => {
            add_native_attribute_equals(native, field, value)
        }
        Query::AttributeRange {
            field,
            lower,
            upper,
        } => {
            let value_kind = attribute_range_kind(lower).or_else(|| attribute_range_kind(upper));
            match value_kind {
                None => {
                    let field = cstring(field)?;
                    let mut error = NativeError::new();
                    let status = unsafe {
                        sys::sk_query_add_attribute_exists(
                            native.0.as_ptr(),
                            field.as_ptr(),
                            error.as_mut_ptr(),
                            error.capacity(),
                        )
                    };
                    check(status, &error)
                }
                Some(AttributeKind::Integer) => {
                    let (lower_kind, lower_value) = integer_attribute_bound(lower)?;
                    let (upper_kind, upper_value) = integer_attribute_bound(upper)?;
                    let field = cstring(field)?;
                    let mut error = NativeError::new();
                    let status = unsafe {
                        sys::sk_query_add_attribute_integer_range(
                            native.0.as_ptr(),
                            field.as_ptr(),
                            lower_kind,
                            lower_value,
                            upper_kind,
                            upper_value,
                            error.as_mut_ptr(),
                            error.capacity(),
                        )
                    };
                    check(status, &error)
                }
                Some(AttributeKind::Float) => {
                    let (lower_kind, lower_value) = float_attribute_bound(lower)?;
                    let (upper_kind, upper_value) = float_attribute_bound(upper)?;
                    let field = cstring(field)?;
                    let mut error = NativeError::new();
                    let status = unsafe {
                        sys::sk_query_add_attribute_float_range(
                            native.0.as_ptr(),
                            field.as_ptr(),
                            lower_kind,
                            lower_value,
                            upper_kind,
                            upper_value,
                            error.as_mut_ptr(),
                            error.capacity(),
                        )
                    };
                    check(status, &error)
                }
                Some(AttributeKind::String | AttributeKind::Boolean) => {
                    Err(Error::message("unsupported attribute range field type"))
                }
            }
        }
        Query::NearestNeighbor {
            document_field,
            query_tensor,
            target_num_hits,
            approximate,
        } => {
            let document_field = cstring(document_field)?;
            let query_tensor = cstring(query_tensor)?;
            let mut error = NativeError::new();
            let status = unsafe {
                sys::sk_query_add_nearest_neighbor(
                    native.0.as_ptr(),
                    document_field.as_ptr(),
                    query_tensor.as_ptr(),
                    *target_num_hits,
                    u8::from(*approximate),
                    error.as_mut_ptr(),
                    error.capacity(),
                )
            };
            check(status, &error)
        }
        Query::NearestNeighborWithOptions {
            document_field,
            query_tensor,
            options,
        } => {
            add_native_nearest_neighbor_with_options(native, document_field, query_tensor, options)
        }
        Query::SameElement {
            field,
            children,
            element_filter,
        } => {
            add_native_same_element(native, field, children.len(), element_filter)?;
            for child in children {
                lower_same_element_child(native, child, field, language)?;
            }
            Ok(())
        }
        Query::GeoLocation {
            field,
            latitude,
            longitude,
            radius,
        } => add_native_geo_location(native, field, *latitude, *longitude, radius),
        Query::GeoBoundingBox {
            field,
            south,
            west,
            north,
            east,
        } => add_native_geo_bounding_box(native, field, *south, *west, *north, *east),
        Query::Predicate {
            field,
            features,
            range_features,
        } => add_native_predicate(native, field, features, range_features),
    }
}

fn relative_same_element_field(field: &str, parent: &str) -> Result<String, Error> {
    if field == parent {
        Ok(String::new())
    } else if let Some(member) = field
        .strip_prefix(parent)
        .and_then(|value| value.strip_prefix('.'))
    {
        Ok(member.to_owned())
    } else {
        Err(invalid_query(format!(
            "same-element child field '{field}' is outside parent field '{parent}'"
        )))
    }
}

fn relative_same_element_query(mut query: Query, parent: &str) -> Result<Query, Error> {
    match &mut query {
        Query::Text { field, .. }
        | Query::Term { field, .. }
        | Query::Phrase { field, .. }
        | Query::Alternatives { field, .. }
        | Query::Matches { field, .. }
        | Query::Fuzzy { field, .. } => {
            *field = relative_same_element_field(field, parent)?;
        }
        Query::Near {
            field, children, ..
        }
        | Query::ONear {
            field, children, ..
        } => {
            *field = relative_same_element_field(field, parent)?;
            let nested = std::mem::take(children)
                .into_iter()
                .map(|child| relative_same_element_query(child, parent))
                .collect::<Result<Vec<_>, _>>()?;
            *children = nested;
        }
        Query::And(children)
        | Query::Or(children)
        | Query::Equiv(children)
        | Query::WeakAnd { children, .. } => {
            let nested = std::mem::take(children)
                .into_iter()
                .map(|child| relative_same_element_query(child, parent))
                .collect::<Result<Vec<_>, _>>()?;
            *children = nested;
        }
        Query::Not(nested) | Query::Labeled { query: nested, .. } => {
            **nested = relative_same_element_query((**nested).clone(), parent)?;
        }
        Query::Rank { matching, ranking } => {
            **matching = relative_same_element_query((**matching).clone(), parent)?;
            let nested = std::mem::take(ranking)
                .into_iter()
                .map(|child| relative_same_element_query(child, parent))
                .collect::<Result<Vec<_>, _>>()?;
            *ranking = nested;
        }
        _ => {
            return Err(invalid_query(
                "same-element children must be text query nodes",
            ));
        }
    }
    Ok(query)
}

fn lower_same_element_child(
    native: &NativeQuery,
    child: &Query,
    parent: &str,
    language: LinguisticsOptions,
) -> Result<(), Error> {
    let child = relative_same_element_query(child.clone(), parent)?;
    lower_query(native, &child, language)
}

pub(crate) fn validate_query_fields(native: &NativeQuery, query: &Query) -> Result<(), Error> {
    match query {
        Query::True => Ok(()),
        Query::Text { field, .. } => validate_native_text_field(native, field),
        Query::Term { field, .. } | Query::Phrase { field, .. } => {
            validate_native_text_field(native, field)
        }
        Query::Alternatives { field, .. } => validate_native_text_field(native, field),
        Query::Matches { field, .. } => {
            validate_native_field(native, sys::SK_QUERY_FIELD_TEXT, field)
                .or_else(|_| validate_native_field(native, sys::SK_QUERY_FIELD_STRING, field))
        }
        Query::And(children) | Query::Or(children) => {
            for child in children {
                validate_query_fields(native, child)?;
            }
            Ok(())
        }
        Query::Not(child) => validate_query_fields(native, child),
        Query::Near {
            field, children, ..
        }
        | Query::ONear {
            field, children, ..
        } => {
            validate_native_text_field(native, field)?;
            for child in children {
                validate_query_fields(native, child)?;
            }
            Ok(())
        }
        Query::Equiv(children) => {
            for child in children {
                validate_query_fields(native, child)?;
            }
            Ok(())
        }
        Query::WeakAnd { children, .. } => {
            for child in children {
                validate_query_fields(native, child)?;
            }
            Ok(())
        }
        Query::Rank { matching, ranking } => {
            validate_query_fields(native, matching)?;
            for child in ranking {
                validate_query_fields(native, child)?;
            }
            Ok(())
        }
        Query::Fuzzy { field, .. } => validate_native_text_field(native, field)
            .or_else(|_| validate_native_field(native, sys::SK_QUERY_FIELD_STRING, field)),
        Query::WeightedSet { field, .. }
        | Query::DotProduct { field, .. }
        | Query::Wand { field, .. }
        | Query::WandWithOptions { field, .. } => {
            validate_native_field(native, sys::SK_QUERY_FIELD_ATTRIBUTE, field)
                .or_else(|_| validate_native_field(native, sys::SK_QUERY_FIELD_TEXT, field))
        }
        Query::Labeled { query, .. } => validate_query_fields(native, query),
        Query::Equals { field, .. } | Query::Range { field, .. } | Query::Exists { field } => {
            validate_native_field(native, sys::SK_QUERY_FIELD_INTEGER, field)
        }
        Query::AttributeEquals { field, value } => validate_native_field(
            native,
            match value {
                Value::String(_) => sys::SK_QUERY_FIELD_STRING,
                Value::Integer(_) => sys::SK_QUERY_FIELD_INTEGER,
                Value::Float(_) => sys::SK_QUERY_FIELD_FLOAT,
                Value::Boolean(_) => sys::SK_QUERY_FIELD_BOOLEAN,
            },
            field,
        ),
        Query::AttributeRange {
            field,
            lower,
            upper,
        } => {
            let kind = attribute_range_kind(lower).or_else(|| attribute_range_kind(upper));
            if let Some(kind) = kind {
                validate_native_field(
                    native,
                    match kind {
                        AttributeKind::Integer => sys::SK_QUERY_FIELD_INTEGER,
                        AttributeKind::Float => sys::SK_QUERY_FIELD_FLOAT,
                        AttributeKind::String => sys::SK_QUERY_FIELD_STRING,
                        AttributeKind::Boolean => sys::SK_QUERY_FIELD_BOOLEAN,
                    },
                    field,
                )
            } else {
                validate_native_field(native, sys::SK_QUERY_FIELD_ATTRIBUTE, field)
            }
        }
        Query::In { field, values } => match values.first() {
            Some(InQueryValue::String(_)) => {
                // The InTerm boundary checks scalar/array string type after
                // this shared attribute check; other string operators retain
                // their scalar-only guard.
                validate_native_field(native, sys::SK_QUERY_FIELD_ATTRIBUTE, field)
            }
            Some(InQueryValue::Integer(_)) => {
                validate_native_field(native, sys::SK_QUERY_FIELD_INTEGER, field)
            }
            None => Err(invalid_query("in query requires at least one value")),
        },
        Query::NearestNeighbor { document_field, .. }
        | Query::NearestNeighborWithOptions { document_field, .. } => {
            validate_native_field(native, sys::SK_QUERY_FIELD_TENSOR, document_field)
        }
        Query::SameElement {
            field, children, ..
        } => {
            validate_native_field(native, sys::SK_QUERY_FIELD_ARRAY, field)?;
            for child in children {
                validate_query_fields(native, child)?;
            }
            Ok(())
        }
        Query::GeoLocation { field, .. } | Query::GeoBoundingBox { field, .. } => {
            validate_native_field(native, sys::SK_QUERY_FIELD_POSITION, field)
        }
        Query::Predicate { field, .. } => {
            validate_native_field(native, sys::SK_QUERY_FIELD_PREDICATE, field)
        }
    }
}

fn validate_native_text_field(native: &NativeQuery, field: &str) -> Result<(), Error> {
    match validate_native_field(native, sys::SK_QUERY_FIELD_TEXT, field) {
        Ok(()) => Ok(()),
        Err(text_error) if text_error.to_string().contains("unknown") => Err(text_error),
        Err(_) => {
            validate_native_field(native, sys::SK_QUERY_FIELD_ARRAY_STRING, field).map_err(|_| {
                invalid_query(format!(
                    "text operation requires an indexed text field: {field}"
                ))
            })
        }
    }
}

#[derive(Clone, Copy)]
enum AttributeKind {
    String,
    Integer,
    Float,
    Boolean,
}

fn attribute_range_kind(bound: &AttributeBound) -> Option<AttributeKind> {
    match bound {
        AttributeBound::Unbounded => None,
        AttributeBound::Included(Value::String(_)) | AttributeBound::Excluded(Value::String(_)) => {
            Some(AttributeKind::String)
        }
        AttributeBound::Included(Value::Integer(_))
        | AttributeBound::Excluded(Value::Integer(_)) => Some(AttributeKind::Integer),
        AttributeBound::Included(Value::Float(_)) | AttributeBound::Excluded(Value::Float(_)) => {
            Some(AttributeKind::Float)
        }
        AttributeBound::Included(Value::Boolean(_))
        | AttributeBound::Excluded(Value::Boolean(_)) => Some(AttributeKind::Boolean),
    }
}

fn integer_attribute_bound(bound: &AttributeBound) -> Result<(u32, i64), Error> {
    match bound {
        AttributeBound::Unbounded => Ok((sys::SK_QUERY_BOUND_UNBOUNDED, 0)),
        AttributeBound::Included(Value::Integer(value)) => {
            Ok((sys::SK_QUERY_BOUND_INCLUDED, *value))
        }
        AttributeBound::Excluded(Value::Integer(value)) => {
            Ok((sys::SK_QUERY_BOUND_EXCLUDED, *value))
        }
        _ => Err(Error::message(
            "wrong value type: attribute range bounds must have matching integer types",
        )),
    }
}

fn float_attribute_bound(bound: &AttributeBound) -> Result<(u32, f64), Error> {
    match bound {
        AttributeBound::Unbounded => Ok((sys::SK_QUERY_BOUND_UNBOUNDED, 0.0)),
        AttributeBound::Included(Value::Float(value)) => {
            if !value.is_finite() {
                return Err(Error::message("attribute float bound must be finite"));
            }
            Ok((sys::SK_QUERY_BOUND_INCLUDED, *value))
        }
        AttributeBound::Excluded(Value::Float(value)) => {
            if !value.is_finite() {
                return Err(Error::message("attribute float bound must be finite"));
            }
            Ok((sys::SK_QUERY_BOUND_EXCLUDED, *value))
        }
        _ => Err(Error::message(
            "wrong value type: attribute range bounds must have matching float types",
        )),
    }
}

fn add_native_attribute_equals(
    native: &NativeQuery,
    field: &str,
    value: &Value,
) -> Result<(), Error> {
    let field = cstring(field)?;
    let mut error = NativeError::new();
    let status = unsafe {
        match value {
            Value::String(value) => {
                let value = cstring(value)?;
                sys::sk_query_add_attribute_string_equals(
                    native.0.as_ptr(),
                    field.as_ptr(),
                    value.as_ptr(),
                    error.as_mut_ptr(),
                    error.capacity(),
                )
            }
            Value::Integer(value) => sys::sk_query_add_equals(
                native.0.as_ptr(),
                field.as_ptr(),
                *value,
                error.as_mut_ptr(),
                error.capacity(),
            ),
            Value::Float(value) => sys::sk_query_add_attribute_float_equals(
                native.0.as_ptr(),
                field.as_ptr(),
                *value,
                error.as_mut_ptr(),
                error.capacity(),
            ),
            Value::Boolean(value) => sys::sk_query_add_attribute_boolean_equals(
                native.0.as_ptr(),
                field.as_ptr(),
                u8::from(*value),
                error.as_mut_ptr(),
                error.capacity(),
            ),
        }
    };
    check(status, &error)
}

fn validate_native_field(native: &NativeQuery, kind: u32, field: &str) -> Result<(), Error> {
    let field = cstring(field)?;
    let mut error = NativeError::new();
    let status = unsafe {
        sys::sk_query_validate_field(
            native.0.as_ptr(),
            kind,
            field.as_ptr(),
            error.as_mut_ptr(),
            error.capacity(),
        )
    };
    check(status, &error)
}

fn add_native_operator(native: &NativeQuery, kind: u32, count: usize) -> Result<(), Error> {
    let count = length_u32(count, "query child")?;
    let mut error = NativeError::new();
    let status = unsafe {
        sys::sk_query_add_operator(
            native.0.as_ptr(),
            kind,
            count,
            error.as_mut_ptr(),
            error.capacity(),
        )
    };
    check(status, &error)
}

fn add_native_proximity(
    native: &NativeQuery,
    ordered: bool,
    field: &str,
    child_count: usize,
    distance: u32,
) -> Result<(), Error> {
    let field = cstring(field)?;
    let child_count = length_u32(child_count, "proximity child")?;
    let mut error = NativeError::new();
    let status = unsafe {
        sys::sk_query_add_proximity(
            native.0.as_ptr(),
            u8::from(ordered),
            field.as_ptr(),
            child_count,
            distance,
            error.as_mut_ptr(),
            error.capacity(),
        )
    };
    check(status, &error)
}

fn add_native_equiv(native: &NativeQuery, child_count: usize) -> Result<(), Error> {
    if child_count < 2 {
        return Err(invalid_query("equiv query requires at least two children"));
    }
    let child_count = length_u32(child_count, "equiv child")?;
    let mut error = NativeError::new();
    let status = unsafe {
        sys::sk_query_add_equiv(
            native.0.as_ptr(),
            child_count,
            error.as_mut_ptr(),
            error.capacity(),
        )
    };
    check(status, &error)
}

fn add_native_weak_and(
    native: &NativeQuery,
    child_count: usize,
    target_num_hits: u32,
) -> Result<(), Error> {
    // The native ABI retains its historical field argument, but WeakAnd's
    // field is determined by its children and may not be shared by them.
    let field = cstring("")?;
    let child_count = length_u32(child_count, "weak-and child")?;
    let mut error = NativeError::new();
    let status = unsafe {
        sys::sk_query_add_weak_and(
            native.0.as_ptr(),
            field.as_ptr(),
            child_count,
            target_num_hits,
            error.as_mut_ptr(),
            error.capacity(),
        )
    };
    check(status, &error)
}

fn add_native_rank(native: &NativeQuery, child_count: usize) -> Result<(), Error> {
    let child_count = length_u32(child_count, "rank child")?;
    let mut error = NativeError::new();
    let status = unsafe {
        sys::sk_query_add_rank(
            native.0.as_ptr(),
            child_count,
            error.as_mut_ptr(),
            error.capacity(),
        )
    };
    check(status, &error)
}

fn add_native_fuzzy(
    native: &NativeQuery,
    field: &str,
    text: &str,
    max_edit_distance: u32,
    prefix_lock_length: u32,
    prefix_match: bool,
) -> Result<(), Error> {
    let field = cstring(field)?;
    let text = cstring(text)?;
    let mut error = NativeError::new();
    let status = unsafe {
        sys::sk_query_add_fuzzy(
            native.0.as_ptr(),
            field.as_ptr(),
            text.as_ptr(),
            max_edit_distance,
            prefix_lock_length,
            u8::from(prefix_match),
            error.as_mut_ptr(),
            error.capacity(),
        )
    };
    check(status, &error)
}

fn add_native_alternatives(
    native: &NativeQuery,
    field: &str,
    alternatives: &[QueryAlternative],
) -> Result<(), Error> {
    let field = cstring(field)?;
    let mut key_bytes = Vec::new();
    let mut key_lengths = Vec::with_capacity(alternatives.len());
    let mut weights = Vec::with_capacity(alternatives.len());
    for alternative in alternatives {
        key_lengths.push(length_u32(alternative.text.len(), "alternative text")?);
        key_bytes.extend_from_slice(alternative.text.as_bytes());
        let weight = (alternative.exactness * 100.0).round();
        if !weight.is_finite() || weight < i32::MIN as f64 || weight > i32::MAX as f64 {
            return Err(invalid_query(
                "alternative exactness cannot be represented by Vespa",
            ));
        }
        weights.push(weight as i64);
    }
    let mut error = NativeError::new();
    let status = unsafe {
        sys::sk_query_add_alternatives(
            native.0.as_ptr(),
            field.as_ptr(),
            key_bytes.as_ptr(),
            length_u32(key_bytes.len(), "alternative text bytes")?,
            key_lengths.as_ptr(),
            weights.as_ptr(),
            length_u32(alternatives.len(), "alternative")?,
            error.as_mut_ptr(),
            error.capacity(),
        )
    };
    check(status, &error)
}

fn add_native_matches(native: &NativeQuery, field: &str, pattern: &str) -> Result<(), Error> {
    let field = cstring(field)?;
    let pattern = cstring(pattern)?;
    let mut error = NativeError::new();
    let status = unsafe {
        sys::sk_query_add_matches(
            native.0.as_ptr(),
            field.as_ptr(),
            pattern.as_ptr(),
            error.as_mut_ptr(),
            error.capacity(),
        )
    };
    check(status, &error)
}

fn add_native_labeled(native: &NativeQuery, unique_id: i32, score: f64) -> Result<(), Error> {
    let mut error = NativeError::new();
    let status = unsafe {
        sys::sk_query_add_labeled(
            native.0.as_ptr(),
            unique_id,
            score,
            error.as_mut_ptr(),
            error.capacity(),
        )
    };
    check(status, &error)
}

fn add_native_weighted(
    native: &NativeQuery,
    kind: u32,
    field: &str,
    items: &[WeightedQueryItem],
) -> Result<(), Error> {
    let field = cstring(field)?;
    add_native_weighted_impl(native, kind, &field, items, None)
}

fn add_native_weighted_with_options(
    native: &NativeQuery,
    field: &str,
    items: &[WeightedQueryItem],
    options: WandOptions,
) -> Result<(), Error> {
    let field = cstring(field)?;
    add_native_weighted_impl(native, 1, &field, items, Some(options))
}

fn add_native_weighted_impl(
    native: &NativeQuery,
    kind: u32,
    field: &std::ffi::CString,
    items: &[WeightedQueryItem],
    wand_options: Option<WandOptions>,
) -> Result<(), Error> {
    let mut error = NativeError::new();
    let status = match items.first() {
        Some(WeightedQueryItem::String { .. }) => {
            let mut key_bytes = Vec::new();
            let mut key_lengths = Vec::with_capacity(items.len());
            let mut weights = Vec::with_capacity(items.len());
            for item in items {
                let WeightedQueryItem::String { key, weight } = item else {
                    return Err(invalid_query(
                        "weighted query cannot mix string and integer keys",
                    ));
                };
                key_lengths.push(length_u32(key.len(), "weighted query key")?);
                key_bytes.extend_from_slice(key.as_bytes());
                weights.push(*weight);
            }
            let item_count = length_u32(items.len(), "weighted query item")?;
            unsafe {
                if kind == 1 {
                    if let Some(options) = wand_options {
                        sys::sk_query_add_wand_strings_with_options(
                            native.0.as_ptr(),
                            field.as_ptr(),
                            key_bytes.as_ptr(),
                            length_u32(key_bytes.len(), "weighted query key bytes")?,
                            key_lengths.as_ptr(),
                            weights.as_ptr(),
                            item_count,
                            options.target_num_hits,
                            options.score_threshold,
                            options.threshold_boost_factor,
                            error.as_mut_ptr(),
                            error.capacity(),
                        )
                    } else {
                        sys::sk_query_add_wand_strings(
                            native.0.as_ptr(),
                            field.as_ptr(),
                            key_bytes.as_ptr(),
                            length_u32(key_bytes.len(), "weighted query key bytes")?,
                            key_lengths.as_ptr(),
                            weights.as_ptr(),
                            item_count,
                            error.as_mut_ptr(),
                            error.capacity(),
                        )
                    }
                } else if kind == 2 {
                    sys::sk_query_add_dot_product_strings(
                        native.0.as_ptr(),
                        field.as_ptr(),
                        key_bytes.as_ptr(),
                        length_u32(key_bytes.len(), "weighted query key bytes")?,
                        key_lengths.as_ptr(),
                        weights.as_ptr(),
                        item_count,
                        error.as_mut_ptr(),
                        error.capacity(),
                    )
                } else {
                    sys::sk_query_add_weighted_set_strings(
                        native.0.as_ptr(),
                        field.as_ptr(),
                        key_bytes.as_ptr(),
                        length_u32(key_bytes.len(), "weighted query key bytes")?,
                        key_lengths.as_ptr(),
                        weights.as_ptr(),
                        item_count,
                        error.as_mut_ptr(),
                        error.capacity(),
                    )
                }
            }
        }
        Some(WeightedQueryItem::Integer { .. }) => {
            let mut keys = Vec::with_capacity(items.len());
            let mut weights = Vec::with_capacity(items.len());
            for item in items {
                let WeightedQueryItem::Integer { key, weight } = item else {
                    return Err(invalid_query(
                        "weighted query cannot mix string and integer keys",
                    ));
                };
                keys.push(*key);
                weights.push(*weight);
            }
            let item_count = length_u32(items.len(), "weighted query item")?;
            unsafe {
                if kind == 1 {
                    if let Some(options) = wand_options {
                        sys::sk_query_add_wand_integers_with_options(
                            native.0.as_ptr(),
                            field.as_ptr(),
                            keys.as_ptr(),
                            weights.as_ptr(),
                            item_count,
                            options.target_num_hits,
                            options.score_threshold,
                            options.threshold_boost_factor,
                            error.as_mut_ptr(),
                            error.capacity(),
                        )
                    } else {
                        sys::sk_query_add_wand_integers(
                            native.0.as_ptr(),
                            field.as_ptr(),
                            keys.as_ptr(),
                            weights.as_ptr(),
                            item_count,
                            error.as_mut_ptr(),
                            error.capacity(),
                        )
                    }
                } else if kind == 2 {
                    sys::sk_query_add_dot_product_integers(
                        native.0.as_ptr(),
                        field.as_ptr(),
                        keys.as_ptr(),
                        weights.as_ptr(),
                        item_count,
                        error.as_mut_ptr(),
                        error.capacity(),
                    )
                } else {
                    sys::sk_query_add_weighted_set_integers(
                        native.0.as_ptr(),
                        field.as_ptr(),
                        keys.as_ptr(),
                        weights.as_ptr(),
                        item_count,
                        error.as_mut_ptr(),
                        error.capacity(),
                    )
                }
            }
        }
        None => return Err(invalid_query("weighted query requires at least one item")),
    };
    check(status, &error)
}

fn add_native_text(
    native: &NativeQuery,
    kind: u32,
    field: &str,
    terms: &NativeQueryTerms,
) -> Result<(), Error> {
    let field = cstring(field)?;
    let mut error = NativeError::new();
    let status = unsafe {
        sys::sk_query_add_text(
            native.0.as_ptr(),
            kind,
            field.as_ptr(),
            terms.term_bytes.as_ptr(),
            length_u32(terms.term_bytes.len(), "query text term bytes")?,
            terms.term_lengths.as_ptr(),
            length_u32(terms.term_lengths.len(), "query text term")?,
            error.as_mut_ptr(),
            error.capacity(),
        )
    };
    check(status, &error)
}

fn add_native_in(native: &NativeQuery, field: &str, values: &[InQueryValue]) -> Result<(), Error> {
    let field = cstring(field)?;
    let mut error = NativeError::new();
    let status = match values.first() {
        Some(InQueryValue::String(_)) => {
            let mut bytes = Vec::new();
            let mut lengths = Vec::with_capacity(values.len());
            for value in values {
                let InQueryValue::String(value) = value else {
                    return Err(invalid_query(
                        "in query cannot mix string and integer values",
                    ));
                };
                lengths.push(length_u32(value.len(), "in query value")?);
                bytes.extend_from_slice(value.as_bytes());
            }
            unsafe {
                sys::sk_query_add_in_strings(
                    native.0.as_ptr(),
                    field.as_ptr(),
                    bytes.as_ptr(),
                    length_u32(bytes.len(), "in query value bytes")?,
                    lengths.as_ptr(),
                    length_u32(values.len(), "in query value")?,
                    error.as_mut_ptr(),
                    error.capacity(),
                )
            }
        }
        Some(InQueryValue::Integer(_)) => {
            let mut integers = Vec::with_capacity(values.len());
            for value in values {
                let InQueryValue::Integer(value) = value else {
                    return Err(invalid_query(
                        "in query cannot mix string and integer values",
                    ));
                };
                integers.push(*value);
            }
            unsafe {
                sys::sk_query_add_in_integers(
                    native.0.as_ptr(),
                    field.as_ptr(),
                    integers.as_ptr(),
                    length_u32(integers.len(), "in query value")?,
                    error.as_mut_ptr(),
                    error.capacity(),
                )
            }
        }
        None => return Err(invalid_query("in query requires at least one value")),
    };
    check(status, &error)
}

fn add_native_nearest_neighbor_with_options(
    native: &NativeQuery,
    document_field: &str,
    query_tensor: &str,
    options: &NearestNeighborOptions,
) -> Result<(), Error> {
    let document_field = cstring(document_field)?;
    let query_tensor = cstring(query_tensor)?;
    let options_handle = nearest_neighbor_options(options)?;
    let mut error = NativeError::new();
    let status = unsafe {
        sys::sk_query_add_nearest_neighbor_with_options_handle(
            native.0.as_ptr(),
            document_field.as_ptr(),
            query_tensor.as_ptr(),
            options.target_num_hits,
            u8::from(options.approximate),
            options_handle.0.as_ptr(),
            error.as_mut_ptr(),
            error.capacity(),
        )
    };
    check(status, &error)
}

fn native_bound(bound: &RangeBound) -> (u32, i64) {
    match bound {
        RangeBound::Unbounded => (sys::SK_QUERY_BOUND_UNBOUNDED, 0),
        RangeBound::Included(value) => (sys::SK_QUERY_BOUND_INCLUDED, *value),
        RangeBound::Excluded(value) => (sys::SK_QUERY_BOUND_EXCLUDED, *value),
    }
}

fn add_native_same_element(
    native: &NativeQuery,
    field: &str,
    child_count: usize,
    element_filter: &[u32],
) -> Result<(), Error> {
    let field = cstring(field)?;
    let child_count = u32::try_from(child_count)
        .map_err(|_| invalid_query("same-element child count exceeds native capacity"))?;
    let mut error = NativeError::new();
    let status = unsafe {
        sys::sk_query_add_same_element_with_filter(
            native.0.as_ptr(),
            field.as_ptr(),
            child_count,
            element_filter.as_ptr(),
            u32::try_from(element_filter.len())
                .map_err(|_| invalid_query("same-element filter exceeds native capacity"))?,
            error.as_mut_ptr(),
            error.capacity(),
        )
    };
    check(status, &error)
}

fn add_native_geo_location(
    native: &NativeQuery,
    field: &str,
    latitude: f64,
    longitude: f64,
    radius: &str,
) -> Result<(), Error> {
    let field = cstring(field)?;
    let radius = cstring(radius)?;
    let mut error = NativeError::new();
    let status = unsafe {
        sys::sk_query_add_geo_location(
            native.0.as_ptr(),
            field.as_ptr(),
            latitude,
            longitude,
            radius.as_ptr(),
            error.as_mut_ptr(),
            error.capacity(),
        )
    };
    check(status, &error)
}

fn add_native_geo_bounding_box(
    native: &NativeQuery,
    field: &str,
    south: f64,
    west: f64,
    north: f64,
    east: f64,
) -> Result<(), Error> {
    let field = cstring(field)?;
    let mut error = NativeError::new();
    let status = unsafe {
        sys::sk_query_add_geo_bounding_box(
            native.0.as_ptr(),
            field.as_ptr(),
            south,
            west,
            north,
            east,
            error.as_mut_ptr(),
            error.capacity(),
        )
    };
    check(status, &error)
}

fn add_native_predicate(
    native: &NativeQuery,
    field: &str,
    features: &[PredicateFeature],
    range_features: &[PredicateRangeFeature],
) -> Result<(), Error> {
    let field = cstring(field)?;
    let feature_keys = features
        .iter()
        .map(|item| item.key.as_bytes())
        .collect::<Vec<_>>();
    let feature_values = features
        .iter()
        .map(|item| item.value.as_bytes())
        .collect::<Vec<_>>();
    let feature_key_lengths = feature_keys
        .iter()
        .map(|value| u32::try_from(value.len()))
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|_| invalid_query("predicate feature key exceeds native capacity"))?;
    let feature_value_lengths = feature_values
        .iter()
        .map(|value| u32::try_from(value.len()))
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|_| invalid_query("predicate feature value exceeds native capacity"))?;
    let feature_key_bytes = feature_keys
        .iter()
        .flat_map(|value| value.iter().copied())
        .collect::<Vec<_>>();
    let feature_value_bytes = feature_values
        .iter()
        .flat_map(|value| value.iter().copied())
        .collect::<Vec<_>>();
    let feature_masks = features
        .iter()
        .map(|item| item.sub_query_bitmask)
        .collect::<Vec<_>>();
    let range_keys = range_features
        .iter()
        .map(|item| item.key.as_bytes())
        .collect::<Vec<_>>();
    let range_key_lengths = range_keys
        .iter()
        .map(|value| u32::try_from(value.len()))
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|_| invalid_query("predicate range feature key exceeds native capacity"))?;
    let range_key_bytes = range_keys
        .iter()
        .flat_map(|value| value.iter().copied())
        .collect::<Vec<_>>();
    let range_values = range_features
        .iter()
        .map(|item| item.value)
        .collect::<Vec<_>>();
    let range_masks = range_features
        .iter()
        .map(|item| item.sub_query_bitmask)
        .collect::<Vec<_>>();
    let feature_count = length_u32(features.len(), "predicate feature")?;
    let range_count = length_u32(range_features.len(), "predicate range feature")?;
    let feature_key_bytes_length =
        length_u32(feature_key_bytes.len(), "predicate feature key bytes")?;
    let feature_value_bytes_length =
        length_u32(feature_value_bytes.len(), "predicate feature value bytes")?;
    let range_key_bytes_length = length_u32(range_key_bytes.len(), "predicate range key bytes")?;
    // All packed byte/length/value vectors stay owned by this stack frame until the
    // native call returns, so every borrowed pointer remains live for the full FFI call.
    let mut error = NativeError::new();
    let status = unsafe {
        sys::sk_query_add_predicate(
            native.0.as_ptr(),
            field.as_ptr(),
            feature_key_bytes.as_ptr(),
            feature_key_bytes_length,
            feature_key_lengths.as_ptr(),
            feature_value_bytes.as_ptr(),
            feature_value_bytes_length,
            feature_value_lengths.as_ptr(),
            feature_masks.as_ptr(),
            feature_count,
            range_key_bytes.as_ptr(),
            range_key_bytes_length,
            range_key_lengths.as_ptr(),
            range_values.as_ptr(),
            range_masks.as_ptr(),
            range_count,
            error.as_mut_ptr(),
            error.capacity(),
        )
    };
    check(status, &error)
}
