use crate::error::{Error, ErrorCode, Result};
use crate::Tensor;
use searchkernel_ir::{
    QueryAlternativeIr, QueryExprIr, QueryExprKindIr, QueryFieldRefIr, QueryRangeBoundIr,
    RequestTensorIr, RequestTensorPayloadIr, RequestValueIr, WeightedQueryItemIr,
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
            target_hits: searchkernel_ir::DEFAULT_WEAK_AND_TARGET_HITS,
            score_threshold: searchkernel_ir::DEFAULT_WAND_SCORE_THRESHOLD,
            threshold_boost_factor: searchkernel_ir::DEFAULT_WAND_THRESHOLD_BOOST_FACTOR,
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
            target_hits: searchkernel_ir::DEFAULT_WEAK_AND_TARGET_HITS,
            approximate: searchkernel_ir::DEFAULT_NEAREST_APPROXIMATE,
            distance_threshold: None,
            explore_additional_hits:
                searchkernel_ir::DEFAULT_NEAREST_EXPLORE_ADDITIONAL_HITS,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct QueryAlternative {
    pub query: Query,
    pub exactness: Option<f64>,
}

impl QueryAlternative {
    pub fn new(query: Query) -> Self {
        Self {
            query,
            exactness: None,
        }
    }

    pub fn exactness(mut self, value: f64) -> Self {
        self.exactness = Some(value);
        self
    }
}

/// A typed query lowered by the IR adapter into native Vespa query nodes.
#[derive(Clone, Debug, PartialEq)]
pub enum Query {
    // Fieldless item variants are the canonical RHS of `contains`; keep the
    // field-bearing helpers below as compatibility sugar over that shape.
    TermValue {
        value: QueryValue,
    },
    Contains {
        field: String,
        item: Box<Query>,
    },
    Term {
        field: String,
        value: QueryValue,
    },
    Phrase {
        field: String,
        text: String,
    },
    PhraseTerms(Vec<Query>),
    Near {
        field: String,
        children: Vec<Query>,
        distance: u32,
    },
    NearTerms {
        children: Vec<Query>,
        distance: u32,
    },
    ONear {
        field: String,
        children: Vec<Query>,
        distance: u32,
    },
    ONearTerms {
        children: Vec<Query>,
        distance: u32,
    },
    And(Vec<Query>),
    Or(Vec<Query>),
    Not(Box<Query>),
    True,
    False,
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
    FuzzyValue {
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
    Matches {
        field: String,
        pattern: String,
    },
    Text(String),
    UserInput {
        value: String,
        default_index: Option<String>,
    },
    UserQuery {
        default_index: Option<String>,
    },
    SameElement {
        field: String,
        children: Vec<Query>,
    },
    SameElementTerms(Vec<Query>),
    Equiv(Vec<Query>),
    Alternatives(Vec<QueryAlternative>),
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
    Labeled {
        query: Box<Query>,
        label: String,
        score: f64,
    },
    NonEmpty(Box<Query>),
    Annotated {
        query: Box<Query>,
        annotations: BTreeMap<String, QueryValue>,
    },
    NearestNeighbor {
        field: String,
        query_tensor: String,
        options: NearestNeighborOptions,
    },
}

impl Query {
    pub fn term_value(value: impl Into<QueryValue>) -> Self {
        Self::TermValue {
            value: value.into(),
        }
    }

    pub fn contains(field: impl Into<String>, item: Query) -> Self {
        Self::Contains {
            field: field.into(),
            item: Box::new(item),
        }
    }

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

    pub fn phrase_terms<I: IntoIterator<Item = Query>>(terms: I) -> Self {
        Self::PhraseTerms(terms.into_iter().collect())
    }
    pub fn near<I: IntoIterator<Item = Query>>(field: impl Into<String>, children: I) -> Self {
        Self::Near {
            field: field.into(),
            children: children.into_iter().collect(),
            distance: searchkernel_ir::DEFAULT_PROXIMITY_DISTANCE,
        }
    }

    pub fn near_terms<I: IntoIterator<Item = Query>>(children: I) -> Self {
        Self::NearTerms {
            children: children.into_iter().collect(),
            distance: searchkernel_ir::DEFAULT_PROXIMITY_DISTANCE,
        }
    }
    pub fn onear<I: IntoIterator<Item = Query>>(field: impl Into<String>, children: I) -> Self {
        Self::ONear {
            field: field.into(),
            children: children.into_iter().collect(),
            distance: searchkernel_ir::DEFAULT_PROXIMITY_DISTANCE,
        }
    }

    pub fn onear_terms<I: IntoIterator<Item = Query>>(children: I) -> Self {
        Self::ONearTerms {
            children: children.into_iter().collect(),
            distance: searchkernel_ir::DEFAULT_PROXIMITY_DISTANCE,
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

    pub fn true_query() -> Self {
        Self::True
    }

    pub fn false_query() -> Self {
        Self::False
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
            max_edit_distance: searchkernel_ir::DEFAULT_FUZZY_MAX_EDIT_DISTANCE,
            prefix_length: searchkernel_ir::DEFAULT_FUZZY_PREFIX_LENGTH,
            prefix: searchkernel_ir::DEFAULT_FUZZY_PREFIX,
        }
    }

    pub fn fuzzy_value(value: impl Into<QueryValue>) -> Self {
        Self::fuzzy_value_with_options(
            value,
            searchkernel_ir::DEFAULT_FUZZY_MAX_EDIT_DISTANCE,
            searchkernel_ir::DEFAULT_FUZZY_PREFIX_LENGTH,
            searchkernel_ir::DEFAULT_FUZZY_PREFIX,
        )
    }

    pub fn fuzzy_value_with_options(
        value: impl Into<QueryValue>,
        max_edit_distance: u32,
        prefix_length: u32,
        prefix: bool,
    ) -> Self {
        Self::FuzzyValue {
            value: value.into(),
            max_edit_distance,
            prefix_length,
            prefix,
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

    pub fn matches(field: impl Into<String>, pattern: impl Into<String>) -> Self {
        Self::Matches {
            field: field.into(),
            pattern: pattern.into(),
        }
    }

    pub fn text(value: impl Into<String>) -> Self {
        Self::Text(value.into())
    }

    pub fn user_input(value: impl Into<String>) -> Self {
        Self::UserInput {
            value: value.into(),
            default_index: None,
        }
    }

    pub fn user_input_with_default_index(
        value: impl Into<String>,
        default_index: impl Into<String>,
    ) -> Self {
        Self::UserInput {
            value: value.into(),
            default_index: Some(default_index.into()),
        }
    }

    pub fn user_query() -> Self {
        Self::UserQuery {
            default_index: None,
        }
    }

    pub fn user_query_with_default_index(default_index: impl Into<String>) -> Self {
        Self::UserQuery {
            default_index: Some(default_index.into()),
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

    pub fn same_element_terms<I: IntoIterator<Item = Query>>(children: I) -> Self {
        Self::SameElementTerms(children.into_iter().collect())
    }

    pub fn equiv<I: IntoIterator<Item = Query>>(children: I) -> Self {
        Self::Equiv(children.into_iter().collect())
    }

    pub fn alternatives<I: IntoIterator<Item = QueryAlternative>>(alternatives: I) -> Self {
        Self::Alternatives(alternatives.into_iter().collect())
    }

    pub fn labeled(query: Query, label: impl Into<String>, score: f64) -> Self {
        Self::Labeled {
            query: Box::new(query),
            label: label.into(),
            score,
        }
    }

    pub fn non_empty(query: Query) -> Self {
        Self::NonEmpty(Box::new(query))
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

    pub fn predicate(
        field: impl Into<String>,
        features: BTreeMap<String, QueryValue>,
        range_features: BTreeMap<String, QueryValue>,
    ) -> Self {
        Self::Predicate {
            field: field.into(),
            features,
            range_features,
        }
    }

    pub fn with_annotation(self, key: impl Into<String>, value: impl Into<QueryValue>) -> Self {
        let mut annotations = BTreeMap::new();
        annotations.insert(key.into(), value.into());
        Self::Annotated {
            query: Box::new(self),
            annotations,
        }
    }

    pub fn annotated<I, K, V>(query: Query, annotations: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<QueryValue>,
    {
        annotations.into_iter().fold(query, |query, (key, value)| {
            query.with_annotation(key.into(), value.into())
        })
    }

    pub(crate) fn into_ir(self) -> Result<QueryExprIr> {
        let mut annotations = BTreeMap::new();
        let expression = match self {
            Self::TermValue { value } => QueryExprKindIr::Term {
                value: value.into_ir()?,
            },
            Self::Contains { field, item } => QueryExprKindIr::Contains {
                field: field_ref(&field)?,
                item: Box::new(item.into_ir()?),
            },
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
            Self::PhraseTerms(terms) => QueryExprKindIr::Phrase(
                terms
                    .into_iter()
                    .map(Query::into_ir)
                    .collect::<Result<Vec<_>>>()?,
            ),
            Self::Near {
                field,
                children,
                distance,
            } => contains_with_annotations(
                &field,
                QueryExprKindIr::Near {
                    children: children
                        .into_iter()
                        .map(Query::into_ir)
                        .collect::<Result<Vec<_>>>()?,
                    distance: searchkernel_ir::DEFAULT_PROXIMITY_DISTANCE,
                },
                if distance == searchkernel_ir::DEFAULT_PROXIMITY_DISTANCE {
                    BTreeMap::new()
                } else {
                    BTreeMap::from([(
                        "distance".to_owned(),
                        RequestValueIr::UInt64(u64::from(distance)),
                    )])
                },
            )?,
            Self::NearTerms { children, distance } => QueryExprKindIr::Near {
                children: children
                    .into_iter()
                    .map(Query::into_ir)
                    .collect::<Result<Vec<_>>>()?,
                distance,
            },
            Self::ONear {
                field,
                children,
                distance,
            } => contains_with_annotations(
                &field,
                QueryExprKindIr::ONear {
                    children: children
                        .into_iter()
                        .map(Query::into_ir)
                        .collect::<Result<Vec<_>>>()?,
                    distance: searchkernel_ir::DEFAULT_PROXIMITY_DISTANCE,
                },
                if distance == searchkernel_ir::DEFAULT_PROXIMITY_DISTANCE {
                    BTreeMap::new()
                } else {
                    BTreeMap::from([(
                        "distance".to_owned(),
                        RequestValueIr::UInt64(u64::from(distance)),
                    )])
                },
            )?,
            Self::ONearTerms { children, distance } => QueryExprKindIr::ONear {
                children: children
                    .into_iter()
                    .map(Query::into_ir)
                    .collect::<Result<Vec<_>>>()?,
                distance,
            },
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
            Self::True => QueryExprKindIr::True,
            Self::False => QueryExprKindIr::False,
            Self::Rank { matching, ranking } => QueryExprKindIr::Rank {
                matching: Box::new(matching.into_ir()?),
                ranking: ranking
                    .into_iter()
                    .map(Query::into_ir)
                    .collect::<Result<Vec<_>>>()?,
            },
            Self::Labeled {
                query,
                label,
                score,
            } => QueryExprKindIr::Labeled {
                query: Box::new(query.into_ir()?),
                label,
                score,
            },
            Self::NonEmpty(query) => QueryExprKindIr::NonEmpty(Box::new(query.into_ir()?)),
            Self::Annotated {
                query,
                annotations: extra,
            } => {
                let mut resolved = query.into_ir()?;
                let annotation_offset = resolved.annotations.len();
                for (index, (key, value)) in extra.into_iter().enumerate() {
                    if resolved.annotations.contains_key(&key) {
                        return Err(duplicate_annotation_error(&key, annotation_offset + index));
                    }
                    resolved.annotations.insert(key, value.into_ir()?);
                }
                return Ok(resolved);
            }
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
            } => {
                if target_hits != searchkernel_ir::DEFAULT_WEAK_AND_TARGET_HITS {
                    annotations.insert(
                        "totalTargetHits".to_owned(),
                        RequestValueIr::UInt64(u64::from(target_hits)),
                    );
                }
                QueryExprKindIr::WeakAnd {
                    children: children
                        .into_iter()
                        .map(Query::into_ir)
                        .collect::<Result<Vec<_>>>()?,
                    target_hits: searchkernel_ir::DEFAULT_WEAK_AND_TARGET_HITS,
                }
            }
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
            } => {
                let mut fuzzy_annotations = BTreeMap::new();
                if max_edit_distance != searchkernel_ir::DEFAULT_FUZZY_MAX_EDIT_DISTANCE
                {
                    fuzzy_annotations.insert(
                        "maxEditDistance".to_owned(),
                        RequestValueIr::UInt64(u64::from(max_edit_distance)),
                    );
                }
                if prefix_length != searchkernel_ir::DEFAULT_FUZZY_PREFIX_LENGTH {
                    fuzzy_annotations.insert(
                        "prefixLength".to_owned(),
                        RequestValueIr::UInt64(u64::from(prefix_length)),
                    );
                }
                if prefix != searchkernel_ir::DEFAULT_FUZZY_PREFIX {
                    fuzzy_annotations.insert("prefix".to_owned(), RequestValueIr::Bool(prefix));
                }
                contains_with_annotations(
                    &field,
                    QueryExprKindIr::Fuzzy {
                        value: value.into_ir()?,
                        max_edit_distance:
                            searchkernel_ir::DEFAULT_FUZZY_MAX_EDIT_DISTANCE,
                        prefix_length: searchkernel_ir::DEFAULT_FUZZY_PREFIX_LENGTH,
                        prefix: searchkernel_ir::DEFAULT_FUZZY_PREFIX,
                    },
                    fuzzy_annotations,
                )?
            }
            Self::FuzzyValue {
                value,
                max_edit_distance,
                prefix_length,
                prefix,
            } => QueryExprKindIr::Fuzzy {
                value: value.into_ir()?,
                max_edit_distance,
                prefix_length,
                prefix,
            },
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
            Self::Matches { field, pattern } => QueryExprKindIr::Matches {
                field: field_ref(&field)?,
                pattern,
            },
            Self::Text(text) => QueryExprKindIr::Text { text },
            Self::UserInput {
                value,
                default_index,
            } => {
                if let Some(default_index) = default_index {
                    annotations.insert(
                        "defaultIndex".to_owned(),
                        RequestValueIr::String(default_index),
                    );
                }
                QueryExprKindIr::UserInput {
                    value,
                    default_index: None,
                    target_hits: searchkernel_ir::DEFAULT_WEAK_AND_TARGET_HITS,
                }
            }
            Self::UserQuery { default_index } => {
                if let Some(default_index) = default_index {
                    annotations.insert(
                        "defaultIndex".to_owned(),
                        RequestValueIr::String(default_index),
                    );
                }
                QueryExprKindIr::UserQuery {
                    default_index: None,
                    target_hits: searchkernel_ir::DEFAULT_WEAK_AND_TARGET_HITS,
                }
            }
            Self::SameElement { field, children } => contains(
                &field,
                QueryExprKindIr::SameElement(
                    children
                        .into_iter()
                        .map(Query::into_ir)
                        .collect::<Result<Vec<_>>>()?,
                ),
            )?,
            Self::SameElementTerms(children) => QueryExprKindIr::SameElement(
                children
                    .into_iter()
                    .map(Query::into_ir)
                    .collect::<Result<Vec<_>>>()?,
            ),
            Self::Equiv(children) => QueryExprKindIr::Equiv(
                children
                    .into_iter()
                    .map(Query::into_ir)
                    .collect::<Result<Vec<_>>>()?,
            ),
            Self::Alternatives(alternatives) => QueryExprKindIr::Alternatives(
                alternatives
                    .into_iter()
                    .map(|alternative| {
                        Ok(QueryAlternativeIr {
                            query: alternative.query.into_ir()?,
                            exactness: alternative.exactness,
                        })
                    })
                    .collect::<Result<Vec<_>>>()?,
            ),
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

fn duplicate_annotation_error(key: &str, index: usize) -> Error {
    Error::search(
        ErrorCode::InvalidArgument,
        format!("property `{key}` occurs more than once"),
    )
    .with_detail(
        "duplicate_property",
        Some(format!("query.annotations[{index}].key")),
        format!("property `{key}` occurs more than once"),
    )
}

fn contains(field: &str, item: QueryExprKindIr) -> Result<QueryExprKindIr> {
    contains_with_annotations(field, item, BTreeMap::new())
}

fn contains_with_annotations(
    field: &str,
    item: QueryExprKindIr,
    annotations: BTreeMap<String, RequestValueIr>,
) -> Result<QueryExprKindIr> {
    Ok(QueryExprKindIr::Contains {
        field: field_ref(field)?,
        item: Box::new(QueryExprIr {
            kind: item,
            annotations,
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
    let mut annotations = BTreeMap::new();
    if options.target_hits != searchkernel_ir::DEFAULT_WEAK_AND_TARGET_HITS {
        annotations.insert(
            "targetNumHits".to_owned(),
            RequestValueIr::UInt64(u64::from(options.target_hits)),
        );
    }
    if options.score_threshold != searchkernel_ir::DEFAULT_WAND_SCORE_THRESHOLD {
        annotations.insert(
            "scoreThreshold".to_owned(),
            RequestValueIr::Int64(options.score_threshold),
        );
    }
    if options.threshold_boost_factor
        != searchkernel_ir::DEFAULT_WAND_THRESHOLD_BOOST_FACTOR
    {
        annotations.insert(
            "thresholdBoostFactor".to_owned(),
            RequestValueIr::Float64(options.threshold_boost_factor),
        );
    }
    annotations
}

fn nearest_annotations(options: NearestNeighborOptions) -> BTreeMap<String, RequestValueIr> {
    let mut annotations = BTreeMap::new();
    if options.target_hits != searchkernel_ir::DEFAULT_WEAK_AND_TARGET_HITS {
        annotations.insert(
            "targetNumHits".to_owned(),
            RequestValueIr::UInt64(u64::from(options.target_hits)),
        );
    }
    if options.approximate != searchkernel_ir::DEFAULT_NEAREST_APPROXIMATE {
        annotations.insert(
            "approximate".to_owned(),
            RequestValueIr::Bool(options.approximate),
        );
    }
    if let Some(distance_threshold) = options.distance_threshold {
        annotations.insert(
            "distanceThreshold".to_owned(),
            RequestValueIr::Float64(distance_threshold),
        );
    }
    if options.explore_additional_hits
        != searchkernel_ir::DEFAULT_NEAREST_EXPLORE_ADDITIONAL_HITS
    {
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
    fn default_typed_query_controls_are_not_materialized_by_rust_facade() {
        let near = Query::near("title", [Query::term("title", "value")])
            .into_ir()
            .expect("near should convert");
        let QueryExprKindIr::Contains { item, .. } = near.kind else {
            panic!("expected near to be field-qualified");
        };
        assert!(item.annotations.is_empty());
        assert!(matches!(
            item.kind,
            QueryExprKindIr::Near {
                distance: searchkernel_ir::DEFAULT_PROXIMITY_DISTANCE,
                ..
            }
        ));

        let fuzzy = Query::fuzzy("title", "value")
            .into_ir()
            .expect("fuzzy should convert");
        let QueryExprKindIr::Contains { item, .. } = fuzzy.kind else {
            panic!("expected fuzzy to be field-qualified");
        };
        assert!(item.annotations.is_empty());
        assert!(matches!(
            item.kind,
            QueryExprKindIr::Fuzzy {
                max_edit_distance: searchkernel_ir::DEFAULT_FUZZY_MAX_EDIT_DISTANCE,
                prefix_length: searchkernel_ir::DEFAULT_FUZZY_PREFIX_LENGTH,
                prefix: searchkernel_ir::DEFAULT_FUZZY_PREFIX,
                ..
            }
        ));

        let weak_and = Query::weak_and([Query::term("title", "value")], 100)
            .into_ir()
            .expect("weak-and should convert");
        assert!(weak_and.annotations.is_empty());
        assert!(matches!(
            weak_and.kind,
            QueryExprKindIr::WeakAnd {
                target_hits: searchkernel_ir::DEFAULT_WEAK_AND_TARGET_HITS,
                ..
            }
        ));

        let wand = Query::wand("title", [WeightedQueryItem::new("value", 1)])
            .into_ir()
            .expect("WAND should convert");
        assert!(wand.annotations.is_empty());

        let nearest = Query::nearest_neighbor("embedding", "query_embedding")
            .into_ir()
            .expect("nearest-neighbor should convert");
        assert!(nearest.annotations.is_empty());
    }

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

    #[test]
    fn typed_query_facade_covers_every_canonical_expression_kind() {
        type QueryCase = (&'static str, Query, fn(QueryExprIr) -> bool);
        let cases: Vec<QueryCase> = vec![
            ("term", Query::term_value("value"), |query| {
                matches!(query.kind, QueryExprKindIr::Term { .. })
            }),
            (
                "contains",
                Query::contains("title", Query::term_value("value")),
                |query| matches!(query.kind, QueryExprKindIr::Contains { .. }),
            ),
            ("and", Query::and([Query::true_query()]), |query| {
                matches!(query.kind, QueryExprKindIr::And(_))
            }),
            ("or", Query::or([Query::false_query()]), |query| {
                matches!(query.kind, QueryExprKindIr::Or(_))
            }),
            ("not", Query::not(Query::true_query()), |query| {
                matches!(query.kind, QueryExprKindIr::Not(_))
            }),
            ("true", Query::true_query(), |query| {
                matches!(query.kind, QueryExprKindIr::True)
            }),
            ("false", Query::false_query(), |query| {
                matches!(query.kind, QueryExprKindIr::False)
            }),
            ("comparison", Query::equals("rating", 5_i64), |query| {
                matches!(query.kind, QueryExprKindIr::Comparison { .. })
            }),
            (
                "range",
                Query::range(
                    "rating",
                    QueryBound::Unbounded,
                    QueryBound::Included(5_i64.into()),
                ),
                |query| matches!(query.kind, QueryExprKindIr::Range { .. }),
            ),
            ("in", Query::in_values("rating", [5_i64.into()]), |query| {
                matches!(query.kind, QueryExprKindIr::In { .. })
            }),
            (
                "phrase",
                Query::phrase_terms([Query::term_value("quick"), Query::term_value("brown")]),
                |query| matches!(query.kind, QueryExprKindIr::Phrase(_)),
            ),
            (
                "near",
                Query::near_terms([Query::term_value("quick"), Query::term_value("brown")]),
                |query| matches!(query.kind, QueryExprKindIr::Near { .. }),
            ),
            (
                "onear",
                Query::onear_terms([Query::term_value("quick"), Query::term_value("brown")]),
                |query| matches!(query.kind, QueryExprKindIr::ONear { .. }),
            ),
            (
                "same_element",
                Query::same_element_terms([Query::term_value("value")]),
                |query| matches!(query.kind, QueryExprKindIr::SameElement(_)),
            ),
            (
                "equiv",
                Query::equiv([Query::term_value("one"), Query::term_value("two")]),
                |query| matches!(query.kind, QueryExprKindIr::Equiv(_)),
            ),
            (
                "alternatives",
                Query::alternatives([
                    QueryAlternative::new(Query::term_value("one")).exactness(0.5)
                ]),
                |query| matches!(query.kind, QueryExprKindIr::Alternatives(_)),
            ),
            (
                "fuzzy",
                Query::contains("title", Query::fuzzy_value("value")),
                |query| {
                    matches!(
                        query.kind,
                        QueryExprKindIr::Contains { item, .. }
                            if matches!(item.kind, QueryExprKindIr::Fuzzy { .. })
                    )
                },
            ),
            ("matches", Query::matches("title", "v.*"), |query| {
                matches!(query.kind, QueryExprKindIr::Matches { .. })
            }),
            ("text", Query::text("free form"), |query| {
                matches!(query.kind, QueryExprKindIr::Text { .. })
            }),
            (
                "user_input",
                Query::user_input_with_default_index("free form", "title"),
                |query| matches!(query.kind, QueryExprKindIr::UserInput { .. }),
            ),
            (
                "user_query",
                Query::user_query_with_default_index("title"),
                |query| matches!(query.kind, QueryExprKindIr::UserQuery { .. }),
            ),
            (
                "dot_product",
                Query::dot_product("brand", [WeightedQueryItem::new("one", 1)]),
                |query| matches!(query.kind, QueryExprKindIr::DotProduct { .. }),
            ),
            (
                "weighted_set",
                Query::weighted_set("brand", [WeightedQueryItem::new("one", 1)]),
                |query| matches!(query.kind, QueryExprKindIr::WeightedSet { .. }),
            ),
            (
                "wand",
                Query::wand("brand", [WeightedQueryItem::new("one", 1)]),
                |query| matches!(query.kind, QueryExprKindIr::Wand { .. }),
            ),
            (
                "weak_and",
                Query::weak_and([Query::true_query()], 100),
                |query| matches!(query.kind, QueryExprKindIr::WeakAnd { .. }),
            ),
            (
                "rank",
                Query::rank(Query::true_query(), [Query::text("rank")]),
                |query| matches!(query.kind, QueryExprKindIr::Rank { .. }),
            ),
            (
                "labeled",
                Query::labeled(Query::true_query(), "label", 1.0),
                |query| matches!(query.kind, QueryExprKindIr::Labeled { .. }),
            ),
            (
                "non_empty",
                Query::non_empty(Query::true_query()),
                |query| matches!(query.kind, QueryExprKindIr::NonEmpty(_)),
            ),
            (
                "geo_location",
                Query::geo_location("location", 1.0, 2.0, "3km"),
                |query| matches!(query.kind, QueryExprKindIr::GeoLocation { .. }),
            ),
            (
                "geo_bounding_box",
                Query::geo_bounding_box("location", 1.0, 2.0, 3.0, 4.0),
                |query| matches!(query.kind, QueryExprKindIr::GeoBoundingBox { .. }),
            ),
            (
                "nearest_neighbor",
                Query::nearest_neighbor("embedding", "query_tensor"),
                |query| matches!(query.kind, QueryExprKindIr::NearestNeighbor { .. }),
            ),
            (
                "predicate",
                Query::predicate("tags", BTreeMap::new(), BTreeMap::new()),
                |query| matches!(query.kind, QueryExprKindIr::Predicate { .. }),
            ),
        ];

        for (name, query, matches_kind) in cases {
            assert!(matches_kind(query.into_ir().expect(name)), "{name}");
        }

        let annotated = Query::text("free form").with_annotation("custom", true);
        let annotated = annotated
            .into_ir()
            .expect("generic annotation should convert");
        assert_eq!(
            annotated.annotations.get("custom"),
            Some(&RequestValueIr::Bool(true))
        );
    }

    fn assert_duplicate_annotation(error: Error, path: &str) {
        assert_eq!(error.code(), ErrorCode::InvalidArgument);
        let detail = error.details().first().expect("duplicate detail");
        assert_eq!(detail.code, "duplicate_property");
        assert_eq!(detail.path.as_deref(), Some(path));
    }

    #[test]
    fn annotated_rejects_duplicate_pairs_without_collapsing_them() {
        let error = Query::annotated(
            Query::text("free form"),
            [
                ("custom", QueryValue::Bool(true)),
                ("custom", QueryValue::Bool(false)),
            ],
        )
        .into_ir()
        .expect_err("duplicate annotations must be rejected");
        assert_duplicate_annotation(error, "query.annotations[1].key");
    }

    #[test]
    fn annotation_cannot_override_nearest_neighbor_control() {
        let query = Query::nearest_neighbor_with_options(
            "embedding",
            "query_tensor",
            NearestNeighborOptions {
                distance_threshold: Some(0.25),
                ..NearestNeighborOptions::default()
            },
        )
        .with_annotation("distanceThreshold", 0.5);
        let error = query.into_ir().expect_err("conflicting control must fail");
        assert_duplicate_annotation(error, "query.annotations[1].key");
    }

    #[test]
    fn annotation_cannot_override_wand_control() {
        let query = Query::Wand {
            field: "tags".to_owned(),
            items: vec![WeightedQueryItem::new("kernel", 10)],
            options: WandOptions {
                target_hits: 7,
                ..WandOptions::default()
            },
        }
        .with_annotation("targetNumHits", 3_u64);
        let error = query.into_ir().expect_err("conflicting control must fail");
        assert_duplicate_annotation(error, "query.annotations[1].key");
    }
}
