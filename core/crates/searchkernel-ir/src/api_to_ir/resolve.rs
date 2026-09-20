use super::*;
use crate::api_to_ir::document::condition_to_ir;
use crate::api_to_ir::filter::{flatten_and, flatten_or};
use crate::api_to_ir::query::normalize_typed_query;
use crate::api_to_ir::schema::analyzer_identity;
use crate::api_to_ir::search::validate_query_input_references;
use crate::filter_expression::{bind_parsed_filter, FilterBindings};
use crate::model::{
    AnalyzeRequestIr, AnalyzerIr, DeleteRequestIr, DocumentId, DocumentIr, FieldCardinality,
    FieldKind, FieldName, FieldValueIr, FilterIr, GetRequestIr, GroupingAggregationIr,
    GroupingAllIr, GroupingExpressionIr, GroupingOperationIr, GroupingOutputIr, GroupingRequestIr,
    HighlightingIr, InsertRequestIr, LanguageHint, MissingPlacement, PatchOperationIr,
    PatchRequestIr, PrefixPolicy, PresentationIr, ProjectionIr, QueryExprIr, QueryExprKindIr,
    QueryInputIr, QueryIr, QueryModelIr, RankingRequestIr, RequestValueIr, ScalarValueIr,
    SearchFieldIr, SearchFieldWeight, SearchRequestIr, SortDirection, SortIr, SortTargetIr,
    TypoPolicy, UpdateArithmeticOperatorIr, UpdateOperationIr, UpdateRequestIr, UpdateTargetIr,
    UpsertRequestIr,
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

/// Resolve a delete request independently of its transport representation.
#[doc(hidden)]
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

/// Resolve an analysis request independently of its transport representation.
#[doc(hidden)]
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

/// Resolve one complete transport-independent search request.
#[doc(hidden)]
pub fn resolve_search(context: &IndexContext, input: SearchInput) -> Result<SearchRequestIr> {
    let query = resolve_query_source(context, &input)?;
    validate_query_input_references(&query, &input.query_inputs, "query")?;
    let search_fields = resolve_search_fields_for_query(context, &input)?;
    if matches!(query, QueryIr::Text(_)) && search_fields.is_empty() {
        return Err(PortableError::failed_precondition(
            "no_searchable_fields",
            Some("search_fields"),
            "a text query requires at least one searchable field",
        ));
    }
    let filter = resolve_filters(context, &input)?;
    let sort = resolve_sorts(context, &input.sorts)?;
    let grouping = resolve_grouping(&input.grouping)?;
    let profile = match input
        .ranking
        .clone()
        .unwrap_or(RankingInput::DefaultRelevanceV1)
    {
        RankingInput::DefaultRelevanceV1 => "default".to_owned(),
        RankingInput::Bm25 => "bm25".to_owned(),
        RankingInput::Named(profile) if !profile.is_empty() => profile,
        RankingInput::Named(_) => {
            return Err(invalid(
                "invalid_ranking",
                Some("ranking"),
                "named ranking profile must not be empty",
            ));
        }
    };
    validate_ranking_profile(context, &profile)?;
    let language = input
        .query_model
        .language
        .as_deref()
        .map(|value| resolve_language(value, "query_model.language"))
        .transpose()?;
    let projection = resolve_projection(context, input.projection.as_deref())?;
    let highlighting = resolve_highlighting(context, input.highlighting.as_deref())?;
    let timeout = input.timeout.map(resolve_timeout).transpose()?;
    let (limit, offset) = resolve_pagination(input.limit, input.offset);
    validate_search_request_combinations(&input, &query)?;
    let query_model = resolve_query_model(input.query_model, language.clone());
    let presentation = resolve_presentation(input.presentation);
    Ok(SearchRequestIr {
        query,
        search_fields,
        filter,
        sort,
        grouping,
        ranking_request: RankingRequestIr {
            profile,
            properties: input.rank_properties,
        },
        inputs: input.query_inputs,
        limit,
        offset,
        projection,
        highlighting,
        language: language.clone(),
        query_model,
        presentation,
        properties: input.properties,
        query_profile: input.query_profile.filter(|value| !value.is_empty()),
        search_chain: input.search_chain.filter(|value| !value.is_empty()),
        timeout,
        prefix_policy: PrefixPolicy::Disabled,
        typo_policy: TypoPolicy::Disabled,
    })
}

fn resolve_query_source(context: &IndexContext, input: &SearchInput) -> Result<QueryIr> {
    match &input.query {
        QuerySourceInput::Text(text) if text.trim().is_empty() => Ok(QueryIr::MatchAll),
        QuerySourceInput::Text(text) => Ok(QueryIr::Text(text.clone())),
        QuerySourceInput::MatchAll => Ok(QueryIr::MatchAll),
        QuerySourceInput::Typed(query) => {
            reject_unannotated_zero_target_hits(query, "query")?;
            let mut query = query.clone();
            normalize_typed_query(&mut query, "query")?;
            context.validate_typed_query(
                &query,
                None,
                input.query_model.default_index.as_deref(),
                "query",
            )?;
            Ok(QueryIr::Typed(Box::new(query)))
        }
        QuerySourceInput::Yql { source, parameters } => Ok(QueryIr::Yql {
            source: source.clone(),
            parameters: parameters.clone(),
        }),
    }
}

fn reject_unannotated_zero_target_hits(query: &QueryExprIr, path: &str) -> Result<()> {
    if let QueryExprKindIr::WeakAnd {
        children,
        target_hits,
    } = &query.kind
    {
        if *target_hits == 0 && !query.annotations.contains_key("totalTargetHits") {
            return Err(invalid(
                "invalid_typed_query_annotation",
                Some(format!("{path}.annotations.totalTargetHits")),
                "totalTargetHits must be at least one",
            ));
        }
        for (index, child) in children.iter().enumerate() {
            reject_unannotated_zero_target_hits(
                child,
                &format!("{path}.weak_and.clauses[{index}]"),
            )?;
        }
        return Ok(());
    }

    match &query.kind {
        QueryExprKindIr::Contains { item, .. } => {
            reject_unannotated_zero_target_hits(item, &format!("{path}.contains.item"))
        }
        QueryExprKindIr::And(children) | QueryExprKindIr::Or(children) => {
            for (index, child) in children.iter().enumerate() {
                reject_unannotated_zero_target_hits(child, &format!("{path}.clauses[{index}]"))?;
            }
            Ok(())
        }
        QueryExprKindIr::Not(child) | QueryExprKindIr::NonEmpty(child) => {
            reject_unannotated_zero_target_hits(child, &format!("{path}.child"))
        }
        QueryExprKindIr::Phrase(children)
        | QueryExprKindIr::Near { children, .. }
        | QueryExprKindIr::ONear { children, .. }
        | QueryExprKindIr::SameElement(children)
        | QueryExprKindIr::Equiv(children) => {
            for (index, child) in children.iter().enumerate() {
                reject_unannotated_zero_target_hits(child, &format!("{path}.clauses[{index}]"))?;
            }
            Ok(())
        }
        QueryExprKindIr::Alternatives(values) => {
            for (index, value) in values.iter().enumerate() {
                reject_unannotated_zero_target_hits(
                    &value.query,
                    &format!("{path}.alternatives[{index}].query"),
                )?;
            }
            Ok(())
        }
        QueryExprKindIr::Rank { matching, ranking } => {
            reject_unannotated_zero_target_hits(matching, &format!("{path}.rank.match"))?;
            for (index, child) in ranking.iter().enumerate() {
                reject_unannotated_zero_target_hits(child, &format!("{path}.rank[{index}]"))?;
            }
            Ok(())
        }
        QueryExprKindIr::Labeled { query, .. } => {
            reject_unannotated_zero_target_hits(query, &format!("{path}.labeled.query"))
        }
        _ => Ok(()),
    }
}

fn resolve_search_fields_for_query(
    context: &IndexContext,
    input: &SearchInput,
) -> Result<Vec<SearchFieldIr>> {
    match (&input.query, input.search_fields.as_deref()) {
        (QuerySourceInput::Text(text), None) if !text.trim().is_empty() => {
            Ok(context.definition().default_search_fields().to_vec())
        }
        (QuerySourceInput::Text(text), None) if text.trim().is_empty() => Ok(Vec::new()),
        (QuerySourceInput::Text(_), Some(fields)) => resolve_search_fields(context, fields),
        (QuerySourceInput::MatchAll, None) | (QuerySourceInput::Yql { .. }, None) => Ok(Vec::new()),
        (QuerySourceInput::Typed(_), None) => Ok(Vec::new()),
        (QuerySourceInput::Text(_), None) => Ok(Vec::new()),
        (_, Some(_)) => Err(invalid(
            "search_fields_not_supported",
            Some("search_fields"),
            "search fields apply only to text query sources",
        )),
    }
}

fn resolve_filters(context: &IndexContext, input: &SearchInput) -> Result<Option<FilterIr>> {
    let mut clauses = input
        .filters
        .iter()
        .enumerate()
        .map(|(index, filter)| {
            resolve_filter(context, filter, &format!("filters.structured[{index}]"))
        })
        .collect::<Result<Vec<_>>>()?;
    for (index, textual) in input.textual_filters.iter().enumerate() {
        let parsed = searchkernel_filter::parse_filter(&textual.source)
            .map_err(crate::filter_expression::parse_error_to_portable)
            .map_err(|error| {
                crate::api_to_ir::filter::decorate_filter_clause_error(
                    error,
                    index,
                    &format!("filters.textual[{index}]"),
                )
            })?;
        let bound = bind_parsed_filter(&parsed, context.definition(), &textual.bindings).map_err(
            |error| {
                crate::api_to_ir::filter::decorate_filter_clause_error(
                    error,
                    index,
                    &format!("filters.textual[{index}]"),
                )
            },
        )?;
        clauses.push(bound.expression().clone());
    }
    Ok(match clauses.len() {
        0 => None,
        1 => clauses.into_iter().next(),
        _ => Some(flatten_and(clauses)),
    })
}

fn resolve_grouping(input: &[GroupingInput]) -> Result<Vec<GroupingRequestIr>> {
    input
        .iter()
        .enumerate()
        .map(|(index, value)| match value {
            GroupingInput::Facet(value) => resolve_facet(value),
            GroupingInput::Structured(value) => Ok(value.clone()),
            GroupingInput::Raw(_) => Err(PortableError::unsupported(
                "raw_grouping_not_supported",
                Some(format!("grouping[{index}].source")),
                "raw grouping text requires a grouping parser before it can enter canonical IR",
            )),
        })
        .collect()
}

fn resolve_query_model(input: QueryModelInput, language: Option<LanguageHint>) -> QueryModelIr {
    QueryModelIr {
        query_string: input.query_string,
        default_index: input.default_index,
        encoding: input.encoding,
        filter: input.filter,
        language,
        locale: input.locale,
        query_type: input.query_type,
        sources: input.sources,
        restrict: input.restrict,
        search_path: input.search_path,
        search_group: input.search_group,
    }
}

fn resolve_presentation(input: PresentationInput) -> PresentationIr {
    PresentationIr {
        summary: input.summary,
        format: input.format,
        bolding: input.bolding,
        timing: input.timing,
    }
}

fn validate_search_request_combinations(input: &SearchInput, query: &QueryIr) -> Result<()> {
    if matches!(query, QueryIr::Yql { .. }) {
        if !input.filters.is_empty() || !input.textual_filters.is_empty() {
            return Err(invalid(
                "yql_conflict",
                Some("filters"),
                "YQL cannot be combined with high-level filters",
            ));
        }
        if !input.sorts.is_empty() {
            return Err(invalid(
                "yql_conflict",
                Some("sort"),
                "YQL cannot be combined with high-level sort",
            ));
        }
        if input.projection.is_some() || input.highlighting.is_some() {
            return Err(invalid(
                "yql_conflict",
                Some("presentation"),
                "YQL owns projection and highlighting",
            ));
        }
    }
    let mut owned = BTreeSet::new();
    if input.ranking.is_some() {
        owned.insert("ranking.profile");
    }
    if input.limit.is_some() {
        owned.insert("hits");
    }
    if input.offset.is_some() {
        owned.insert("offset");
    }
    if input.timeout.is_some() {
        owned.insert("timeout");
    }
    if query_model_is_present(&input.query_model) {
        owned.insert("model");
    }
    if input.projection.is_some()
        || input.highlighting.is_some()
        || input.presentation != PresentationInput::default()
    {
        owned.insert("presentation");
    }
    if input
        .query_profile
        .as_deref()
        .is_some_and(|value| !value.is_empty())
    {
        owned.insert("queryProfile");
    }
    if input
        .search_chain
        .as_deref()
        .is_some_and(|value| !value.is_empty())
    {
        owned.insert("searchChain");
    }
    for key in input.properties.keys() {
        if owned
            .iter()
            .any(|prefix| key == *prefix || key.starts_with(&format!("{prefix}.")))
        {
            return Err(invalid(
                "property_overlaps_typed_field",
                Some(format!("properties.{key}")),
                format!("property `{key}` overlaps a populated typed request field"),
            ));
        }
    }
    Ok(())
}

fn query_model_is_present(value: &QueryModelInput) -> bool {
    value.query_string.is_some()
        || value.default_index.is_some()
        || value.encoding.is_some()
        || value.filter.is_some()
        || value.language.is_some()
        || value.locale.is_some()
        || value.query_type.is_some()
        || !value.sources.is_empty()
        || !value.restrict.is_empty()
        || value.search_path.is_some()
        || value.search_group.is_some()
}

fn validate_ranking_profile(context: &IndexContext, profile: &str) -> Result<()> {
    let configured = context
        .definition()
        .rank_profiles()
        .iter()
        .any(|candidate| candidate.name == profile);
    let generated_bm25 = profile == "bm25"
        && !context
            .definition()
            .rank_profiles()
            .iter()
            .any(|candidate| candidate.name == "bm25")
        && context
            .definition()
            .fields()
            .iter()
            .any(|field| field.kind() == FieldKind::Text);
    if profile == "default" || configured || generated_bm25 {
        return Ok(());
    }
    Err(invalid(
        "invalid_rank_profile",
        Some("ranking"),
        format!("rank profile '{profile}' is not configured for this index"),
    ))
}

pub(crate) fn resolve_search_fields(
    context: &IndexContext,
    fields: &[SearchFieldInput],
) -> Result<Vec<SearchFieldIr>> {
    resolve_search_fields_at(context, fields, "search_fields")
}

pub(crate) fn resolve_search_fields_at(
    context: &IndexContext,
    fields: &[SearchFieldInput],
    path: &str,
) -> Result<Vec<SearchFieldIr>> {
    let mut output = Vec::new();
    let mut seen = BTreeSet::new();
    for (index, input) in fields.iter().enumerate() {
        let field_path = format!("{path}.fields[{index}].field");
        let field = context.query_field_for_resolver(&input.field, field_path.clone())?;
        if !field.capabilities().searchable() {
            return Err(invalid(
                "field_not_searchable",
                Some(field_path),
                "search field must be searchable",
            ));
        }
        if !seen.insert(field.name().clone()) {
            return Err(invalid(
                "duplicate_search_field",
                Some(format!("{path}.fields[{index}].field")),
                "search fields must be unique",
            ));
        }
        let weight = SearchFieldWeight::new(input.weight.unwrap_or(1.0)).map_err(|_| {
            invalid(
                "invalid_search_weight",
                Some(format!("{path}.fields[{index}].weight")),
                "search-field weight must be finite and non-negative",
            )
        })?;
        output.push(SearchFieldIr {
            field: field.name().clone(),
            weight,
        });
    }
    Ok(output)
}

pub(crate) fn resolve_filter(
    context: &IndexContext,
    input: &FilterInput,
    path: &str,
) -> Result<FilterIr> {
    match input {
        FilterInput::All(filters) => {
            if filters.is_empty() {
                return Err(invalid(
                    "empty_boolean_filter",
                    Some(format!("{path}.all")),
                    "all requires at least one clause",
                ));
            }
            Ok(flatten_and(
                filters
                    .iter()
                    .enumerate()
                    .map(|(index, filter)| {
                        resolve_filter(context, filter, &format!("{path}.all[{index}]"))
                    })
                    .collect::<Result<Vec<_>>>()?,
            ))
        }
        FilterInput::Any(filters) => {
            if filters.is_empty() {
                return Err(invalid(
                    "empty_boolean_filter",
                    Some(format!("{path}.any")),
                    "any requires at least one clause",
                ));
            }
            Ok(flatten_or(
                filters
                    .iter()
                    .enumerate()
                    .map(|(index, filter)| {
                        resolve_filter(context, filter, &format!("{path}.any[{index}]"))
                    })
                    .collect::<Result<Vec<_>>>()?,
            ))
        }
        FilterInput::Not(filter) => Ok(FilterIr::Not(Box::new(resolve_filter(
            context,
            filter,
            &format!("{path}.negation.clause"),
        )?))),
        FilterInput::Exists { field } => {
            let field = context.query_field_for_resolver(field, format!("{path}.field"))?;
            if !field.capabilities().filterable() {
                return Err(invalid(
                    "field_not_filterable",
                    Some(format!("{path}.field")),
                    "exists requires a filterable field",
                ));
            }
            Ok(FilterIr::Exists {
                field: field.name().clone(),
            })
        }
        FilterInput::Comparison { field, op, value } => {
            let schema_field = context.query_field_for_resolver(field, format!("{path}.field"))?;
            if !schema_field.capabilities().filterable() {
                return Err(invalid(
                    "field_not_filterable",
                    Some(format!("{path}.field")),
                    "comparison requires a filterable field",
                ));
            }
            if matches!(
                op,
                crate::model::ComparisonOp::Lt
                    | crate::model::ComparisonOp::Lte
                    | crate::model::ComparisonOp::Gt
                    | crate::model::ComparisonOp::Gte
            ) && !matches!(schema_field.kind(), FieldKind::Int64 | FieldKind::Float64)
            {
                return Err(invalid(
                    "invalid_filter",
                    Some(format!("{path}.op")),
                    "ordered comparison requires a numeric field",
                ));
            }
            Ok(FilterIr::Comparison(crate::model::ComparisonIr {
                field: schema_field.name().clone(),
                op: *op,
                value: resolve_scalar(value.clone(), schema_field.kind(), format!("{path}.value"))?,
            }))
        }
        FilterInput::In { field, values } => {
            let schema_field = context.query_field_for_resolver(field, format!("{path}.field"))?;
            if !schema_field.capabilities().filterable() {
                return Err(invalid(
                    "field_not_filterable",
                    Some(format!("{path}.field")),
                    "IN requires a filterable field",
                ));
            }
            if !matches!(schema_field.kind(), FieldKind::Keyword | FieldKind::Int64) {
                return Err(invalid(
                    "invalid_filter",
                    Some(path),
                    "IN requires a Keyword or Int64 field",
                ));
            }
            let values = values
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    resolve_scalar(
                        value.clone(),
                        schema_field.kind(),
                        format!("{path}.values[{index}]"),
                    )
                })
                .collect::<Result<Vec<_>>>()?;
            if values.is_empty() {
                return Err(invalid(
                    "empty_in",
                    Some(path),
                    "IN requires at least one value",
                ));
            }
            Ok(FilterIr::In {
                field: schema_field.name().clone(),
                values,
            })
        }
        FilterInput::Range {
            field,
            lower,
            upper,
            include_lower,
            include_upper,
        } => {
            let schema_field = context.query_field_for_resolver(field, format!("{path}.field"))?;
            if !schema_field.capabilities().filterable() {
                return Err(invalid(
                    "field_not_filterable",
                    Some(format!("{path}.field")),
                    "range requires a filterable field",
                ));
            }
            if !matches!(schema_field.kind(), FieldKind::Int64 | FieldKind::Float64) {
                return Err(invalid(
                    "invalid_filter",
                    Some(path),
                    "range requires an Int64 or Float64 field",
                ));
            }
            let lower = resolve_range_bound(lower, schema_field.kind(), &format!("{path}.lower"))?;
            let upper = resolve_range_bound(upper, schema_field.kind(), &format!("{path}.upper"))?;
            if matches!(lower, crate::model::FilterRangeBoundIr::Unbounded)
                && matches!(upper, crate::model::FilterRangeBoundIr::Unbounded)
            {
                return Err(invalid(
                    "invalid_range",
                    Some(path),
                    "range requires at least one bound",
                ));
            }
            let explicit_lower_inclusion = *include_lower == Some(true);
            let explicit_upper_inclusion = *include_upper == Some(true);
            let include_lower = (*include_lower).unwrap_or(true);
            let include_upper = (*include_upper).unwrap_or(true);
            if matches!(lower, crate::model::FilterRangeBoundIr::Unbounded)
                && include_lower
                && explicit_lower_inclusion
            {
                return Err(invalid(
                    "invalid_range",
                    Some(format!("{path}.include_lower")),
                    "lower inclusion requires a lower bound",
                ));
            }
            if matches!(upper, crate::model::FilterRangeBoundIr::Unbounded)
                && include_upper
                && explicit_upper_inclusion
            {
                return Err(invalid(
                    "invalid_range",
                    Some(format!("{path}.include_upper")),
                    "upper inclusion requires an upper bound",
                ));
            }
            if crate::api_to_ir::filter::filter_bound_greater(&lower, &upper) {
                return Err(invalid(
                    "invalid_range",
                    Some(path),
                    "range lower bound must not exceed upper bound",
                ));
            }
            Ok(FilterIr::Range(crate::model::RangeIr {
                field: schema_field.name().clone(),
                lower,
                upper,
                include_lower,
                include_upper,
            }))
        }
    }
}

fn resolve_scalar(value: RequestValueIr, kind: FieldKind, path: String) -> Result<ScalarValueIr> {
    let scalar = match value {
        RequestValueIr::Bool(value) => ScalarValueIr::Bool(value),
        RequestValueIr::Int64(value) => ScalarValueIr::Int64(value),
        RequestValueIr::Float64(value) if value.is_finite() => ScalarValueIr::Float64(value),
        RequestValueIr::String(value) => ScalarValueIr::String(value),
        _ => {
            return Err(invalid(
                "type_mismatch",
                Some(path),
                "filter value must be scalar",
            ))
        }
    };
    let valid = matches!(
        (&scalar, kind),
        (
            ScalarValueIr::String(_),
            FieldKind::Text | FieldKind::Keyword
        ) | (ScalarValueIr::Int64(_), FieldKind::Int64)
            | (ScalarValueIr::Float64(_), FieldKind::Float64)
            | (ScalarValueIr::Bool(_), FieldKind::Bool)
    );
    if !valid {
        return Err(invalid(
            "type_mismatch",
            Some(path),
            "filter value type does not match field kind",
        ));
    }
    Ok(scalar)
}

fn resolve_range_bound(
    value: &FilterBoundInput,
    kind: FieldKind,
    path: &str,
) -> Result<crate::model::FilterRangeBoundIr> {
    match value {
        FilterBoundInput::Finite(value) => resolve_scalar(value.clone(), kind, path.to_owned())
            .map(crate::model::FilterRangeBoundIr::Finite),
        FilterBoundInput::NegativeInfinity => {
            Ok(crate::model::FilterRangeBoundIr::NegativeInfinity)
        }
        FilterBoundInput::PositiveInfinity => {
            Ok(crate::model::FilterRangeBoundIr::PositiveInfinity)
        }
        FilterBoundInput::Unbounded => Ok(crate::model::FilterRangeBoundIr::Unbounded),
    }
}

pub(crate) fn resolve_sort(
    context: &IndexContext,
    input: &SortInput,
    path: &str,
) -> Result<SortIr> {
    let target = match &input.target {
        SortTargetInput::Field(name) => {
            let field = context.query_field_for_resolver(name, format!("{path}.field"))?;
            if !field.capabilities().sortable() {
                return Err(invalid(
                    "field_not_sortable",
                    Some(format!("{path}.field")),
                    "sort requires a sortable field",
                ));
            }
            if field.cardinality() != FieldCardinality::Single {
                return Err(invalid(
                    "invalid_capability",
                    Some(format!("{path}.field")),
                    "multi-valued fields cannot be sortable",
                ));
            }
            SortTargetIr::Field(field.name().clone())
        }
        SortTargetInput::Relevance => SortTargetIr::Relevance,
        SortTargetInput::RankFeature(feature) if !feature.is_empty() => {
            SortTargetIr::RankFeature(feature.clone())
        }
        SortTargetInput::RankFeature(_) => {
            return Err(invalid(
                "invalid_sort",
                Some(format!("{path}.target.feature")),
                "rank-feature sort requires a non-empty feature name",
            ));
        }
        SortTargetInput::Raw => {
            return Err(PortableError::unsupported(
                "raw_sort_not_supported",
                Some(format!("{path}.target")),
                "raw sort expressions must be normalized before entering resolved IR",
            ));
        }
    };
    Ok(SortIr {
        target,
        direction: input.direction,
        missing: input.missing.clone().unwrap_or(MissingPlacement::Last),
        annotations: input.annotations.clone(),
    })
}

pub(crate) fn resolve_sorts(context: &IndexContext, sorts: &[SortInput]) -> Result<Vec<SortIr>> {
    let mut seen = BTreeSet::new();
    sorts
        .iter()
        .enumerate()
        .map(|(index, input)| {
            let value = resolve_sort(context, input, &format!("sort[{index}]"))?;
            if value.field().is_some() && !seen.insert(value.field().cloned()) {
                return Err(invalid(
                    "duplicate_sort_field",
                    Some(format!("sort[{index}].field")),
                    "sort fields must be unique",
                ));
            }
            Ok(value)
        })
        .collect()
}

pub(crate) fn resolve_highlighting(
    context: &IndexContext,
    fields: Option<&[String]>,
) -> Result<HighlightingIr> {
    let Some(fields) = fields else {
        return Ok(HighlightingIr::Disabled);
    };
    let mut output = Vec::new();
    let mut seen = BTreeSet::new();
    for (index, name) in fields.iter().enumerate() {
        let field =
            context.query_field_for_resolver(name, format!("highlighting.fields[{index}]"))?;
        if field.kind() != FieldKind::Text || !field.capabilities().searchable() {
            return Err(invalid(
                "invalid_highlight",
                Some(format!("highlighting.fields[{index}]")),
                "highlighting requires a searchable text field",
            ));
        }
        if !field.capabilities().stored() {
            return Err(invalid(
                "field_not_stored",
                Some(format!("highlighting.fields[{index}]")),
                "highlighting requires a stored field",
            ));
        }
        if !seen.insert(field.name().clone()) {
            return Err(invalid(
                "duplicate_highlight_field",
                Some(format!("highlighting.fields[{index}]")),
                "highlight fields must be unique",
            ));
        }
        output.push(field.name().clone());
    }
    Ok(HighlightingIr::Requested { fields: output })
}

fn resolve_facet(input: &FacetInput) -> Result<GroupingRequestIr> {
    let field = GroupingExpressionIr::Attribute(input.field.clone());
    let count = GroupingExpressionIr::Aggregation(Box::new(GroupingAggregationIr::Count));
    let order = match input.sort.unwrap_or(FacetSortInput::CountDesc) {
        FacetSortInput::CountDesc | FacetSortInput::ValueDesc => GroupingExpressionIr::Negative(
            Box::new(if matches!(input.sort, Some(FacetSortInput::ValueDesc)) {
                field.clone()
            } else {
                count.clone()
            }),
        ),
        FacetSortInput::CountAsc => count.clone(),
        FacetSortInput::ValueAsc => field.clone(),
    };
    let operations = vec![
        GroupingOperationIr::Group(field.clone()),
        GroupingOperationIr::Each(GroupingAllIr {
            operations: vec![
                GroupingOperationIr::Max(Some(input.limit.unwrap_or(10))),
                GroupingOperationIr::Order(vec![order]),
                GroupingOperationIr::Output(vec![GroupingOutputIr {
                    label: None,
                    expression: count,
                }]),
            ],
            label: None,
        }),
    ];
    Ok(GroupingRequestIr {
        root: GroupingAllIr {
            operations,
            label: None,
        },
        continuations: Vec::new(),
    })
}

pub(crate) fn resolve_language(value: &str, path: &str) -> Result<LanguageHint> {
    LanguageHint::parse(value.to_owned()).map_err(|_| {
        invalid(
            "invalid_language",
            Some(path),
            "language must be a basic BCP-47-style ASCII tag",
        )
    })
}

pub(crate) fn resolve_pagination(limit: Option<u32>, offset: Option<u32>) -> (u32, u32) {
    (limit.unwrap_or(20), offset.unwrap_or(0))
}

pub(crate) fn resolve_timeout(timeout: Duration) -> Result<Duration> {
    let millis = u64::try_from(timeout.as_millis()).map_err(|_| {
        invalid(
            "invalid_argument",
            Some("timeout"),
            "search timeout is too large to encode",
        )
    })?;
    if !timeout.is_zero() && millis == 0 {
        return Err(invalid(
            "invalid_argument",
            Some("timeout"),
            "search timeout must be at least one millisecond",
        ));
    }
    Ok(Duration::from_millis(millis))
}

/// Resolve a get request independently of its transport representation.
#[doc(hidden)]
pub fn resolve_get(context: &IndexContext, input: GetInput) -> Result<GetRequestIr> {
    let id = DocumentId::parse(input.id).map_err(|_| {
        invalid(
            "invalid_document_id",
            Some("id"),
            "document ID must not be empty",
        )
    })?;
    let projection = resolve_get_projection(context, input.projection.as_deref())?;
    Ok(GetRequestIr { id, projection })
}

/// Resolve a document against the index schema.
#[doc(hidden)]
pub fn resolve_document(context: &IndexContext, input: DocumentInput) -> Result<DocumentIr> {
    let id = DocumentId::parse(input.id).map_err(|_| {
        invalid(
            "invalid_document_id",
            Some("document.id"),
            "document ID must not be empty",
        )
    })?;
    let mut fields = BTreeMap::new();
    for (name, value) in input.fields {
        let field = context.field_for_resolver(&name, format!("document.fields.{name}"))?;
        let value = resolve_field_value(field, value, format!("document.fields.{name}"))?;
        fields.insert(field.name().clone(), value);
    }
    Ok(DocumentIr { id, fields })
}

/// Resolve an insert request against the index schema.
#[doc(hidden)]
pub fn resolve_insert(context: &IndexContext, input: DocumentInput) -> Result<InsertRequestIr> {
    resolve_document(context, input).map(|document| InsertRequestIr { document })
}

/// Resolve an upsert request against the index schema.
#[doc(hidden)]
pub fn resolve_upsert(
    context: &IndexContext,
    input: DocumentInput,
    condition: Option<&str>,
) -> Result<UpsertRequestIr> {
    resolve_document(context, input).map(|document| UpsertRequestIr {
        document,
        condition: condition_to_ir(condition),
    })
}

/// Resolve a patch against the index schema.
#[doc(hidden)]
pub fn resolve_patch(
    context: &IndexContext,
    id: &str,
    operations: &[PatchOperationInput],
) -> Result<PatchRequestIr> {
    if operations.is_empty() {
        return Err(invalid(
            "empty_patch",
            Some("operations"),
            "patch must contain at least one operation",
        ));
    }
    let id = DocumentId::parse(id.to_owned()).map_err(|_| {
        invalid(
            "invalid_document_id",
            Some("id"),
            "document ID must not be empty",
        )
    })?;
    let mut seen = BTreeSet::new();
    let mut resolved = Vec::with_capacity(operations.len());
    for (index, operation) in operations.iter().enumerate() {
        let path = format!("operations[{index}]");
        match operation {
            PatchOperationInput::Set { field, value } => {
                let schema_field =
                    context.field_for_resolver(field, format!("{path}.set.field"))?;
                ensure_field_is_not_repeated(&mut seen, schema_field.name(), &path)?;
                resolved.push(PatchOperationIr::Set {
                    field: schema_field.name().clone(),
                    value: resolve_field_value(
                        schema_field,
                        value.clone(),
                        format!("{path}.set.value"),
                    )?,
                });
            }
            PatchOperationInput::Unset { field } => {
                let schema_field =
                    context.field_for_resolver(field, format!("{path}.unset.field"))?;
                ensure_field_is_not_repeated(&mut seen, schema_field.name(), &path)?;
                resolved.push(PatchOperationIr::Unset {
                    field: schema_field.name().clone(),
                });
            }
        }
    }
    Ok(PatchRequestIr {
        id,
        operations: resolved,
    })
}

/// Resolve a full document update against the index schema.
#[doc(hidden)]
pub fn resolve_update(
    context: &IndexContext,
    id: &str,
    operations: &[UpdateOperationInput],
    condition: Option<&str>,
    create: Option<bool>,
) -> Result<UpdateRequestIr> {
    if operations.is_empty() {
        return Err(invalid(
            "empty_update",
            Some("operations"),
            "update must contain at least one operation",
        ));
    }
    let id = DocumentId::parse(id.to_owned()).map_err(|_| {
        invalid(
            "invalid_document_id",
            Some("id"),
            "document ID must not be empty",
        )
    })?;
    let mut seen = BTreeSet::new();
    let mut resolved = Vec::with_capacity(operations.len());
    for (index, operation) in operations.iter().enumerate() {
        let path = format!("operations[{index}]");
        let target = match operation {
            UpdateOperationInput::Assign { target, .. }
            | UpdateOperationInput::Clear { target }
            | UpdateOperationInput::Arithmetic { target, .. } => target,
        };
        let target = UpdateTargetIr::parse(target.clone()).map_err(|_| {
            invalid(
                "invalid_update_target",
                Some(format!("{path}.target.path")),
                "update target must not be empty",
            )
        })?;
        if !target.is_simple_field() {
            return Err(PortableError::unsupported(
                "update_field_path_not_supported",
                Some(format!("{path}.target.path")),
                "the current native DocumentUpdate adapter supports simple fields only",
            ));
        }
        if !seen.insert(target.as_str().to_owned()) {
            return Err(PortableError::unsupported(
                "multiple_updates_per_target_not_supported",
                Some(path),
                "ordered multiple updates for one target are not supported",
            ));
        }
        let field = context.field_for_resolver(target.as_str(), format!("{path}.target.path"))?;
        match operation {
            UpdateOperationInput::Assign { value, .. } => {
                resolved.push(UpdateOperationIr::Assign {
                    target,
                    value: resolve_field_value(
                        field,
                        value.clone(),
                        format!("{path}.operation.value"),
                    )?,
                })
            }
            UpdateOperationInput::Clear { .. } => {
                resolved.push(UpdateOperationIr::Clear { target })
            }
            UpdateOperationInput::Arithmetic {
                operator, operand, ..
            } => {
                if !matches!(field.kind(), FieldKind::Int64 | FieldKind::Float64) {
                    return Err(invalid(
                        "arithmetic_requires_numeric_field",
                        Some(format!("{path}.target.path")),
                        "arithmetic updates require an integer or float field",
                    ));
                }
                let operand = match operand {
                    RequestValueIr::Int64(value) => *value as f64,
                    RequestValueIr::UInt64(value) => *value as f64,
                    RequestValueIr::Float64(value) if value.is_finite() => *value,
                    _ => {
                        return Err(invalid(
                            "arithmetic_operand_type",
                            Some(format!("{path}.operation.operand")),
                            "arithmetic operands must be finite numbers",
                        ));
                    }
                };
                resolved.push(UpdateOperationIr::Arithmetic {
                    target,
                    operator: *operator,
                    operand,
                });
            }
        }
    }
    Ok(UpdateRequestIr {
        id,
        operations: resolved,
        condition: condition_to_ir(condition),
        create,
    })
}

fn ensure_field_is_not_repeated(
    seen_fields: &mut BTreeSet<FieldName>,
    field: &FieldName,
    path: &str,
) -> Result<()> {
    if !seen_fields.insert(field.clone()) {
        return Err(invalid(
            "duplicate_patch_field",
            Some(path),
            "a patch may mention each field at most once",
        ));
    }
    Ok(())
}

pub(crate) fn resolve_field_value(
    field: &crate::model::FieldIr,
    value: RequestValueIr,
    path: String,
) -> Result<FieldValueIr> {
    match field.data_type() {
        crate::model::DataTypeIr::Position => return resolve_position_value(value, path),
        crate::model::DataTypeIr::Predicate => return resolve_predicate_value(value, path),
        _ => {}
    }
    if matches!(field.data_type(), crate::model::DataTypeIr::Tensor { .. }) {
        return match value {
            RequestValueIr::Tensor(value) => Ok(FieldValueIr::Tensor(value)),
            _ => Err(invalid(
                "type_mismatch",
                Some(path),
                "tensor fields require tensor values",
            )),
        };
    }
    let list = match (&field.cardinality(), value) {
        (FieldCardinality::Single, RequestValueIr::List(_)) => {
            return Err(invalid(
                "cardinality_mismatch",
                Some(path),
                "single-valued fields require a scalar value",
            ));
        }
        (FieldCardinality::Multi, RequestValueIr::List(values)) => {
            let values = values
                .into_iter()
                .enumerate()
                .map(|(index, value)| {
                    resolve_scalar_value(value, field.kind(), format!("{path}.values[{index}]"))
                })
                .collect::<Result<Vec<_>>>()?;
            return Ok(FieldValueIr::Multi(values));
        }
        (FieldCardinality::Multi, _) => {
            return Err(invalid(
                "cardinality_mismatch",
                Some(path),
                "multi-valued fields require a list value",
            ));
        }
        (FieldCardinality::Single, value) => value,
    };
    Ok(FieldValueIr::Single(resolve_scalar_value(
        list,
        field.kind(),
        path,
    )?))
}

pub(crate) fn resolve_position_value(value: RequestValueIr, path: String) -> Result<FieldValueIr> {
    let RequestValueIr::Object(mut fields) = value else {
        return Err(invalid(
            "type_mismatch",
            Some(path),
            "position fields require an object with latitude and longitude",
        ));
    };
    let latitude = resolve_position_coordinate(fields.remove("latitude"), "latitude", &path)?;
    let longitude = resolve_position_coordinate(fields.remove("longitude"), "longitude", &path)?;
    if !fields.is_empty() {
        return Err(invalid(
            "invalid_value",
            Some(path),
            "position objects may contain only latitude and longitude",
        ));
    }
    Ok(FieldValueIr::Position {
        latitude,
        longitude,
    })
}

fn resolve_position_coordinate(
    value: Option<RequestValueIr>,
    name: &str,
    path: &str,
) -> Result<f64> {
    let Some(value) = value else {
        return Err(invalid(
            "missing_value",
            Some(format!("{path}.{name}")),
            format!("position {name} is required"),
        ));
    };
    let value = match value {
        RequestValueIr::Int64(value) => value as f64,
        RequestValueIr::Float64(value) if value.is_finite() => value,
        _ => {
            return Err(invalid(
                "type_mismatch",
                Some(format!("{path}.{name}")),
                "position coordinates must be finite numbers",
            ));
        }
    };
    Ok(value)
}

pub(crate) fn resolve_predicate_value(value: RequestValueIr, path: String) -> Result<FieldValueIr> {
    let RequestValueIr::Object(mut fields) = value else {
        return Err(invalid(
            "type_mismatch",
            Some(path),
            "predicate fields require a Vespa predicate expression object or a features object",
        ));
    };
    if let Some(expression) = fields.remove("expression") {
        if !fields.is_empty() {
            return Err(invalid(
                "invalid_value",
                Some(path.clone()),
                "predicate expression objects may contain only expression",
            ));
        }
        let RequestValueIr::Object(expression) = expression else {
            return Err(invalid(
                "type_mismatch",
                Some(format!("{path}.expression")),
                "predicate expression must be an object",
            ));
        };
        return Ok(FieldValueIr::PredicateExpression {
            expression: serde_json::to_string(&request_value_to_json(
                RequestValueIr::Object(expression),
                &format!("{path}.expression"),
            )?)
            .map_err(|error| invalid("invalid_value", Some(path), error.to_string()))?,
        });
    }
    if fields.contains_key("type") {
        return Ok(FieldValueIr::PredicateExpression {
            expression: serde_json::to_string(&request_value_to_json(
                RequestValueIr::Object(fields),
                &path,
            )?)
            .map_err(|error| invalid("invalid_value", Some(path), error.to_string()))?,
        });
    }
    let Some(RequestValueIr::Object(features)) = fields.remove("features") else {
        return Err(invalid(
            "missing_value",
            Some(format!("{path}.features")),
            "predicate features are required",
        ));
    };
    if !fields.is_empty() {
        return Err(invalid(
            "invalid_value",
            Some(path.clone()),
            "predicate objects may contain only features",
        ));
    }
    let features = features
        .into_iter()
        .map(|(key, value)| match value {
            RequestValueIr::String(value) => Ok((key, value)),
            _ => Err(invalid(
                "type_mismatch",
                Some(format!("{path}.features.{key}")),
                "predicate feature values must be strings",
            )),
        })
        .collect::<Result<std::collections::BTreeMap<_, _>>>()?;
    if features.is_empty() {
        return Err(invalid(
            "invalid_value",
            Some(format!("{path}.features")),
            "predicate fields require at least one feature",
        ));
    }
    Ok(FieldValueIr::Predicate { features })
}

fn request_value_to_json(value: RequestValueIr, path: &str) -> Result<serde_json::Value> {
    match value {
        RequestValueIr::Null => Ok(serde_json::Value::Null),
        RequestValueIr::Bool(value) => Ok(serde_json::Value::Bool(value)),
        RequestValueIr::Int64(value) => Ok(serde_json::Value::Number(value.into())),
        RequestValueIr::UInt64(value) => Ok(serde_json::Value::Number(value.into())),
        RequestValueIr::Float64(value) if value.is_finite() => {
            Ok(serde_json::Number::from_f64(value)
                .map_or(serde_json::Value::Null, serde_json::Value::Number))
        }
        RequestValueIr::String(value) => Ok(serde_json::Value::String(value)),
        RequestValueIr::List(values) => values
            .into_iter()
            .enumerate()
            .map(|(index, value)| request_value_to_json(value, &format!("{path}[{index}]")))
            .collect::<Result<Vec<_>>>()
            .map(serde_json::Value::Array),
        RequestValueIr::Object(values) => values
            .into_iter()
            .map(|(name, value)| {
                request_value_to_json(value, &format!("{path}.{name}")).map(|value| (name, value))
            })
            .collect::<Result<serde_json::Map<_, _>>>()
            .map(serde_json::Value::Object),
        RequestValueIr::Float64(_)
        | RequestValueIr::NonFiniteFloat(_)
        | RequestValueIr::Bytes(_)
        | RequestValueIr::Tensor(_) => Err(invalid(
            "unsupported_value",
            Some(path.to_owned()),
            "predicate expressions support JSON scalar, list, and object values only",
        )),
    }
}

fn resolve_scalar_value(
    value: RequestValueIr,
    kind: FieldKind,
    path: String,
) -> Result<ScalarValueIr> {
    let scalar = match value {
        RequestValueIr::Bool(value) => ScalarValueIr::Bool(value),
        RequestValueIr::Int64(value) => ScalarValueIr::Int64(value),
        RequestValueIr::Float64(value) if value.is_finite() => ScalarValueIr::Float64(value),
        RequestValueIr::String(value) => ScalarValueIr::String(value),
        _ => {
            return Err(invalid(
                "type_mismatch",
                Some(path),
                "value form is not supported for this field",
            ));
        }
    };
    let matches = matches!(
        (&scalar, kind),
        (
            ScalarValueIr::String(_),
            FieldKind::Text | FieldKind::Keyword
        ) | (ScalarValueIr::Int64(_), FieldKind::Int64)
            | (ScalarValueIr::Float64(_), FieldKind::Float64)
            | (ScalarValueIr::Bool(_), FieldKind::Bool)
    );
    if !matches {
        return Err(invalid(
            "type_mismatch",
            Some(path),
            "value type does not match field kind",
        ));
    }
    Ok(scalar)
}

/// Resolve a field projection using the index schema.  `None` means the
/// effective stored-field projection; `Some(empty)` remains an explicit empty
/// projection.
#[doc(hidden)]
pub fn resolve_projection(
    context: &IndexContext,
    fields: Option<&[String]>,
) -> Result<ProjectionIr> {
    resolve_projection_inner(context, fields, false)
}

fn resolve_get_projection(
    context: &IndexContext,
    fields: Option<&[String]>,
) -> Result<ProjectionIr> {
    resolve_projection_inner(context, fields, true)
}

fn resolve_projection_inner(
    context: &IndexContext,
    fields: Option<&[String]>,
    allow_tensor: bool,
) -> Result<ProjectionIr> {
    let mut output = Vec::new();
    match fields {
        None => output.extend(
            context
                .definition()
                .query_fields()
                .filter(|field| {
                    field.capabilities().stored()
                        && (allow_tensor
                            || !matches!(
                                field.data_type(),
                                crate::model::DataTypeIr::Tensor { .. }
                            ))
                })
                .map(|field| field.name().clone()),
        ),
        Some(fields) => {
            let mut seen = std::collections::BTreeSet::new();
            for (index, name) in fields.iter().enumerate() {
                let field = context
                    .query_field_for_resolver(name, format!("projection.fields[{index}]"))?;
                if !field.capabilities().stored() {
                    return Err(invalid(
                        "field_not_stored",
                        Some(format!("projection.fields[{index}]")),
                        "projection requires a stored field",
                    ));
                }
                if !allow_tensor
                    && matches!(field.data_type(), crate::model::DataTypeIr::Tensor { .. })
                {
                    return Err(PortableError::unsupported(
                        "tensor_search_projection_not_supported",
                        Some(format!("projection.fields[{index}]")),
                        "tensor fields are readable through get but are not native summary fields",
                    ));
                }
                if !seen.insert(field.name().clone()) {
                    return Err(invalid(
                        "duplicate_projection_field",
                        Some(format!("projection.fields[{index}]")),
                        "projection fields must be unique",
                    ));
                }
                output.push(field.name().clone());
            }
        }
    }
    Ok(ProjectionIr { fields: output })
}
