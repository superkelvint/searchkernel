use super::*;
use crate::api_to_ir::resolve::{
    resolve_highlighting, resolve_search, resolve_search_fields_at, resolve_sort, SearchInput,
};
use crate::model::*;
use searchkernel_protocol::wire::v1 as api;
use std::collections::BTreeMap;

impl IndexContext {
    pub fn inspect_query(&self, request: api::SearchRequest<'_>) -> Result<SearchRequestIr> {
        self.search_to_ir(request)
    }
    pub fn search_to_ir(&self, request: api::SearchRequest<'_>) -> Result<SearchRequestIr> {
        resolve_search(self, decode_search_input(self, request)?)
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
                self.typed_query_to_ir(expression, "query_expr", None)
            }
            None if text.trim().is_empty() => Err(invalid(
                "invalid_query",
                Some("query"),
                "text query must not be empty or whitespace-only; use MatchAllQuery for browse searches",
            )),
            None => Ok(QueryIr::Text(text.to_owned())),
        }
    }

    pub(crate) fn query_source_to_input(
        &self,
        source: Option<api::QuerySource<'_>>,
    ) -> Result<crate::api_to_ir::resolve::QuerySourceInput> {
        let source = source.ok_or_else(|| {
            invalid(
                "missing_query",
                Some("query"),
                "search request requires a query",
            )
        })?;
        match source.source_type() {
            api::QuerySourceKind::SearchKernelTextQuery => source
                .source_as_search_kernel_text_query()
                .ok_or_else(|| {
                    invalid(
                        "missing_query",
                        Some("query"),
                        "text query payload is missing",
                    )
                })
                .and_then(|query| {
                    query
                        .text()
                        .map(|text| {
                            crate::api_to_ir::resolve::QuerySourceInput::Text(text.to_owned())
                        })
                        .ok_or_else(|| {
                            invalid(
                                "missing_query",
                                Some("query.text"),
                                "text query requires text",
                            )
                        })
                }),
            api::QuerySourceKind::TypedQuery => {
                let query = source
                    .source_as_typed_query()
                    .and_then(|query| query.query())
                    .ok_or_else(|| {
                        invalid("missing_query", Some("query"), "typed query is missing")
                    })?;
                self.typed_query_expr_to_ir(query, "query")
                    .map(crate::api_to_ir::resolve::QuerySourceInput::Typed)
            }
            api::QuerySourceKind::MatchAllQuery => {
                Ok(crate::api_to_ir::resolve::QuerySourceInput::MatchAll)
            }
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
                Ok(crate::api_to_ir::resolve::QuerySourceInput::Yql {
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
    pub(crate) fn sorts_to_input(
        &self,
        sorts: Option<flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<api::SortExpr<'_>>>>,
    ) -> Result<Vec<SortInput>> {
        let mut output = Vec::new();
        if let Some(sorts) = sorts {
            for (index, value) in sorts.iter().enumerate() {
                let path = format!("sort[{index}]");
                output.push(self.sort_to_input(value, &path)?);
            }
        }
        Ok(output)
    }
    pub(crate) fn sort_to_input(&self, value: api::SortExpr<'_>, path: &str) -> Result<SortInput> {
        let direction = match value.direction().0 {
            0 | 1 => SortDirection::Asc,
            2 => SortDirection::Desc,
            _ => {
                return Err(invalid(
                    "invalid_sort",
                    Some(format!("{path}.direction")),
                    "sort direction is unknown",
                ));
            }
        };
        let missing = match value.missing() {
            None => None,
            Some(policy) => match policy.policy_type() {
                api::MissingPolicyKind::MissingFirst
                    if policy.policy_as_missing_first().is_some() =>
                {
                    Some(MissingPlacement::First)
                }
                api::MissingPolicyKind::MissingLast
                    if policy.policy_as_missing_last().is_some() =>
                {
                    Some(MissingPlacement::Last)
                }
                api::MissingPolicyKind::MissingAs => {
                    Some(MissingPlacement::As(RequestValueIr::from_api(
                        policy
                            .policy_as_missing_as()
                            .and_then(|value| value.value())
                            .ok_or_else(|| {
                                invalid(
                                    "invalid_sort",
                                    Some(format!("{path}.missing")),
                                    "missing-as policy requires a value",
                                )
                            })?,
                        format!("{path}.missing.value"),
                    )?))
                }
                _ => {
                    return Err(invalid(
                        "invalid_sort",
                        Some(format!("{path}.missing")),
                        "missing policy is unsupported or missing its payload",
                    ));
                }
            },
        };
        let annotations =
            request_values_to_map(value.annotations(), &format!("{path}.annotations"))?;
        let target = value.target().ok_or_else(|| {
            invalid(
                "invalid_sort",
                Some(path),
                "sort expression requires a target",
            )
        })?;
        let target = match target.target_type() {
            api::SortTargetKind::FieldSortTarget => SortTargetInput::Field(
                target
                    .target_as_field_sort_target()
                    .and_then(|value| value.field())
                    .unwrap_or_default()
                    .to_owned(),
            ),
            api::SortTargetKind::RelevanceSortTarget => SortTargetInput::Relevance,
            api::SortTargetKind::RankFeatureSortTarget => SortTargetInput::RankFeature(
                target
                    .target_as_rank_feature_sort_target()
                    .and_then(|value| value.feature())
                    .unwrap_or_default()
                    .to_owned(),
            ),
            api::SortTargetKind::RawSortTarget => SortTargetInput::Raw,
            _ => {
                return Err(invalid(
                    "invalid_sort",
                    Some(format!("{path}.target")),
                    "sort target must select a known variant",
                ));
            }
        };
        Ok(SortInput {
            target,
            direction,
            missing,
            annotations,
        })
    }
    pub fn sort_to_ir(&self, sort: api::SortExpr<'_>) -> Result<SortIr> {
        let input = self.sort_to_input(sort, "sort")?;
        resolve_sort(self, &input, "sort")
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
}

fn decode_search_input(
    context: &IndexContext,
    request: api::SearchRequest<'_>,
) -> Result<SearchInput> {
    let query_model = query_model_to_input(request.query_model())?;
    let query = context.query_source_to_input(request.query())?;
    let search_fields = request.search_fields().map(|selection| {
        selection
            .fields()
            .map(|fields| {
                fields
                    .iter()
                    .map(|field| SearchFieldInput {
                        field: field.field().unwrap_or_default().to_owned(),
                        weight: field.weight().map(|value| value.value()),
                    })
                    .collect()
            })
            .unwrap_or_default()
    });
    let (filters, textual_filters) = context.filters_to_input(request.filters())?;
    let sorts = context.sorts_to_input(request.sort())?;
    let grouping = grouping_to_input(request.grouping())?;
    let (ranking, rank_properties) = ranking_to_input(request.ranking())?;
    let inputs = query_inputs_to_ir(request.inputs())?;
    let presentation = presentation_to_input(request.presentation());
    let projection = request
        .presentation()
        .and_then(|value| value.fields())
        .map(|value| {
            value
                .fields()
                .map(|fields| fields.iter().map(ToOwned::to_owned).collect())
                .unwrap_or_default()
        });
    let highlighting = request
        .presentation()
        .and_then(|value| value.highlighting())
        .map(|value| {
            value
                .fields()
                .map(|fields| fields.iter().map(ToOwned::to_owned).collect())
                .unwrap_or_default()
        });
    let timeout = request
        .timeout_ms()
        .map(|value| std::time::Duration::from_millis(value.value()));
    Ok(SearchInput {
        query,
        search_fields,
        filters,
        textual_filters,
        sorts,
        grouping,
        ranking,
        query_inputs: inputs,
        rank_properties,
        query_model,
        projection,
        highlighting,
        presentation,
        properties: request_values_to_map(request.properties(), "properties")?,
        // Keep present-empty controls intact until resolve_search decides
        // whether they mean default or invalid input.
        query_profile: request.query_profile().map(ToOwned::to_owned),
        search_chain: request.search_chain().map(ToOwned::to_owned),
        timeout,
        limit: request.limit().map(|value| value.value()),
        offset: request.offset().map(|value| value.value()),
    })
}

fn query_model_to_input(
    value: Option<api::QueryModel<'_>>,
) -> Result<crate::api_to_ir::resolve::QueryModelInput> {
    let Some(value) = value else {
        return Ok(Default::default());
    };
    let query_type = value.type_().map(|value| QueryModelTypeIr {
        composite: normalized_optional_string(value.composite()),
        tokenization: normalized_optional_string(value.tokenization()),
        syntax: normalized_optional_string(value.syntax()),
        profile: normalized_optional_string(value.profile()),
        is_yql_default: value.is_yql_default().map(|value| value.value()),
    });
    Ok(crate::api_to_ir::resolve::QueryModelInput {
        query_string: normalized_optional_string(value.query_string()),
        default_index: normalized_optional_string(value.default_index()),
        encoding: normalized_optional_string(value.encoding()),
        filter: normalized_optional_string(value.filter()),
        language: value.language().map(ToOwned::to_owned),
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

fn presentation_to_input(
    value: Option<api::PresentationRequest<'_>>,
) -> crate::api_to_ir::resolve::PresentationInput {
    let Some(value) = value else {
        return Default::default();
    };
    crate::api_to_ir::resolve::PresentationInput {
        summary: value.summary().map(ToOwned::to_owned),
        format: value.format().map(ToOwned::to_owned),
        bolding: value.bolding().map(|value| value.value()),
        timing: value.timing().map(|value| value.value()),
    }
}

pub(crate) fn ranking_to_input(
    value: Option<api::RankingRequest<'_>>,
) -> Result<(Option<RankingInput>, BTreeMap<String, String>)> {
    let Some(value) = value else {
        return Ok((None, BTreeMap::new()));
    };
    let ranking = value
        .profile()
        .map(|profile| RankingInput::Named(profile.to_owned()));
    let mut properties = BTreeMap::new();
    if let Some(entries) = value.properties() {
        for (index, entry) in entries.iter().enumerate() {
            let key = entry.key().to_owned();
            if properties
                .insert(key.clone(), entry.value().unwrap_or_default().to_owned())
                .is_some()
            {
                return Err(invalid(
                    "duplicate_ranking_property",
                    Some(format!("ranking.properties[{index}]")),
                    format!("ranking property `{key}` occurs more than once"),
                ));
            }
        }
    }
    Ok((ranking, properties))
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
        QueryExprKindIr::NearestNeighbor { query_tensor, .. } => {
            let Some(input) = inputs.get(query_tensor) else {
                return Err(invalid(
                    "unknown_query_input",
                    Some(format!("{path}.nearest_neighbor.query_tensor")),
                    format!("query tensor input {query_tensor} is not defined"),
                ));
            };
            match input {
                // Expressions are intentionally left for the execution
                // backend: their runtime tensor type is not statically
                // inspectable at the IR boundary.
                QueryInputIr::Expression(_) => {}
                QueryInputIr::Literal(RequestValueIr::Tensor(tensor))
                    if matches!(tensor.payload, RequestTensorPayloadIr::Dense(_)) =>
                {
                    if tensor.type_spec.trim().is_empty()
                        || matches!(
                            &tensor.payload,
                            RequestTensorPayloadIr::Dense(values)
                                if values.iter().any(|value| !value.is_finite())
                        )
                    {
                        return Err(invalid(
                            "invalid_query_input",
                            Some(format!("{path}.nearest_neighbor.query_tensor")),
                            "nearest-neighbor tensor inputs require a non-empty type spec and finite dense values",
                        ));
                    }
                }
                QueryInputIr::Literal(RequestValueIr::Tensor(_)) => {
                    return Err(PortableError::unsupported(
                        "nearest_neighbor_tensor_not_supported",
                        Some(format!("{path}.nearest_neighbor.query_tensor")),
                        "nearest-neighbor execution currently supports dense tensor query inputs only",
                    ));
                }
                QueryInputIr::Literal(_) => {
                    return Err(invalid(
                        "nearest_neighbor_tensor_required",
                        Some(format!("{path}.nearest_neighbor.query_tensor")),
                        "nearest-neighbor execution requires a literal dense tensor query input",
                    ));
                }
            }
        }
        _ => {}
    }
    for (child, child_path) in children {
        validate_query_input_references_in_expr(child, inputs, &child_path)?;
    }
    Ok(())
}
