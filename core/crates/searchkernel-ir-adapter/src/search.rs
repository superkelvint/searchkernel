//! Search, query, filter, sort, and facet lowering for the native Vespa adapter.

use crate::{analysis_options, unsupported};
use searchkernel_ir::{
    FilterIr, PortableError, QueryInputIr, QueryIr, RangeIr, RequestValueIr, Result, ScalarValueIr,
    SearchRequestIr,
};
use searchkernel_vespa_engine::{
    FacetRequest, Query, SearchRequest, SortField, SortOrder, SummaryRequest, Value as VespaValue,
};

pub(crate) fn lower_search_query(
    request: &SearchRequestIr,
    language: searchkernel_vespa_engine::LinguisticsOptions,
) -> Result<Query> {
    let mut clauses = vec![lower_query_with_language(
        request.query(),
        request.search_fields(),
        language,
    )?];
    if let Some(filter) = request.filter() {
        clauses.push(lower_filter(filter)?);
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
    let mut m_request = SearchRequest::new(query)
        .limit(request.limit())
        .offset(request.offset())
        .rank_profile(request.ranking_request().profile.clone())
        .language(language);
    for (name, input) in request.inputs() {
        m_request = m_request.query_feature(name, query_feature_value(name, input)?);
    }
    for (name, value) in &request.ranking_request().properties {
        m_request = m_request.rank_property(name, value);
    }
    if let Some(timeout) = request.timeout() {
        m_request = m_request.timeout(timeout);
    }
    for sort in request.sort() {
        m_request = m_request.sort(lower_sort(sort)?);
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FacetLowering {
    pub(crate) field: String,
    pub(crate) limit: u32,
    pub(crate) order: searchkernel_vespa_engine::GroupingOrder,
}

pub(crate) fn lower_facets_into_request(
    request: SearchRequest,
    facets: &[FacetLowering],
) -> SearchRequest {
    facets.iter().fold(request, |request, facet| {
        request.facet(FacetRequest::new(&facet.field, facet.limit, facet.order))
    })
}

/// Recognize the exact structured grouping emitted by the Rust facet facade:
/// `all(group(attribute(field)), each(max(limit), order(...), output(count)))`.
/// Other valid V2 grouping programs remain unsupported until they have a
/// faithful native lowering.
pub(crate) fn lower_facet_groupings(
    grouping: &[searchkernel_ir::GroupingRequestIr],
) -> Result<Vec<FacetLowering>> {
    grouping
        .iter()
        .enumerate()
        .map(|(index, value)| {
            facet_lowering(value).ok_or_else(|| {
                PortableError::unsupported(
                    "grouping_not_supported",
                    Some(format!("grouping[{index}]")),
                    "the current native adapter supports only the structured grouping emitted by the facet convenience API",
                )
            })
        })
        .collect()
}

pub(crate) fn facet_lowering(value: &searchkernel_ir::GroupingRequestIr) -> Option<FacetLowering> {
    use searchkernel_ir::GroupingOperationIr as Operation;
    use searchkernel_ir::{GroupingAggregationIr as Aggregation, GroupingExpressionIr as Expr};

    if !value.continuations.is_empty() || value.root.label.is_some() {
        return None;
    }
    let [Operation::Group(Expr::Attribute(field)), Operation::Each(each)] =
        value.root.operations.as_slice()
    else {
        return None;
    };
    if each.label.is_some() {
        return None;
    }
    let [Operation::Max(Some(limit)), Operation::Order(order), Operation::Output(output)] =
        each.operations.as_slice()
    else {
        return None;
    };
    let [order] = order.as_slice() else {
        return None;
    };
    let [output] = output.as_slice() else {
        return None;
    };
    if output.label.is_some()
        || !matches!(output.expression, Expr::Aggregation(ref value) if matches!(value.as_ref(), Aggregation::Count))
    {
        return None;
    }
    let order = match order {
        Expr::Negative(value) if matches!(value.as_ref(), Expr::Aggregation(value) if matches!(value.as_ref(), Aggregation::Count)) => {
            searchkernel_vespa_engine::GroupingOrder::CountDescending
        }
        Expr::Aggregation(value) if matches!(value.as_ref(), Aggregation::Count) => {
            searchkernel_vespa_engine::GroupingOrder::CountAscending
        }
        Expr::Negative(value) if matches!(value.as_ref(), Expr::Attribute(value) if value == field) => {
            searchkernel_vespa_engine::GroupingOrder::ValueDescending
        }
        Expr::Attribute(value) if value == field => {
            searchkernel_vespa_engine::GroupingOrder::ValueAscending
        }
        _ => return None,
    };
    Some(FacetLowering {
        field: field.clone(),
        limit: *limit,
        order,
    })
}

pub(crate) fn validate_search_request(request: &SearchRequestIr) -> Result<()> {
    lower_facet_groupings(request.grouping())?;
    validate_ranking_inputs(request)?;
    if !request.properties().is_empty() {
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
    if request.query_model().query_string.is_some()
        || request.query_model().default_index.is_some()
        || request.query_model().encoding.is_some()
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
            "the current native adapter supports only query_model.language",
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
    if matches!(request.query(), QueryIr::Yql { .. }) {
        return Err(PortableError::unsupported(
            "yql_not_supported",
            Some("query"),
            "the current native adapter does not expose a YQL execution path",
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
        query_feature_value(name, input)?;
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
pub(crate) fn lower_query(
    query: &QueryIr,
    fields: &[searchkernel_ir::SearchFieldIr],
) -> Result<Query> {
    lower_query_with_language(
        query,
        fields,
        searchkernel_vespa_engine::LinguisticsOptions::default(),
    )
}

pub(crate) fn lower_query_with_language(
    query: &QueryIr,
    fields: &[searchkernel_ir::SearchFieldIr],
    language: searchkernel_vespa_engine::LinguisticsOptions,
) -> Result<Query> {
    match query {
        QueryIr::MatchAll => Ok(Query::True),
        QueryIr::Text(text) => {
            let term_count = searchkernel_vespa_engine::Linguistics
                .analyze_query(text, language)
                .terms
                .len();
            let clauses = fields
                .iter()
                .map(|field| {
                    if term_count > 1 {
                        Query::phrase(field.field().as_str(), text)
                    } else {
                        Query::term(field.field().as_str(), text)
                    }
                })
                .collect::<Vec<_>>();
            match clauses.as_slice() {
                [] => Err(unsupported("text query has no searchable fields")),
                [query] => Ok(query.clone()),
                _ => Ok(Query::Or(clauses)),
            }
        }
        QueryIr::Term { field, value } => match value {
            ScalarValueIr::String(value) => Ok(Query::term(field.as_str(), value)),
            ScalarValueIr::Int64(value) => {
                Ok(Query::equals(field.as_str(), integer_query_value(*value)?))
            }
            _ => Err(unsupported("unsupported typed query value")),
        },
        QueryIr::And(children) => Ok(Query::And(
            children
                .iter()
                .map(|child| lower_query_with_language(child, fields, language))
                .collect::<Result<Vec<_>>>()?,
        )),
        QueryIr::Or(children) => Ok(Query::Or(
            children
                .iter()
                .map(|child| lower_query_with_language(child, fields, language))
                .collect::<Result<Vec<_>>>()?,
        )),
        QueryIr::Not(child) => Ok(Query::Not(Box::new(lower_query_with_language(
            child, fields, language,
        )?))),
        QueryIr::Yql { .. } => Err(PortableError::unsupported(
            "yql_not_supported",
            Some("query"),
            "the current native adapter does not expose a YQL execution path",
        )),
        QueryIr::Typed(_) => Err(PortableError::unsupported(
            "typed_query_not_supported",
            Some("query"),
            "the current native adapter does not expose execution for typed V2 query expressions",
        )),
    }
}

pub(crate) fn lower_filter(filter: &FilterIr) -> Result<Query> {
    match filter {
        FilterIr::And(children) => Ok(Query::And(
            children
                .iter()
                .map(lower_filter)
                .collect::<Result<Vec<_>>>()?,
        )),
        FilterIr::Or(children) => Ok(Query::Or(
            children
                .iter()
                .map(lower_filter)
                .collect::<Result<Vec<_>>>()?,
        )),
        FilterIr::Not(child) => Ok(Query::Not(Box::new(lower_filter(child)?))),
        FilterIr::Exists { field } => Ok(Query::AttributeRange {
            field: field.to_string(),
            lower: searchkernel_vespa_engine::AttributeBound::Unbounded,
            upper: searchkernel_vespa_engine::AttributeBound::Unbounded,
        }),
        FilterIr::Comparison(comparison) => lower_comparison(comparison),
        FilterIr::Range(range) => lower_range(range),
        FilterIr::In { field, values } => Ok(Query::Or(
            values
                .iter()
                .map(|value| {
                    Ok(Query::AttributeEquals {
                        field: field.to_string(),
                        value: lower_attribute_value(value)?,
                    })
                })
                .collect::<Result<Vec<_>>>()?,
        )),
    }
}

pub(crate) fn lower_comparison(comparison: &searchkernel_ir::ComparisonIr) -> Result<Query> {
    use searchkernel_ir::ComparisonOp;
    let field = comparison.field().to_string();
    let value = lower_attribute_value(comparison.value())?;
    match comparison.op() {
        ComparisonOp::Eq => Ok(Query::AttributeEquals { field, value }),
        ComparisonOp::Ne => Ok(Query::Not(Box::new(Query::AttributeEquals {
            field,
            value,
        }))),
        ComparisonOp::Lt | ComparisonOp::Lte | ComparisonOp::Gt | ComparisonOp::Gte => {
            let lower_bound = matches!(comparison.op(), ComparisonOp::Gt | ComparisonOp::Gte);
            let included = matches!(comparison.op(), ComparisonOp::Gte | ComparisonOp::Lte);
            if !matches!(value, VespaValue::Integer(_) | VespaValue::Float(_)) {
                return Err(unsupported("ordered comparisons require a numeric field"));
            }
            let bound = if included {
                searchkernel_vespa_engine::AttributeBound::Included(value)
            } else {
                searchkernel_vespa_engine::AttributeBound::Excluded(value)
            };
            Ok(Query::AttributeRange {
                field,
                lower: if lower_bound {
                    bound.clone()
                } else {
                    searchkernel_vespa_engine::AttributeBound::Unbounded
                },
                upper: if lower_bound {
                    searchkernel_vespa_engine::AttributeBound::Unbounded
                } else {
                    bound
                },
            })
        }
    }
}

pub(crate) fn lower_range(range: &RangeIr) -> Result<Query> {
    Ok(Query::AttributeRange {
        field: range.field().to_string(),
        lower: attribute_bound(range.lower_bound(), range.include_lower())?,
        upper: attribute_bound(range.upper_bound(), range.include_upper())?,
    })
}

pub(crate) fn attribute_bound(
    value: &searchkernel_ir::FilterRangeBoundIr,
    included: bool,
) -> Result<searchkernel_vespa_engine::AttributeBound> {
    match value {
        searchkernel_ir::FilterRangeBoundIr::Unbounded => {
            Ok(searchkernel_vespa_engine::AttributeBound::Unbounded)
        }
        searchkernel_ir::FilterRangeBoundIr::NegativeInfinity
        | searchkernel_ir::FilterRangeBoundIr::PositiveInfinity => Err(unsupported(
            "infinite filter bounds are not supported by the native adapter",
        )),
        searchkernel_ir::FilterRangeBoundIr::Finite(value) => {
            let value = lower_attribute_value(value)?;
            Ok(if included {
                searchkernel_vespa_engine::AttributeBound::Included(value)
            } else {
                searchkernel_vespa_engine::AttributeBound::Excluded(value)
            })
        }
    }
}

pub(crate) fn lower_attribute_value(value: &ScalarValueIr) -> Result<VespaValue> {
    match value {
        ScalarValueIr::Int64(value) => Ok(VespaValue::Integer(integer_query_value(*value)?)),
        ScalarValueIr::Float64(value) if value.is_finite() => Ok(VespaValue::Float(*value)),
        ScalarValueIr::String(value) => Ok(VespaValue::String(value.clone())),
        ScalarValueIr::Bool(value) => Ok(VespaValue::Boolean(*value)),
        ScalarValueIr::Float64(_) => Err(unsupported("float comparison value must be finite")),
    }
}

pub(crate) fn integer_query_value(value: i64) -> Result<i64> {
    if value == i64::MIN {
        return Err(unsupported(
            "INT64_MIN is reserved as Vespa's undefined integer attribute sentinel",
        ));
    }
    Ok(value)
}
