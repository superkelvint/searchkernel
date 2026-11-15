use super::*;

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
    let grouping = resolve_grouping(context, &input.grouping)?;
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
        query_profile: input.query_profile,
        search_chain: input.search_chain,
        timeout,
        prefix_policy: PrefixPolicy::Disabled,
        typo_policy: TypoPolicy::Disabled,
        diagnostic_source: false,
    })
}

fn resolve_query_source(context: &IndexContext, input: &SearchInput) -> Result<QueryIr> {
    match &input.query {
        QuerySourceInput::Text(text) if text.trim().is_empty() => Err(invalid(
            "invalid_query",
            Some("query"),
            "text query must not be empty or whitespace-only; use MatchAllQuery for browse searches",
        )),
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
        (QuerySourceInput::Text(_), None) => {
            Ok(context.definition().default_search_fields().to_vec())
        }
        (QuerySourceInput::Text(_), Some(fields)) => resolve_search_fields(context, fields),
        (QuerySourceInput::MatchAll, None) | (QuerySourceInput::Yql { .. }, None) => Ok(Vec::new()),
        (QuerySourceInput::Typed(_), None) => Ok(Vec::new()),
        (_, Some(_)) => Err(invalid(
            "search_fields_not_supported",
            Some("search_fields"),
            "search fields apply only to text query sources",
        )),
    }
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
        // An empty summary name is the portable spelling for an unspecified
        // summary class. Keep this normalization here so direct Rust input
        // and FlatBuffers input share the same presentation semantics.
        summary: input.summary.filter(|value| !value.is_empty()),
        format: input.format.filter(|value| !value.is_empty()),
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
    if input.query_profile.is_some() {
        owned.insert("queryProfile");
    }
    if input.search_chain.is_some() {
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

pub(crate) fn resolve_language(value: &str, path: &str) -> Result<LanguageHint> {
    LanguageHint::parse(value.to_owned()).map_err(|_| {
        invalid(
            "invalid_language",
            Some(path),
            "language must be a basic BCP-47-style ASCII tag",
        )
    })
}

/// Canonical result limit used when a search request omits `limit`.
pub const DEFAULT_SEARCH_LIMIT: u32 = 20;

pub(crate) fn resolve_pagination(limit: Option<u32>, offset: Option<u32>) -> (u32, u32) {
    (limit.unwrap_or(DEFAULT_SEARCH_LIMIT), offset.unwrap_or(0))
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
