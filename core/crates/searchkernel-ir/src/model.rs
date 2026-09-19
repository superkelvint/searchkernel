//! Owned, backend-independent Search Kernel IR and result values.

use crate::error::{PortableError, Result};
use std::fmt;
use std::time::Duration;

macro_rules! validated_string_type {
    ($name:ident, $detail:literal, $description:literal) => {
        #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn parse(value: impl Into<String>) -> Result<Self> {
                let value = value.into();
                if value.is_empty() {
                    return Err(PortableError::invalid(
                        $detail,
                        None::<String>,
                        concat!($description, " must not be empty"),
                    ));
                }
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                self.as_str()
            }
        }
    };
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FieldName(String);

impl FieldName {
    pub fn parse(value: impl Into<String>) -> Result<Self> {
        let value = value.into();
        let mut chars = value.chars();
        let valid = chars
            .next()
            .is_some_and(|first| first.is_ascii_alphabetic())
            && chars.all(|character| character.is_ascii_alphanumeric() || character == '_');
        if !valid {
            return Err(PortableError::invalid(
                "invalid_field_name",
                None::<String>,
                "field name must match [A-Za-z][A-Za-z0-9_]*",
            ));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for FieldName {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

validated_string_type!(DocumentId, "invalid_document_id", "document ID");
validated_string_type!(IndexName, "invalid_index_name", "index name");

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LanguageHint(String);

impl LanguageHint {
    pub fn parse(value: impl Into<String>) -> Result<Self> {
        let value = value.into();
        let valid = !value.is_empty()
            && value.split('-').enumerate().all(|(index, part)| {
                let length = part.len();
                let length_valid = if index == 0 {
                    (2..=8).contains(&length)
                } else {
                    (1..=8).contains(&length)
                };
                length_valid
                    && part.is_ascii()
                    && if index == 0 {
                        part.bytes().all(|byte| byte.is_ascii_alphabetic())
                    } else {
                        part.bytes().all(|byte| byte.is_ascii_alphanumeric())
                    }
            });
        if !valid {
            return Err(PortableError::invalid(
                "invalid_language",
                None::<String>,
                "language must be a basic BCP-47-style ASCII tag",
            ));
        }
        Ok(Self(value.to_ascii_lowercase()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for LanguageHint {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

/// The physical V2 document type.  Matching and index configuration are
/// intentionally modelled separately; `Text` and `Keyword` are not document
/// types in the portable contract.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DataTypeIr {
    Primitive(PrimitiveDataTypeIr),
    Tensor {
        type_spec: String,
    },
    Array(Box<DataTypeIr>),
    Map {
        key_type: Box<DataTypeIr>,
        value_type: Box<DataTypeIr>,
    },
    WeightedSet(Box<DataTypeIr>),
    Struct {
        fields: Vec<StructFieldIr>,
    },
    StructReference {
        name: String,
    },
    Predicate,
    Position,
    Reference {
        document_type: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrimitiveDataTypeIr {
    String,
    Bool,
    Byte,
    Int32,
    Int64,
    Float32,
    Float64,
    Raw,
}

/// Derived query type used by the existing request validator.  It is not a
/// schema representation: `FieldIr` stores the complete `DataTypeIr` above.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldKind {
    Text,
    Keyword,
    Int64,
    Float64,
    Bool,
}

/// Derived collection shape used only by existing request validation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldCardinality {
    Single,
    Multi,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnalyzerIdentity {
    StandardV1,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StructFieldIr {
    pub name: String,
    pub data_type: DataTypeIr,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatchingModeIr {
    Unspecified,
    Text,
    Word,
    Exact,
    Gram,
}

pub type SchemaPropertiesIr = std::collections::BTreeMap<String, RequestValueIr>;

#[derive(Clone, Debug, PartialEq)]
pub struct AnalyzerIr {
    pub name: String,
    pub version: u32,
    pub options: SchemaPropertiesIr,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MatchingConfigIr {
    pub mode: MatchingModeIr,
    pub options: SchemaPropertiesIr,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HnswIndexConfigIr {
    pub max_links_per_node: Option<u32>,
    pub neighbors_to_explore_at_insert: Option<u32>,
    pub options: SchemaPropertiesIr,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IndexConfigIr {
    pub enabled: Option<bool>,
    pub hnsw: Option<HnswIndexConfigIr>,
    pub options: SchemaPropertiesIr,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DistanceMetricIr {
    Unspecified,
    Euclidean,
    Angular,
    DotProduct,
    PrenormalizedAngular,
    GeoDegrees,
    Hamming,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AttributeConfigIr {
    pub enabled: Option<bool>,
    pub fast_search: Option<bool>,
    pub fast_access: Option<bool>,
    pub distance_metric: DistanceMetricIr,
    pub options: SchemaPropertiesIr,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SummaryConfigIr {
    pub enabled: Option<bool>,
    pub options: SchemaPropertiesIr,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FieldCapabilities {
    pub(crate) stored: bool,
    pub(crate) searchable: bool,
    pub(crate) filterable: bool,
    pub(crate) sortable: bool,
    /// Canonical grouping/attribute capability resolved from V2 `facetable`.
    pub(crate) groupable: bool,
}

impl FieldCapabilities {
    #[cfg(any(test, feature = "test-support"))]
    pub fn new(
        stored: bool,
        searchable: bool,
        filterable: bool,
        sortable: bool,
        groupable: bool,
    ) -> Self {
        Self {
            stored,
            searchable,
            filterable,
            sortable,
            groupable,
        }
    }

    pub fn stored(&self) -> bool {
        self.stored
    }

    pub fn searchable(&self) -> bool {
        self.searchable
    }

    pub fn filterable(&self) -> bool {
        self.filterable
    }

    pub fn sortable(&self) -> bool {
        self.sortable
    }

    pub fn groupable(&self) -> bool {
        self.groupable
    }

    /// Compatibility accessor for callers that still use the old facade name.
    /// The canonical IR stores this as the resolved grouping capability.
    pub fn facetable(&self) -> bool {
        self.groupable()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FieldIr {
    pub(crate) name: FieldName,
    pub(crate) data_type: DataTypeIr,
    pub(crate) analyzer: Option<AnalyzerIr>,
    pub(crate) capabilities: FieldCapabilities,
    pub(crate) matching: Option<MatchingConfigIr>,
    pub(crate) index: Option<IndexConfigIr>,
    pub(crate) attribute: Option<AttributeConfigIr>,
    pub(crate) summary: Option<SummaryConfigIr>,
    pub(crate) indexing_script: Option<String>,
    pub(crate) properties: SchemaPropertiesIr,
    pub(crate) struct_fields: Vec<StructFieldConfigIr>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StructFieldConfigIr {
    pub path: String,
    pub indexing_script: Option<String>,
    pub matching: Option<MatchingConfigIr>,
    pub index: Option<IndexConfigIr>,
    pub attribute: Option<AttributeConfigIr>,
    pub summary: Option<SummaryConfigIr>,
    pub rank: Option<String>,
    pub query_commands: Vec<String>,
    pub properties: SchemaPropertiesIr,
}

impl FieldIr {
    #[cfg(any(test, feature = "test-support"))]
    pub fn new(
        name: FieldName,
        data_type: DataTypeIr,
        analyzer: Option<AnalyzerIr>,
        capabilities: FieldCapabilities,
    ) -> Self {
        Self {
            name,
            data_type,
            analyzer,
            capabilities,
            matching: None,
            index: None,
            attribute: None,
            summary: None,
            indexing_script: None,
            properties: SchemaPropertiesIr::new(),
            struct_fields: Vec::new(),
        }
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn set_attribute_config(&mut self, config: Option<AttributeConfigIr>) {
        self.attribute = config;
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn set_index_config(&mut self, config: Option<IndexConfigIr>) {
        self.index = config;
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn set_summary_config(&mut self, config: Option<SummaryConfigIr>) {
        self.summary = config;
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn set_matching_config(&mut self, config: Option<MatchingConfigIr>) {
        self.matching = config;
    }

    pub fn name(&self) -> &FieldName {
        &self.name
    }

    pub fn data_type(&self) -> &DataTypeIr {
        &self.data_type
    }

    pub fn kind(&self) -> FieldKind {
        match self.query_scalar_type().unwrap_or(QueryScalarType::Keyword) {
            QueryScalarType::Text => FieldKind::Text,
            QueryScalarType::Keyword => FieldKind::Keyword,
            QueryScalarType::Int64 => FieldKind::Int64,
            QueryScalarType::Float64 => FieldKind::Float64,
            QueryScalarType::Bool => FieldKind::Bool,
        }
    }

    pub fn cardinality(&self) -> FieldCardinality {
        if self.is_multi_valued() {
            FieldCardinality::Multi
        } else {
            FieldCardinality::Single
        }
    }

    pub fn analyzer(&self) -> Option<&AnalyzerIr> {
        self.analyzer.as_ref()
    }

    pub fn capabilities(&self) -> &FieldCapabilities {
        &self.capabilities
    }

    pub fn matching(&self) -> Option<&MatchingConfigIr> {
        self.matching.as_ref()
    }
    pub fn index(&self) -> Option<&IndexConfigIr> {
        self.index.as_ref()
    }
    pub fn attribute(&self) -> Option<&AttributeConfigIr> {
        self.attribute.as_ref()
    }
    pub fn summary(&self) -> Option<&SummaryConfigIr> {
        self.summary.as_ref()
    }
    pub fn indexing_script(&self) -> Option<&str> {
        self.indexing_script.as_deref()
    }
    pub fn properties(&self) -> &SchemaPropertiesIr {
        &self.properties
    }
    pub fn struct_fields(&self) -> &[StructFieldConfigIr] {
        &self.struct_fields
    }

    pub(crate) fn query_scalar_type(&self) -> Option<QueryScalarType> {
        let primitive = self.data_type.scalar_primitive()?;
        match primitive {
            PrimitiveDataTypeIr::String => match self.matching.as_ref().map(|m| m.mode) {
                Some(MatchingModeIr::Exact | MatchingModeIr::Word) => {
                    Some(QueryScalarType::Keyword)
                }
                _ => Some(QueryScalarType::Text),
            },
            PrimitiveDataTypeIr::Bool => Some(QueryScalarType::Bool),
            PrimitiveDataTypeIr::Int64 => Some(QueryScalarType::Int64),
            PrimitiveDataTypeIr::Float64 => Some(QueryScalarType::Float64),
            _ => None,
        }
    }

    pub(crate) fn is_multi_valued(&self) -> bool {
        self.data_type.is_multi_valued()
    }
}

impl DataTypeIr {
    pub(crate) fn scalar_primitive(&self) -> Option<PrimitiveDataTypeIr> {
        match self {
            Self::Primitive(kind) => Some(*kind),
            Self::Array(element) | Self::WeightedSet(element) => element.scalar_primitive(),
            _ => None,
        }
    }

    pub(crate) fn is_multi_valued(&self) -> bool {
        matches!(self, Self::Array(_) | Self::WeightedSet(_))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum QueryScalarType {
    Text,
    Keyword,
    Int64,
    Float64,
    Bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SearchFieldWeight(f64);

impl SearchFieldWeight {
    pub fn new(value: f64) -> Result<Self> {
        if !value.is_finite() || value < 0.0 {
            return Err(PortableError::invalid(
                "invalid_search_weight",
                None::<String>,
                "search-field weight must be finite and non-negative",
            ));
        }
        Ok(Self(value))
    }

    pub fn get(self) -> f64 {
        self.0
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SearchFieldIr {
    pub(crate) field: FieldName,
    pub(crate) weight: SearchFieldWeight,
}

impl SearchFieldIr {
    #[cfg(any(test, feature = "test-support"))]
    pub fn new(field: FieldName, weight: SearchFieldWeight) -> Self {
        Self { field, weight }
    }

    pub fn field(&self) -> &FieldName {
        &self.field
    }

    pub fn weight(&self) -> SearchFieldWeight {
        self.weight
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ScalarValueIr {
    Bool(bool),
    Int64(i64),
    Float64(f64),
    String(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum FieldValueIr {
    Single(ScalarValueIr),
    Multi(Vec<ScalarValueIr>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct DocumentIr {
    pub(crate) id: DocumentId,
    pub(crate) fields: std::collections::BTreeMap<FieldName, FieldValueIr>,
}

impl DocumentIr {
    #[cfg(any(test, feature = "test-support"))]
    pub fn new(
        id: DocumentId,
        fields: std::collections::BTreeMap<FieldName, FieldValueIr>,
    ) -> Self {
        Self { id, fields }
    }

    pub fn id(&self) -> &DocumentId {
        &self.id
    }

    pub fn fields(&self) -> &std::collections::BTreeMap<FieldName, FieldValueIr> {
        &self.fields
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct InsertRequestIr {
    pub(crate) document: DocumentIr,
}

impl InsertRequestIr {
    #[cfg(any(test, feature = "test-support"))]
    pub fn new(document: DocumentIr) -> Self {
        Self { document }
    }

    pub fn document(&self) -> &DocumentIr {
        &self.document
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct UpsertRequestIr {
    pub(crate) document: DocumentIr,
    /// Document-selection test-and-set condition. `None` is an
    /// unconditional upsert; an empty portable string is normalized to None.
    pub(crate) condition: Option<String>,
}

impl UpsertRequestIr {
    #[cfg(any(test, feature = "test-support"))]
    pub fn new(document: DocumentIr) -> Self {
        Self {
            document,
            condition: None,
        }
    }

    pub fn document(&self) -> &DocumentIr {
        &self.document
    }

    pub fn condition(&self) -> Option<&str> {
        self.condition.as_deref()
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn with_condition(mut self, condition: impl Into<String>) -> Self {
        self.condition = Some(condition.into());
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum PatchOperationIr {
    Set {
        field: FieldName,
        value: FieldValueIr,
    },
    Unset {
        field: FieldName,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct PatchRequestIr {
    pub(crate) id: DocumentId,
    pub(crate) operations: Vec<PatchOperationIr>,
}

impl PatchRequestIr {
    #[cfg(any(test, feature = "test-support"))]
    pub fn new(id: DocumentId, operations: Vec<PatchOperationIr>) -> Self {
        Self { id, operations }
    }

    pub fn id(&self) -> &DocumentId {
        &self.id
    }

    pub fn operations(&self) -> &[PatchOperationIr] {
        &self.operations
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeleteRequestIr {
    pub(crate) id: DocumentId,
    /// Document-selection test-and-set condition. `None` is an
    /// unconditional delete; an empty portable string is normalized to None.
    pub(crate) condition: Option<String>,
}

impl DeleteRequestIr {
    #[cfg(any(test, feature = "test-support"))]
    pub fn new(id: DocumentId) -> Self {
        Self {
            id,
            condition: None,
        }
    }

    pub fn id(&self) -> &DocumentId {
        &self.id
    }

    pub fn condition(&self) -> Option<&str> {
        self.condition.as_deref()
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn with_condition(mut self, condition: impl Into<String>) -> Self {
        self.condition = Some(condition.into());
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectionIr {
    pub(crate) fields: Vec<FieldName>,
}

impl ProjectionIr {
    #[cfg(any(test, feature = "test-support"))]
    pub fn new(fields: Vec<FieldName>) -> Self {
        Self { fields }
    }

    pub fn fields(&self) -> &[FieldName] {
        &self.fields
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GetRequestIr {
    pub(crate) id: DocumentId,
    pub(crate) projection: ProjectionIr,
}

impl GetRequestIr {
    #[cfg(any(test, feature = "test-support"))]
    pub fn new(id: DocumentId, projection: ProjectionIr) -> Self {
        Self { id, projection }
    }

    pub fn id(&self) -> &DocumentId {
        &self.id
    }

    pub fn projection(&self) -> &ProjectionIr {
        &self.projection
    }
}

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RankingPolicyIr {
    DefaultRelevanceV1,
    Bm25,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrefixPolicy {
    Disabled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TypoPolicy {
    Disabled,
}

#[derive(Clone, Debug, PartialEq)]
pub enum QueryIr {
    MatchAll,
    Text(String),
    Term {
        field: FieldName,
        value: ScalarValueIr,
    },
    And(Vec<QueryIr>),
    Or(Vec<QueryIr>),
    Not(Box<QueryIr>),
    /// A YQL source is intentionally kept separate from the portable typed
    /// query tree. Its syntax belongs to the backend, but its request-level
    /// parameters are still portable request semantics.
    Yql {
        source: String,
        parameters: std::collections::BTreeMap<String, RequestValueIr>,
    },
    /// A lossless typed V2 query expression. The legacy variants above remain
    /// as the normalized execution forms used by the current native adapter.
    Typed(Box<QueryExprIr>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct QueryExprIr {
    pub kind: QueryExprKindIr,
    pub annotations: std::collections::BTreeMap<String, RequestValueIr>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum QueryExprKindIr {
    Term {
        value: RequestValueIr,
    },
    Contains {
        field: QueryFieldRefIr,
        item: Box<QueryExprIr>,
    },
    And(Vec<QueryExprIr>),
    Or(Vec<QueryExprIr>),
    Not(Box<QueryExprIr>),
    True,
    False,
    Comparison {
        field: QueryFieldRefIr,
        op: ComparisonOp,
        value: RequestValueIr,
    },
    Range {
        field: QueryFieldRefIr,
        lower: QueryRangeBoundIr,
        upper: QueryRangeBoundIr,
        include_lower: Option<bool>,
        include_upper: Option<bool>,
    },
    In {
        field: QueryFieldRefIr,
        values: Vec<RequestValueIr>,
    },
    Phrase(Vec<QueryExprIr>),
    Near(Vec<QueryExprIr>),
    ONear(Vec<QueryExprIr>),
    SameElement(Vec<QueryExprIr>),
    Equiv(Vec<QueryExprIr>),
    Alternatives(Vec<QueryAlternativeIr>),
    Fuzzy {
        value: RequestValueIr,
    },
    Matches {
        field: QueryFieldRefIr,
        pattern: String,
    },
    Text {
        text: String,
    },
    UserInput {
        value: String,
    },
    UserQuery,
    DotProduct {
        field: QueryFieldRefIr,
        items: Vec<WeightedQueryItemIr>,
    },
    WeightedSet {
        field: QueryFieldRefIr,
        items: Vec<WeightedQueryItemIr>,
    },
    Wand {
        field: QueryFieldRefIr,
        items: Vec<WeightedQueryItemIr>,
    },
    WeakAnd(Vec<QueryExprIr>),
    Rank {
        matching: Box<QueryExprIr>,
        ranking: Vec<QueryExprIr>,
    },
    Labeled {
        query: Box<QueryExprIr>,
        label: String,
        score: f64,
    },
    NonEmpty(Box<QueryExprIr>),
    GeoLocation {
        field: QueryFieldRefIr,
        latitude: f64,
        longitude: f64,
        radius: String,
    },
    GeoBoundingBox {
        field: QueryFieldRefIr,
        south: f64,
        west: f64,
        north: f64,
        east: f64,
    },
    NearestNeighbor {
        document_field: QueryFieldRefIr,
        query_tensor: String,
    },
    Predicate {
        field: QueryFieldRefIr,
        features: std::collections::BTreeMap<String, RequestValueIr>,
        range_features: std::collections::BTreeMap<String, RequestValueIr>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueryFieldRefIr {
    pub path: FieldName,
    pub element_index: Option<u32>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum QueryRangeBoundIr {
    Finite(RequestValueIr),
    NegativeInfinity,
    PositiveInfinity,
    Unbounded,
}

#[derive(Clone, Debug, PartialEq)]
pub struct QueryAlternativeIr {
    pub query: QueryExprIr,
    pub exactness: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WeightedQueryItemIr {
    pub key: RequestValueIr,
    pub weight: i64,
}

/// The complete V2 value domain used by request controls.  Document values
/// remain deliberately narrower while the native document ABI has that limit;
/// request properties and query inputs must not lose valid V2 values for that
/// reason.
#[derive(Clone, Debug, PartialEq)]
pub enum RequestValueIr {
    Null,
    Bool(bool),
    Int64(i64),
    UInt64(u64),
    Float64(f64),
    NonFiniteFloat(RequestNonFiniteFloatIr),
    String(String),
    Bytes(Vec<u8>),
    List(Vec<RequestValueIr>),
    Object(std::collections::BTreeMap<String, RequestValueIr>),
    Tensor(RequestTensorIr),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RequestNonFiniteFloatIr {
    NaN,
    PositiveInfinity,
    NegativeInfinity,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RequestTensorIr {
    pub type_spec: String,
    pub payload: RequestTensorPayloadIr,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RequestTensorPayloadIr {
    Dense(Vec<f64>),
    Sparse(Vec<RequestTensorCellIr>),
    Mixed(Vec<RequestTensorBlockIr>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct RequestTensorCellIr {
    pub address: std::collections::BTreeMap<String, String>,
    pub value: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RequestTensorBlockIr {
    pub address: std::collections::BTreeMap<String, String>,
    pub values: Vec<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum QueryInputIr {
    Literal(RequestValueIr),
    Expression(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RankingRequestIr {
    pub profile: String,
    pub properties: std::collections::BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct QueryModelIr {
    pub query_string: Option<String>,
    pub default_index: Option<String>,
    pub encoding: Option<String>,
    pub filter: Option<String>,
    pub language: Option<LanguageHint>,
    pub locale: Option<String>,
    pub query_type: Option<QueryModelTypeIr>,
    pub sources: Vec<String>,
    pub restrict: Vec<String>,
    pub search_path: Option<String>,
    pub search_group: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct QueryModelTypeIr {
    pub composite: Option<String>,
    pub tokenization: Option<String>,
    pub syntax: Option<String>,
    pub profile: Option<String>,
    pub is_yql_default: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct PresentationIr {
    pub summary: Option<String>,
    pub format: Option<String>,
    pub bolding: Option<bool>,
    pub timing: Option<bool>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GroupingRequestIr {
    pub root: GroupingAllIr,
    pub continuations: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GroupingAllIr {
    pub operations: Vec<GroupingOperationIr>,
    pub label: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum GroupingOperationIr {
    All(GroupingAllIr),
    Each(GroupingAllIr),
    Group(GroupingExpressionIr),
    Output(Vec<GroupingOutputIr>),
    Max(Option<u32>),
    Precision(u32),
    Order(Vec<GroupingExpressionIr>),
    Alias {
        name: String,
        expression: GroupingExpressionIr,
    },
    Filter(GroupingFilterIr),
    Keep(GroupingFilterIr),
    Where(GroupingExpressionIr),
}

#[derive(Clone, Debug, PartialEq)]
pub struct GroupingOutputIr {
    pub label: Option<String>,
    pub expression: GroupingExpressionIr,
}

#[derive(Clone, Debug, PartialEq)]
pub enum GroupingExpressionIr {
    Attribute(String),
    Literal(RequestValueIr),
    Function {
        name: String,
        arguments: Vec<GroupingExpressionIr>,
    },
    Positive(Box<GroupingExpressionIr>),
    Negative(Box<GroupingExpressionIr>),
    Binary {
        operator: GroupingBinaryOperatorIr,
        left: Box<GroupingExpressionIr>,
        right: Box<GroupingExpressionIr>,
    },
    Alias(String),
    Member {
        base: Box<GroupingExpressionIr>,
        member: String,
    },
    MapLookup {
        base: Box<GroupingExpressionIr>,
        key: Box<GroupingExpressionIr>,
    },
    Bucket(GroupingBucketIr),
    Aggregation(Box<GroupingAggregationIr>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroupingBinaryOperatorIr {
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GroupingBucketIr {
    pub lower: GroupingRangeBoundIr,
    pub upper: GroupingRangeBoundIr,
    pub include_lower: bool,
    pub include_upper: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum GroupingRangeBoundIr {
    Finite(RequestValueIr),
    NegativeInfinity,
    PositiveInfinity,
    Unbounded,
}

#[derive(Clone, Debug, PartialEq)]
pub enum GroupingAggregationIr {
    Count,
    Sum(GroupingExpressionIr),
    Avg(GroupingExpressionIr),
    Min(GroupingExpressionIr),
    Max(GroupingExpressionIr),
    Xor(GroupingExpressionIr),
    StdDev(GroupingExpressionIr),
    Quantiles {
        quantiles: Vec<f64>,
        expression: GroupingExpressionIr,
    },
    Summary(Option<String>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum GroupingFilterIr {
    And(Vec<GroupingFilterIr>),
    Or(Vec<GroupingFilterIr>),
    Not(Box<GroupingFilterIr>),
    Regex {
        expression: Box<GroupingExpressionIr>,
        pattern: String,
    },
    Range {
        expression: Box<GroupingExpressionIr>,
        lower: Box<GroupingExpressionIr>,
        upper: Box<GroupingExpressionIr>,
        include_lower: bool,
        include_upper: bool,
    },
    IsTrue(Box<GroupingExpressionIr>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HighlightingIr {
    Disabled,
    Requested { fields: Vec<FieldName> },
}

#[derive(Clone, Debug, PartialEq)]
pub struct NamedStructDefinitionIr {
    pub name: String,
    pub fields: Vec<StructFieldIr>,
    pub properties: SchemaPropertiesIr,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FieldSetDefinitionIr {
    pub name: String,
    pub fields: Vec<FieldName>,
    pub matching: Option<MatchingConfigIr>,
    pub query_commands: Vec<String>,
    pub properties: SchemaPropertiesIr,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ImportedFieldDefinitionIr {
    pub reference_field: FieldName,
    pub target_field: String,
    pub alias: FieldName,
    pub properties: SchemaPropertiesIr,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DocumentSummaryFieldDefinitionIr {
    pub name: String,
    pub source: String,
    pub data_type: DataTypeIr,
    pub struct_fields: Vec<String>,
    pub properties: SchemaPropertiesIr,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DocumentSummaryDefinitionIr {
    pub name: String,
    pub inherits: Vec<String>,
    pub fields: Vec<DocumentSummaryFieldDefinitionIr>,
    pub from_disk: Option<bool>,
    pub omit_summary_features: Option<bool>,
    pub properties: SchemaPropertiesIr,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RankProfileInputIr {
    pub name: String,
    pub data_type: DataTypeIr,
    pub default_value: RequestValueIr,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RankFunctionDefinitionIr {
    pub name: String,
    pub arguments: Vec<String>,
    pub expression: String,
    pub inline: Option<bool>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RankPhaseDefinitionIr {
    pub expression: Option<String>,
    pub rerank_count: Option<u32>,
    pub rank_score_drop_limit: Option<f64>,
    pub properties: SchemaPropertiesIr,
    pub total_rerank_count: Option<u32>,
    pub keep_rank_count: Option<u32>,
    pub total_keep_rank_count: Option<u32>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ConstantSourceIr {
    Literal(RequestValueIr),
    File { path: String },
}

#[derive(Clone, Debug, PartialEq)]
pub struct ConstantDefinitionIr {
    pub name: String,
    pub data_type: DataTypeIr,
    pub source: ConstantSourceIr,
    pub properties: SchemaPropertiesIr,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OnnxModelDefinitionIr {
    pub name: String,
    pub file: String,
    pub inputs: std::collections::BTreeMap<String, String>,
    pub outputs: std::collections::BTreeMap<String, String>,
    pub gpu_device: Option<u32>,
    pub intraop_threads: Option<u32>,
    pub interop_threads: Option<u32>,
    pub properties: SchemaPropertiesIr,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RankProfileDefinitionIr {
    pub name: String,
    pub inherits: Vec<String>,
    pub inputs: Vec<RankProfileInputIr>,
    pub functions: Vec<RankFunctionDefinitionIr>,
    pub first_phase: Option<RankPhaseDefinitionIr>,
    pub second_phase: Option<RankPhaseDefinitionIr>,
    pub global_phase: Option<RankPhaseDefinitionIr>,
    pub rank_features: Vec<String>,
    pub match_features: Vec<String>,
    pub summary_features: Vec<String>,
    pub sort_features: Vec<String>,
    pub constants: Vec<ConstantDefinitionIr>,
    pub onnx_models: Vec<OnnxModelDefinitionIr>,
    pub rank_properties: std::collections::BTreeMap<String, String>,
    pub strict: Option<bool>,
    pub properties: SchemaPropertiesIr,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IndexDefinitionIr {
    pub(crate) name: Option<IndexName>,
    pub(crate) fields: Vec<FieldIr>,
    pub(crate) default_search_fields: Vec<SearchFieldIr>,
    pub(crate) default_ranking: RankingPolicyIr,
    pub(crate) default_prefix_policy: PrefixPolicy,
    pub(crate) default_typo_policy: TypoPolicy,
    pub(crate) properties: SchemaPropertiesIr,
    pub(crate) document_type: Option<String>,
    pub(crate) schema_inherits: Vec<String>,
    pub(crate) document_inherits: Vec<String>,
    pub(crate) structs: Vec<NamedStructDefinitionIr>,
    pub(crate) fieldsets: Vec<FieldSetDefinitionIr>,
    pub(crate) schema_fields: Vec<FieldIr>,
    pub(crate) imported_fields: Vec<ImportedFieldDefinitionIr>,
    pub(crate) document_summaries: Vec<DocumentSummaryDefinitionIr>,
    pub(crate) rank_profiles: Vec<RankProfileDefinitionIr>,
    pub(crate) constants: Vec<ConstantDefinitionIr>,
    pub(crate) onnx_models: Vec<OnnxModelDefinitionIr>,
}

impl IndexDefinitionIr {
    #[cfg(any(test, feature = "test-support"))]
    pub fn new(
        name: Option<IndexName>,
        fields: Vec<FieldIr>,
        default_search_fields: Vec<SearchFieldIr>,
        default_ranking: RankingPolicyIr,
        default_prefix_policy: PrefixPolicy,
        default_typo_policy: TypoPolicy,
    ) -> Result<Self> {
        if fields.is_empty() {
            return Err(PortableError::invalid(
                "invalid_schema",
                Some("fields"),
                "an index must define at least one field",
            ));
        }
        Ok(Self {
            name,
            fields,
            default_search_fields,
            default_ranking,
            default_prefix_policy,
            default_typo_policy,
            properties: SchemaPropertiesIr::new(),
            document_type: None,
            schema_inherits: Vec::new(),
            document_inherits: Vec::new(),
            structs: Vec::new(),
            fieldsets: Vec::new(),
            schema_fields: Vec::new(),
            imported_fields: Vec::new(),
            document_summaries: Vec::new(),
            rank_profiles: Vec::new(),
            constants: Vec::new(),
            onnx_models: Vec::new(),
        })
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn set_document_type(&mut self, document_type: impl Into<String>) {
        self.document_type = Some(document_type.into());
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn set_schema_fields(&mut self, fields: Vec<FieldIr>) {
        self.schema_fields = fields;
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn set_rank_profiles(&mut self, profiles: Vec<RankProfileDefinitionIr>) {
        self.rank_profiles = profiles;
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn set_imported_fields(&mut self, fields: Vec<ImportedFieldDefinitionIr>) {
        self.imported_fields = fields;
    }

    pub fn name(&self) -> Option<&IndexName> {
        self.name.as_ref()
    }

    pub fn fields(&self) -> &[FieldIr] {
        &self.fields
    }

    /// Fields that may be addressed by query-side operations.  Document
    /// fields are feedable and queryable; schema-only fields are queryable but
    /// are derived by indexing expressions and must not be accepted in a
    /// document write.
    pub(crate) fn query_fields(&self) -> impl Iterator<Item = &FieldIr> {
        self.fields.iter().chain(self.schema_fields.iter())
    }

    pub(crate) fn imported_alias(&self, name: &str) -> bool {
        self.imported_fields
            .iter()
            .any(|field| field.alias.as_str() == name)
    }

    pub fn default_search_fields(&self) -> &[SearchFieldIr] {
        &self.default_search_fields
    }

    pub fn default_ranking(&self) -> RankingPolicyIr {
        self.default_ranking
    }

    pub fn default_prefix_policy(&self) -> PrefixPolicy {
        self.default_prefix_policy
    }

    pub fn default_typo_policy(&self) -> TypoPolicy {
        self.default_typo_policy
    }

    pub fn properties(&self) -> &SchemaPropertiesIr {
        &self.properties
    }
    pub fn document_type(&self) -> Option<&str> {
        self.document_type.as_deref()
    }
    pub fn schema_inherits(&self) -> &[String] {
        &self.schema_inherits
    }
    pub fn document_inherits(&self) -> &[String] {
        &self.document_inherits
    }
    pub fn structs(&self) -> &[NamedStructDefinitionIr] {
        &self.structs
    }
    pub fn fieldsets(&self) -> &[FieldSetDefinitionIr] {
        &self.fieldsets
    }
    pub fn schema_fields(&self) -> &[FieldIr] {
        &self.schema_fields
    }
    pub fn imported_fields(&self) -> &[ImportedFieldDefinitionIr] {
        &self.imported_fields
    }
    pub fn document_summaries(&self) -> &[DocumentSummaryDefinitionIr] {
        &self.document_summaries
    }
    pub fn rank_profiles(&self) -> &[RankProfileDefinitionIr] {
        &self.rank_profiles
    }
    pub fn constants(&self) -> &[ConstantDefinitionIr] {
        &self.constants
    }
    pub fn onnx_models(&self) -> &[OnnxModelDefinitionIr] {
        &self.onnx_models
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SearchRequestIr {
    pub(crate) query: QueryIr,
    pub(crate) search_fields: Vec<SearchFieldIr>,
    pub(crate) filter: Option<FilterIr>,
    pub(crate) sort: Vec<SortIr>,
    /// V2's canonical aggregation mechanism.
    pub(crate) grouping: Vec<GroupingRequestIr>,
    pub(crate) ranking: RankingPolicyIr,
    pub(crate) ranking_request: RankingRequestIr,
    pub(crate) inputs: std::collections::BTreeMap<String, QueryInputIr>,
    pub(crate) limit: u32,
    pub(crate) offset: u32,
    pub(crate) projection: ProjectionIr,
    pub(crate) highlighting: HighlightingIr,
    pub(crate) language: Option<LanguageHint>,
    pub(crate) query_model: QueryModelIr,
    pub(crate) presentation: PresentationIr,
    pub(crate) properties: std::collections::BTreeMap<String, RequestValueIr>,
    pub(crate) query_profile: Option<String>,
    pub(crate) search_chain: Option<String>,
    pub(crate) timeout: Option<Duration>,
    pub(crate) prefix_policy: PrefixPolicy,
    pub(crate) typo_policy: TypoPolicy,
}

impl SearchRequestIr {
    #[cfg(any(test, feature = "test-support"))]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        query: QueryIr,
        search_fields: Vec<SearchFieldIr>,
        filter: Option<FilterIr>,
        sort: Vec<SortIr>,
        ranking: RankingPolicyIr,
        limit: u32,
        offset: u32,
        projection: ProjectionIr,
        highlighting: HighlightingIr,
        language: Option<LanguageHint>,
        prefix_policy: PrefixPolicy,
        typo_policy: TypoPolicy,
    ) -> Self {
        Self {
            query,
            search_fields,
            filter,
            sort,
            grouping: Vec::new(),
            ranking,
            ranking_request: RankingRequestIr {
                profile: "default".to_owned(),
                properties: std::collections::BTreeMap::new(),
            },
            inputs: std::collections::BTreeMap::new(),
            limit,
            offset,
            projection,
            highlighting,
            language,
            query_model: QueryModelIr::default(),
            presentation: PresentationIr::default(),
            properties: std::collections::BTreeMap::new(),
            query_profile: None,
            search_chain: None,
            timeout: None,
            prefix_policy,
            typo_policy,
        }
    }

    pub fn query(&self) -> &QueryIr {
        &self.query
    }

    pub fn search_fields(&self) -> &[SearchFieldIr] {
        &self.search_fields
    }

    pub fn filter(&self) -> Option<&FilterIr> {
        self.filter.as_ref()
    }

    pub fn sort(&self) -> &[SortIr] {
        &self.sort
    }

    pub fn grouping(&self) -> &[GroupingRequestIr] {
        &self.grouping
    }

    pub fn ranking(&self) -> RankingPolicyIr {
        self.ranking
    }

    pub fn ranking_request(&self) -> &RankingRequestIr {
        &self.ranking_request
    }

    pub fn inputs(&self) -> &std::collections::BTreeMap<String, QueryInputIr> {
        &self.inputs
    }

    pub fn limit(&self) -> u32 {
        self.limit
    }

    pub fn offset(&self) -> u32 {
        self.offset
    }

    pub fn projection(&self) -> &ProjectionIr {
        &self.projection
    }

    pub fn highlighting(&self) -> &HighlightingIr {
        &self.highlighting
    }

    pub fn language(&self) -> Option<&LanguageHint> {
        self.language.as_ref()
    }

    pub fn query_model(&self) -> &QueryModelIr {
        &self.query_model
    }

    pub fn presentation(&self) -> &PresentationIr {
        &self.presentation
    }

    pub fn properties(&self) -> &std::collections::BTreeMap<String, RequestValueIr> {
        &self.properties
    }

    pub fn query_profile(&self) -> Option<&str> {
        self.query_profile.as_deref()
    }

    pub fn search_chain(&self) -> Option<&str> {
        self.search_chain.as_deref()
    }

    pub fn timeout(&self) -> Option<Duration> {
        self.timeout
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn with_grouping(mut self, grouping: Vec<GroupingRequestIr>) -> Self {
        self.grouping = grouping;
        self
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn with_timeout(mut self, timeout: Option<Duration>) -> Self {
        self.timeout = timeout;
        self
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn with_ranking_request(
        mut self,
        profile: impl Into<String>,
        properties: std::collections::BTreeMap<String, String>,
    ) -> Self {
        self.ranking_request = RankingRequestIr {
            profile: profile.into(),
            properties,
        };
        self
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn with_inputs(mut self, inputs: std::collections::BTreeMap<String, QueryInputIr>) -> Self {
        self.inputs = inputs;
        self
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn with_query_model(mut self, query_model: QueryModelIr) -> Self {
        self.query_model = query_model;
        self
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn with_query(mut self, query: QueryIr) -> Self {
        self.query = query;
        self
    }

    pub fn prefix_policy(&self) -> PrefixPolicy {
        self.prefix_policy
    }

    pub fn typo_policy(&self) -> TypoPolicy {
        self.typo_policy
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnalyzeRequestIr {
    pub(crate) analyzer: AnalyzerIdentity,
    pub(crate) text: String,
    pub(crate) language: Option<LanguageHint>,
}

impl AnalyzeRequestIr {
    pub fn analyzer(&self) -> AnalyzerIdentity {
        self.analyzer
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn language(&self) -> Option<&LanguageHint> {
        self.language.as_ref()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum WriteRequestIr {
    Insert(InsertRequestIr),
    Upsert(UpsertRequestIr),
    Patch(PatchRequestIr),
    Delete(DeleteRequestIr),
}

#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq)]
pub enum OperationIr {
    Insert(InsertRequestIr),
    Upsert(UpsertRequestIr),
    Patch(PatchRequestIr),
    Delete(DeleteRequestIr),
    Get(GetRequestIr),
    Search(SearchRequestIr),
    Analyze(AnalyzeRequestIr),
}

/// Complete value representation used by owned backend results.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Int64(i64),
    UInt64(u64),
    Float64(f64),
    String(String),
    Bytes(Vec<u8>),
    List(Vec<Value>),
    Object(std::collections::BTreeMap<String, Value>),
    Tensor(RequestTensorIr),
}

#[derive(Clone, Debug, PartialEq)]
pub struct DocumentResult {
    pub id: String,
    pub fields: std::collections::BTreeMap<String, Value>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InsertResult {
    pub id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpsertResult {
    pub id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PatchResult {
    pub id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeleteResult {
    pub id: String,
    pub deleted: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GetResult {
    pub document: Option<DocumentResult>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TotalHitsRelation {
    Unspecified,
    Exact,
    LowerBound,
    Approximate,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TotalHits {
    pub value: u64,
    pub relation: TotalHitsRelation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HighlightSpan {
    pub start_byte: u32,
    pub end_byte: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HighlightFragment {
    pub text: String,
    pub matches: Vec<HighlightSpan>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HighlightResult {
    pub field: String,
    pub fragments: Vec<HighlightFragment>,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Hit {
    pub id: String,
    pub score: Option<f64>,
    pub fields: std::collections::BTreeMap<String, Value>,
    pub highlights: Vec<HighlightResult>,
    pub source: Option<String>,
    pub types: Vec<String>,
    pub match_features: std::collections::BTreeMap<String, Value>,
    pub summary_features: std::collections::BTreeMap<String, Value>,
    pub rank_features: std::collections::BTreeMap<String, Value>,
    pub sort_values: Vec<Value>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResultLimits {
    pub from: Option<Value>,
    pub to: Option<Value>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResultGroup {
    pub id: Option<String>,
    pub label: Option<String>,
    pub value: Option<Value>,
    pub fields: std::collections::BTreeMap<String, Value>,
    pub children: Vec<ResultNode>,
    pub continuations: std::collections::BTreeMap<String, String>,
    pub source: Option<String>,
    pub types: Vec<String>,
    pub score: Option<f64>,
    pub limits: Option<ResultLimits>,
    pub metadata: std::collections::BTreeMap<String, Value>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ResultNode {
    Hit(Hit),
    Group(ResultGroup),
}

#[derive(Clone, Debug, PartialEq)]
pub struct CoverageDegraded {
    pub match_phase: Option<bool>,
    pub timeout: Option<bool>,
    pub adaptive_timeout: Option<bool>,
    pub ann_timeout: Option<bool>,
    pub non_ideal_state: Option<bool>,
    pub metadata: std::collections::BTreeMap<String, Value>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Coverage {
    pub coverage_percent: Option<f64>,
    pub documents: Option<u64>,
    pub full: Option<bool>,
    pub nodes: Option<u32>,
    pub results: Option<u64>,
    pub results_full: Option<u64>,
    pub degraded: Option<CoverageDegraded>,
    pub metadata: std::collections::BTreeMap<String, Value>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SearchTiming {
    pub query_time_seconds: Option<f64>,
    pub search_time_seconds: Option<f64>,
    pub summary_fetch_time_seconds: Option<f64>,
    pub metadata: std::collections::BTreeMap<String, Value>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SearchResultError {
    pub code: i64,
    pub summary: Option<String>,
    pub message: Option<String>,
    pub source: Option<String>,
    pub stack_trace: Option<String>,
    pub transient: Option<bool>,
    pub metadata: std::collections::BTreeMap<String, Value>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SearchResult {
    pub root: ResultNode,
    pub total_hits: TotalHits,
    pub coverage: Option<Coverage>,
    pub timing: Option<SearchTiming>,
    pub errors: Vec<SearchResultError>,
    pub trace: Option<Value>,
    pub metadata: std::collections::BTreeMap<String, Value>,
}

impl ResultGroup {
    pub fn root(children: Vec<ResultNode>) -> Self {
        Self {
            id: None,
            label: None,
            value: None,
            fields: std::collections::BTreeMap::new(),
            children,
            continuations: std::collections::BTreeMap::new(),
            source: None,
            types: Vec::new(),
            score: None,
            limits: None,
            metadata: std::collections::BTreeMap::new(),
        }
    }
}

impl SearchResult {
    pub fn empty(total_hits: TotalHits) -> Self {
        Self {
            root: ResultNode::Group(ResultGroup::root(Vec::new())),
            total_hits,
            coverage: None,
            timing: None,
            errors: Vec::new(),
            trace: None,
            metadata: std::collections::BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnalyzedToken {
    pub surface: String,
    pub term: String,
    pub position: u32,
    pub start_byte: u32,
    pub end_byte: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnalyzeResult {
    pub tokens: Vec<AnalyzedToken>,
}

impl fmt::Display for FieldName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl fmt::Display for DocumentId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl fmt::Display for IndexName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}
