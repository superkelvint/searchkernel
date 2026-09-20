use crate::error::{Error, Result};
use crate::Tensor;
use searchkernel_ir::{
    QueryExprIr, QueryExprKindIr, QueryFieldRefIr, QueryRangeBoundIr, RequestTensorIr,
    RequestTensorPayloadIr, RequestValueIr, WeightedQueryItemIr,
};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq)]
pub enum QueryValue {
    Bool(bool),
    Int64(i64),
    UInt64(u64),
    Float64(f64),
    String(String),
    Tensor(Tensor),
}

impl From<bool> for QueryValue {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}
impl From<i64> for QueryValue {
    fn from(value: i64) -> Self {
        Self::Int64(value)
    }
}
impl From<i32> for QueryValue {
    fn from(value: i32) -> Self {
        Self::Int64(i64::from(value))
    }
}
impl From<u64> for QueryValue {
    fn from(value: u64) -> Self {
        Self::UInt64(value)
    }
}
impl From<u32> for QueryValue {
    fn from(value: u32) -> Self {
        Self::UInt64(u64::from(value))
    }
}
impl From<f64> for QueryValue {
    fn from(value: f64) -> Self {
        Self::Float64(value)
    }
}
impl From<String> for QueryValue {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}
impl From<&str> for QueryValue {
    fn from(value: &str) -> Self {
        Self::String(value.to_owned())
    }
}
impl From<Tensor> for QueryValue {
    fn from(value: Tensor) -> Self {
        Self::Tensor(value)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum QueryBound {
    Unbounded,
    Included(QueryValue),
    Excluded(QueryValue),
}

#[derive(Clone, Debug, PartialEq)]
pub struct WeightedQueryItem {
    pub key: QueryValue,
    pub weight: i64,
}

impl WeightedQueryItem {
    pub fn new(key: impl Into<QueryValue>, weight: i64) -> Self {
        Self {
            key: key.into(),
            weight,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct WandOptions {
    pub target_hits: u32,
    pub score_threshold: i64,
    pub threshold_boost_factor: f64,
}

impl Default for WandOptions {
    fn default() -> Self {
        Self {
            target_hits: 100,
            score_threshold: 0,
            threshold_boost_factor: 1.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NearestNeighborOptions {
    pub target_hits: u32,
    pub approximate: bool,
    pub distance_threshold: Option<f64>,
    pub explore_additional_hits: u32,
}

impl Default for NearestNeighborOptions {
    fn default() -> Self {
        Self {
            target_hits: 100,
            approximate: true,
            distance_threshold: None,
            explore_additional_hits: 0,
        }
    }
}

/// A typed query lowered by the IR adapter into native Vespa query nodes.
#[derive(Clone, Debug, PartialEq)]
pub enum Query {
    Term {
        field: String,
        value: QueryValue,
    },
    Phrase {
        field: String,
        text: String,
    },
    Near {
        field: String,
        children: Vec<Query>,
        distance: u32,
    },
    ONear {
        field: String,
        children: Vec<Query>,
        distance: u32,
    },
    And(Vec<Query>),
    Or(Vec<Query>),
    Not(Box<Query>),
    Rank {
        matching: Box<Query>,
        ranking: Vec<Query>,
    },
    Equality {
        field: String,
        value: QueryValue,
    },
    Range {
        field: String,
        lower: QueryBound,
        upper: QueryBound,
    },
    In {
        field: String,
        values: Vec<QueryValue>,
    },
    WeakAnd {
        children: Vec<Query>,
        target_hits: u32,
    },
    Wand {
        field: String,
        items: Vec<WeightedQueryItem>,
        options: WandOptions,
    },
    WeightedSet {
        field: String,
        items: Vec<WeightedQueryItem>,
    },
    DotProduct {
        field: String,
        items: Vec<WeightedQueryItem>,
    },
    Fuzzy {
        field: String,
        value: QueryValue,
        max_edit_distance: u32,
        prefix_length: u32,
        prefix: bool,
    },
    Prefix {
        field: String,
        value: String,
    },
    Substring {
        field: String,
        value: String,
    },
    Suffix {
        field: String,
        value: String,
    },
    SameElement {
        field: String,
        children: Vec<Query>,
    },
    GeoLocation {
        field: String,
        latitude: f64,
        longitude: f64,
        radius: String,
    },
    GeoBoundingBox {
        field: String,
        south: f64,
        west: f64,
        north: f64,
        east: f64,
    },
    Predicate {
        field: String,
        features: BTreeMap<String, QueryValue>,
        range_features: BTreeMap<String, QueryValue>,
    },
    NearestNeighbor {
        field: String,
        query_tensor: String,
        options: NearestNeighborOptions,
    },
}

impl Query {
    pub fn term(field: impl Into<String>, value: impl Into<QueryValue>) -> Self {
        Self::Term {
            field: field.into(),
            value: value.into(),
        }
    }
    pub fn phrase(field: impl Into<String>, text: impl Into<String>) -> Self {
        Self::Phrase {
            field: field.into(),
            text: text.into(),
        }
    }
    pub fn near<I: IntoIterator<Item = Query>>(field: impl Into<String>, children: I) -> Self {
        Self::Near {
            field: field.into(),
            children: children.into_iter().collect(),
            distance: 2,
        }
    }
    pub fn onear<I: IntoIterator<Item = Query>>(field: impl Into<String>, children: I) -> Self {
        Self::ONear {
            field: field.into(),
            children: children.into_iter().collect(),
            distance: 2,
        }
    }
    pub fn and<I: IntoIterator<Item = Query>>(children: I) -> Self {
        Self::And(children.into_iter().collect())
    }
    pub fn or<I: IntoIterator<Item = Query>>(children: I) -> Self {
        Self::Or(children.into_iter().collect())
    }
    #[allow(clippy::should_implement_trait)]
    pub fn not(query: Query) -> Self {
        Self::Not(Box::new(query))
    }
    pub fn rank<I: IntoIterator<Item = Query>>(matching: Query, ranking: I) -> Self {
        Self::Rank {
            matching: Box::new(matching),
            ranking: ranking.into_iter().collect(),
        }
    }
    pub fn equals(field: impl Into<String>, value: impl Into<QueryValue>) -> Self {
        Self::Equality {
            field: field.into(),
            value: value.into(),
        }
    }
    pub fn range(field: impl Into<String>, lower: QueryBound, upper: QueryBound) -> Self {
        Self::Range {
            field: field.into(),
            lower,
            upper,
        }
    }
    pub fn in_values<I: IntoIterator<Item = QueryValue>>(
        field: impl Into<String>,
        values: I,
    ) -> Self {
        Self::In {
            field: field.into(),
            values: values.into_iter().collect(),
        }
    }
    pub fn weak_and<I: IntoIterator<Item = Query>>(children: I, target_hits: u32) -> Self {
        Self::WeakAnd {
            children: children.into_iter().collect(),
            target_hits,
        }
    }
    pub fn weighted_set<I: IntoIterator<Item = WeightedQueryItem>>(
        field: impl Into<String>,
        items: I,
    ) -> Self {
        Self::WeightedSet {
            field: field.into(),
            items: items.into_iter().collect(),
        }
    }
    pub fn wand<I: IntoIterator<Item = WeightedQueryItem>>(
        field: impl Into<String>,
        items: I,
    ) -> Self {
        Self::Wand {
            field: field.into(),
            items: items.into_iter().collect(),
            options: WandOptions::default(),
        }
    }
    pub fn dot_product<I: IntoIterator<Item = WeightedQueryItem>>(
        field: impl Into<String>,
        items: I,
    ) -> Self {
        Self::DotProduct {
            field: field.into(),
            items: items.into_iter().collect(),
        }
    }
    pub fn fuzzy(field: impl Into<String>, value: impl Into<QueryValue>) -> Self {
        Self::Fuzzy {
            field: field.into(),
            value: value.into(),
            max_edit_distance: 2,
            prefix_length: 0,
            prefix: false,
        }
    }
    pub fn prefix(field: impl Into<String>, value: impl Into<String>) -> Self {
        Self::Prefix {
            field: field.into(),
            value: value.into(),
        }
    }
    pub fn substring(field: impl Into<String>, value: impl Into<String>) -> Self {
        Self::Substring {
            field: field.into(),
            value: value.into(),
        }
    }
    pub fn suffix(field: impl Into<String>, value: impl Into<String>) -> Self {
        Self::Suffix {
            field: field.into(),
            value: value.into(),
        }
    }
    pub fn same_element<I: IntoIterator<Item = Query>>(
        field: impl Into<String>,
        children: I,
    ) -> Self {
        Self::SameElement {
            field: field.into(),
            children: children.into_iter().collect(),
        }
    }
    pub fn nearest_neighbor(field: impl Into<String>, query_tensor: impl Into<String>) -> Self {
        Self::nearest_neighbor_with_options(field, query_tensor, NearestNeighborOptions::default())
    }

    /// Create an approximate or exact nearest-neighbor query with explicit
    /// Vespa target-hit and distance controls.
    pub fn nearest_neighbor_with_options(
        field: impl Into<String>,
        query_tensor: impl Into<String>,
        options: NearestNeighborOptions,
    ) -> Self {
        Self::NearestNeighbor {
            field: field.into(),
            query_tensor: query_tensor.into(),
            options,
        }
    }
    pub fn geo_location(
        field: impl Into<String>,
        latitude: f64,
        longitude: f64,
        radius: impl Into<String>,
    ) -> Self {
        Self::GeoLocation {
            field: field.into(),
            latitude,
            longitude,
            radius: radius.into(),
        }
    }
    pub fn geo_bounding_box(
        field: impl Into<String>,
        south: f64,
        west: f64,
        north: f64,
        east: f64,
    ) -> Self {
        Self::GeoBoundingBox {
            field: field.into(),
            south,
            west,
            north,
            east,
        }
    }

    pub(crate) fn into_ir(self) -> Result<QueryExprIr> {
        let mut annotations = BTreeMap::new();
        let expression = match self {
            Self::Term { field, value } => contains(
                &field,
                QueryExprKindIr::Term {
                    value: value.into_ir()?,
                },
            )?,
            Self::Phrase { field, text } => {
                let terms = text
                    .split_whitespace()
                    .map(|term| QueryExprIr {
                        kind: QueryExprKindIr::Term {
                            value: RequestValueIr::String(term.to_owned()),
                        },
                        annotations: BTreeMap::new(),
                    })
                    .collect();
                contains(&field, QueryExprKindIr::Phrase(terms))?
            }
            Self::Near {
                field,
                children,
                distance,
            } => contains(
                &field,
                QueryExprKindIr::Near {
                    children: children
                        .into_iter()
                        .map(Query::into_ir)
                        .collect::<Result<Vec<_>>>()?,
                    distance,
                },
            )?,
            Self::ONear {
                field,
                children,
                distance,
            } => contains(
                &field,
                QueryExprKindIr::ONear {
                    children: children
                        .into_iter()
                        .map(Query::into_ir)
                        .collect::<Result<Vec<_>>>()?,
                    distance,
                },
            )?,
            Self::And(children) => QueryExprKindIr::And(
                children
                    .into_iter()
                    .map(Query::into_ir)
                    .collect::<Result<Vec<_>>>()?,
            ),
            Self::Or(children) => QueryExprKindIr::Or(
                children
                    .into_iter()
                    .map(Query::into_ir)
                    .collect::<Result<Vec<_>>>()?,
            ),
            Self::Not(child) => QueryExprKindIr::Not(Box::new(child.into_ir()?)),
            Self::Rank { matching, ranking } => QueryExprKindIr::Rank {
                matching: Box::new(matching.into_ir()?),
                ranking: ranking
                    .into_iter()
                    .map(Query::into_ir)
                    .collect::<Result<Vec<_>>>()?,
            },
            Self::Equality { field, value } => {
                comparison(&field, searchkernel_ir::ComparisonOp::Eq, value.into_ir()?)?
            }
            Self::Range {
                field,
                lower,
                upper,
            } => {
                let (lower, include_lower) = lower.into_ir()?;
                let (upper, include_upper) = upper.into_ir()?;
                QueryExprKindIr::Range {
                    field: field_ref(&field)?,
                    lower,
                    upper,
                    include_lower,
                    include_upper,
                }
            }
            Self::In { field, values } => QueryExprKindIr::In {
                field: field_ref(&field)?,
                values: values
                    .into_iter()
                    .map(QueryValue::into_ir)
                    .collect::<Result<Vec<_>>>()?,
            },
            Self::WeakAnd {
                children,
                target_hits,
            } => QueryExprKindIr::WeakAnd {
                children: children
                    .into_iter()
                    .map(Query::into_ir)
                    .collect::<Result<Vec<_>>>()?,
                target_hits,
            },
            Self::Wand {
                field,
                items,
                options,
            } => {
                annotations = wand_annotations(options);
                QueryExprKindIr::Wand {
                    field: field_ref(&field)?,
                    items: items
                        .into_iter()
                        .map(WeightedQueryItem::into_ir)
                        .collect::<Result<Vec<_>>>()?,
                }
            }
            Self::WeightedSet { field, items } => QueryExprKindIr::WeightedSet {
                field: field_ref(&field)?,
                items: items
                    .into_iter()
                    .map(WeightedQueryItem::into_ir)
                    .collect::<Result<Vec<_>>>()?,
            },
            Self::DotProduct { field, items } => QueryExprKindIr::DotProduct {
                field: field_ref(&field)?,
                items: items
                    .into_iter()
                    .map(WeightedQueryItem::into_ir)
                    .collect::<Result<Vec<_>>>()?,
            },
            Self::Fuzzy {
                field,
                value,
                max_edit_distance,
                prefix_length,
                prefix,
            } => contains(
                &field,
                QueryExprKindIr::Fuzzy {
                    value: value.into_ir()?,
                    max_edit_distance,
                    prefix_length,
                    prefix,
                },
            )?,
            Self::Prefix { field, value } => QueryExprKindIr::Matches {
                field: field_ref(&field)?,
                pattern: format!("{}.*", escape_regexp_literal(&value)),
            },
            Self::Substring { field, value } => QueryExprKindIr::Matches {
                field: field_ref(&field)?,
                pattern: format!(".*{}.*", escape_regexp_literal(&value)),
            },
            Self::Suffix { field, value } => QueryExprKindIr::Matches {
                field: field_ref(&field)?,
                pattern: format!(".*{}", escape_regexp_literal(&value)),
            },
            Self::SameElement { field, children } => contains(
                &field,
                QueryExprKindIr::SameElement(
                    children
                        .into_iter()
                        .map(Query::into_ir)
                        .collect::<Result<Vec<_>>>()?,
                ),
            )?,
            Self::GeoLocation {
                field,
                latitude,
                longitude,
                radius,
            } => QueryExprKindIr::GeoLocation {
                field: field_ref(&field)?,
                latitude,
                longitude,
                radius,
            },
            Self::GeoBoundingBox {
                field,
                south,
                west,
                north,
                east,
            } => QueryExprKindIr::GeoBoundingBox {
                field: field_ref(&field)?,
                south,
                west,
                north,
                east,
            },
            Self::Predicate {
                field,
                features,
                range_features,
            } => QueryExprKindIr::Predicate {
                field: field_ref(&field)?,
                features: into_map(features)?,
                range_features: into_map(range_features)?,
            },
            Self::NearestNeighbor {
                field,
                query_tensor,
                options,
            } => {
                annotations = nearest_annotations(options);
                QueryExprKindIr::NearestNeighbor {
                    document_field: field_ref(&field)?,
                    query_tensor,
                }
            }
        };
        Ok(QueryExprIr {
            kind: expression,
            annotations,
        })
    }
}

impl std::ops::Not for Query {
    type Output = Self;

    fn not(self) -> Self::Output {
        Self::Not(Box::new(self))
    }
}

fn field_ref(field: &str) -> Result<QueryFieldRefIr> {
    let path =
        searchkernel_ir::FieldName::parse_query_path(field.to_owned()).map_err(Error::from_ir)?;
    Ok(QueryFieldRefIr {
        path,
        element_index: None,
    })
}

fn contains(field: &str, item: QueryExprKindIr) -> Result<QueryExprKindIr> {
    Ok(QueryExprKindIr::Contains {
        field: field_ref(field)?,
        item: Box::new(QueryExprIr {
            kind: item,
            annotations: BTreeMap::new(),
        }),
    })
}

fn comparison(
    field: &str,
    op: searchkernel_ir::ComparisonOp,
    value: RequestValueIr,
) -> Result<QueryExprKindIr> {
    Ok(QueryExprKindIr::Comparison {
        field: field_ref(field)?,
        op,
        value,
    })
}

fn into_map(values: BTreeMap<String, QueryValue>) -> Result<BTreeMap<String, RequestValueIr>> {
    values
        .into_iter()
        .map(|(key, value)| Ok((key, value.into_ir()?)))
        .collect()
}

fn escape_regexp_literal(value: &str) -> String {
    value
        .chars()
        .flat_map(|character| {
            if matches!(
                character,
                '.' | '^' | '$' | '*' | '+' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '|' | '\\'
            ) {
                vec!['\\', character]
            } else {
                vec![character]
            }
        })
        .collect()
}

fn wand_annotations(options: WandOptions) -> BTreeMap<String, RequestValueIr> {
    BTreeMap::from([
        (
            "targetNumHits".to_owned(),
            RequestValueIr::UInt64(u64::from(options.target_hits)),
        ),
        (
            "scoreThreshold".to_owned(),
            RequestValueIr::Int64(options.score_threshold),
        ),
        (
            "thresholdBoostFactor".to_owned(),
            RequestValueIr::Float64(options.threshold_boost_factor),
        ),
    ])
}

fn nearest_annotations(options: NearestNeighborOptions) -> BTreeMap<String, RequestValueIr> {
    let mut annotations = BTreeMap::from([
        (
            "targetNumHits".to_owned(),
            RequestValueIr::UInt64(u64::from(options.target_hits)),
        ),
        (
            "approximate".to_owned(),
            RequestValueIr::Bool(options.approximate),
        ),
    ]);
    if let Some(distance_threshold) = options.distance_threshold {
        annotations.insert(
            "distanceThreshold".to_owned(),
            RequestValueIr::Float64(distance_threshold),
        );
    }
    if options.explore_additional_hits > 0 {
        annotations.insert(
            "exploreAdditionalHits".to_owned(),
            RequestValueIr::UInt64(u64::from(options.explore_additional_hits)),
        );
    }
    annotations
}

impl QueryValue {
    fn into_ir(self) -> Result<RequestValueIr> {
        Ok(match self {
            Self::Bool(value) => RequestValueIr::Bool(value),
            Self::Int64(value) => RequestValueIr::Int64(value),
            Self::UInt64(value) => RequestValueIr::UInt64(value),
            Self::Float64(value) if value.is_finite() => RequestValueIr::Float64(value),
            Self::Float64(_) => {
                return Err(Error::search(
                    crate::ErrorCode::InvalidArgument,
                    "query values must be finite",
                ))
            }
            Self::String(value) => RequestValueIr::String(value),
            Self::Tensor(value) => RequestValueIr::Tensor(RequestTensorIr {
                type_spec: value.type_spec().to_owned(),
                payload: RequestTensorPayloadIr::Dense(value.values().to_vec()),
            }),
        })
    }
}

impl QueryBound {
    fn into_ir(self) -> Result<(QueryRangeBoundIr, Option<bool>)> {
        Ok(match self {
            Self::Unbounded => (QueryRangeBoundIr::Unbounded, None),
            Self::Included(value) => (QueryRangeBoundIr::Finite(value.into_ir()?), Some(true)),
            Self::Excluded(value) => (QueryRangeBoundIr::Finite(value.into_ir()?), Some(false)),
        })
    }
}

impl WeightedQueryItem {
    fn into_ir(self) -> Result<WeightedQueryItemIr> {
        Ok(WeightedQueryItemIr {
            key: self.key.into_ir()?,
            weight: self.weight,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_bounds_preserve_inclusion_flags() {
        let query = Query::range(
            "price",
            QueryBound::Excluded(QueryValue::from(10_i64)),
            QueryBound::Included(QueryValue::from(20_i64)),
        )
        .into_ir()
        .expect("range should convert");

        let QueryExprKindIr::Range {
            include_lower,
            include_upper,
            ..
        } = query.kind
        else {
            panic!("expected range IR");
        };
        assert_eq!(include_lower, Some(false));
        assert_eq!(include_upper, Some(true));
    }

    #[test]
    fn unbounded_range_sides_have_no_inclusion_flag() {
        let query = Query::range(
            "price",
            QueryBound::Unbounded,
            QueryBound::Excluded(QueryValue::from(20_i64)),
        )
        .into_ir()
        .expect("range should convert");

        let QueryExprKindIr::Range {
            include_lower,
            include_upper,
            ..
        } = query.kind
        else {
            panic!("expected range IR");
        };
        assert_eq!(include_lower, None);
        assert_eq!(include_upper, Some(false));
    }

    #[test]
    fn literal_pattern_queries_escape_regexp_metacharacters() {
        let query = Query::prefix("brand", "A.c+")
            .into_ir()
            .expect("prefix should convert");
        let QueryExprKindIr::Matches { pattern, .. } = query.kind else {
            panic!("expected matches IR");
        };
        assert_eq!(pattern, r"A\.c\+.*");
    }
}
