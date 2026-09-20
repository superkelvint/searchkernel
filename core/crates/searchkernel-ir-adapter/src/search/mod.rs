//! Search request orchestration and adapter-wide validation.

pub(crate) mod filter;
mod grouping;
mod typed_query;

use crate::analysis_options;
use searchkernel_ir::{
    PortableError, QueryInputIr, QueryIr, RequestTensorPayloadIr, RequestValueIr, Result,
    SearchRequestIr,
};
use searchkernel_vespa_engine::{
    Query, SearchRequest, SortField, SortOrder, SummaryRequest, TensorValue,
};
use std::collections::{BTreeMap, BTreeSet};

use typed_query::lower_query_with_context;

pub(crate) fn lower_search_query(
    request: &SearchRequestIr,
    language: searchkernel_vespa_engine::LinguisticsOptions,
) -> Result<Query> {
    let rank_properties = effective_rank_properties(request)?;
    let mut clauses = vec![match request.query() {
        QueryIr::Yql { source, parameters } => {
            let properties = yql_properties(request, parameters)?;
            SearchRequest::from_yql_with_properties(source, &properties)
                .map(|request| request.query)
                .map_err(|error| {
                    PortableError::invalid("invalid_yql", Some("query"), error.to_string())
                })?
        }
        query => lower_query_with_context(
            query,
            request.search_fields(),
            language,
            request.query_model().default_index.as_deref(),
            request.query_model().query_string.as_deref(),
            &rank_properties,
        )?,
    }];
    if let Some(filter) = request.filter() {
        clauses.push(filter::lower_filter(filter)?);
    }
    if clauses.len() == 1 {
        Ok(clauses.pop().expect("one clause was inserted"))
    } else {
        Ok(Query::And(clauses))
    }
}

pub(crate) fn lower_search_options(
    request: &SearchRequestIr,
    query: Query,
    language: searchkernel_vespa_engine::LinguisticsOptions,
) -> Result<SearchRequest> {
    let rank_properties = effective_rank_properties(request)?;
    let mut m_request = SearchRequest::new(query)
        .limit(request.limit())
        .offset(request.offset())
        .rank_profile(request.ranking_request().profile.clone())
        .language(language);
    if let QueryIr::Yql { source, parameters } = request.query() {
        let properties = yql_properties(request, parameters)?;
        let yql_request =
            SearchRequest::from_yql_with_properties(source, &properties).map_err(|error| {
                PortableError::invalid("invalid_yql", Some("query"), error.to_string())
            })?;
        m_request = m_request.source(yql_request.source);
        for sort in yql_request.sort {
            m_request = m_request.sort(sort);
        }
    }
    for (name, input) in request.inputs() {
        match input {
            QueryInputIr::Literal(RequestValueIr::Tensor(tensor)) => {
                m_request = m_request.query_tensor(name, lower_query_tensor(name, tensor)?);
            }
            _ => {
                m_request = m_request.query_feature(name, query_feature_value(name, input)?);
            }
        }
    }
    for (name, value) in &rank_properties {
        m_request = m_request.rank_property(name, value);
    }
    if let Some(timeout) = request.timeout() {
        m_request = m_request.timeout(timeout);
    }
    if request.sort().iter().any(sort_requires_raw_native_spec) {
        let sort_spec = request
            .sort()
            .iter()
            .map(raw_sort_spec)
            .collect::<Result<Vec<_>>>()?
            .join(" ");
        m_request = m_request.raw_sort_spec(sort_spec);
    } else {
        for sort in request.sort() {
            m_request = m_request.sort(lower_sort(sort)?);
        }
    }

    // C2 has already resolved the projection. An empty list means that no
    // summary fields should be fetched; the native API rejects an explicit
    // empty field list, so use its no-summary mode instead.
    let fields = request.projection().fields();
    if fields.is_empty() {
        m_request = m_request.summary(SummaryRequest::None);
    } else {
        m_request = m_request.fields(fields.iter().map(|field| field.as_str()));
    }
    Ok(m_request)
}

fn sort_requires_raw_native_spec(sort: &searchkernel_ir::SortIr) -> bool {
    !matches!(sort.target(), searchkernel_ir::SortTargetIr::Field(_))
        || matches!(sort.missing(), searchkernel_ir::MissingPlacement::As(_))
}

fn raw_sort_spec(sort: &searchkernel_ir::SortIr) -> Result<String> {
    if !sort.annotations().is_empty() {
        return Err(PortableError::unsupported(
            "sort_annotations_not_supported",
            Some("sort.annotations"),
            "the current native sort escape hatch cannot execute annotated sort expressions",
        ));
    }
    let direction = match sort.direction() {
        searchkernel_ir::SortDirection::Asc => '+',
        searchkernel_ir::SortDirection::Desc => '-',
    };
    let target = match sort.target() {
        searchkernel_ir::SortTargetIr::Field(field) => field.as_str().to_owned(),
        searchkernel_ir::SortTargetIr::Relevance => "relevance".to_owned(),
        searchkernel_ir::SortTargetIr::RankFeature(feature) => format!("feature({feature})"),
    };
    let target = match sort.missing() {
        searchkernel_ir::MissingPlacement::First => format!("missing({target},first)"),
        searchkernel_ir::MissingPlacement::Last => format!("missing({target},last)"),
        searchkernel_ir::MissingPlacement::As(value) => {
            format!("missing({target},as,{})", raw_sort_value(&value)?)
        }
    };
    Ok(format!("{direction}{target}"))
}

fn raw_sort_value(value: &searchkernel_ir::RequestValueIr) -> Result<String> {
    match value {
        searchkernel_ir::RequestValueIr::String(value) => Ok(format!(
            "\"{}\"",
            value.replace('\\', "\\\\").replace('"', "\\\"")
        )),
        searchkernel_ir::RequestValueIr::Bool(value) => Ok(value.to_string()),
        searchkernel_ir::RequestValueIr::Int64(value) => Ok(value.to_string()),
        searchkernel_ir::RequestValueIr::UInt64(value) => Ok(value.to_string()),
        searchkernel_ir::RequestValueIr::Float64(value) if value.is_finite() => {
            Ok(value.to_string())
        }
        _ => Err(PortableError::invalid(
            "invalid_sort_missing_value",
            Some("sort.missing"),
            "missing-as sort values must be scalar finite values",
        )),
    }
}

fn effective_rank_properties(request: &SearchRequestIr) -> Result<BTreeMap<String, String>> {
    let mut properties = request.ranking_request().properties.clone();
    let labels = match request.query() {
        QueryIr::Typed(expression) => {
            let mut labels = BTreeSet::new();
            collect_label_names(expression, &mut labels);
            labels
        }
        _ => BTreeSet::new(),
    };
    for property in properties.keys() {
        if property.starts_with("vespa.label.") && property.ends_with(".id") {
            return Err(PortableError::invalid(
                "reserved_rank_property",
                Some("ranking.properties"),
                "vespa.label.*.id is allocated internally for labeled queries",
            ));
        }
    }
    for (index, label) in labels.into_iter().enumerate() {
        let id = i32::try_from(index + 1).map_err(|_| {
            PortableError::invalid(
                "too_many_labels",
                Some("query"),
                "labeled query ID allocation exceeded the native range",
            )
        })?;
        properties.insert(format!("vespa.label.{label}.id"), id.to_string());
    }
    Ok(properties)
}

fn yql_properties(
    request: &SearchRequestIr,
    parameters: &BTreeMap<String, RequestValueIr>,
) -> Result<BTreeMap<String, String>> {
    let mut values = BTreeMap::new();
    for (name, value) in parameters {
        values.insert(name.clone(), yql_property_value(value, name)?);
    }
    for (name, value) in request.properties() {
        let value = yql_property_value(value, name)?;
        if let Some(previous) = values.insert(name.clone(), value.clone()) {
            if previous != value {
                return Err(PortableError::invalid(
                    "yql_property_conflict",
                    Some(format!("properties.{name}")),
                    "YQL parameter and request property provide different values",
                ));
            }
        }
    }
    Ok(values)
}

fn yql_property_value(value: &RequestValueIr, name: &str) -> Result<String> {
    match value {
        RequestValueIr::String(value) => Ok(value.clone()),
        RequestValueIr::Bool(value) => Ok(value.to_string()),
        RequestValueIr::Int64(value) => Ok(value.to_string()),
        RequestValueIr::UInt64(value) => Ok(value.to_string()),
        RequestValueIr::Float64(value) if value.is_finite() => Ok(value.to_string()),
        _ => Err(PortableError::invalid(
            "invalid_yql_property",
            Some(format!("properties.{name}")),
            "YQL text properties must be scalar finite values",
        )),
    }
}

fn collect_label_names(expression: &searchkernel_ir::QueryExprIr, labels: &mut BTreeSet<String>) {
    use searchkernel_ir::QueryExprKindIr;
    match &expression.kind {
        QueryExprKindIr::Contains { item, .. }
        | QueryExprKindIr::Not(item)
        | QueryExprKindIr::NonEmpty(item) => collect_label_names(item, labels),
        QueryExprKindIr::And(children)
        | QueryExprKindIr::Or(children)
        | QueryExprKindIr::Phrase(children)
        | QueryExprKindIr::Near { children, .. }
        | QueryExprKindIr::ONear { children, .. }
        | QueryExprKindIr::SameElement(children)
        | QueryExprKindIr::Equiv(children)
        | QueryExprKindIr::WeakAnd { children, .. } => {
            for child in children {
                collect_label_names(child, labels);
            }
        }
        QueryExprKindIr::Alternatives(values) => {
            for value in values {
                collect_label_names(&value.query, labels);
            }
        }
        QueryExprKindIr::Rank { matching, ranking } => {
            collect_label_names(matching, labels);
            for child in ranking {
                collect_label_names(child, labels);
            }
        }
        QueryExprKindIr::Labeled { query, label, .. } => {
            labels.insert(label.clone());
            collect_label_names(query, labels);
        }
        _ => {}
    }
}

pub(crate) fn lower_sort(sort: &searchkernel_ir::SortIr) -> Result<SortField> {
    if !sort.annotations().is_empty() {
        return Err(PortableError::unsupported(
            "sort_annotations_not_supported",
            Some("sort.annotations"),
            "the current native adapter cannot execute annotated sort expressions",
        ));
    }
    let field = match sort.target() {
        searchkernel_ir::SortTargetIr::Field(field) => field,
        searchkernel_ir::SortTargetIr::Relevance => {
            return Err(PortableError::unsupported(
                "relevance_sort_not_supported",
                Some("sort.target"),
                "the current native adapter cannot execute relevance sort targets",
            ));
        }
        searchkernel_ir::SortTargetIr::RankFeature(_) => {
            return Err(PortableError::unsupported(
                "rank_feature_sort_not_supported",
                Some("sort.target"),
                "the current native adapter cannot execute rank-feature sort targets",
            ));
        }
    };
    let sort_field = match sort.direction() {
        searchkernel_ir::SortDirection::Asc => SortField::new(field.as_str(), SortOrder::Ascending),
        searchkernel_ir::SortDirection::Desc => {
            SortField::new(field.as_str(), SortOrder::Descending)
        }
    };
    Ok(match sort.missing() {
        searchkernel_ir::MissingPlacement::First => sort_field.missing_first(),
        searchkernel_ir::MissingPlacement::Last => sort_field.missing_last(),
        searchkernel_ir::MissingPlacement::As(_) => {
            return Err(PortableError::unsupported(
                "sort_missing_as_not_supported",
                Some("sort.missing"),
                "the current native adapter cannot execute missing-as sort policies",
            ));
        }
    })
}

pub(crate) fn validate_search_request(request: &SearchRequestIr) -> Result<()> {
    grouping::lower_groupings(request.grouping())?;
    validate_ranking_inputs(request)?;
    validate_nearest_neighbor_inputs(request)?;
    validate_yql_controls(request)?;
    if !request.properties().is_empty() && !matches!(request.query(), QueryIr::Yql { .. }) {
        return Err(PortableError::unsupported(
            "query_properties_not_supported",
            Some("properties"),
            "the current native adapter does not expose generic Vespa query properties",
        ));
    }
    if request.query_profile().is_some() {
        return Err(PortableError::unsupported(
            "query_profile_not_supported",
            Some("query_profile"),
            "query profiles require a container-aware Vespa execution path",
        ));
    }
    if request.search_chain().is_some() {
        return Err(PortableError::unsupported(
            "search_chain_not_supported",
            Some("search_chain"),
            "search chains require a container-aware Vespa execution path",
        ));
    }
    if request.query_model().encoding.is_some()
        || request.query_model().filter.is_some()
        || request.query_model().locale.is_some()
        || request.query_model().query_type.is_some()
        || !request.query_model().sources.is_empty()
        || !request.query_model().restrict.is_empty()
        || request.query_model().search_path.is_some()
        || request.query_model().search_group.is_some()
    {
        return Err(PortableError::unsupported(
            "query_model_not_supported",
            Some("query_model"),
            "the current native adapter supports query_model.language, query_string, and default_index only",
        ));
    }
    if request.presentation().summary.is_some()
        || request.presentation().format.is_some()
        || request.presentation().bolding.is_some()
        || request.presentation().timing.is_some()
    {
        return Err(PortableError::unsupported(
            "presentation_not_supported",
            Some("presentation"),
            "the current native adapter supports only fields and highlighting presentation controls",
        ));
    }
    if !matches!(
        request.highlighting(),
        searchkernel_ir::HighlightingIr::Disabled
    ) {
        return Err(PortableError::unsupported(
            "highlighting_native_result_unavailable",
            Some("highlighting"),
            "native Vespa result path does not expose generated highlight fragments and spans",
        ));
    }
    if request
        .search_fields()
        .iter()
        .any(|field| field.weight().get() != 1.0)
    {
        return Err(PortableError::unsupported(
            "ranking_weights_not_supported",
            Some("search_fields"),
            "non-default search-field weights require the M10/I7 ranking integration surface",
        ));
    }
    analysis_options(request.language())?;
    Ok(())
}

fn validate_yql_controls(request: &SearchRequestIr) -> Result<()> {
    if !matches!(request.query(), QueryIr::Yql { .. }) {
        return Ok(());
    }
    if request.filter().is_some() {
        return Err(PortableError::invalid(
            "yql_conflict",
            Some("filter"),
            "YQL cannot be combined with a separately supplied filter",
        ));
    }
    if !request.sort().is_empty() {
        return Err(PortableError::invalid(
            "yql_conflict",
            Some("sort"),
            "YQL ORDER BY cannot be combined with separately supplied sort controls",
        ));
    }
    if request.query_model().query_string.is_some() || request.query_model().default_index.is_some()
    {
        return Err(PortableError::invalid(
            "yql_conflict",
            Some("query_model"),
            "YQL cannot be combined with typed query-model controls",
        ));
    }
    Ok(())
}

fn validate_nearest_neighbor_inputs(request: &SearchRequestIr) -> Result<()> {
    let QueryIr::Typed(expression) = request.query() else {
        return Ok(());
    };
    validate_nearest_neighbor_expression(expression, request.inputs(), "query")
}

fn validate_nearest_neighbor_expression(
    expression: &searchkernel_ir::QueryExprIr,
    inputs: &std::collections::BTreeMap<String, QueryInputIr>,
    path: &str,
) -> Result<()> {
    use searchkernel_ir::QueryExprKindIr;

    let children: Vec<(&searchkernel_ir::QueryExprIr, String)> = match &expression.kind {
        QueryExprKindIr::Contains { item, .. } => vec![(item, format!("{path}.contains.item"))],
        QueryExprKindIr::And(children)
        | QueryExprKindIr::Or(children)
        | QueryExprKindIr::Phrase(children)
        | QueryExprKindIr::Near { children, .. }
        | QueryExprKindIr::ONear { children, .. }
        | QueryExprKindIr::SameElement(children)
        | QueryExprKindIr::Equiv(children)
        | QueryExprKindIr::WeakAnd { children, .. } => children
            .iter()
            .enumerate()
            .map(|(index, child)| (child, format!("{path}.children[{index}]")))
            .collect(),
        QueryExprKindIr::Alternatives(values) => values
            .iter()
            .enumerate()
            .map(|(index, value)| (&value.query, format!("{path}.alternatives[{index}]")))
            .collect(),
        QueryExprKindIr::Not(child) | QueryExprKindIr::NonEmpty(child) => {
            vec![(child, format!("{path}.child"))]
        }
        QueryExprKindIr::Rank { matching, ranking } => {
            let mut children = vec![(matching.as_ref(), format!("{path}.rank.match"))];
            children.extend(
                ranking
                    .iter()
                    .enumerate()
                    .map(|(index, child)| (child, format!("{path}.rank.ranking[{index}]"))),
            );
            children
        }
        QueryExprKindIr::Labeled { query, .. } => vec![(query, format!("{path}.labeled.query"))],
        _ => Vec::new(),
    };

    if let QueryExprKindIr::NearestNeighbor { query_tensor, .. } = &expression.kind {
        let Some(input) = inputs.get(query_tensor) else {
            return Err(PortableError::invalid(
                "missing_query_input",
                Some(format!("{path}.nearest_neighbor.query_tensor")),
                format!("nearest-neighbor query input {query_tensor} is missing"),
            ));
        };
        match input {
            QueryInputIr::Literal(RequestValueIr::Tensor(tensor))
                if matches!(tensor.payload, RequestTensorPayloadIr::Dense(_)) => {}
            QueryInputIr::Literal(RequestValueIr::Tensor(_)) => {
                return Err(PortableError::unsupported(
                    "nearest_neighbor_tensor_not_supported",
                    Some(format!("{path}.nearest_neighbor.query_tensor")),
                    "nearest-neighbor execution currently supports dense tensor query inputs only",
                ));
            }
            _ => {
                return Err(PortableError::invalid(
                    "nearest_neighbor_tensor_required",
                    Some(format!("{path}.nearest_neighbor.query_tensor")),
                    "nearest-neighbor execution requires a literal dense tensor query input",
                ));
            }
        }
    }
    for (child, child_path) in children {
        validate_nearest_neighbor_expression(child, inputs, &child_path)?;
    }
    Ok(())
}

#[cfg(test)]
pub(crate) fn ranking_profile(ranking: searchkernel_ir::RankingPolicyIr) -> Result<&'static str> {
    match ranking {
        searchkernel_ir::RankingPolicyIr::DefaultRelevanceV1 => Ok("default"),
        searchkernel_ir::RankingPolicyIr::Bm25 => Ok("bm25"),
    }
}

const RESERVED_RANK_PROPERTY: &str = "vespa.matching.numthreadspersearch";
const MAX_RANK_INPUT_NAME_BYTES: usize = 256;

fn validate_ranking_inputs(request: &SearchRequestIr) -> Result<()> {
    for (name, input) in request.inputs() {
        if let QueryInputIr::Literal(RequestValueIr::Tensor(tensor)) = input {
            if !matches!(tensor.payload, RequestTensorPayloadIr::Dense(_)) {
                return Err(PortableError::unsupported(
                    "query_input_tensor_not_supported",
                    Some(format!("inputs.{name}")),
                    "only dense tensor query inputs are executable by the current native request interface",
                ));
            }
            if tensor.type_spec.trim().is_empty() {
                return Err(PortableError::invalid(
                    "invalid_query_input",
                    Some(format!("inputs.{name}")),
                    "tensor query input type spec must not be empty",
                ));
            }
        } else {
            query_feature_value(name, input)?;
        }
        if name.is_empty() || name.len() > MAX_RANK_INPUT_NAME_BYTES {
            return Err(PortableError::invalid(
                "invalid_query_feature",
                Some(format!("inputs.{name}")),
                "query feature name must be non-empty and at most 256 bytes",
            ));
        }
        if name == RESERVED_RANK_PROPERTY {
            return Err(PortableError::invalid(
                "invalid_query_feature",
                Some(format!("inputs.{name}")),
                "the Search Kernel-owned rank property is not caller-configurable",
            ));
        }
    }
    for (name, value) in &request.ranking_request().properties {
        if name.is_empty() || name.len() > MAX_RANK_INPUT_NAME_BYTES || value.is_empty() {
            return Err(PortableError::invalid(
                "invalid_rank_property",
                Some(format!("ranking.properties.{name}")),
                "rank property names must be non-empty and at most 256 bytes, and values must be non-empty",
            ));
        }
        if name == RESERVED_RANK_PROPERTY {
            return Err(PortableError::invalid(
                "invalid_rank_property",
                Some(format!("ranking.properties.{name}")),
                "the Search Kernel-owned rank property is not caller-configurable",
            ));
        }
        if request.inputs().contains_key(name) {
            return Err(PortableError::invalid(
                "rank_property_collision",
                Some(format!("ranking.properties.{name}")),
                format!("rank property `{name}` collides with a query feature"),
            ));
        }
    }
    Ok(())
}

fn lower_query_tensor(
    name: &str,
    tensor: &searchkernel_ir::RequestTensorIr,
) -> Result<TensorValue> {
    let RequestTensorPayloadIr::Dense(values) = &tensor.payload else {
        return Err(PortableError::unsupported(
            "query_input_tensor_not_supported",
            Some(format!("inputs.{name}")),
            "only dense tensor query inputs are executable by the current native request interface",
        ));
    };
    if tensor.type_spec.trim().is_empty() || values.iter().any(|value| !value.is_finite()) {
        return Err(PortableError::invalid(
            "invalid_query_input",
            Some(format!("inputs.{name}")),
            "dense tensor query inputs require a non-empty type spec and finite values",
        ));
    }
    Ok(TensorValue::dense(tensor.type_spec.clone(), values.clone()))
}

pub(crate) fn query_feature_value(name: &str, input: &QueryInputIr) -> Result<f64> {
    let QueryInputIr::Literal(value) = input else {
        return Err(PortableError::unsupported(
            "query_input_expression_not_supported",
            Some(format!("inputs.{name}")),
            "expression query inputs require a container-aware Vespa execution path",
        ));
    };
    match value {
        RequestValueIr::Float64(value) if value.is_finite() => Ok(*value),
        RequestValueIr::Int64(value) => {
            let converted = *value as f64;
            // Rust's float-to-int cast saturates at the integer boundary.
            // Check the half-open f64 domain explicitly before round-tripping
            // so i64::MAX cannot masquerade as the exact value 2^63.
            if converted >= i64::MIN as f64
                && converted < 2_f64.powi(63)
                && converted as i64 == *value
            {
                Ok(converted)
            } else {
                Err(PortableError::unsupported(
                    "query_input_precision_loss",
                    Some(format!("inputs.{name}")),
                    "the integer query input cannot be represented losslessly as native f64",
                ))
            }
        }
        RequestValueIr::UInt64(value) => {
            let converted = *value as f64;
            // The upper bound is exclusive because 2^64 is outside u64 and
            // Rust saturates that cast to u64::MAX.
            if converted >= 0.0 && converted < 2_f64.powi(64) && converted as u64 == *value {
                Ok(converted)
            } else {
                Err(PortableError::unsupported(
                    "query_input_precision_loss",
                    Some(format!("inputs.{name}")),
                    "the integer query input cannot be represented losslessly as native f64",
                ))
            }
        }
        RequestValueIr::Float64(_) | RequestValueIr::NonFiniteFloat(_) => {
            Err(PortableError::invalid(
                "invalid_query_input",
                Some(format!("inputs.{name}")),
                "query feature values must be finite",
            ))
        }
        _ => Err(PortableError::unsupported(
            "query_input_value_not_supported",
            Some(format!("inputs.{name}")),
            "only finite scalar numeric query inputs are executable by the native f64 feature interface",
        )),
    }
}

#[cfg(test)]
pub(crate) use grouping::lower_facet_groupings;
pub(crate) use grouping::{
    lower_facets_into_request, lower_groupings, FacetLowering, GroupingLowering,
};
#[cfg(test)]
pub(crate) use typed_query::lower_query;
