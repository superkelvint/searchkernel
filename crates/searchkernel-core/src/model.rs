use crate::error::{PortableError, Result};
use searchkernel_api::v1 as api;
use std::fmt;

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
    pub stored: bool,
    pub searchable: bool,
    pub filterable: bool,
    pub sortable: bool,
    pub facetable: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedField {
    pub name: FieldName,
    pub kind: FieldKind,
    pub cardinality: FieldCardinality,
    pub analyzer: Option<AnalyzerIdentity>,
    pub capabilities: FieldCapabilities,
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
pub struct ResolvedSearchField {
    pub field: FieldName,
    pub weight: SearchFieldWeight,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ResolvedScalarValue {
    Bool(bool),
    Int64(i64),
    Float64(f64),
    String(String),
}

pub type OwnedScalarValue = ResolvedScalarValue;
pub type ResolvedScalar = ResolvedScalarValue;

#[derive(Clone, Debug, PartialEq)]
pub enum ResolvedFieldValue {
    Single(OwnedScalarValue),
    Multi(Vec<OwnedScalarValue>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedDocument {
    pub id: DocumentId,
    pub fields: std::collections::BTreeMap<FieldName, ResolvedFieldValue>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedInsertRequest {
    pub document: ResolvedDocument,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedUpsertRequest {
    pub document: ResolvedDocument,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ResolvedPatchOperation {
    Set {
        field: FieldName,
        value: ResolvedFieldValue,
    },
    Unset {
        field: FieldName,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedPatchRequest {
    pub id: DocumentId,
    pub operations: Vec<ResolvedPatchOperation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedDeleteRequest {
    pub id: DocumentId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedProjection {
    pub fields: Vec<FieldName>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedGetRequest {
    pub id: DocumentId,
    pub projection: ResolvedProjection,
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
pub struct ResolvedComparison {
    pub field: FieldName,
    pub op: ComparisonOp,
    pub value: OwnedScalarValue,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedRange {
    pub field: FieldName,
    pub lower: Option<OwnedScalarValue>,
    pub upper: Option<OwnedScalarValue>,
    pub include_lower: bool,
    pub include_upper: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ResolvedFilterExpr {
    And(Vec<ResolvedFilterExpr>),
    Or(Vec<ResolvedFilterExpr>),
    Not(Box<ResolvedFilterExpr>),
    Comparison(ResolvedComparison),
    Range(ResolvedRange),
    Exists { field: FieldName },
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
pub struct ResolvedSortExpr {
    pub field: FieldName,
    pub direction: SortDirection,
    pub missing: MissingPlacement,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FacetSort {
    CountDesc,
    CountAsc,
    ValueAsc,
    ValueDesc,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedFacetRequest {
    pub field: FieldName,
    pub limit: u32,
    pub sort: FacetSort,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResolvedRankingPolicy {
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
pub enum ResolvedQuery {
    MatchAll,
    Text(String),
    Term {
        field: FieldName,
        value: OwnedScalarValue,
    },
    And(Vec<ResolvedQuery>),
    Or(Vec<ResolvedQuery>),
    Not(Box<ResolvedQuery>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResolvedHighlighting {
    Disabled,
    Requested { fields: Vec<FieldName> },
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedIndexDefinition {
    pub name: Option<IndexName>,
    pub fields: Vec<ResolvedField>,
    pub default_search_fields: Vec<ResolvedSearchField>,
    pub default_ranking: ResolvedRankingPolicy,
    pub default_prefix_policy: PrefixPolicy,
    pub default_typo_policy: TypoPolicy,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedSearchRequest {
    pub query: ResolvedQuery,
    pub search_fields: Vec<ResolvedSearchField>,
    pub filter: Option<ResolvedFilterExpr>,
    pub sort: Vec<ResolvedSortExpr>,
    pub facets: Vec<ResolvedFacetRequest>,
    pub ranking: ResolvedRankingPolicy,
    pub limit: u32,
    pub offset: u32,
    pub projection: ResolvedProjection,
    pub highlighting: ResolvedHighlighting,
    pub language: Option<LanguageHint>,
    pub prefix_policy: PrefixPolicy,
    pub typo_policy: TypoPolicy,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedAnalyzeRequest {
    pub analyzer: AnalyzerIdentity,
    pub text: String,
    pub language: Option<LanguageHint>,
}

// This is the canonical semantic operation shape from C2. Keeping each
// operation inline makes the enum pleasant for recording backends to inspect.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq)]
pub enum ResolvedOperation {
    Insert(ResolvedInsertRequest),
    Upsert(ResolvedUpsertRequest),
    Patch(ResolvedPatchRequest),
    Delete(ResolvedDeleteRequest),
    Get(ResolvedGetRequest),
    Search(ResolvedSearchRequest),
    Analyze(ResolvedAnalyzeRequest),
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

impl From<OwnedScalarValue> for api::Value {
    fn from(value: OwnedScalarValue) -> Self {
        let kind = match value {
            OwnedScalarValue::Bool(value) => api::value::Kind::BoolValue(value),
            OwnedScalarValue::Int64(value) => api::value::Kind::Int64Value(value),
            OwnedScalarValue::Float64(value) => api::value::Kind::Float64Value(value),
            OwnedScalarValue::String(value) => api::value::Kind::StringValue(value),
        };
        Self { kind: Some(kind) }
    }
}
