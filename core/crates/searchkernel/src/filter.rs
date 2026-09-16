use crate::document::{pending, PendingValue};
use serde::Serialize;
use serde_json::Value;

#[derive(Clone, Debug, PartialEq)]
/// A filter expression evaluated by the backend.
pub struct Filter(pub(crate) FilterNode);

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum FilterNode {
    All(Vec<Filter>),
    Any(Vec<Filter>),
    Not(Box<Filter>),
    Comparison {
        field: String,
        op: ComparisonOp,
        value: PendingValue,
    },
    Range {
        field: String,
        bounds: Range,
    },
    Exists {
        field: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ComparisonOp {
    Eq,
    Ne,
    Lt,
    Lte,
    Gt,
    Gte,
}

#[derive(Clone, Debug, PartialEq)]
/// Inclusive or exclusive bounds for a numeric range filter.
pub struct Range {
    pub(crate) lower: Option<PendingValue>,
    pub(crate) upper: Option<PendingValue>,
    pub(crate) include_lower: Option<bool>,
    pub(crate) include_upper: Option<bool>,
}

impl Range {
    /// Start an empty range.
    pub fn new() -> Self {
        Self {
            lower: None,
            upper: None,
            include_lower: None,
            include_upper: None,
        }
    }

    /// Set the lower bound of the range.
    pub fn lower<T: Serialize>(mut self, value: T) -> Self {
        self.lower = Some(pending(&value));
        self
    }

    /// Set the upper bound of the range.
    pub fn upper<T: Serialize>(mut self, value: T) -> Self {
        self.upper = Some(pending(&value));
        self
    }

    /// Include or exclude the lower bound.
    pub fn include_lower(mut self, value: bool) -> Self {
        self.include_lower = Some(value);
        self
    }

    /// Include or exclude the upper bound.
    pub fn include_upper(mut self, value: bool) -> Self {
        self.include_upper = Some(value);
        self
    }
}

impl Default for Range {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// A field name from which a filter can be built.
pub struct FieldRef(String);

/// Start a filter expression for `name`.
pub fn field(name: impl Into<String>) -> FieldRef {
    FieldRef(name.into())
}

impl FieldRef {
    fn comparison<T: Serialize>(self, op: ComparisonOp, value: T) -> Filter {
        Filter(FilterNode::Comparison {
            field: self.0,
            op,
            value: pending(&value),
        })
    }

    /// Match values equal to `value`.
    pub fn eq<T: Serialize>(self, value: T) -> Filter {
        self.comparison(ComparisonOp::Eq, value)
    }
    /// Match values not equal to `value`.
    pub fn ne<T: Serialize>(self, value: T) -> Filter {
        self.comparison(ComparisonOp::Ne, value)
    }
    /// Match values less than `value`.
    pub fn lt<T: Serialize>(self, value: T) -> Filter {
        self.comparison(ComparisonOp::Lt, value)
    }
    /// Match values less than or equal to `value`.
    pub fn lte<T: Serialize>(self, value: T) -> Filter {
        self.comparison(ComparisonOp::Lte, value)
    }
    /// Match values greater than `value`.
    pub fn gt<T: Serialize>(self, value: T) -> Filter {
        self.comparison(ComparisonOp::Gt, value)
    }
    /// Match values greater than or equal to `value`.
    pub fn gte<T: Serialize>(self, value: T) -> Filter {
        self.comparison(ComparisonOp::Gte, value)
    }
    /// Match values within `bounds`.
    pub fn range(self, bounds: Range) -> Filter {
        Filter(FilterNode::Range {
            field: self.0,
            bounds,
        })
    }
    /// Match documents where this field has a value.
    pub fn exists(self) -> Filter {
        Filter(FilterNode::Exists { field: self.0 })
    }
}

impl Filter {
    /// Require every filter in the collection to match.
    pub fn all<I: IntoIterator<Item = Filter>>(filters: I) -> Self {
        Self(FilterNode::All(filters.into_iter().collect()))
    }

    /// Require at least one filter in the collection to match.
    pub fn any<I: IntoIterator<Item = Filter>>(filters: I) -> Self {
        Self(FilterNode::Any(filters.into_iter().collect()))
    }

    /// Negate a filter.
    #[allow(clippy::should_implement_trait)]
    pub fn not(filter: Filter) -> Self {
        Self(FilterNode::Not(Box::new(filter)))
    }
}

impl std::ops::Not for Filter {
    type Output = Self;

    fn not(self) -> Self::Output {
        Self::not(self)
    }
}

pub(crate) fn pending_value(value: &PendingValue) -> Result<&Value, String> {
    value.as_ref().map_err(|error| error.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FailingSerialize;

    impl serde::Serialize for FailingSerialize {
        fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            Err(serde::ser::Error::custom(
                "intentional serialization failure",
            ))
        }
    }

    #[test]
    fn field_builders_cover_all_filter_nodes() {
        assert!(matches!(
            field("name").eq("Alice").0,
            FilterNode::Comparison { field, op: ComparisonOp::Eq, .. } if field == "name"
        ));
        assert!(matches!(
            field("name").ne("Alice").0,
            FilterNode::Comparison {
                op: ComparisonOp::Ne,
                ..
            }
        ));
        assert!(matches!(
            field("age").lt(10).0,
            FilterNode::Comparison {
                op: ComparisonOp::Lt,
                ..
            }
        ));
        assert!(matches!(
            field("age").lte(10).0,
            FilterNode::Comparison {
                op: ComparisonOp::Lte,
                ..
            }
        ));
        assert!(matches!(
            field("age").gt(10).0,
            FilterNode::Comparison {
                op: ComparisonOp::Gt,
                ..
            }
        ));
        assert!(matches!(
            field("age").gte(10).0,
            FilterNode::Comparison {
                op: ComparisonOp::Gte,
                ..
            }
        ));
        assert!(matches!(field("age").exists().0, FilterNode::Exists { field } if field == "age"));
        assert!(matches!(
            field("age").range(Range::new().lower(1).upper(10)).0,
            FilterNode::Range { field, bounds } if field == "age" && bounds.lower.is_some() && bounds.upper.is_some()
        ));
        assert!(matches!(
            Filter::all([field("a").exists(), field("b").exists()]).0,
            FilterNode::All(filters) if filters.len() == 2
        ));
        assert!(matches!(
            Filter::any([field("a").exists()]).0,
            FilterNode::Any(filters) if filters.len() == 1
        ));
        assert!(matches!((!field("a").exists()).0, FilterNode::Not(_)));
    }

    #[test]
    fn range_defaults_and_pending_errors_are_preserved() {
        let range = Range::default();
        assert!(range.lower.is_none());
        assert!(range.upper.is_none());
        assert!(range.include_lower.is_none());
        assert!(range.include_upper.is_none());

        let range = Range::new().include_lower(true).include_upper(false);
        assert_eq!(range.include_lower, Some(true));
        assert_eq!(range.include_upper, Some(false));

        let filter = field("ratio").eq(FailingSerialize);
        let FilterNode::Comparison { value, .. } = filter.0 else {
            panic!("expected comparison");
        };
        assert!(pending_value(&value).is_err());
    }
}
