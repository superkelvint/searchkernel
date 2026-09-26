//! Search request orchestration and adapter-wide validation.

pub(crate) mod filter;
mod grouping;
mod typed_query;

use crate::analysis_options;
use searchkernel_ir::{
    PortableError, PrefixPolicy, QueryInputIr, QueryIr, RequestTensorPayloadIr, RequestValueIr,
    Result, SearchRequestIr, TypoPolicy,
};
use searchkernel_vespa_engine::{
    Query, SearchRequest, SortField, SortOrder, SummaryRequest, TensorValue,
};
use std::collections::{BTreeMap, BTreeSet};

use typed_query::lower_query_with_context;

/// The complete executable subset produced before the native engine is called.
///
/// Keeping the lowered request and grouping plans together makes the boundary
/// auditable: backend execution consumes this result instead of revalidating
/// selected fields while other fields can be silently ignored.
#[derive(Debug)]
pub(crate) struct SearchPreflight {
    pub(crate) request: SearchRequest,
    pub(crate) groupings: Vec<GroupingLowering>,
}

pub(crate) fn preflight_search_request(request: &SearchRequestIr) -> Result<SearchPreflight> {
    validate_request_controls(request)?;
    // Validate grouping topology before lowering the query. A malformed
    // grouping request has a stable structured error that must not be masked
    // by an unrelated query-lowering failure.
    let groupings = lower_groupings(request.grouping())?;
    let language = analysis_options(request.language())?;
    let query = lower_search_query(request, language)?;
    let facets = groupings
        .iter()
        .filter_map(|grouping| match grouping {
            GroupingLowering::Facet(facet) => Some(facet.clone()),
            GroupingLowering::Native(_) => None,
        })
        .collect::<Vec<_>>();
    let native_request =
        lower_facets_into_request(lower_search_options(request, query, language)?, &facets);
    Ok(SearchPreflight {
        request: native_request,
        groupings,
    })
}

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

    if let Some(summary_class) = request.presentation().summary.as_deref() {
        m_request = m_request.summary_class(summary_class);
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
    if let searchkernel_ir::HighlightingIr::Requested { fields, .. } = request.highlighting() {
        m_request = m_request.highlight(fields.iter().map(|field| field.as_str()));
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

fn validate_request_controls(request: &SearchRequestIr) -> Result<()> {
    validate_ranking_inputs(request)?;
    validate_highlighting_execution_boundary(request)?;
    validate_highlighting_query_compatibility(request)?;
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
    if request.presentation().format.is_some()
        || request.presentation().bolding.is_some()
        || request.presentation().timing.is_some()
    {
        return Err(PortableError::unsupported(
            "presentation_not_supported",
            Some("presentation"),
            "the current native adapter supports summary, fields, and highlighting presentation controls",
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
    // `lower_sort` only has a native representation for the target, direction,
    // and missing policy. Reject annotations here rather than allowing a
    // field-target sort to lose them on the way to the native request.
    if request
        .sort()
        .iter()
        .any(|sort| !sort.annotations().is_empty())
    {
        return Err(PortableError::unsupported(
            "sort_annotations_not_supported",
            Some("sort.annotations"),
            "the current native adapter cannot execute annotated sort expressions",
        ));
    }
    match request.prefix_policy() {
        PrefixPolicy::Disabled => {}
    }
    match request.typo_policy() {
        TypoPolicy::Disabled => {}
    }
    analysis_options(request.language())?;
    Ok(())
}

fn validate_highlighting_execution_boundary(request: &SearchRequestIr) -> Result<()> {
    let searchkernel_ir::HighlightingIr::Requested {
        fields,
        multi_valued_fields,
    } = request.highlighting()
    else {
        return Ok(());
    };

    if let Some(field) = fields.iter().find(|field| {
        multi_valued_fields
            .iter()
            .any(|candidate| candidate == *field)
    }) {
        return Err(PortableError::unsupported(
            "highlighting_multi_valued_field_not_supported",
            Some("highlighting.fields"),
            format!(
                "Vespa dynamic-teaser highlighting currently supports only scalar stored indexed text; field '{}' is multi-valued",
                field.as_str()
            ),
        ));
    }
    Ok(())
}

fn validate_highlighting_query_compatibility(request: &SearchRequestIr) -> Result<()> {
    let searchkernel_ir::HighlightingIr::Requested { fields, .. } = request.highlighting() else {
        return Ok(());
    };

    let Some(expression) = (match request.query() {
        QueryIr::Typed(expression) => Some(expression.as_ref()),
        _ => None,
    }) else {
        return Ok(());
    };

    let highlighted_fields: BTreeSet<&str> = fields.iter().map(|field| field.as_str()).collect();
    if let Some(shape) = unsupported_highlighting_query_shape(expression, &highlighted_fields, None)
    {
        return Err(PortableError::unsupported(
            "highlighting_query_not_supported",
            Some("query"),
            format!(
                "Vespa Juniper cannot faithfully expose highlight terms for the {shape} query shape"
            ),
        ));
    }
    Ok(())
}

fn unsupported_highlighting_query_shape<'a>(
    expression: &'a searchkernel_ir::QueryExprIr,
    highlighted_fields: &BTreeSet<&str>,
    field_context: Option<&'a searchkernel_ir::QueryFieldRefIr>,
) -> Option<&'static str> {
    use searchkernel_ir::QueryExprKindIr;

    let field_is_highlighted =
        |field: &searchkernel_ir::QueryFieldRefIr| highlighted_fields.contains(field.path.as_str());
    let children = |children: &[searchkernel_ir::QueryExprIr], context| {
        children.iter().find_map(|child| {
            unsupported_highlighting_query_shape(child, highlighted_fields, context)
        })
    };

    match &expression.kind {
        QueryExprKindIr::Fuzzy { .. } => field_context
            .is_some_and(field_is_highlighted)
            .then_some("fuzzy"),
        QueryExprKindIr::Matches { field, .. } => {
            field_is_highlighted(field).then_some("regexp/prefix/substring/suffix")
        }
        QueryExprKindIr::Contains { field, item } => {
            unsupported_highlighting_query_shape(item, highlighted_fields, Some(field))
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
        } => children(values, field_context),
        QueryExprKindIr::Alternatives(values) => values.iter().find_map(|value| {
            unsupported_highlighting_query_shape(&value.query, highlighted_fields, field_context)
        }),
        QueryExprKindIr::Rank { matching, ranking } => {
            unsupported_highlighting_query_shape(matching, highlighted_fields, field_context)
                .or_else(|| children(ranking, field_context))
        }
        QueryExprKindIr::Labeled { query, .. }
        | QueryExprKindIr::Not(query)
        | QueryExprKindIr::NonEmpty(query) => {
            unsupported_highlighting_query_shape(query, highlighted_fields, field_context)
        }
        QueryExprKindIr::WeightedSet { field, .. }
        | QueryExprKindIr::Wand { field, .. }
        | QueryExprKindIr::DotProduct { field, .. } => {
            field_is_highlighted(field).then_some(match &expression.kind {
                QueryExprKindIr::WeightedSet { .. } => "weighted_set",
                QueryExprKindIr::Wand { .. } => "wand",
                QueryExprKindIr::DotProduct { .. } => "dot_product",
                _ => unreachable!(),
            })
        }
        _ => None,
    }
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
pub(crate) fn validate_search_request(request: &SearchRequestIr) -> Result<()> {
    preflight_search_request(request).map(|_| ())
}

#[cfg(test)]
pub(crate) use grouping::lower_facet_groupings;
pub(crate) use grouping::{
    lower_facets_into_request, lower_groupings, FacetLowering, GroupingLowering,
};
#[cfg(test)]
pub(crate) use typed_query::lower_query;

#[cfg(test)]
mod tests {
    use super::*;
    use searchkernel_ir::{
        FieldName, FilterIr, GroupingAggregationIr, GroupingAllIr, GroupingExpressionIr,
        GroupingOperationIr, GroupingOutputIr, GroupingRequestIr, HighlightingIr, LanguageHint,
        MissingPlacement, PrefixPolicy, PresentationIr, QueryExprIr, QueryExprKindIr, QueryInputIr,
        QueryIr, QueryModelIr, RequestValueIr, SearchFieldIr, SearchFieldWeight, SortDirection,
        SortIr, TypoPolicy,
    };

    fn base_request(query: QueryIr) -> SearchRequestIr {
        SearchRequestIr::new(
            query,
            vec![SearchFieldIr::new(
                FieldName::parse("title").unwrap(),
                SearchFieldWeight::new(1.0).unwrap(),
            )],
            None,
            Vec::new(),
            10,
            0,
            searchkernel_ir::ProjectionIr::new(Vec::new()),
            HighlightingIr::Disabled,
            None,
            PrefixPolicy::Disabled,
            TypoPolicy::Disabled,
        )
    }

    fn count_grouping() -> GroupingRequestIr {
        GroupingRequestIr {
            root: GroupingAllIr {
                operations: vec![
                    GroupingOperationIr::Group(GroupingExpressionIr::Attribute("brand".to_owned())),
                    GroupingOperationIr::Each(GroupingAllIr {
                        operations: vec![GroupingOperationIr::Output(vec![GroupingOutputIr {
                            label: None,
                            expression: GroupingExpressionIr::Aggregation(Box::new(
                                GroupingAggregationIr::Count,
                            )),
                        }])],
                        label: None,
                    }),
                ],
                label: None,
            },
            continuations: Vec::new(),
        }
    }

    #[test]
    fn field_sort_annotations_are_not_silently_dropped() {
        let mut annotations = BTreeMap::new();
        annotations.insert(
            "locale".to_owned(),
            RequestValueIr::String("en-US".to_owned()),
        );
        let request = SearchRequestIr::new(
            QueryIr::MatchAll,
            vec![SearchFieldIr::new(
                FieldName::parse("title").unwrap(),
                SearchFieldWeight::new(1.0).unwrap(),
            )],
            None,
            vec![SortIr::with_target(
                searchkernel_ir::SortTargetIr::Field(FieldName::parse("price").unwrap()),
                SortDirection::Asc,
                MissingPlacement::First,
                annotations,
            )],
            10,
            0,
            searchkernel_ir::ProjectionIr::new(Vec::new()),
            HighlightingIr::Disabled,
            None,
            PrefixPolicy::Disabled,
            TypoPolicy::Disabled,
        );

        let error = validate_search_request(&request)
            .expect_err("field sort annotations must be rejected before lowering");
        assert_eq!(error.code, searchkernel_ir::ErrorCode::Unsupported);
        assert_eq!(error.details[0].code, "sort_annotations_not_supported");
    }

    #[test]
    fn preflight_matrix_covers_request_controls_without_silent_drops() {
        let typed = QueryIr::Typed(Box::new(QueryExprIr {
            kind: QueryExprKindIr::True,
            annotations: BTreeMap::new(),
        }));
        let cases = vec![
            ("match_all", base_request(QueryIr::MatchAll)),
            ("text", base_request(QueryIr::Text("headphones".to_owned()))),
            ("typed", base_request(typed)),
            (
                "yql",
                base_request(QueryIr::Yql {
                    source: "select * from products where title contains \"fox\"".to_owned(),
                    parameters: BTreeMap::new(),
                }),
            ),
            (
                "yql_properties",
                base_request(QueryIr::Yql {
                    source: "select * from products where title contains \"fox\"".to_owned(),
                    parameters: BTreeMap::new(),
                })
                .with_properties(BTreeMap::from([(
                    "tracelevel".to_owned(),
                    RequestValueIr::Int64(1),
                )])),
            ),
            (
                "filter",
                SearchRequestIr::new(
                    QueryIr::MatchAll,
                    Vec::new(),
                    Some(FilterIr::Exists {
                        field: FieldName::parse("brand").unwrap(),
                    }),
                    Vec::new(),
                    10,
                    0,
                    searchkernel_ir::ProjectionIr::new(Vec::new()),
                    HighlightingIr::Disabled,
                    None,
                    PrefixPolicy::Disabled,
                    TypoPolicy::Disabled,
                ),
            ),
        ];
        for (label, request) in cases {
            let plan = preflight_search_request(&request)
                .unwrap_or_else(|error| panic!("{label} must be lowered: {error:?}"));
            assert!(plan.request.limit == 10, "{label} lost pagination");
        }

        let mut request = base_request(QueryIr::MatchAll)
            .with_grouping(vec![count_grouping()])
            .with_inputs(BTreeMap::from([(
                "rank_input".to_owned(),
                QueryInputIr::Literal(RequestValueIr::Float64(1.5)),
            )]))
            .with_ranking_request(
                "default",
                BTreeMap::from([("rank_property".to_owned(), "value".to_owned())]),
            )
            .with_timeout(Some(std::time::Duration::from_millis(250)))
            .with_query_model(QueryModelIr {
                query_string: Some("headphones".to_owned()),
                default_index: Some("title".to_owned()),
                ..QueryModelIr::default()
            })
            .with_presentation(PresentationIr {
                summary: Some("summary".to_owned()),
                ..PresentationIr::default()
            });
        request = SearchRequestIr::new(
            request.query().clone(),
            request.search_fields().to_vec(),
            request.filter().cloned(),
            vec![SortIr::new(
                FieldName::parse("price").unwrap(),
                SortDirection::Desc,
                MissingPlacement::Last,
            )],
            request.limit(),
            request.offset(),
            searchkernel_ir::ProjectionIr::new(vec![FieldName::parse("title").unwrap()]),
            HighlightingIr::Requested {
                fields: vec![FieldName::parse("title").unwrap()],
                multi_valued_fields: Vec::new(),
            },
            Some(LanguageHint::parse("en").unwrap()),
            PrefixPolicy::Disabled,
            TypoPolicy::Disabled,
        )
        .with_grouping(request.grouping().to_vec())
        .with_inputs(request.inputs().clone())
        .with_ranking_request(
            request.ranking_request().profile.clone(),
            request.ranking_request().properties.clone(),
        )
        .with_timeout(request.timeout())
        .with_query_model(request.query_model().clone())
        .with_presentation(request.presentation().clone());
        let plan = preflight_search_request(&request).expect("complete request must be lowered");
        assert_eq!(plan.request.limit, 10);
        assert_eq!(plan.request.sort.len(), 1);
        assert_eq!(
            plan.request.query_features,
            vec![("rank_input".to_owned(), 1.5)]
        );
        assert_eq!(plan.request.rank_properties.len(), 1);
        assert_eq!(plan.request.summary_class.as_deref(), Some("summary"));
        assert_eq!(plan.request.highlight_fields, vec!["title"]);
        assert_eq!(plan.request.timeout, std::time::Duration::from_millis(250));
        assert_eq!(plan.groupings.len(), 1);
    }

    #[test]
    fn preflight_matrix_rejects_unexecutable_controls() {
        let cases = vec![
            (
                "query_model",
                base_request(QueryIr::MatchAll).with_query_model(QueryModelIr {
                    encoding: Some("utf-8".to_owned()),
                    ..QueryModelIr::default()
                }),
                "query_model_not_supported",
            ),
            (
                "presentation",
                base_request(QueryIr::MatchAll).with_presentation(PresentationIr {
                    format: Some("json".to_owned()),
                    ..PresentationIr::default()
                }),
                "presentation_not_supported",
            ),
            (
                "expression_input",
                base_request(QueryIr::MatchAll).with_inputs(BTreeMap::from([(
                    "rank_input".to_owned(),
                    QueryInputIr::Expression("rankingExpression".to_owned()),
                )])),
                "query_input_expression_not_supported",
            ),
            (
                "grouping",
                base_request(QueryIr::MatchAll).with_grouping(vec![GroupingRequestIr {
                    root: GroupingAllIr {
                        operations: Vec::new(),
                        label: None,
                    },
                    continuations: Vec::new(),
                }]),
                "grouping_level_missing_classifier",
            ),
            (
                "properties",
                base_request(QueryIr::MatchAll).with_properties(BTreeMap::from([(
                    "tracelevel".to_owned(),
                    RequestValueIr::Int64(1),
                )])),
                "query_properties_not_supported",
            ),
            (
                "query_profile",
                base_request(QueryIr::MatchAll).with_query_profile("default"),
                "query_profile_not_supported",
            ),
            (
                "search_chain",
                base_request(QueryIr::MatchAll).with_search_chain("default"),
                "search_chain_not_supported",
            ),
        ];
        for (label, request, code) in cases {
            let error = preflight_search_request(&request)
                .expect_err("unsupported controls must not be silently dropped");
            assert_eq!(error.details[0].code, code, "{label} error code");
        }
    }

    #[test]
    fn present_empty_container_controls_are_rejected_at_preflight() {
        for (label, request, code) in [
            (
                "query_profile",
                base_request(QueryIr::MatchAll).with_query_profile(""),
                "query_profile_not_supported",
            ),
            (
                "search_chain",
                base_request(QueryIr::MatchAll).with_search_chain(""),
                "search_chain_not_supported",
            ),
        ] {
            let error = preflight_search_request(&request)
                .expect_err("present-empty control must not execute as omitted");
            assert_eq!(error.code, searchkernel_ir::ErrorCode::Unsupported, "{label}");
            assert_eq!(error.details[0].code, code, "{label}");
        }
    }

    #[test]
    fn preflight_disposition_matrix_covers_entire_search_request_surface() {
        let request = base_request(QueryIr::MatchAll);
        let dispositions = [
            ("query", "lowered"),
            ("search_fields", "lowered_or_rejected"),
            ("filter", "lowered"),
            ("sort", "lowered_or_rejected"),
            ("grouping", "lowered_or_rejected"),
            ("ranking_request", "lowered"),
            ("inputs", "lowered_or_rejected"),
            ("limit", "lowered"),
            ("offset", "lowered"),
            ("projection", "lowered"),
            ("highlighting", "lowered_or_rejected"),
            ("language", "lowered_or_rejected"),
            ("query_model", "lowered_or_rejected"),
            ("presentation", "lowered_or_rejected"),
            ("properties", "lowered_for_yql_or_rejected"),
            ("query_profile", "rejected"),
            ("search_chain", "rejected"),
            ("timeout", "lowered"),
            ("prefix_policy", "lowered_or_rejected"),
            ("typo_policy", "lowered_or_rejected"),
        ];
        let classified = dispositions
            .iter()
            .map(|(field, _)| *field)
            .collect::<Vec<_>>();

        assert_eq!(
            request.semantic_field_names(),
            classified.as_slice(),
            "every canonical SearchRequestIr field needs an explicit adapter disposition"
        );
    }
}
