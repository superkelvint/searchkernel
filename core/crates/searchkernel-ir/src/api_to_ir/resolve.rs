use super::*;
use crate::api_to_ir::document::condition_to_ir;
use crate::api_to_ir::filter::{flatten_and, flatten_or};
use crate::api_to_ir::schema::analyzer_identity;
use crate::api_to_ir::search::parse_language;
use crate::model::{
    AnalyzeRequestIr, AnalyzerIr, DeleteRequestIr, DocumentId, DocumentIr, FieldCardinality,
    FieldKind, FieldName, FieldValueIr, FilterIr, GetRequestIr, GroupingAggregationIr,
    GroupingAllIr, GroupingExpressionIr, GroupingOperationIr, GroupingOutputIr, GroupingRequestIr,
    HighlightingIr, InsertRequestIr, MissingPlacement, PatchOperationIr, PatchRequestIr,
    PrefixPolicy, PresentationIr, ProjectionIr, QueryIr, QueryModelIr, RankingPolicyIr,
    RankingRequestIr, RequestValueIr, ScalarValueIr, SearchFieldIr, SearchFieldWeight,
    SearchRequestIr, SortDirection, SortIr, SortTargetIr, TypoPolicy, UpsertRequestIr,
};
use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

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
        lower: Option<RequestValueIr>,
        upper: Option<RequestValueIr>,
        include_lower: Option<bool>,
        include_upper: Option<bool>,
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
pub struct SortInput {
    pub field: String,
    pub direction: SortDirection,
    pub missing: Option<MissingPlacement>,
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RankingInput {
    DefaultRelevanceV1,
    Bm25,
}

#[doc(hidden)]
#[derive(Clone, Debug, PartialEq)]
pub struct SearchIntent {
    pub query: String,
    pub search_fields: Option<Vec<SearchFieldInput>>,
    pub filters: Vec<FilterInput>,
    pub sorts: Vec<SortInput>,
    pub facets: Vec<FacetInput>,
    pub ranking: Option<RankingInput>,
    pub projection: Option<Vec<String>>,
    pub highlighting: Option<Vec<String>>,
    pub language: Option<String>,
    pub timeout: Option<Duration>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

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
        .map(|value| parse_language(value, "language"))
        .transpose()?;
    Ok(AnalyzeRequestIr {
        analyzer,
        text: input.text,
        language,
    })
}

/// Resolve the subset of search intent exposed by the high-level Rust API.
#[doc(hidden)]
pub fn resolve_search(context: &IndexContext, input: SearchIntent) -> Result<SearchRequestIr> {
    let query = context.query_to_ir(&input.query, None)?;
    let search_fields = match input.search_fields {
        None => context.definition().default_search_fields().to_vec(),
        Some(fields) => resolve_search_fields(context, &fields)?,
    };
    if matches!(query, QueryIr::Text(_)) && search_fields.is_empty() {
        return Err(PortableError::failed_precondition(
            "no_searchable_fields",
            Some("search_fields"),
            "a text query requires at least one searchable field",
        ));
    }
    let filter = match input.filters.len() {
        0 => None,
        1 => Some(resolve_filter(context, &input.filters[0], "filter")?),
        _ => Some(flatten_and(
            input
                .filters
                .iter()
                .enumerate()
                .map(|(index, filter)| {
                    resolve_filter(context, filter, &format!("filters[{index}]"))
                })
                .collect::<Result<Vec<_>>>()?,
        )),
    };
    let sort = resolve_sorts(context, &input.sorts)?;
    let grouping = input
        .facets
        .iter()
        .map(resolve_facet)
        .collect::<Result<Vec<_>>>()?;
    let ranking = match input.ranking.unwrap_or(RankingInput::DefaultRelevanceV1) {
        RankingInput::DefaultRelevanceV1 => RankingPolicyIr::DefaultRelevanceV1,
        RankingInput::Bm25 => RankingPolicyIr::Bm25,
    };
    let profile = match ranking {
        RankingPolicyIr::DefaultRelevanceV1 => "default",
        RankingPolicyIr::Bm25 => "bm25",
    };
    let language = input
        .language
        .as_deref()
        .map(|value| parse_language(value, "language"))
        .transpose()?;
    let projection = resolve_projection(context, input.projection.as_deref())?;
    let highlighting = resolve_highlighting(context, input.highlighting.as_deref())?;
    let timeout = input.timeout.map(resolve_timeout).transpose()?;
    Ok(SearchRequestIr {
        query,
        search_fields,
        filter,
        sort,
        grouping,
        ranking,
        ranking_request: RankingRequestIr {
            profile: profile.to_owned(),
            properties: std::collections::BTreeMap::new(),
        },
        inputs: std::collections::BTreeMap::new(),
        limit: input.limit.unwrap_or(20),
        offset: input.offset.unwrap_or(0),
        projection,
        highlighting,
        language: language.clone(),
        query_model: QueryModelIr {
            language,
            ..Default::default()
        },
        presentation: PresentationIr::default(),
        properties: std::collections::BTreeMap::new(),
        query_profile: None,
        search_chain: None,
        timeout,
        prefix_policy: PrefixPolicy::Disabled,
        typo_policy: TypoPolicy::Disabled,
    })
}

fn resolve_search_fields(
    context: &IndexContext,
    fields: &[SearchFieldInput],
) -> Result<Vec<SearchFieldIr>> {
    let mut output = Vec::new();
    let mut seen = BTreeSet::new();
    for (index, input) in fields.iter().enumerate() {
        let path = format!("search_fields.fields[{index}].field");
        let field = context.query_field_for_resolver(&input.field, path.clone())?;
        if !field.capabilities().searchable() {
            return Err(invalid(
                "field_not_searchable",
                Some(path),
                "search field must be searchable",
            ));
        }
        if !seen.insert(field.name().clone()) {
            return Err(invalid(
                "duplicate_search_field",
                Some(format!("search_fields.fields[{index}].field")),
                "search fields must be unique",
            ));
        }
        let weight = SearchFieldWeight::new(input.weight.unwrap_or(1.0)).map_err(|_| {
            invalid(
                "invalid_search_weight",
                Some(format!("search_fields.fields[{index}].weight")),
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

fn resolve_filter(context: &IndexContext, input: &FilterInput, path: &str) -> Result<FilterIr> {
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
            let include_lower = include_lower.unwrap_or(true);
            let include_upper = include_upper.unwrap_or(true);
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
    value: &Option<RequestValueIr>,
    kind: FieldKind,
    path: &str,
) -> Result<crate::model::FilterRangeBoundIr> {
    value
        .as_ref()
        .map(|value| {
            resolve_scalar(value.clone(), kind, path.to_owned())
                .map(crate::model::FilterRangeBoundIr::Finite)
        })
        .transpose()
        .map(|value| value.unwrap_or(crate::model::FilterRangeBoundIr::Unbounded))
}

fn resolve_sorts(context: &IndexContext, sorts: &[SortInput]) -> Result<Vec<SortIr>> {
    let mut seen = BTreeSet::new();
    sorts
        .iter()
        .enumerate()
        .map(|(index, input)| {
            let field_path = format!("sort[{index}].field");
            let field = context.query_field_for_resolver(&input.field, field_path.clone())?;
            if !field.capabilities().sortable() {
                return Err(invalid(
                    "field_not_sortable",
                    Some(field_path),
                    "sort requires a sortable field",
                ));
            }
            if field.cardinality() != FieldCardinality::Single {
                return Err(invalid(
                    "invalid_capability",
                    Some(field_path),
                    "multi-valued fields cannot be sortable",
                ));
            }
            if !seen.insert(field.name().clone()) {
                return Err(invalid(
                    "duplicate_sort_field",
                    Some(format!("sort[{index}].field")),
                    "sort fields must be unique",
                ));
            }
            Ok(SortIr {
                target: SortTargetIr::Field(field.name().clone()),
                direction: input.direction,
                missing: input.missing.clone().unwrap_or(MissingPlacement::Last),
                annotations: BTreeMap::new(),
            })
        })
        .collect()
}

fn resolve_highlighting(
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

fn resolve_timeout(timeout: Duration) -> Result<Duration> {
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
    let projection = resolve_projection(context, input.projection.as_deref())?;
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

fn resolve_field_value(
    field: &crate::model::FieldIr,
    value: RequestValueIr,
    path: String,
) -> Result<FieldValueIr> {
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
    let mut output = Vec::new();
    match fields {
        None => output.extend(
            context
                .definition()
                .query_fields()
                .filter(|field| field.capabilities().stored())
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
