use crate::document::FieldValue;
use crate::linguistics::LinguisticsOptions;
use crate::query::{self, Query, Value};
use crate::tensor::TensorValue;
use crate::{Error, Result};
use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortOrder {
    Ascending,
    Descending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissingPlacement {
    Default,
    First,
    Last,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SortField {
    pub field: String,
    pub order: SortOrder,
    pub missing: MissingPlacement,
}

impl SortField {
    pub fn new(field: impl Into<String>, order: SortOrder) -> Self {
        Self {
            field: field.into(),
            order,
            missing: MissingPlacement::Default,
        }
    }

    pub fn ascending(field: impl Into<String>) -> Self {
        Self::new(field, SortOrder::Ascending)
    }

    pub fn descending(field: impl Into<String>) -> Self {
        Self::new(field, SortOrder::Descending)
    }

    pub fn missing_first(mut self) -> Self {
        self.missing = MissingPlacement::First;
        self
    }

    pub fn missing_last(mut self) -> Self {
        self.missing = MissingPlacement::Last;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SummaryRequest {
    None,
    Default,
    Fields(Vec<String>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HighlightSpan {
    pub start_byte: u32,
    pub end_byte: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighlightFragment {
    pub text: String,
    pub matches: Vec<HighlightSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighlightResult {
    pub field: String,
    pub fragments: Vec<HighlightFragment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// The document source selected by a search request.
pub enum Source {
    /// Search all document sources configured by the engine.
    All,
    /// Search one configured document source by name.
    Named(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// The accuracy relation for Vespa's reported total hit count.
///
/// Vespa's low-level `SearchReply` carries the count and coverage metadata,
/// but no result-side exactness bit.  `Unknown` is therefore the only honest
/// value until the adapter has evidence that a particular count is exact.
pub enum TotalHitsRelation {
    Exact,
    LowerBound,
    Unknown,
}

/// A Vespa match feature is either a scalar number or an opaque native data
/// payload. Tensor interpretation, when supported, belongs above this
/// representation; the native Vespa boundary does not expose tensors here.
#[derive(Debug, Clone, PartialEq)]
pub enum MatchFeatureValue {
    Double(f64),
    Data(Vec<u8>),
}

impl MatchFeatureValue {
    pub fn as_double(&self) -> Option<f64> {
        match self {
            Self::Double(value) => Some(*value),
            Self::Data(_) => None,
        }
    }

    pub fn as_data(&self) -> Option<&[u8]> {
        match self {
            Self::Double(_) => None,
            Self::Data(value) => Some(value),
        }
    }
}

impl PartialEq<f64> for MatchFeatureValue {
    fn eq(&self, other: &f64) -> bool {
        self.as_double() == Some(*other)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    pub document_id: String,
    pub score: f64,
    pub fields: BTreeMap<String, FieldValue>,
    pub highlights: Vec<HighlightResult>,
    pub match_features: BTreeMap<String, MatchFeatureValue>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SearchResult {
    pub total_hits: u64,
    pub total_hits_relation: TotalHitsRelation,
    pub hits: Vec<Hit>,
    pub facets: Vec<FacetResult>,
    /// Structured grouping trees returned by the same native match reply.
    pub groupings: Vec<GroupingResult>,
    pub coverage: SearchCoverage,
    pub issues: Vec<String>,
    pub diagnostics: Option<SearchDiagnostics>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SearchDiagnostics {
    pub trace: String,
    pub truncated: bool,
    pub matching_seconds: f64,
    pub summary_seconds: f64,
    pub query_seconds: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchDiagnosticControls {
    pub trace_level: u32,
    pub matching_depth: u32,
    pub first_phase_depth: u32,
    pub second_phase_depth: u32,
    pub raw_bytes: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchCoverage {
    pub active: u64,
    pub covered: u64,
    pub target_active: u64,
    pub degradation_flags: u32,
}

impl SearchCoverage {
    pub fn degraded_by_match_phase(&self) -> bool {
        self.degradation_flags & searchkernel_vespa_bindings::SK_DEGRADATION_MATCH_PHASE != 0
    }

    pub fn degraded_by_timeout(&self) -> bool {
        self.degradation_flags & searchkernel_vespa_bindings::SK_DEGRADATION_TIMEOUT != 0
    }

    pub fn degraded_by_ann_timeout(&self) -> bool {
        self.degradation_flags & searchkernel_vespa_bindings::SK_DEGRADATION_ANN_TIMEOUT != 0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SearchRequest {
    pub query: Query,
    pub source: Source,
    pub rank_profile: String,
    pub query_features: Vec<(String, f64)>,
    pub query_inputs: Vec<(String, QueryInput)>,
    pub rank_properties: Vec<(String, String)>,
    pub limit: u32,
    pub offset: u32,
    pub sort: Vec<SortField>,
    pub raw_sort_spec: Option<String>,
    pub summary: SummaryRequest,
    /// Summary class selected for the native docsums request. This is
    /// independent of `summary`, which controls whether docsums are fetched
    /// and which fields are requested from the selected class.
    pub summary_class: Option<String>,
    pub highlight_fields: Vec<String>,
    pub language: LinguisticsOptions,
    pub facets: Vec<FacetRequest>,
    pub timeout: Duration,
    pub diagnostics: Option<SearchDiagnosticControls>,
}

impl SearchRequest {
    pub fn new(query: Query) -> Self {
        Self {
            query,
            source: Source::All,
            rank_profile: "default".to_owned(),
            query_features: Vec::new(),
            query_inputs: Vec::new(),
            rank_properties: Vec::new(),
            limit: 10,
            offset: 0,
            sort: Vec::new(),
            raw_sort_spec: None,
            summary: SummaryRequest::None,
            summary_class: None,
            highlight_fields: Vec::new(),
            language: LinguisticsOptions::default(),
            facets: Vec::new(),
            timeout: Duration::from_secs(60),
            diagnostics: None,
        }
    }

    /// Parse YQL while retaining its document-source selection.
    pub fn from_yql(yql: &str) -> Result<Self, Error> {
        Self::from_yql_with_properties(yql, &BTreeMap::new())
    }

    pub fn from_yql_with_properties(
        yql: &str,
        properties: &BTreeMap<String, String>,
    ) -> Result<Self, Error> {
        let parsed = query::parse_yql_with_properties(yql, properties)?;
        query::validate_query(&parsed.query)?;
        let source = if parsed.source == "*" {
            Source::All
        } else {
            Source::Named(parsed.source)
        };
        let mut request = Self::new(parsed.query).source(source);
        for (field, ascending) in parsed.sort {
            request = request.sort(if ascending {
                SortField::ascending(field)
            } else {
                SortField::descending(field)
            });
        }
        Ok(request)
    }

    pub fn source(mut self, source: Source) -> Self {
        self.source = source;
        self
    }

    /// Limit the time the native engine may spend executing this search.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn rank_profile(mut self, rank_profile: impl Into<String>) -> Self {
        self.rank_profile = rank_profile.into();
        self
    }

    pub fn query_feature(mut self, name: impl Into<String>, value: f64) -> Self {
        self.query_features.push((name.into(), value));
        self
    }

    pub fn input(mut self, name: impl Into<String>, input: QueryInput) -> Self {
        self.query_inputs.push((name.into(), input));
        self
    }

    pub fn query_tensor(mut self, name: impl Into<String>, tensor: TensorValue) -> Self {
        self.query_inputs
            .push((name.into(), QueryInput::Literal(tensor)));
        self
    }

    pub fn rank_property(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.rank_properties.push((name.into(), value.into()));
        self
    }

    pub fn limit(mut self, limit: u32) -> Self {
        self.limit = limit;
        self
    }

    pub fn offset(mut self, offset: u32) -> Self {
        self.offset = offset;
        self
    }

    pub fn sort(mut self, sort: SortField) -> Self {
        self.sort.push(sort);
        self
    }

    pub fn raw_sort_spec(mut self, sort_spec: impl Into<String>) -> Self {
        self.raw_sort_spec = Some(sort_spec.into());
        self
    }

    pub fn fields<I, S>(mut self, fields: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.summary = SummaryRequest::Fields(fields.into_iter().map(Into::into).collect());
        self
    }

    pub fn summary(mut self, summary: SummaryRequest) -> Self {
        self.summary = summary;
        self
    }

    pub fn summary_class(mut self, name: impl Into<String>) -> Self {
        self.summary_class = Some(name.into());
        self
    }

    pub fn highlight<I, S>(mut self, fields: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.highlight_fields = fields.into_iter().map(Into::into).collect();
        self
    }

    pub fn facet(mut self, facet: FacetRequest) -> Self {
        self.facets.push(facet);
        self
    }

    pub fn language(mut self, language: LinguisticsOptions) -> Self {
        self.language = language;
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum QueryInput {
    Literal(TensorValue),
    Expression(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FacetRequest {
    pub field: String,
    pub max_groups: u32,
    pub order: GroupingOrder,
}

impl FacetRequest {
    pub fn new(field: impl Into<String>, max_groups: u32, order: GroupingOrder) -> Self {
        Self {
            field: field.into(),
            max_groups,
            order,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupingOrder {
    None,
    CountAscending,
    CountDescending,
    ValueAscending,
    ValueDescending,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GroupingLiteral {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
}

#[derive(Debug, Clone, PartialEq)]
pub enum GroupingExpression {
    Attribute(String),
    AggregationReference(usize),
    Literal(GroupingLiteral),
    Function {
        name: String,
        arguments: Vec<GroupingExpression>,
    },
    Negative(Box<GroupingExpression>),
    Binary {
        operator: GroupingBinaryOperator,
        left: Box<GroupingExpression>,
        right: Box<GroupingExpression>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupingBinaryOperator {
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GroupingFilter {
    Regex {
        field: String,
        pattern: String,
    },
    RegexExpression {
        expression: GroupingExpression,
        pattern: String,
    },
    And(Vec<GroupingFilter>),
    Or(Vec<GroupingFilter>),
    Not(Box<GroupingFilter>),
    Range {
        expression: GroupingExpression,
        lower: f64,
        upper: f64,
        include_lower: bool,
        include_upper: bool,
    },
    IsTrue(GroupingExpression),
}

#[derive(Debug, Clone, PartialEq)]
pub struct GroupingOrderBy {
    pub expression: GroupingExpression,
    pub ascending: bool,
}

impl GroupingExpression {
    pub fn attribute(field: impl Into<String>) -> Self {
        Self::Attribute(field.into())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum GroupingAggregation {
    Count,
    Average(GroupingExpression),
    Sum(GroupingExpression),
    Min(GroupingExpression),
    Max(GroupingExpression),
    StandardDeviation(GroupingExpression),
    Xor(GroupingExpression),
    Quantiles {
        expression: GroupingExpression,
        quantiles: Vec<f64>,
    },
    Summary {
        summary_class: String,
        max_hits: u32,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct GroupingLevel {
    pub classifier: GroupingExpression,
    pub label: Option<String>,
    pub aggregations: Vec<GroupingAggregation>,
    /// Output labels are request metadata.  The native grouping protocol is
    /// positional, so they are kept alongside the plan rather than encoded as
    /// part of an expression string.
    pub output_labels: Vec<Option<String>>,
    pub output_visible: Vec<bool>,
    pub max_groups: Option<u64>,
    pub precision: Option<u64>,
    pub filter: Option<GroupingFilter>,
    pub order_by: Vec<GroupingOrderBy>,
}

impl GroupingLevel {
    pub fn new(classifier: GroupingExpression) -> Self {
        Self {
            classifier,
            label: None,
            aggregations: Vec::new(),
            output_labels: Vec::new(),
            output_visible: Vec::new(),
            max_groups: None,
            precision: None,
            filter: None,
            order_by: Vec::new(),
        }
    }

    pub fn aggregation(mut self, aggregation: GroupingAggregation) -> Self {
        self.aggregations.push(aggregation);
        self.output_labels.push(None);
        self.output_visible.push(true);
        self
    }

    pub fn aggregation_label(
        mut self,
        aggregation: GroupingAggregation,
        label: impl Into<String>,
    ) -> Self {
        self.aggregations.push(aggregation);
        self.output_labels.push(Some(label.into()));
        self.output_visible.push(true);
        self
    }

    pub fn max_groups(mut self, max_groups: u64) -> Self {
        self.max_groups = Some(max_groups);
        self
    }

    /// Request all groups. The native Vespa grouping ABI represents this as
    /// `-1`; the public Rust API keeps the field unsigned and uses
    /// `u64::MAX` as its unlimited sentinel.
    pub fn unlimited_groups(mut self) -> Self {
        self.max_groups = Some(u64::MAX);
        self
    }

    pub fn precision(mut self, precision: u64) -> Self {
        self.precision = Some(precision);
        self
    }

    pub fn filter(mut self, filter: GroupingFilter) -> Self {
        self.filter = Some(filter);
        self
    }

    pub fn order_by(mut self, expression: GroupingExpression, ascending: bool) -> Self {
        self.order_by.push(GroupingOrderBy {
            expression,
            ascending,
        });
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct GroupingPlan {
    pub levels: Vec<GroupingLevel>,
    pub group_all: bool,
    pub root_label: Option<String>,
    pub root_aggregations: Vec<GroupingAggregation>,
    pub root_output_labels: Vec<Option<String>>,
}

impl GroupingPlan {
    pub fn new() -> Self {
        Self {
            levels: Vec::new(),
            group_all: false,
            root_label: None,
            root_aggregations: Vec::new(),
            root_output_labels: Vec::new(),
        }
    }

    pub fn level(mut self, level: GroupingLevel) -> Self {
        self.levels.push(level);
        self
    }

    pub fn root_aggregation(mut self, aggregation: GroupingAggregation) -> Self {
        self.root_aggregations.push(aggregation);
        self.root_output_labels.push(None);
        self
    }

    pub fn root_aggregation_label(
        mut self,
        aggregation: GroupingAggregation,
        label: impl Into<String>,
    ) -> Self {
        self.root_aggregations.push(aggregation);
        self.root_output_labels.push(Some(label.into()));
        self
    }
}

impl Default for GroupingPlan {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum GroupingResultValue {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Quantiles(Vec<(f64, f64)>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum GroupingOutput {
    Value(GroupingResultValue),
    HitList(Vec<Hit>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct GroupingResult {
    pub key: Option<GroupingResultValue>,
    pub outputs: Vec<GroupingOutput>,
    pub groups: Vec<GroupingResult>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FacetResult {
    pub groups: Vec<FacetGroup>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FacetGroup {
    pub key: Value,
    pub count: u64,
}

const RESERVED_RANK_PROPERTIES: &[&str] = &["vespa.matching.numthreadspersearch"];

pub(crate) fn validate_search_ranking_inputs(request: &SearchRequest) -> Result<()> {
    let mut query_feature_names = BTreeSet::new();
    for (name, value) in &request.query_features {
        if name.is_empty() {
            return Err(Error::message("query feature name must not be empty"));
        }
        if name.len() > 256 {
            return Err(Error::message("query feature name is too long"));
        }
        if RESERVED_RANK_PROPERTIES.contains(&name.as_str()) {
            return Err(Error::message(format!(
                "query feature '{name}' is reserved by Search Kernel"
            )));
        }
        if !value.is_finite() {
            return Err(Error::message("query feature value must be finite"));
        }
        if !query_feature_names.insert(name) {
            return Err(Error::message(format!("duplicate query feature '{name}'")));
        }
    }
    let mut query_input_names = BTreeSet::new();
    for (name, input) in &request.query_inputs {
        if name.is_empty() {
            return Err(Error::message("query input name must not be empty"));
        }
        if !query_input_names.insert(name) {
            return Err(Error::message("duplicate query input"));
        }
        if query_feature_names.contains(name) {
            return Err(Error::message(format!(
                "query input collision for key '{name}'"
            )));
        }
        match input {
            QueryInput::Literal(tensor) => tensor.validate().map_err(Error::message)?,
            QueryInput::Expression(expression) if expression.trim().is_empty() => {
                return Err(Error::message("query input expression must not be empty"));
            }
            QueryInput::Expression(_) => {}
        }
    }

    let mut rank_property_names = BTreeSet::new();
    for (name, value) in &request.rank_properties {
        if name.is_empty() || name.len() > 256 {
            return Err(Error::message(
                "rank property name must not be empty or too long",
            ));
        }
        if RESERVED_RANK_PROPERTIES.contains(&name.as_str()) {
            return Err(Error::message(format!(
                "rank property '{name}' is reserved by Search Kernel"
            )));
        }
        if value.is_empty() {
            return Err(Error::message("rank property value must not be empty"));
        }
        if !rank_property_names.insert(name) {
            return Err(Error::message(format!("duplicate rank property '{name}'")));
        }
        if query_feature_names.contains(name) || query_input_names.contains(name) {
            return Err(Error::message(format!(
                "rank property collision for key '{name}'"
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::MatchFeatureValue;

    #[test]
    fn query_dense_tensor_shape_is_validated_in_safe_ranking_inputs() {
        use super::{validate_search_ranking_inputs, SearchRequest};
        use crate::{Query, TensorValue};

        let request = SearchRequest::new(Query::True).query_tensor(
            "query_embedding",
            TensorValue::dense("tensor<float>(x[3])", [1.0, 0.0]),
        );
        let error = validate_search_ranking_inputs(&request)
            .expect_err("query tensor cell-count mismatch must be rejected in Rust");
        assert!(error.to_string().contains("cell count"));
    }

    #[test]
    fn match_feature_values_keep_the_feature_set_boundary() {
        let scalar = MatchFeatureValue::Double(3.5);
        assert_eq!(scalar.as_double(), Some(3.5));
        assert_eq!(scalar.as_data(), None);
        assert_eq!(scalar, 3.5);

        let bytes = vec![0x00, 0x7f, 0xff];
        let data = MatchFeatureValue::Data(bytes.clone());
        assert_eq!(data.as_double(), None);
        assert_eq!(data.as_data(), Some(bytes.as_slice()));
        assert_ne!(data, 3.5);
    }
}
