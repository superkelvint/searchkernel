use super::*;

mod document;
mod filter;
mod grouping;
mod projection;
mod ranking;
mod search;
use crate::api_to_ir::document::condition_to_ir;
use crate::api_to_ir::filter::{flatten_and, flatten_or};
use crate::api_to_ir::grouping::supported_grouping_function;
use crate::api_to_ir::query::normalize_typed_query;
use crate::api_to_ir::schema::analyzer_identity;
use crate::api_to_ir::search::validate_query_input_references;
use crate::filter_expression::{bind_parsed_filter, FilterBindings};
use crate::model::{
    AnalyzeRequestIr, AnalyzerIr, DeleteRequestIr, DocumentId, DocumentIr, FieldCardinality,
    FieldKind, FieldName, FieldValueIr, FilterIr, GetRequestIr, GroupingAggregationIr,
    GroupingAllIr, GroupingExpressionIr, GroupingFilterIr, GroupingOperationIr, GroupingOutputIr,
    GroupingRequestIr, HighlightingIr, InsertRequestIr, LanguageHint, MissingPlacement,
    PatchOperationIr, PatchRequestIr, PrefixPolicy, PresentationIr, ProjectionIr, QueryExprIr,
    QueryExprKindIr, QueryInputIr, QueryIr, QueryModelIr, RankingRequestIr, RequestValueIr,
    ScalarValueIr, SearchFieldIr, SearchFieldWeight, SearchRequestIr, SortDirection, SortIr,
    SortTargetIr, TypoPolicy, UpdateArithmeticOperatorIr, UpdateOperationIr, UpdateRequestIr,
    UpdateTargetIr, UpsertRequestIr,
};
use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

/// The unresolved query source shared by the direct Rust and FlatBuffers
/// ingress paths.  This is caller intent, not validated query IR.
#[doc(hidden)]
#[derive(Clone, Debug, PartialEq)]
pub enum QuerySourceInput {
    Text(String),
    MatchAll,
    Typed(QueryExprIr),
    Yql {
        source: String,
        parameters: BTreeMap<String, RequestValueIr>,
    },
}

/// One textual filter clause before schema-aware parsing and binding.
#[doc(hidden)]
#[derive(Clone, Debug, PartialEq)]
pub struct TextualFilterInput {
    pub source: String,
    pub bindings: FilterBindings,
}

/// A grouping source before request-level grouping resolution.
#[doc(hidden)]
#[derive(Clone, Debug, PartialEq)]
pub enum GroupingInput {
    Facet(FacetInput),
    Structured(GroupingRequestIr),
    Raw(String),
}

/// A query model before language normalization.
#[doc(hidden)]
#[derive(Clone, Debug, PartialEq, Default)]
pub struct QueryModelInput {
    pub query_string: Option<String>,
    pub default_index: Option<String>,
    pub encoding: Option<String>,
    pub filter: Option<String>,
    pub language: Option<String>,
    pub locale: Option<String>,
    pub query_type: Option<crate::model::QueryModelTypeIr>,
    pub sources: Vec<String>,
    pub restrict: Vec<String>,
    pub search_path: Option<String>,
    pub search_group: Option<u32>,
}

/// Presentation controls before field-aware projection/highlighting
/// resolution.
#[doc(hidden)]
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct PresentationInput {
    pub summary: Option<String>,
    pub format: Option<String>,
    pub bolding: Option<bool>,
    pub timing: Option<bool>,
}

/// The semantic input for a delete request.  This is intentionally smaller
/// than the portable request model: the FlatBuffers adapter supplies the
/// already-extracted values and the Rust facade supplies the same values
/// directly.
#[doc(hidden)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeleteInput {
    pub id: String,
    pub condition: Option<String>,
}

/// The semantic input for a get request.
#[doc(hidden)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GetInput {
    pub id: String,
    pub projection: Option<Vec<String>>,
}

/// The semantic input for a document write.
#[doc(hidden)]
#[derive(Clone, Debug, PartialEq)]
pub struct DocumentInput {
    pub id: String,
    pub fields: BTreeMap<String, RequestValueIr>,
}

/// One semantic patch operation.
#[doc(hidden)]
#[derive(Clone, Debug, PartialEq)]
pub enum PatchOperationInput {
    Set {
        field: String,
        value: RequestValueIr,
    },
    Unset {
        field: String,
    },
}

#[doc(hidden)]
#[derive(Clone, Debug, PartialEq)]
pub enum UpdateOperationInput {
    Assign {
        target: String,
        value: RequestValueIr,
    },
    Clear {
        target: String,
    },
    Arithmetic {
        target: String,
        operator: UpdateArithmeticOperatorIr,
        operand: RequestValueIr,
    },
}

/// An analyzer supplied to the semantic resolver.
#[doc(hidden)]
#[derive(Clone, Debug, PartialEq)]
pub struct AnalyzerInput {
    pub name: String,
    pub version: u32,
    pub options: BTreeMap<String, RequestValueIr>,
}

/// The analysis target supplied to the semantic resolver.
#[doc(hidden)]
#[derive(Clone, Debug, PartialEq)]
pub enum AnalyzeTargetInput {
    Analyzer(AnalyzerInput),
    Field(String),
}

/// The semantic input for an analysis request.
#[doc(hidden)]
#[derive(Clone, Debug, PartialEq)]
pub struct AnalyzeInput {
    pub target: Option<AnalyzeTargetInput>,
    pub text: String,
    pub language: Option<String>,
}

#[doc(hidden)]
#[derive(Clone, Debug, PartialEq)]
pub enum FilterBoundInput {
    Finite(RequestValueIr),
    NegativeInfinity,
    PositiveInfinity,
    Unbounded,
}

#[doc(hidden)]
#[derive(Clone, Debug, PartialEq)]
pub enum FilterInput {
    All(Vec<FilterInput>),
    Any(Vec<FilterInput>),
    Not(Box<FilterInput>),
    Comparison {
        field: String,
        op: crate::model::ComparisonOp,
        value: RequestValueIr,
    },
    Range {
        field: String,
        lower: FilterBoundInput,
        upper: FilterBoundInput,
        include_lower: Option<bool>,
        include_upper: Option<bool>,
    },
    In {
        field: String,
        values: Vec<RequestValueIr>,
    },
    Exists {
        field: String,
    },
}

#[doc(hidden)]
#[derive(Clone, Debug, PartialEq)]
pub struct SearchFieldInput {
    pub field: String,
    pub weight: Option<f64>,
}

#[doc(hidden)]
#[derive(Clone, Debug, PartialEq)]
pub enum SortTargetInput {
    Field(String),
    Relevance,
    RankFeature(String),
    Raw,
}

#[doc(hidden)]
#[derive(Clone, Debug, PartialEq)]
pub struct SortInput {
    pub target: SortTargetInput,
    pub direction: SortDirection,
    pub missing: Option<MissingPlacement>,
    pub annotations: BTreeMap<String, RequestValueIr>,
}

#[doc(hidden)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FacetSortInput {
    CountDesc,
    CountAsc,
    ValueAsc,
    ValueDesc,
}

#[doc(hidden)]
#[derive(Clone, Debug, PartialEq)]
pub struct FacetInput {
    pub field: String,
    pub name: Option<String>,
    pub limit: Option<u32>,
    pub sort: Option<FacetSortInput>,
}

#[doc(hidden)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RankingInput {
    DefaultRelevanceV1,
    Bm25,
    Named(String),
}

#[doc(hidden)]
#[derive(Clone, Debug, PartialEq)]
pub struct SearchInput {
    pub query: QuerySourceInput,
    pub search_fields: Option<Vec<SearchFieldInput>>,
    pub filters: Vec<FilterInput>,
    pub textual_filters: Vec<TextualFilterInput>,
    pub sorts: Vec<SortInput>,
    pub grouping: Vec<GroupingInput>,
    pub ranking: Option<RankingInput>,
    pub query_inputs: BTreeMap<String, QueryInputIr>,
    pub rank_properties: BTreeMap<String, String>,
    pub query_model: QueryModelInput,
    pub projection: Option<Vec<String>>,
    pub highlighting: Option<Vec<String>>,
    pub presentation: PresentationInput,
    pub properties: BTreeMap<String, RequestValueIr>,
    pub query_profile: Option<String>,
    pub search_chain: Option<String>,
    pub timeout: Option<Duration>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Compatibility name for callers that used the old unresolved search type.
#[doc(hidden)]
pub type SearchIntent = SearchInput;

pub fn resolve_delete(id: &str, condition: Option<&str>) -> Result<DeleteRequestIr> {
    let id = DocumentId::parse(id.to_owned()).map_err(|_| {
        invalid(
            "invalid_document_id",
            Some("id"),
            "document ID must not be empty",
        )
    })?;

    Ok(DeleteRequestIr {
        id,
        condition: condition_to_ir(condition),
    })
}

pub fn resolve_analyze(context: &IndexContext, input: AnalyzeInput) -> Result<AnalyzeRequestIr> {
    let analyzer = match input.target {
        Some(AnalyzeTargetInput::Analyzer(input)) => analyzer_identity(
            &AnalyzerIr {
                name: input.name,
                version: input.version,
                options: input.options,
            },
            "target.analyzer",
        )?,
        Some(AnalyzeTargetInput::Field(field)) => {
            let field = context.query_field_for_resolver(&field, "target.field".to_owned())?;
            if field.kind() != FieldKind::Text {
                return Err(invalid(
                    "invalid_analyzer",
                    Some("target.field"),
                    "analysis requires a text field",
                ));
            }
            let analyzer = field.analyzer().ok_or_else(|| {
                invalid(
                    "invalid_analyzer",
                    Some("target.field"),
                    "field has no analyzer",
                )
            })?;
            analyzer_identity(analyzer, "target.field")?
        }
        None => {
            return Err(invalid(
                "invalid_analyze_target",
                Some("target"),
                "analyze request must select an analyzer or field",
            ));
        }
    };
    let language = input
        .language
        .as_deref()
        .map(|value| resolve_language(value, "language"))
        .transpose()?;
    Ok(AnalyzeRequestIr {
        analyzer,
        text: input.text,
        language,
    })
}

use self::filter::resolve_filters;
use self::grouping::resolve_grouping;
use self::projection::resolve_get_projection;
use self::ranking::validate_ranking_profile;

pub use self::document::{
    resolve_document, resolve_get, resolve_insert, resolve_patch, resolve_update, resolve_upsert,
};
pub(crate) use self::document::{
    resolve_field_value, resolve_position_value, resolve_predicate_value,
};
pub(crate) use self::filter::resolve_filter;
pub use self::grouping::{facet_grouping_input, resolve_facets, resolve_facets_at};
pub use self::projection::resolve_projection;
pub(crate) use self::ranking::{resolve_highlighting, resolve_sort, resolve_sorts};
pub use self::search::resolve_search;
pub use self::search::DEFAULT_SEARCH_LIMIT;
pub(crate) use self::search::{resolve_language, resolve_search_fields_at};
