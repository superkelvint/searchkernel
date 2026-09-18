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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldKind {
    Text,
    Keyword,
    Int64,
    Float64,
    Bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldCardinality {
    Single,
    Multi,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnalyzerIdentity {
    StandardV1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FieldCapabilities {
    pub(crate) stored: bool,
    pub(crate) searchable: bool,
    pub(crate) filterable: bool,
    pub(crate) sortable: bool,
    pub(crate) facetable: bool,
}

impl FieldCapabilities {
    #[cfg(any(test, feature = "test-support"))]
    pub fn new(
        stored: bool,
        searchable: bool,
        filterable: bool,
        sortable: bool,
        facetable: bool,
    ) -> Self {
        Self {
            stored,
            searchable,
            filterable,
            sortable,
            facetable,
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

    pub fn facetable(&self) -> bool {
        self.facetable
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldIr {
    pub(crate) name: FieldName,
    pub(crate) kind: FieldKind,
    pub(crate) cardinality: FieldCardinality,
    pub(crate) analyzer: Option<AnalyzerIdentity>,
    pub(crate) capabilities: FieldCapabilities,
}

impl FieldIr {
    #[cfg(any(test, feature = "test-support"))]
    pub fn new(
        name: FieldName,
        kind: FieldKind,
        cardinality: FieldCardinality,
        analyzer: Option<AnalyzerIdentity>,
        capabilities: FieldCapabilities,
    ) -> Self {
        Self {
            name,
            kind,
            cardinality,
            analyzer,
            capabilities,
        }
    }

    pub fn name(&self) -> &FieldName {
        &self.name
    }

    pub fn kind(&self) -> FieldKind {
        self.kind
    }

    pub fn cardinality(&self) -> FieldCardinality {
        self.cardinality
    }

    pub fn analyzer(&self) -> Option<AnalyzerIdentity> {
        self.analyzer
    }

    pub fn capabilities(&self) -> &FieldCapabilities {
        &self.capabilities
    }
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
    /// Vespa document-selection test-and-set condition. `None` is an
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
    /// Vespa document-selection test-and-set condition. `None` is an
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
    pub(crate) lower: Option<ScalarValueIr>,
    pub(crate) upper: Option<ScalarValueIr>,
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
            lower,
            upper,
            include_lower,
            include_upper,
        }
    }

    pub fn field(&self) -> &FieldName {
        &self.field
    }

    pub fn lower(&self) -> Option<&ScalarValueIr> {
        self.lower.as_ref()
    }

    pub fn upper(&self) -> Option<&ScalarValueIr> {
        self.upper.as_ref()
    }

    pub fn include_lower(&self) -> bool {
        self.include_lower
    }

    pub fn include_upper(&self) -> bool {
        self.include_upper
    }
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MissingPlacement {
    First,
    Last,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SortIr {
    pub(crate) field: FieldName,
    pub(crate) direction: SortDirection,
    pub(crate) missing: MissingPlacement,
}

impl SortIr {
    #[cfg(any(test, feature = "test-support"))]
    pub fn new(field: FieldName, direction: SortDirection, missing: MissingPlacement) -> Self {
        Self {
            field,
            direction,
            missing,
        }
    }

    pub fn field(&self) -> &FieldName {
        &self.field
    }

    pub fn direction(&self) -> SortDirection {
        self.direction
    }

    pub fn missing(&self) -> MissingPlacement {
        self.missing
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FacetSort {
    CountDesc,
    CountAsc,
    ValueAsc,
    ValueDesc,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FacetIr {
    pub(crate) field: FieldName,
    pub(crate) limit: u32,
    pub(crate) sort: FacetSort,
}

impl FacetIr {
    #[cfg(any(test, feature = "test-support"))]
    pub fn new(field: FieldName, limit: u32, sort: FacetSort) -> Self {
        Self { field, limit, sort }
    }

    pub fn field(&self) -> &FieldName {
        &self.field
    }

    pub fn limit(&self) -> u32 {
        self.limit
    }

    pub fn sort(&self) -> FacetSort {
        self.sort
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
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HighlightingIr {
    Disabled,
    Requested { fields: Vec<FieldName> },
}

#[derive(Clone, Debug, PartialEq)]
pub struct IndexDefinitionIr {
    pub(crate) name: Option<IndexName>,
    pub(crate) fields: Vec<FieldIr>,
    pub(crate) default_search_fields: Vec<SearchFieldIr>,
    pub(crate) default_ranking: RankingPolicyIr,
    pub(crate) default_prefix_policy: PrefixPolicy,
    pub(crate) default_typo_policy: TypoPolicy,
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
        })
    }

    pub fn name(&self) -> Option<&IndexName> {
        self.name.as_ref()
    }

    pub fn fields(&self) -> &[FieldIr] {
        &self.fields
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
}

#[derive(Clone, Debug, PartialEq)]
pub struct SearchRequestIr {
    pub(crate) query: QueryIr,
    pub(crate) search_fields: Vec<SearchFieldIr>,
    pub(crate) filter: Option<FilterIr>,
    pub(crate) sort: Vec<SortIr>,
    pub(crate) facets: Vec<FacetIr>,
    pub(crate) ranking: RankingPolicyIr,
    pub(crate) limit: u32,
    pub(crate) offset: u32,
    pub(crate) projection: ProjectionIr,
    pub(crate) highlighting: HighlightingIr,
    pub(crate) language: Option<LanguageHint>,
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
        facets: Vec<FacetIr>,
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
            facets,
            ranking,
            limit,
            offset,
            projection,
            highlighting,
            language,
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

    pub fn facets(&self) -> &[FacetIr] {
        &self.facets
    }

    pub fn ranking(&self) -> RankingPolicyIr {
        self.ranking
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

    pub fn timeout(&self) -> Option<Duration> {
        self.timeout
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn with_timeout(mut self, timeout: Option<Duration>) -> Self {
        self.timeout = timeout;
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
    Exact,
    LowerBound,
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

#[derive(Clone, Debug, PartialEq)]
pub struct Hit {
    pub id: String,
    pub score: Option<f64>,
    pub fields: std::collections::BTreeMap<String, Value>,
    pub highlights: Vec<HighlightResult>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FacetValue {
    pub value: Value,
    pub count: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FacetResult {
    pub field: String,
    pub values: Vec<FacetValue>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SearchResult {
    pub hits: Vec<Hit>,
    pub total_hits: TotalHits,
    pub facets: Vec<FacetResult>,
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
