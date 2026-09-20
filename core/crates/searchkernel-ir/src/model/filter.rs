//! Filters and sorting.

use super::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComparisonOp {
    Eq,
    Ne,
    Lt,
    Lte,
    Gt,
    Gte,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ComparisonIr {
    pub(crate) field: FieldName,
    pub(crate) op: ComparisonOp,
    pub(crate) value: ScalarValueIr,
}

impl ComparisonIr {
    #[cfg(any(test, feature = "test-support"))]
    pub fn new(field: FieldName, op: ComparisonOp, value: ScalarValueIr) -> Self {
        Self { field, op, value }
    }

    pub fn field(&self) -> &FieldName {
        &self.field
    }

    pub fn op(&self) -> ComparisonOp {
        self.op
    }

    pub fn value(&self) -> &ScalarValueIr {
        &self.value
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RangeIr {
    pub(crate) field: FieldName,
    pub(crate) lower: FilterRangeBoundIr,
    pub(crate) upper: FilterRangeBoundIr,
    pub(crate) include_lower: bool,
    pub(crate) include_upper: bool,
}

impl RangeIr {
    #[cfg(any(test, feature = "test-support"))]
    pub fn new(
        field: FieldName,
        lower: Option<ScalarValueIr>,
        upper: Option<ScalarValueIr>,
        include_lower: bool,
        include_upper: bool,
    ) -> Self {
        Self {
            field,
            lower: lower.map_or(FilterRangeBoundIr::Unbounded, FilterRangeBoundIr::Finite),
            upper: upper.map_or(FilterRangeBoundIr::Unbounded, FilterRangeBoundIr::Finite),
            include_lower,
            include_upper,
        }
    }

    pub fn field(&self) -> &FieldName {
        &self.field
    }

    pub fn lower(&self) -> &FilterRangeBoundIr {
        &self.lower
    }

    pub fn upper(&self) -> &FilterRangeBoundIr {
        &self.upper
    }

    pub fn lower_bound(&self) -> &FilterRangeBoundIr {
        &self.lower
    }

    pub fn upper_bound(&self) -> &FilterRangeBoundIr {
        &self.upper
    }

    pub fn include_lower(&self) -> bool {
        self.include_lower
    }

    pub fn include_upper(&self) -> bool {
        self.include_upper
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum FilterRangeBoundIr {
    Finite(ScalarValueIr),
    NegativeInfinity,
    PositiveInfinity,
    Unbounded,
}

#[derive(Clone, Debug, PartialEq)]
pub enum FilterIr {
    And(Vec<FilterIr>),
    Or(Vec<FilterIr>),
    Not(Box<FilterIr>),
    Comparison(ComparisonIr),
    Range(RangeIr),
    In {
        field: FieldName,
        values: Vec<ScalarValueIr>,
    },
    Exists {
        field: FieldName,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortDirection {
    Asc,
    Desc,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SortTargetIr {
    Field(FieldName),
    Relevance,
    RankFeature(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum MissingPlacement {
    First,
    Last,
    As(RequestValueIr),
}

#[derive(Clone, Debug, PartialEq)]
pub struct SortIr {
    pub(crate) target: SortTargetIr,
    pub(crate) direction: SortDirection,
    pub(crate) missing: MissingPlacement,
    pub(crate) annotations: std::collections::BTreeMap<String, RequestValueIr>,
}

impl SortIr {
    #[cfg(any(test, feature = "test-support"))]
    pub fn new(field: FieldName, direction: SortDirection, missing: MissingPlacement) -> Self {
        Self {
            target: SortTargetIr::Field(field),
            direction,
            missing,
            annotations: std::collections::BTreeMap::new(),
        }
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn with_target(
        target: SortTargetIr,
        direction: SortDirection,
        missing: MissingPlacement,
        annotations: std::collections::BTreeMap<String, RequestValueIr>,
    ) -> Self {
        Self {
            target,
            direction,
            missing,
            annotations,
        }
    }

    pub fn target(&self) -> &SortTargetIr {
        &self.target
    }

    pub fn field(&self) -> Option<&FieldName> {
        match &self.target {
            SortTargetIr::Field(field) => Some(field),
            SortTargetIr::Relevance | SortTargetIr::RankFeature(_) => None,
        }
    }

    pub fn direction(&self) -> SortDirection {
        self.direction
    }

    pub fn missing(&self) -> MissingPlacement {
        self.missing.clone()
    }

    pub fn annotations(&self) -> &std::collections::BTreeMap<String, RequestValueIr> {
        &self.annotations
    }
}
