use super::*;
use crate::api_to_ir::resolve::{
    resolve_highlighting, resolve_language, resolve_pagination, resolve_search_fields_at,
    resolve_sort, resolve_timeout,
};
use crate::model::*;
use searchkernel_protocol::v2 as api;
use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

impl IndexContext {
    pub fn inspect_query(&self, request: api::SearchRequest<'_>) -> Result<SearchRequestIr> {
        self.search_to_ir(request)
    }
    pub fn search_to_ir(&self, request: api::SearchRequest<'_>) -> Result<SearchRequestIr> {
        let query_model = query_model_to_ir(request.query_model())?;
        let query =
            self.query_source_to_ir(request.query(), query_model.default_index.as_deref())?;
        let search_fields_wrapper = request.search_fields();
        validate_search_request_combinations(request, &query)?;
        let source_kind = request.query().map(|source| source.source_type());
        let search_fields = match (source_kind, search_fields_wrapper) {
            (Some(api::QuerySourceKind::SearchKernelTextQuery), None) => {
                self.definition.default_search_fields.clone()
            }
            (Some(api::QuerySourceKind::SearchKernelTextQuery), Some(selection)) => {
                self.search_fields_to_ir(selection)?
            }
            (_, None) => Vec::new(),
            (_, Some(_)) => {
                return Err(invalid(
                    "search_fields_not_supported",
                    Some("search_fields"),
                    "search fields apply only to text query sources",
                ));
            }
        };
        if matches!(query, QueryIr::Text(_)) && search_fields.is_empty() {
            return Err(PortableError::failed_precondition(
                "no_searchable_fields",
                Some("search_fields"),
                "a text query requires at least one searchable field",
            ));
        }
        let filter = self.filters_to_ir(request.filters())?;
        let sort = self.sorts_to_ir(request.sort())?;
        let grouping = grouping_to_ir(request.grouping())?;
        let language = query_model.language.clone();
        let presentation = presentation_to_ir(request.presentation())?;
        let timeout = request
            .timeout_ms()
            .map(|value| resolve_timeout(Duration::from_millis(value.value())))
            .transpose()?;
        let inputs = query_inputs_to_ir(request.inputs())?;
        validate_query_input_references(&query, &inputs, "query")?;
        let (limit, offset) = resolve_pagination(
            request.limit().map(|value| value.value()),
            request.offset().map(|value| value.value()),
        );
        Ok(SearchRequestIr {
            query,
            search_fields,
            filter,
            sort,
            grouping,
            ranking_request: ranking_request_to_ir(request.ranking())?,
            inputs,
            limit,
            offset,
            projection: self.projection_to_ir(request.presentation().and_then(|p| p.fields()))?,
            highlighting: self
                .highlighting_to_ir(request.presentation().and_then(|p| p.highlighting()))?,
            language,
            query_model,
            presentation,
            properties: request_values_to_map(request.properties(), "properties")?,
            query_profile: normalized_optional_string(request.query_profile()),
            search_chain: normalized_optional_string(request.search_chain()),
            timeout,
            prefix_policy: PrefixPolicy::Disabled,
            typo_policy: TypoPolicy::Disabled,
        })
    }
    pub fn analyze_to_ir(&self, request: api::AnalyzeRequest<'_>) -> Result<AnalyzeRequestIr> {
        let target = match request.target_type() {
            api::AnalyzeTarget::Analyzer => request
                .target_as_analyzer()
                .map(|analyzer| {
                    Ok(AnalyzeTargetInput::Analyzer(AnalyzerInput {
                        name: analyzer.name().unwrap_or_default().to_owned(),
                        version: analyzer.version(),
                        options: request_values_to_map(
                            analyzer.options(),
                            "target.analyzer.options",
                        )?,
                    }))
                })
                .transpose()?,
            api::AnalyzeTarget::AnalyzeFieldTarget => {
                let target = request.target_as_analyze_field_target().ok_or_else(|| {
                    invalid(
                        "invalid_analyzer",
                        Some("target"),
                        "analyze request must select an analyzer or field",
                    )
                })?;
                Some(AnalyzeTargetInput::Field(
                    target.field().unwrap_or_default().to_owned(),
                ))
            }
            _ => {
                return Err(invalid(
                    "invalid_analyze_target",
                    Some("target"),
                    "analyze request must select an analyzer or field",
                ))
            }
        };
        resolve_analyze(
            self,
            AnalyzeInput {
                target,
                text: request.text().unwrap_or_default().to_owned(),
                language: request.language().map(ToOwned::to_owned),
            },
        )
    }
    pub fn search_fields_to_ir(
        &self,
        selection: api::SearchFieldSelection<'_>,
    ) -> Result<Vec<SearchFieldIr>> {
        let fields = selection
            .fields()
            .map(|fields| {
                fields
                    .iter()
                    .map(|field| SearchFieldInput {
                        field: field.field().unwrap_or_default().to_owned(),
                        weight: field.weight().map(|value| value.value()),
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        resolve_search_fields_at(self, &fields, "search_fields")
    }
    pub fn query_to_ir(
        &self,
        text: &str,
        expression: Option<api::QueryExpr<'_>>,
    ) -> Result<QueryIr> {
        match expression {
            Some(expression) => {
                if !text.trim().is_empty() {
                    return Err(invalid(
                        "invalid_query",
                        Some("query"),
                        "query and query_expr cannot both be supplied",
                    ));
                }
                self.query_expr_to_ir(expression, "query_expr")
            }
            None if text.trim().is_empty() => Ok(QueryIr::MatchAll),
            None => Ok(QueryIr::Text(text.to_owned())),
        }
    }

    pub(crate) fn query_source_to_ir(
        &self,
        source: Option<api::QuerySource<'_>>,
        default_index: Option<&str>,
    ) -> Result<QueryIr> {
        let source = source.ok_or_else(|| {
            invalid(
                "missing_query",
                Some("query"),
                "search request requires a query",
            )
        })?;
        match source.source_type() {
            api::QuerySourceKind::SearchKernelTextQuery => self.query_to_ir(
                source
                    .source_as_search_kernel_text_query()
                    .and_then(|query| query.text())
                    .unwrap_or_default(),
                None,
            ),
            api::QuerySourceKind::TypedQuery => {
                let query = source
                    .source_as_typed_query()
                    .and_then(|query| query.query())
                    .ok_or_else(|| {
                        invalid("missing_query", Some("query"), "typed query is missing")
                    })?;
                self.query_expr_to_ir_with_default_index(query, "query", default_index)
            }
            api::QuerySourceKind::MatchAllQuery => Ok(QueryIr::MatchAll),
            api::QuerySourceKind::YqlQuery => {
                let yql = source.source_as_yql_query().ok_or_else(|| {
                    invalid("invalid_query", Some("query"), "YQL query is missing")
                })?;
                let source = yql.source().ok_or_else(|| {
                    invalid(
                        "invalid_query",
                        Some("query.source"),
                        "YQL source is required",
                    )
                })?;
                Ok(QueryIr::Yql {
                    source: source.to_owned(),
                    parameters: request_values_to_map(yql.parameters(), "query.parameters")?,
                })
            }
            _ => Err(invalid(
                "invalid_query",
                Some("query"),
                "unknown query source",
            )),
        }
    }
    pub fn sort_to_ir(&self, sort: api::SortExpr<'_>) -> Result<SortIr> {
        self.sort_to_ir_at(sort, "sort")
    }
    pub fn projection_to_ir(
        &self,
        projection: Option<api::FieldProjection<'_>>,
    ) -> Result<ProjectionIr> {
        let fields = projection.map(|projection| {
            projection
                .fields()
                .map(|fields| fields.iter().map(ToOwned::to_owned).collect::<Vec<_>>())
                .unwrap_or_default()
        });
        resolve_projection(self, fields.as_deref())
    }
    pub fn highlighting_to_ir(
        &self,
        highlighting: Option<api::HighlightRequest<'_>>,
    ) -> Result<HighlightingIr> {
        let fields = highlighting.map(|highlighting| {
            highlighting
                .fields()
                .map(|fields| fields.iter().map(ToOwned::to_owned).collect::<Vec<_>>())
                .unwrap_or_default()
        });
        resolve_highlighting(self, fields.as_deref())
    }
    pub(crate) fn sort_to_ir_at(&self, s: api::SortExpr<'_>, path: &str) -> Result<SortIr> {
        let direction = match s.direction().0 {
            0 | 1 => SortDirection::Asc,
            2 => SortDirection::Desc,
            _ => {
                return Err(invalid(
                    "invalid_filter",
                    Some(format!("{path}.direction")),
                    "sort direction is unknown",
                ))
            }
        };
        let missing = match s.missing() {
            None => MissingPlacement::Last,
            Some(v) => match v.policy_type() {
                api::MissingPolicyKind::MissingFirst if v.policy_as_missing_first().is_some() => {
                    MissingPlacement::First
                }
                api::MissingPolicyKind::MissingLast if v.policy_as_missing_last().is_some() => {
                    MissingPlacement::Last
                }
                api::MissingPolicyKind::MissingAs => {
                    let value = v
                        .policy_as_missing_as()
                        .and_then(|value| value.value())
                        .ok_or_else(|| {
                            invalid(
                                "invalid_sort",
                                Some(format!("{path}.missing")),
                                "missing-as policy requires a value",
                            )
                        })?;
                    MissingPlacement::As(RequestValueIr::from_api(
                        value,
                        format!("{path}.missing.value"),
                    )?)
                }
                _ => {
                    return Err(invalid(
                        "invalid_sort",
                        Some(format!("{path}.missing")),
                        "missing policy is unsupported or missing its payload",
                    ))
                }
            },
        };
        let annotations = request_values_to_map(s.annotations(), &format!("{path}.annotations"))?;
        let target = s.target().ok_or_else(|| {
            invalid(
                "invalid_sort",
                Some(path),
                "sort expression requires a target",
            )
        })?;
        let target = match target.target_type() {
            api::SortTargetKind::FieldSortTarget => {
                let field = target
                    .target_as_field_sort_target()
                    .and_then(|target| target.field())
                    .unwrap_or_default()
                    .to_owned();
                let mut sort = resolve_sort(
                    self,
                    &SortInput {
                        field,
                        direction,
                        missing: Some(missing),
                    },
                    &format!("{path}.target"),
                )?;
                sort.annotations = annotations;
                return Ok(sort);
            }
            api::SortTargetKind::RelevanceSortTarget => SortTargetIr::Relevance,
            api::SortTargetKind::RankFeatureSortTarget => {
                let feature = target
                    .target_as_rank_feature_sort_target()
                    .and_then(|target| target.feature())
                    .ok_or_else(|| {
                        invalid(
                            "invalid_sort",
                            Some(format!("{path}.target.feature")),
                            "rank-feature sort requires a feature name",
                        )
                    })?;
                if feature.is_empty() {
                    return Err(invalid(
                        "invalid_sort",
                        Some(format!("{path}.target.feature")),
                        "rank-feature sort requires a non-empty feature name",
                    ));
                }
                SortTargetIr::RankFeature(feature.to_owned())
            }
            api::SortTargetKind::RawSortTarget => {
                return Err(PortableError::unsupported(
                    "raw_sort_not_supported",
                    Some(format!("{path}.target")),
                    "raw sort expressions must be normalized before entering resolved IR",
                ));
            }
            _ => {
                return Err(invalid(
                    "invalid_sort",
                    Some(format!("{path}.target")),
                    "sort target must select a known variant",
                ));
            }
        };
        Ok(SortIr {
            target,
            direction,
            missing,
            annotations,
        })
    }
    pub(crate) fn sorts_to_ir(
        &self,
        sorts: Option<flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<api::SortExpr<'_>>>>,
    ) -> Result<Vec<SortIr>> {
        let mut seen = BTreeSet::new();
        let mut out = Vec::new();
        if let Some(sorts) = sorts {
            for (i, s) in sorts.iter().enumerate() {
                let v = self.sort_to_ir_at(s, &format!("sort[{i}]"))?;
                if let Some(field) = v.field() {
                    if !seen.insert(field.clone()) {
                        return Err(invalid(
                            "duplicate_sort_field",
                            Some(format!("sort[{i}].field")),
                            "sort fields must be unique",
                        ));
                    }
                }
                out.push(v)
            }
        }
        Ok(out)
    }
}

pub(crate) fn validate_search_request_combinations(
    request: api::SearchRequest<'_>,
    query: &QueryIr,
) -> Result<()> {
    if matches!(query, QueryIr::Yql { .. }) {
        if request.filters().is_some() {
            return Err(invalid(
                "yql_conflict",
                Some("filters"),
                "YQL cannot be combined with high-level filters",
            ));
        }
        if request.sort().is_some_and(|sort| !sort.is_empty()) {
            return Err(invalid(
                "yql_conflict",
                Some("sort"),
                "YQL cannot be combined with high-level sort",
            ));
        }
        if let Some(presentation) = request.presentation() {
            if presentation.fields().is_some() || presentation.highlighting().is_some() {
                return Err(invalid(
                    "yql_conflict",
                    Some("presentation"),
                    "YQL owns projection and highlighting",
                ));
            }
        }
    }
    validate_property_overlaps(request)
}

pub(crate) fn validate_property_overlaps(request: api::SearchRequest<'_>) -> Result<()> {
    let Some(properties) = request.properties() else {
        return Ok(());
    };
    let mut owned = BTreeSet::new();
    if request.ranking().is_some() {
        owned.insert("ranking.profile");
    }
    if request.limit().is_some() {
        owned.insert("hits");
    }
    if request.offset().is_some() {
        owned.insert("offset");
    }
    if request.timeout_ms().is_some() {
        owned.insert("timeout");
    }
    if request.query_model().is_some() {
        owned.insert("model");
    }
    if request.presentation().is_some() {
        owned.insert("presentation");
    }
    if request
        .query_profile()
        .is_some_and(|value| !value.is_empty())
    {
        owned.insert("queryProfile");
    }
    if request
        .search_chain()
        .is_some_and(|value| !value.is_empty())
    {
        owned.insert("searchChain");
    }
    for (index, property) in properties.iter().enumerate() {
        let key = property.key();
        if owned
            .iter()
            .any(|prefix| key == *prefix || key.starts_with(&format!("{prefix}.")))
        {
            return Err(invalid(
                "property_overlaps_typed_field",
                Some(format!("properties[{index}].key")),
                format!("property `{key}` overlaps a populated typed request field"),
            ));
        }
    }
    Ok(())
}

pub(crate) fn query_inputs_to_ir(
    values: Option<flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<api::QueryInput<'_>>>>,
) -> Result<BTreeMap<String, QueryInputIr>> {
    let mut inputs = BTreeMap::new();
    if let Some(values) = values {
        for (index, value) in values.iter().enumerate() {
            let name = value.name().to_owned();
            let input = match value.input_type() {
                api::QueryInputKind::LiteralQueryInput => RequestValueIr::from_api(
                    value
                        .input_as_literal_query_input()
                        .and_then(|input| input.value())
                        .ok_or_else(|| {
                            invalid(
                                "missing_value",
                                Some(format!("inputs[{index}].input.value")),
                                "literal query input requires a value",
                            )
                        })?,
                    format!("inputs[{index}].input.value"),
                )
                .map(QueryInputIr::Literal)?,
                api::QueryInputKind::ExpressionQueryInput => QueryInputIr::Expression(
                    value
                        .input_as_expression_query_input()
                        .and_then(|input| input.expression())
                        .ok_or_else(|| {
                            invalid(
                                "missing_value",
                                Some(format!("inputs[{index}].input.expression")),
                                "expression query input requires an expression",
                            )
                        })?
                        .to_owned(),
                ),
                _ => {
                    return Err(invalid(
                        "invalid_query_input",
                        Some(format!("inputs[{index}].input")),
                        "query input must select a literal or expression",
                    ))
                }
            };
            if inputs.insert(name.clone(), input).is_some() {
                return Err(invalid(
                    "duplicate_query_input",
                    Some(format!("inputs[{index}].name")),
                    format!("query input `{name}` occurs more than once"),
                ));
            }
        }
    }
    Ok(inputs)
}

pub(crate) fn validate_query_input_references(
    query: &QueryIr,
    inputs: &BTreeMap<String, QueryInputIr>,
    path: &str,
) -> Result<()> {
    let QueryIr::Typed(expression) = query else {
        return Ok(());
    };
    validate_query_input_references_in_expr(expression, inputs, path)
}

pub(crate) fn validate_query_input_references_in_expr(
    expression: &QueryExprIr,
    inputs: &BTreeMap<String, QueryInputIr>,
    path: &str,
) -> Result<()> {
    let mut children: Vec<(&QueryExprIr, String)> = Vec::new();
    match &expression.kind {
        QueryExprKindIr::Contains { item, .. }
        | QueryExprKindIr::Not(item)
        | QueryExprKindIr::NonEmpty(item)
        | QueryExprKindIr::Labeled { query: item, .. } => {
            children.push((item, path.to_owned()));
        }
        QueryExprKindIr::And(values)
        | QueryExprKindIr::Or(values)
        | QueryExprKindIr::Phrase(values)
        | QueryExprKindIr::Near {
            children: values, ..
        }
        | QueryExprKindIr::ONear {
            children: values, ..
        }
        | QueryExprKindIr::SameElement(values)
        | QueryExprKindIr::Equiv(values)
        | QueryExprKindIr::WeakAnd {
            children: values, ..
        } => {
            children.extend(values.iter().map(|value| (value, path.to_owned())));
        }
        QueryExprKindIr::Alternatives(values) => {
            children.extend(values.iter().map(|value| (&value.query, path.to_owned())));
        }
        QueryExprKindIr::Rank { matching, ranking } => {
            children.push((matching, path.to_owned()));
            children.extend(ranking.iter().map(|value| (value, path.to_owned())));
        }
        QueryExprKindIr::NearestNeighbor { query_tensor, .. }
            if !inputs.contains_key(query_tensor) =>
        {
            return Err(invalid(
                "unknown_query_input",
                Some(format!("{path}.nearest_neighbor.query_tensor")),
                format!("query tensor input {query_tensor} is not defined"),
            ));
        }
        _ => {}
    }
    for (child, child_path) in children {
        validate_query_input_references_in_expr(child, inputs, &child_path)?;
    }
    Ok(())
}
pub(crate) fn query_model_to_ir(value: Option<api::QueryModel<'_>>) -> Result<QueryModelIr> {
    let Some(value) = value else {
        return Ok(QueryModelIr::default());
    };
    let query_type = value.type_().map(|value| QueryModelTypeIr {
        composite: normalized_optional_string(value.composite()),
        tokenization: normalized_optional_string(value.tokenization()),
        syntax: normalized_optional_string(value.syntax()),
        profile: normalized_optional_string(value.profile()),
        is_yql_default: value.is_yql_default().map(|value| value.value()),
    });
    Ok(QueryModelIr {
        query_string: normalized_optional_string(value.query_string()),
        default_index: normalized_optional_string(value.default_index()),
        encoding: normalized_optional_string(value.encoding()),
        filter: normalized_optional_string(value.filter()),
        language: value
            .language()
            .map(|value| resolve_language(value, "query_model.language"))
            .transpose()?,
        locale: normalized_optional_string(value.locale()),
        query_type,
        sources: value
            .sources()
            .map(|values| values.iter().map(ToOwned::to_owned).collect())
            .unwrap_or_default(),
        restrict: value
            .restrict()
            .map(|values| values.iter().map(ToOwned::to_owned).collect())
            .unwrap_or_default(),
        search_path: normalized_optional_string(value.search_path()),
        search_group: value.search_group().map(|value| value.value()),
    })
}

pub(crate) fn presentation_to_ir(
    value: Option<api::PresentationRequest<'_>>,
) -> Result<PresentationIr> {
    let Some(value) = value else {
        return Ok(PresentationIr::default());
    };
    Ok(PresentationIr {
        summary: normalized_optional_string(value.summary()),
        format: normalized_optional_string(value.format()),
        bolding: value.bolding().map(|value| value.value()),
        timing: value.timing().map(|value| value.value()),
    })
}

pub(crate) fn search_fields_to_ir_at(
    selection: api::SearchFieldSelection<'_>,
    fields: &[&FieldIr],
    path: &str,
) -> Result<Vec<SearchFieldIr>> {
    let inputs = selection
        .fields()
        .map(|fields| {
            fields
                .iter()
                .map(|field| SearchFieldInput {
                    field: field.field().unwrap_or_default().to_owned(),
                    weight: field.weight().map(|value| value.value()),
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let mut output = Vec::new();
    let mut seen = BTreeSet::new();
    for (index, input) in inputs.iter().enumerate() {
        let field_path = format!("{path}.fields[{index}].field");
        let field = fields
            .iter()
            .find(|field| field.name.as_str() == input.field)
            .copied()
            .ok_or_else(|| {
                invalid(
                    "unknown_field",
                    Some(field_path.clone()),
                    "field is not defined",
                )
            })?;
        if !field.capabilities.searchable {
            return Err(invalid(
                "field_not_searchable",
                Some(field_path),
                "search field must be searchable",
            ));
        }
        if !seen.insert(field.name.clone()) {
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
            field: field.name.clone(),
            weight,
        });
    }
    Ok(output)
}
