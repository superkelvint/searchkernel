use crate::document::{pending, PendingValue};
use serde::Serialize;
use serde_json::Value;

#[derive(Clone, Debug, PartialEq)]
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
pub struct Range {
    pub(crate) lower: Option<PendingValue>,
    pub(crate) upper: Option<PendingValue>,
    pub(crate) include_lower: Option<bool>,
    pub(crate) include_upper: Option<bool>,
}

impl Range {
    pub fn new() -> Self {
        Self {
            lower: None,
            upper: None,
            include_lower: None,
            include_upper: None,
        }
    }

    pub fn lower<T: Serialize>(mut self, value: T) -> Self {
        self.lower = Some(pending(&value));
        self
    }

    pub fn upper<T: Serialize>(mut self, value: T) -> Self {
        self.upper = Some(pending(&value));
        self
    }

    pub fn include_lower(mut self, value: bool) -> Self {
        self.include_lower = Some(value);
        self
    }

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
pub struct FieldRef(String);

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

    pub fn eq<T: Serialize>(self, value: T) -> Filter {
        self.comparison(ComparisonOp::Eq, value)
    }
    pub fn ne<T: Serialize>(self, value: T) -> Filter {
        self.comparison(ComparisonOp::Ne, value)
    }
    pub fn lt<T: Serialize>(self, value: T) -> Filter {
        self.comparison(ComparisonOp::Lt, value)
    }
    pub fn lte<T: Serialize>(self, value: T) -> Filter {
        self.comparison(ComparisonOp::Lte, value)
    }
    pub fn gt<T: Serialize>(self, value: T) -> Filter {
        self.comparison(ComparisonOp::Gt, value)
    }
    pub fn gte<T: Serialize>(self, value: T) -> Filter {
        self.comparison(ComparisonOp::Gte, value)
    }
    pub fn range(self, bounds: Range) -> Filter {
        Filter(FilterNode::Range {
            field: self.0,
            bounds,
        })
    }
    pub fn exists(self) -> Filter {
        Filter(FilterNode::Exists { field: self.0 })
    }
}

impl Filter {
    pub fn all<I: IntoIterator<Item = Filter>>(filters: I) -> Self {
        Self(FilterNode::All(filters.into_iter().collect()))
    }

    pub fn any<I: IntoIterator<Item = Filter>>(filters: I) -> Self {
        Self(FilterNode::Any(filters.into_iter().collect()))
    }

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
