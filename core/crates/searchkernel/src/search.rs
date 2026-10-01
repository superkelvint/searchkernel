use crate::error::Result;
use crate::filter::{Filter, SearchFilterInput};
use crate::grouping::GroupingPlan;
use crate::index::Index;
use crate::query::Query;
use crate::result::SearchResult;
use serde::de::DeserializeOwned;
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// The direction in which a sort field is ordered.
pub enum SortDirection {
    Asc,
    Desc,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Where documents with no value for a sort field are placed.
pub enum MissingPlacement {
    First,
    Last,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SortTarget {
    Field,
    Relevance,
    RankFeature,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// A field and direction used to order search hits.
pub struct Sort {
    pub(crate) target: SortTarget,
    pub(crate) name: Option<String>,
    pub(crate) direction: SortDirection,
    pub(crate) missing: Option<MissingPlacement>,
}

impl Sort {
    /// Sort by `field` from the smallest value to the largest value.
    pub fn asc(field: impl Into<String>) -> Self {
        Self {
            target: SortTarget::Field,
            name: Some(field.into()),
            direction: SortDirection::Asc,
            missing: None,
        }
    }

    /// Sort by `field` from the largest value to the smallest value.
    pub fn desc(field: impl Into<String>) -> Self {
        Self {
            target: SortTarget::Field,
            name: Some(field.into()),
            direction: SortDirection::Desc,
            missing: None,
        }
    }

    /// Sort by the engine's relevance score.
    pub fn relevance(direction: SortDirection) -> Self {
        Self {
            target: SortTarget::Relevance,
            name: None,
            direction,
            missing: None,
        }
    }

    /// Sort by a rank feature without treating it as a document field.
    pub fn feature(name: impl Into<String>, direction: SortDirection) -> Self {
        Self {
            target: SortTarget::RankFeature,
            name: Some(name.into()),
            direction,
            missing: None,
        }
    }

    /// Put documents without this field before documents with a value.
    pub fn missing_first(mut self) -> Self {
        self.missing = Some(MissingPlacement::First);
        self
    }

    /// Put documents without this field after documents with a value.
    pub fn missing_last(mut self) -> Self {
        self.missing = Some(MissingPlacement::Last);
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// The order used to return facet values.
pub enum FacetSort {
    CountDesc,
    CountAsc,
    ValueAsc,
    ValueDesc,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// A request for the distinct values of one field.
pub struct Facet {
    pub(crate) field: String,
    pub(crate) name: Option<String>,
    pub(crate) limit: Option<u32>,
    pub(crate) sort: Option<FacetSort>,
}

impl Facet {
    /// Create a facet request for `field`.
    pub fn new(field: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            name: None,
            limit: None,
            sort: None,
        }
    }

    /// Set the stable result name for this facet.
    pub fn name(mut self, value: impl Into<String>) -> Self {
        let value = value.into();
        self.name = (!value.is_empty()).then_some(value);
        self
    }

    /// Limit the number of values returned for this facet.
    pub fn limit(mut self, value: u32) -> Self {
        self.limit = Some(value);
        self
    }

    /// Choose how values in this facet are ordered.
    pub fn sort(mut self, value: FacetSort) -> Self {
        self.sort = Some(value);
        self
    }
}

impl From<&str> for Facet {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for Facet {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// The ranking policy used to score matching documents.
pub struct Ranking {
    pub(crate) kind: RankingKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum RankingKind {
    DefaultRelevanceV1,
    Bm25,
    Named(String),
}

impl Ranking {
    /// Use Search Kernel's default relevance ranking profile.
    pub fn default_relevance_v1() -> Self {
        Self {
            kind: RankingKind::DefaultRelevanceV1,
        }
    }

    /// Rank text matches with BM25.
    pub fn bm25() -> Self {
        Self {
            kind: RankingKind::Bm25,
        }
    }

    /// Select a configured Vespa rank profile by name.
    pub fn named(profile: impl Into<String>) -> Self {
        Self {
            kind: RankingKind::Named(profile.into()),
        }
    }

    /// Alias for [`Ranking::named`].
    pub fn profile(profile: impl Into<String>) -> Self {
        Self::named(profile)
    }
}

impl From<String> for Ranking {
    fn from(value: String) -> Self {
        Self::named(value)
    }
}

impl From<&str> for Ranking {
    fn from(value: &str) -> Self {
        Self::named(value)
    }
}

#[derive(Clone, Debug, PartialEq)]
/// A literal value supplied to a query input.
pub enum QueryInputValue {
    Bool(bool),
    Int64(i64),
    UInt64(u64),
    Float64(f64),
    String(String),
    Json(serde_json::Value),
    Tensor(crate::Tensor),
}

impl From<bool> for QueryInputValue {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum QueryInput {
    Literal(QueryInputValue),
    Expression(String),
}

/// Query model type controls accepted by the canonical portable request.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct QueryModelType {
    composite: Option<String>,
    tokenization: Option<String>,
    syntax: Option<String>,
    profile: Option<String>,
    yql_default: Option<bool>,
}

impl QueryModelType {
    pub fn composite(mut self, value: impl Into<String>) -> Self {
        self.composite = Some(value.into());
        self
    }

    pub fn tokenization(mut self, value: impl Into<String>) -> Self {
        self.tokenization = Some(value.into());
        self
    }

    pub fn syntax(mut self, value: impl Into<String>) -> Self {
        self.syntax = Some(value.into());
        self
    }

    pub fn profile(mut self, value: impl Into<String>) -> Self {
        self.profile = Some(value.into());
        self
    }

    pub fn yql_default(mut self, value: bool) -> Self {
        self.yql_default = Some(value);
        self
    }
}

impl From<i64> for QueryInputValue {
    fn from(value: i64) -> Self {
        Self::Int64(value)
    }
}

impl From<String> for QueryInputValue {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<&str> for QueryInputValue {
    fn from(value: &str) -> Self {
        Self::String(value.to_owned())
    }
}

impl From<u64> for QueryInputValue {
    fn from(value: u64) -> Self {
        Self::UInt64(value)
    }
}

impl From<f64> for QueryInputValue {
    fn from(value: f64) -> Self {
        Self::Float64(value)
    }
}

impl From<serde_json::Value> for QueryInputValue {
    fn from(value: serde_json::Value) -> Self {
        Self::Json(value)
    }
}

impl From<i32> for QueryInputValue {
    fn from(value: i32) -> Self {
        Self::Int64(i64::from(value))
    }
}

impl From<u32> for QueryInputValue {
    fn from(value: u32) -> Self {
        Self::UInt64(u64::from(value))
    }
}

impl From<crate::Tensor> for QueryInputValue {
    fn from(value: crate::Tensor) -> Self {
        Self::Tensor(value)
    }
}

/// The normalized meaning of a search request after schema defaults are
/// applied.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct InspectedQuery {
    pub query: String,
    pub search_fields: Vec<crate::schema::SearchField>,
    pub filter: Option<Filter>,
    pub sorts: Vec<Sort>,
    pub facets: Vec<Facet>,
    pub ranking: Ranking,
    pub limit: u32,
    pub offset: u32,
    pub projection: Vec<String>,
    pub highlighting: Option<Vec<String>>,
    pub language: Option<String>,
    pub timeout: Option<Duration>,
}

/// A fluent search request.
///
/// The builder stores caller intent. Validation, defaults, and conversion to
/// the backend-independent request happen when [`SearchBuilder::execute`] is
/// called.
pub struct SearchBuilder {
    pub(crate) index: Index,
    pub(crate) query_source: QuerySource,
    pub(crate) search_fields: Option<Vec<crate::schema::SearchField>>,
    pub(crate) filters: Vec<Filter>,
    pub(crate) textual_filters: Vec<(String, crate::filter::FilterBindings)>,
    pub(crate) sorts: Vec<Sort>,
    pub(crate) facets: Vec<Facet>,
    pub(crate) ranking: Option<Ranking>,
    pub(crate) query_inputs: std::collections::BTreeMap<String, QueryInput>,
    pub(crate) yql_parameters: std::collections::BTreeMap<String, QueryInputValue>,
    pub(crate) rank_properties: std::collections::BTreeMap<String, String>,
    pub(crate) groupings: Vec<GroupingPlan>,
    aggregations: Vec<crate::GroupingOutput>,
    pub(crate) projection: Option<Vec<String>>,
    pub(crate) summary: Option<String>,
    pub(crate) highlighting: Option<Vec<String>>,
    pub(crate) language: Option<String>,
    pub(crate) timeout: Option<Duration>,
    pub(crate) limit: Option<u32>,
    pub(crate) offset: Option<u32>,
    page: Option<(i64, i64)>,
    page_repeated: bool,
    query_string: Option<String>,
    default_index: Option<String>,
    locale: Option<String>,
    sources: Vec<String>,
    restrict: Vec<String>,
    search_path: Option<String>,
    search_group: Option<u32>,
    query_model_type: Option<QueryModelType>,
    format: Option<String>,
    bolding: Option<bool>,
    timing: Option<bool>,
    properties: std::collections::BTreeMap<String, serde_json::Value>,
    query_profile: Option<String>,
    search_chain: Option<String>,
    raw_groupings: Vec<String>,
}

/// A YQL-origin search builder. It shares canonical search lowering with
/// `SearchBuilder`; unsupported combinations are rejected by the resolver.
pub type YqlBuilder = SearchBuilder;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum QuerySource {
    Text(String),
    MatchAll,
    Typed(Query),
    Yql(String),
}

impl SearchBuilder {
    pub(crate) fn new(index: Index, query: impl Into<String>) -> Self {
        Self::with_source(index, QuerySource::Text(query.into()))
    }

    pub(crate) fn match_all(index: Index) -> Self {
        Self::with_source(index, QuerySource::MatchAll)
    }

    pub(crate) fn yql(index: Index, source: impl Into<String>) -> Self {
        Self::with_source(index, QuerySource::Yql(source.into()))
    }

    fn with_source(index: Index, query_source: QuerySource) -> Self {
        Self {
            index,
            query_source,
            search_fields: None,
            filters: Vec::new(),
            textual_filters: Vec::new(),
            sorts: Vec::new(),
            facets: Vec::new(),
            ranking: None,
            query_inputs: std::collections::BTreeMap::new(),
            yql_parameters: std::collections::BTreeMap::new(),
            rank_properties: std::collections::BTreeMap::new(),
            groupings: Vec::new(),
            aggregations: Vec::new(),
            projection: None,
            summary: None,
            highlighting: None,
            language: None,
            timeout: None,
            limit: None,
            offset: None,
            page: None,
            page_repeated: false,
            query_string: None,
            default_index: None,
            locale: None,
            sources: Vec::new(),
            restrict: Vec::new(),
            search_path: None,
            search_group: None,
            query_model_type: None,
            format: None,
            bolding: None,
            timing: None,
            properties: std::collections::BTreeMap::new(),
            query_profile: None,
            search_chain: None,
            raw_groupings: Vec::new(),
        }
    }

    /// Select the fields used for a text query.
    pub fn search_fields<I>(mut self, fields: I) -> Self
    where
        I: IntoIterator,
        I::Item: Into<crate::schema::SearchField>,
    {
        self.search_fields = Some(fields.into_iter().map(Into::into).collect());
        self
    }

    /// Append one field used for a text query.
    pub fn search_field(mut self, field: impl Into<crate::schema::SearchField>) -> Self {
        self.search_fields
            .get_or_insert_with(Vec::new)
            .push(field.into());
        self
    }

    /// Add one structured or textual filter to the search request.
    ///
    /// Textual clauses keep their typed bindings separate from the source and
    /// are parsed later by the canonical IR resolver. For example:
    ///
    /// ```no_run
    /// # use searchkernel::{params, Index};
    /// # let index: Index = todo!();
    /// let request = index.search("").filter((
    ///     "price <= :max",
    ///     params! { "max" => 120.0 },
    /// ));
    /// # let _ = request;
    /// ```
    pub fn filter(mut self, filter: impl Into<SearchFilterInput>) -> Self {
        match filter.into() {
            SearchFilterInput::Structured(filter) => self.filters.push(filter),
            SearchFilterInput::Textual { source, bindings } => {
                self.textual_filters.push((source, bindings));
            }
        }
        self
    }

    /// Add one sort order to the search request.
    pub fn sort(mut self, sort: Sort) -> Self {
        self.sorts.push(sort);
        self
    }

    /// Sort by relevance in the supplied direction.
    pub fn sort_relevance(self, direction: SortDirection) -> Self {
        self.sort(Sort::relevance(direction))
    }

    /// Sort by a rank feature in the supplied direction.
    pub fn sort_feature(self, feature: impl Into<String>, direction: SortDirection) -> Self {
        self.sort(Sort::feature(feature, direction))
    }

    /// Add one facet request to the search request.
    pub fn facet(mut self, facet: Facet) -> Self {
        self.facets.push(facet);
        self
    }

    /// Add facets by field name.
    pub fn facets<I>(mut self, facets: I) -> Self
    where
        I: IntoIterator,
        I::Item: Into<Facet>,
    {
        self.facets.extend(facets.into_iter().map(Into::into));
        self
    }

    /// Select the ranking policy for the search.
    pub fn ranking(mut self, ranking: impl Into<Ranking>) -> Self {
        self.ranking = Some(ranking.into());
        self
    }

    /// Supply a finite scalar query feature to the selected rank profile.
    pub fn query_input(
        mut self,
        name: impl Into<String>,
        value: impl Into<QueryInputValue>,
    ) -> Self {
        self.query_inputs
            .insert(name.into(), QueryInput::Literal(value.into()));
        self
    }

    /// Supply a literal query input value.
    pub fn input(self, name: impl Into<String>, value: impl Into<QueryInputValue>) -> Self {
        self.query_input(name, value)
    }

    /// Supply an expression query input, such as `embed(transformer,@query)`.
    pub fn input_expr(mut self, name: impl Into<String>, expression: impl Into<String>) -> Self {
        self.query_inputs
            .insert(name.into(), QueryInput::Expression(expression.into()));
        self
    }

    /// Supply a request rank property to the selected rank profile.
    pub fn rank_property(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.rank_properties.insert(name.into(), value.into());
        self
    }

    /// Execute a typed query through the canonical IR query path.
    pub fn typed_query(mut self, query: Query) -> Self {
        self.query_source = QuerySource::Typed(query);
        self
    }

    /// Bind a scalar or tensor value to a YQL `@parameter` reference.
    pub fn yql_parameter(
        mut self,
        name: impl Into<String>,
        value: impl Into<QueryInputValue>,
    ) -> Self {
        self.yql_parameters.insert(name.into(), value.into());
        self
    }

    /// Add a general grouping plan. Facets remain available as a compatibility
    /// convenience, while plans are returned as recursive grouping results.
    pub fn grouping(mut self, grouping: GroupingPlan) -> Self {
        self.groupings.push(grouping);
        self
    }

    /// Add a grouping plan. Alias for [`SearchBuilder::grouping`].
    pub fn group(self, grouping: GroupingPlan) -> Self {
        self.grouping(grouping)
    }

    /// Add a root-level aggregation using the canonical grouping result model.
    pub fn aggregate(mut self, aggregation: impl Into<crate::GroupingOutput>) -> Self {
        self.aggregations.push(aggregation.into());
        self
    }

    /// Pass raw grouping text to the canonical resolver.
    ///
    /// The current resolver returns an explicit `Unsupported` error because
    /// raw grouping has no normalized representation yet.
    pub fn raw_grouping(mut self, source: impl Into<String>) -> Self {
        self.raw_groupings.push(source.into());
        self
    }

    /// Restrict returned documents to these fields.
    pub fn projection<I>(mut self, fields: I) -> Self
    where
        I: IntoIterator,
        I::Item: Into<String>,
    {
        self.projection = Some(fields.into_iter().map(Into::into).collect());
        self
    }

    /// Restrict returned documents to these fields. Alias for `projection`.
    pub fn fields<I>(self, fields: I) -> Self
    where
        I: IntoIterator,
        I::Item: Into<String>,
    {
        self.projection(fields)
    }

    /// Select the document summary class used for this search.
    pub fn summary(mut self, name: impl Into<String>) -> Self {
        self.summary = Some(name.into());
        self
    }

    /// Request highlighted fragments for these fields.
    pub fn highlight<I>(mut self, fields: I) -> Self
    where
        I: IntoIterator,
        I::Item: Into<String>,
    {
        self.highlighting = Some(fields.into_iter().map(Into::into).collect());
        self
    }

    /// Set the language hint used by the backend.
    pub fn language(mut self, language: impl Into<String>) -> Self {
        self.language = Some(language.into());
        self
    }

    pub fn locale(mut self, locale: impl Into<String>) -> Self {
        self.locale = Some(locale.into());
        self
    }

    pub fn default_index(mut self, index: impl Into<String>) -> Self {
        self.default_index = Some(index.into());
        self
    }

    pub fn sources<I, S>(mut self, sources: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.sources.extend(sources.into_iter().map(Into::into));
        self
    }

    pub fn restrict<I, S>(mut self, fields: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.restrict.extend(fields.into_iter().map(Into::into));
        self
    }

    pub fn search_path(mut self, path: impl Into<String>) -> Self {
        self.search_path = Some(path.into());
        self
    }

    pub fn search_group(mut self, group: u32) -> Self {
        self.search_group = Some(group);
        self
    }

    pub fn query_string(mut self, query: impl Into<String>) -> Self {
        self.query_string = Some(query.into());
        self
    }

    pub fn query_model_type(mut self, value: QueryModelType) -> Self {
        self.query_model_type = Some(value);
        self
    }

    pub fn format(mut self, value: impl Into<String>) -> Self {
        self.format = Some(value.into());
        self
    }

    pub fn bolding(mut self, value: bool) -> Self {
        self.bolding = Some(value);
        self
    }

    pub fn timing(mut self, value: bool) -> Self {
        self.timing = Some(value);
        self
    }

    pub fn property(
        mut self,
        name: impl Into<String>,
        value: impl Into<serde_json::Value>,
    ) -> Self {
        self.properties.insert(name.into(), value.into());
        self
    }

    pub fn properties<I, K, V>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<serde_json::Value>,
    {
        self.properties.extend(
            values
                .into_iter()
                .map(|(key, value)| (key.into(), value.into())),
        );
        self
    }

    pub fn query_profile(mut self, value: impl Into<String>) -> Self {
        self.query_profile = Some(value.into());
        self
    }

    pub fn search_chain(mut self, value: impl Into<String>) -> Self {
        self.search_chain = Some(value.into());
        self
    }

    /// Stop the engine from spending more than this duration on the search.
    /// This is part of the portable request and is distinct from cancelling a
    /// client-side future.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Limit the number of hits returned.
    pub fn limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Skip this many hits before returning results.
    pub fn offset(mut self, offset: u32) -> Self {
        self.offset = Some(offset);
        self
    }

    /// Select a one-based page and page size.
    ///
    /// The page end must fit the portable u32 range, matching the other
    /// first-party clients.
    pub fn page(mut self, page: i64, size: i64) -> Self {
        if self.page.is_some() {
            self.page_repeated = true;
        }
        self.page = Some((page, size));
        self
    }

    /// Execute the completed search request.
    pub fn execute<T: DeserializeOwned>(self) -> crate::Result<SearchResult<T>> {
        self.index.execute_search(&self)
    }

    /// Inspect the normalized request without executing a search.
    pub fn inspect(self) -> crate::Result<InspectedQuery> {
        self.index.inspect_search(&self)
    }
}

pub(crate) fn to_resolver_input(request: &SearchBuilder) -> Result<searchkernel_ir::SearchInput> {
    let search_fields = request.search_fields.as_ref().map(|fields| {
        fields
            .iter()
            .map(|field| searchkernel_ir::SearchFieldInput {
                field: field.field.clone(),
                weight: field.weight,
            })
            .collect()
    });
    let filters = request
        .filters
        .iter()
        .map(crate::filter::to_resolver_input)
        .collect::<Result<Vec<_>>>()?;
    let textual_filters = request
        .textual_filters
        .iter()
        .map(|(source, bindings)| {
            let bindings = bindings
                .iter()
                .map(|(name, value)| {
                    searchkernel_ir::filter_binding_value_from_json(
                        value,
                        format!("filter.bindings.{name}"),
                    )
                    .map(|value| (name.clone(), value))
                    .map_err(crate::error::Error::from_ir)
                })
                .collect::<Result<std::collections::BTreeMap<_, _>>>()?;
            Ok(searchkernel_ir::TextualFilterInput {
                source: source.clone(),
                bindings,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let sorts = request
        .sorts
        .iter()
        .map(|sort| searchkernel_ir::SortInput {
            target: match sort.target {
                SortTarget::Field => {
                    searchkernel_ir::SortTargetInput::Field(sort.name.clone().unwrap_or_default())
                }
                SortTarget::Relevance => searchkernel_ir::SortTargetInput::Relevance,
                SortTarget::RankFeature => searchkernel_ir::SortTargetInput::RankFeature(
                    sort.name.clone().unwrap_or_default(),
                ),
            },
            direction: match sort.direction {
                SortDirection::Asc => searchkernel_ir::SortDirection::Asc,
                SortDirection::Desc => searchkernel_ir::SortDirection::Desc,
            },
            missing: sort.missing.map(|missing| match missing {
                MissingPlacement::First => searchkernel_ir::MissingPlacement::First,
                MissingPlacement::Last => searchkernel_ir::MissingPlacement::Last,
            }),
            annotations: std::collections::BTreeMap::new(),
        })
        .collect();
    let mut grouping = request
        .facets
        .iter()
        .map(|facet| {
            let input = searchkernel_ir::FacetInput {
                field: facet.field.clone(),
                name: facet.name.clone(),
                limit: facet.limit,
                sort: facet.sort.map(|sort| match sort {
                    FacetSort::CountDesc => searchkernel_ir::FacetSortInput::CountDesc,
                    FacetSort::CountAsc => searchkernel_ir::FacetSortInput::CountAsc,
                    FacetSort::ValueAsc => searchkernel_ir::FacetSortInput::ValueAsc,
                    FacetSort::ValueDesc => searchkernel_ir::FacetSortInput::ValueDesc,
                }),
            };
            searchkernel_ir::GroupingInput::Structured(searchkernel_ir::facet_grouping_input(
                &input,
            ))
        })
        .collect::<Vec<_>>();
    grouping.extend(
        request
            .groupings
            .clone()
            .into_iter()
            .map(|grouping| {
                grouping
                    .into_ir()
                    .map(searchkernel_ir::GroupingInput::Structured)
            })
            .collect::<Result<Vec<_>>>()?,
    );
    if !request.aggregations.is_empty() {
        grouping.push(searchkernel_ir::GroupingInput::Structured(
            crate::grouping::aggregate_request(&request.aggregations)?,
        ));
    }
    grouping.extend(
        request
            .raw_groupings
            .iter()
            .cloned()
            .map(searchkernel_ir::GroupingInput::Raw),
    );
    let ranking = request.ranking.as_ref().map(|ranking| match &ranking.kind {
        RankingKind::DefaultRelevanceV1 => searchkernel_ir::RankingInput::DefaultRelevanceV1,
        RankingKind::Bm25 => searchkernel_ir::RankingInput::Bm25,
        RankingKind::Named(profile) => searchkernel_ir::RankingInput::Named(profile.clone()),
    });
    let query_inputs = request
        .query_inputs
        .iter()
        .map(|(name, value)| {
            let value = match value {
                QueryInput::Expression(expression) => {
                    return Ok((
                        name.clone(),
                        searchkernel_ir::QueryInputIr::Expression(expression.clone()),
                    ));
                }
                QueryInput::Literal(value) => match value {
                    QueryInputValue::Bool(value) => searchkernel_ir::RequestValueIr::Bool(*value),
                    QueryInputValue::Int64(value) => searchkernel_ir::RequestValueIr::Int64(*value),
                    QueryInputValue::UInt64(value) => {
                        searchkernel_ir::RequestValueIr::UInt64(*value)
                    }
                    QueryInputValue::Float64(value) => {
                        searchkernel_ir::RequestValueIr::Float64(*value)
                    }
                    QueryInputValue::String(value) => {
                        searchkernel_ir::RequestValueIr::String(value.clone())
                    }
                    QueryInputValue::Json(value) => searchkernel_ir::request_value_from_json(
                        value,
                        format!("query_inputs.{name}"),
                    )
                    .map_err(crate::error::Error::from_ir)?,
                    QueryInputValue::Tensor(value) => {
                        searchkernel_ir::RequestValueIr::Tensor(searchkernel_ir::RequestTensorIr {
                            type_spec: value.type_spec().to_owned(),
                            payload: searchkernel_ir::RequestTensorPayloadIr::Dense(
                                value.values().to_vec(),
                            ),
                        })
                    }
                },
            };
            Ok((name.clone(), searchkernel_ir::QueryInputIr::Literal(value)))
        })
        .collect::<Result<std::collections::BTreeMap<_, _>>>()?;
    let query = match &request.query_source {
        QuerySource::Text(text) => searchkernel_ir::QuerySourceInput::Text(text.clone()),
        QuerySource::MatchAll => searchkernel_ir::QuerySourceInput::MatchAll,
        QuerySource::Typed(query) => {
            searchkernel_ir::QuerySourceInput::Typed(query.clone().into_ir()?)
        }
        QuerySource::Yql(source) => searchkernel_ir::QuerySourceInput::Yql {
            source: source.clone(),
            parameters: request
                .yql_parameters
                .iter()
                .map(|(name, value)| {
                    let value = match value {
                        QueryInputValue::Int64(value) => {
                            searchkernel_ir::RequestValueIr::Int64(*value)
                        }
                        QueryInputValue::Bool(value) => {
                            searchkernel_ir::RequestValueIr::Bool(*value)
                        }
                        QueryInputValue::UInt64(value) => {
                            searchkernel_ir::RequestValueIr::UInt64(*value)
                        }
                        QueryInputValue::Float64(value) if value.is_finite() => {
                            searchkernel_ir::RequestValueIr::Float64(*value)
                        }
                        QueryInputValue::Float64(_) => {
                            return Err(crate::error::Error::search(
                                crate::ErrorCode::InvalidArgument,
                                "YQL parameters must be finite",
                            ))
                        }
                        QueryInputValue::String(value) => {
                            searchkernel_ir::RequestValueIr::String(value.clone())
                        }
                        QueryInputValue::Json(value) => searchkernel_ir::request_value_from_json(
                            value,
                            format!("yql_parameters.{name}"),
                        )
                        .map_err(crate::error::Error::from_ir)?,
                        QueryInputValue::Tensor(value) => searchkernel_ir::RequestValueIr::Tensor(
                            searchkernel_ir::RequestTensorIr {
                                type_spec: value.type_spec().to_owned(),
                                payload: searchkernel_ir::RequestTensorPayloadIr::Dense(
                                    value.values().to_vec(),
                                ),
                            },
                        ),
                    };
                    Ok((name.clone(), value))
                })
                .collect::<Result<_>>()?,
        },
    };
    let query_model_type =
        request
            .query_model_type
            .as_ref()
            .map(|value| searchkernel_ir::model::QueryModelTypeIr {
                composite: value.composite.clone(),
                tokenization: value.tokenization.clone(),
                syntax: value.syntax.clone(),
                profile: value.profile.clone(),
                is_yql_default: value.yql_default,
            });
    let (limit, offset) = lower_pagination(request)?;
    let properties = request
        .properties
        .iter()
        .map(|(name, value)| {
            searchkernel_ir::request_value_from_json(value, format!("properties.{name}"))
                .map(|value| (name.clone(), value))
                .map_err(crate::error::Error::from_ir)
        })
        .collect::<Result<std::collections::BTreeMap<_, _>>>()?;
    Ok(searchkernel_ir::SearchInput {
        query,
        search_fields,
        filters,
        textual_filters,
        sorts,
        grouping,
        ranking,
        query_inputs,
        rank_properties: request.rank_properties.clone(),
        query_model: searchkernel_ir::QueryModelInput {
            language: request.language.clone(),
            query_string: request.query_string.clone(),
            default_index: request.default_index.clone(),
            locale: request.locale.clone(),
            sources: request.sources.clone(),
            restrict: request.restrict.clone(),
            search_path: request.search_path.clone(),
            search_group: request.search_group,
            query_type: query_model_type,
            ..Default::default()
        },
        projection: request.projection.clone(),
        highlighting: request.highlighting.clone(),
        presentation: searchkernel_ir::PresentationInput {
            summary: request.summary.clone(),
            format: request.format.clone(),
            bolding: request.bolding,
            timing: request.timing,
        },
        properties,
        query_profile: request.query_profile.clone(),
        search_chain: request.search_chain.clone(),
        timeout: request.timeout,
        limit,
        offset,
    })
}

fn lower_pagination(request: &SearchBuilder) -> Result<(Option<u32>, Option<u32>)> {
    if let Some((page, size)) = request.page {
        if request.page_repeated {
            return Err(crate::Error::search(
                crate::ErrorCode::InvalidArgument,
                "page may only be specified once",
            )
            .with_detail(
                "pagination_conflict",
                Some("page"),
                "page cannot be specified more than once",
            ));
        }
        if request.offset.is_some() || request.limit.is_some() {
            return Err(crate::Error::search(
                crate::ErrorCode::InvalidArgument,
                "page cannot be combined with offset or limit",
            )
            .with_detail(
                "pagination_conflict",
                Some("page"),
                "choose page or offset/limit",
            ));
        }
        if page < 1 {
            return Err(crate::Error::search(
                crate::ErrorCode::InvalidArgument,
                "page number must be at least one",
            )
            .with_detail(
                "invalid_page",
                Some("page"),
                "page number must be at least one",
            ));
        }
        if size < 0 {
            return Err(crate::Error::search(
                crate::ErrorCode::InvalidArgument,
                "page size must not be negative",
            )
            .with_detail(
                "invalid_page_size",
                Some("page.size"),
                "page size must not be negative",
            ));
        }
        let page_end = page.checked_mul(size).ok_or_else(|| {
            crate::Error::search(
                crate::ErrorCode::InvalidArgument,
                "page range exceeds the supported range",
            )
            .with_detail(
                "pagination_overflow",
                Some("page"),
                "page number multiplied by page size exceeds u32",
            )
        })?;
        if page_end > i64::from(u32::MAX) {
            return Err(crate::Error::search(
                crate::ErrorCode::InvalidArgument,
                "page range exceeds the supported range",
            )
            .with_detail(
                "pagination_overflow",
                Some("page"),
                "page number multiplied by page size exceeds u32",
            ));
        }
        let offset = page
            .checked_sub(1)
            .and_then(|value| value.checked_mul(size))
            .and_then(|value| u32::try_from(value).ok())
            .ok_or_else(|| {
                crate::Error::search(
                    crate::ErrorCode::InvalidArgument,
                    "page offset exceeds the supported range",
                )
                .with_detail(
                    "pagination_overflow",
                    Some("page"),
                    "computed offset exceeds u32",
                )
            })?;
        let limit = u32::try_from(size).map_err(|_| {
            crate::Error::search(
                crate::ErrorCode::InvalidArgument,
                "page size exceeds the supported range",
            )
            .with_detail(
                "invalid_page_size",
                Some("page.size"),
                "page size exceeds u32",
            )
        })?;
        return Ok((Some(limit), Some(offset)));
    }
    Ok((request.limit, request.offset))
}

impl std::fmt::Debug for SearchBuilder {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SearchBuilder")
            .field("query_source", &self.query_source)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field;
    use crate::index::IndexRuntime;
    use crate::schema::{Field, IndexDefinition};
    use crate::ErrorCode;
    use searchkernel_testkit::RecordingBackend;
    use std::sync::Arc;

    #[test]
    fn sort_builders_encode_direction_and_missing_placement() {
        let ascending = Sort::asc("price");
        assert_eq!(ascending.target, SortTarget::Field);
        assert_eq!(ascending.name.as_deref(), Some("price"));
        assert_eq!(ascending.direction, SortDirection::Asc);
        assert_eq!(ascending.missing, None);

        let descending = Sort::desc("score").missing_first();
        assert_eq!(descending.direction, SortDirection::Desc);
        assert_eq!(descending.missing, Some(MissingPlacement::First));
        assert_eq!(
            Sort::asc("brand").missing_last().missing,
            Some(MissingPlacement::Last)
        );
    }

    #[test]
    fn facet_builders_encode_limits_and_each_sort_variant() {
        let defaults = Facet::new("brand");
        assert_eq!(defaults.field, "brand");
        assert_eq!(defaults.name, None);
        assert_eq!(defaults.limit, None);
        assert_eq!(defaults.sort, None);
        assert_eq!(
            Facet::new("brand").name("brands").name.as_deref(),
            Some("brands")
        );

        for sort in [
            FacetSort::CountDesc,
            FacetSort::CountAsc,
            FacetSort::ValueAsc,
            FacetSort::ValueDesc,
        ] {
            let facet = Facet::new("brand").limit(0).sort(sort);
            assert_eq!(facet.limit, Some(0));
            assert_eq!(facet.sort, Some(sort));
        }
        assert_eq!(Facet::from("brand"), Facet::new("brand"));
    }

    #[test]
    fn named_facet_inspection_preserves_canonical_name_and_zero_limit() {
        let runtime = IndexRuntime::from_backend(
            IndexDefinition::new().field(Field::keyword("brand").facetable(true)),
            Arc::new(RecordingBackend::new()),
        )
        .expect("test index");

        let inspected = runtime
            .index()
            .browse()
            .facet(Facet::new("brand").name("brands").limit(0))
            .inspect()
            .expect("inspect named facet");

        assert_eq!(inspected.facets.len(), 1);
        assert_eq!(inspected.facets[0].field, "brand");
        assert_eq!(inspected.facets[0].name.as_deref(), Some("brands"));
        assert_eq!(inspected.facets[0].limit, Some(0));
        assert_eq!(inspected.facets[0].sort, Some(FacetSort::CountDesc));
    }

    #[test]
    fn rust_grouping_facade_uses_canonical_output_collision_validation() {
        let runtime = IndexRuntime::from_backend(
            IndexDefinition::new().field(Field::keyword("brand").facetable(true)),
            Arc::new(RecordingBackend::new()),
        )
        .expect("test index");

        let grouping = GroupingPlan::new().level(
            GroupingPlan::by("brand")
                .output(crate::GroupingAggregation::count())
                .output(crate::GroupingAggregation::count()),
        );
        let error = runtime
            .index()
            .browse()
            .grouping(grouping)
            .execute::<serde_json::Value>()
            .expect_err("duplicate output names must fail before backend execution");

        assert_eq!(error.code(), ErrorCode::InvalidArgument);
        assert_eq!(error.details()[0].code, "grouping_output_name_collision");
    }

    #[test]
    fn search_builder_owns_a_clone_of_the_index() {
        let runtime = IndexRuntime::from_backend(
            IndexDefinition::new().field(Field::text("title")),
            Arc::new(RecordingBackend::new()),
        )
        .expect("test index");
        let request = {
            let index = runtime.index();
            index.search("headphones")
        };

        let inspected = request.inspect().expect("inspect search");
        assert_eq!(inspected.query, "headphones");
    }

    #[test]
    fn search_builder_preserves_engine_timeout() {
        let runtime = IndexRuntime::from_backend(
            IndexDefinition::new().field(Field::text("title")),
            Arc::new(RecordingBackend::new()),
        )
        .expect("test index");
        let index = runtime.index();

        let inspected = index
            .search("headphones")
            .timeout(Duration::from_millis(200))
            .inspect()
            .expect("inspect search");

        assert_eq!(inspected.timeout, Some(Duration::from_millis(200)));
    }

    #[test]
    fn public_typed_queries_use_schema_validation_before_backend_execution() {
        let runtime = IndexRuntime::from_backend(
            IndexDefinition::new().field(Field::text("title")),
            Arc::new(RecordingBackend::new()),
        )
        .expect("test index");

        let error = runtime
            .index()
            .browse()
            .typed_query(Query::term("missing", "value"))
            .execute::<serde_json::Value>()
            .expect_err("unknown typed-query field should fail validation");
        assert_eq!(error.code(), ErrorCode::InvalidArgument);
        assert_eq!(error.details()[0].code, "unknown_field");
        assert_eq!(
            error.details()[0].path.as_deref(),
            Some("query.contains.field.path")
        );
    }

    #[test]
    fn public_advanced_typed_queries_use_schema_validation_before_backend_execution() {
        let runtime = IndexRuntime::from_backend(
            IndexDefinition::new()
                .field(Field::text("title"))
                .field(Field::int64("price").filterable(true)),
            Arc::new(RecordingBackend::new()),
        )
        .expect("test index");

        let wrong_weight_key = runtime
            .index()
            .browse()
            .typed_query(Query::weighted_set(
                "price",
                [crate::WeightedQueryItem::new("not-an-integer", 10)],
            ))
            .execute::<serde_json::Value>()
            .expect_err("weighted query key type should be validated before execution");
        assert_eq!(wrong_weight_key.details()[0].code, "type_mismatch");

        let empty_weighted_query = runtime
            .index()
            .browse()
            .typed_query(Query::dot_product(
                "price",
                Vec::<crate::WeightedQueryItem>::new(),
            ))
            .execute::<serde_json::Value>()
            .expect_err("empty weighted query should be validated before execution");
        assert_eq!(
            empty_weighted_query.details()[0].code,
            "empty_weighted_query"
        );
    }

    #[test]
    fn public_nearest_neighbor_options_reach_the_ir_annotations() {
        let runtime = IndexRuntime::from_backend(
            IndexDefinition::new().field(Field::text("title")),
            Arc::new(RecordingBackend::new()),
        )
        .expect("test index");
        let query = Query::NearestNeighbor {
            field: "embedding".to_owned(),
            query_tensor: "query_embedding".to_owned(),
            options: crate::NearestNeighborOptions {
                target_hits: 100,
                approximate: true,
                distance_threshold: Some(0.25),
                explore_additional_hits: 50,
            },
        };
        let request = runtime.index().search("").typed_query(query);
        let input = to_resolver_input(&request).expect("public query should resolve to input");
        let searchkernel_ir::QuerySourceInput::Typed(expression) = input.query else {
            panic!("expected typed query input");
        };
        assert_eq!(
            expression.annotations.get("distanceThreshold"),
            Some(&searchkernel_ir::RequestValueIr::Float64(0.25))
        );
        assert_eq!(
            expression.annotations.get("exploreAdditionalHits"),
            Some(&searchkernel_ir::RequestValueIr::UInt64(50))
        );
    }

    #[test]
    fn summary_and_projection_enter_the_canonical_presentation_input() {
        let runtime = IndexRuntime::from_backend(
            IndexDefinition::new().field(Field::text("title")),
            Arc::new(RecordingBackend::new()),
        )
        .expect("test index");
        let request = runtime
            .index()
            .search("foo")
            .summary("default")
            .projection(["title"])
            .highlight(["title"]);

        let input = to_resolver_input(&request).expect("public query should resolve to input");
        assert_eq!(input.presentation.summary.as_deref(), Some("default"));
        assert_eq!(input.projection, Some(vec!["title".to_owned()]));
        assert_eq!(input.highlighting, Some(vec!["title".to_owned()]));
    }

    #[test]
    fn textual_filter_without_bindings_is_routed_to_textual_input() {
        let runtime = IndexRuntime::from_backend(
            IndexDefinition::new().field(Field::bool("in_stock").filterable(true)),
            Arc::new(RecordingBackend::new()),
        )
        .expect("test index");

        let request = runtime.index().search("").filter("in_stock = true");
        let input = to_resolver_input(&request).expect("textual filter should become input");

        assert!(input.filters.is_empty());
        assert_eq!(input.textual_filters.len(), 1);
        assert_eq!(input.textual_filters[0].source, "in_stock = true");
        assert!(input.textual_filters[0].bindings.is_empty());
    }

    #[test]
    fn textual_filter_bindings_preserve_scalar_and_list_types() {
        let runtime = IndexRuntime::from_backend(
            IndexDefinition::new()
                .field(Field::float64("price").filterable(true))
                .field(Field::keyword("brand").filterable(true)),
            Arc::new(RecordingBackend::new()),
        )
        .expect("test index");

        let request = runtime
            .index()
            .browse()
            .filter(("price <= :max", crate::params! { "max" => 120.0_f64 }))
            .filter((
                "brand IN (:brands)",
                crate::params! { "brands" => vec!["Acme", "TrailCo"] },
            ));
        let input = to_resolver_input(&request).expect("textual bindings should become input");

        assert_eq!(input.filters.len(), 0);
        assert_eq!(input.textual_filters.len(), 2);
        assert_eq!(
            input.textual_filters[0].bindings.get("max"),
            Some(&searchkernel_ir::Value::Float64(120.0))
        );
        assert_eq!(
            input.textual_filters[1].bindings.get("brands"),
            Some(&searchkernel_ir::Value::List(vec![
                searchkernel_ir::Value::String("Acme".to_owned()),
                searchkernel_ir::Value::String("TrailCo".to_owned()),
            ]))
        );
    }

    #[test]
    fn structured_and_textual_filters_keep_separate_input_lanes_and_resolve_together() {
        let runtime = IndexRuntime::from_backend(
            IndexDefinition::new()
                .field(Field::float64("price").filterable(true))
                .field(Field::bool("in_stock").filterable(true)),
            Arc::new(RecordingBackend::new()),
        )
        .expect("test index");

        let request = runtime
            .index()
            .browse()
            .filter(field("price").lte(120.0_f64))
            .filter((
                "in_stock = :available",
                crate::params! { "available" => true },
            ));
        let input = to_resolver_input(&request).expect("mixed filter intent should route");

        assert_eq!(input.filters.len(), 1);
        assert_eq!(input.textual_filters.len(), 1);
        let inspected = request
            .inspect()
            .expect("structured and textual filters should resolve with AND semantics");
        assert!(inspected.filter.is_some());
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Coverage {
        Executable,
        /// The facade represents the full method, but only a documented subset
        /// reaches the embedded native execution boundary.
        PartiallyExecutable(&'static str),
        RepresentedUnsupported,
        BackendDeferred,
    }

    const SEARCH_BUILDER_COVERAGE: &[(&str, Coverage)] = &[
        ("filter", Coverage::Executable),
        ("search_field", Coverage::Executable),
        ("search_fields", Coverage::Executable),
        ("sort", Coverage::Executable),
        ("sort_relevance", Coverage::Executable),
        ("sort_feature", Coverage::Executable),
        ("raw_sort", Coverage::BackendDeferred),
        ("facet", Coverage::Executable),
        ("facets", Coverage::Executable),
        ("aggregate", Coverage::Executable),
        ("group", Coverage::Executable),
        ("raw_grouping", Coverage::BackendDeferred),
        ("ranking", Coverage::Executable),
        ("rank_property", Coverage::Executable),
        ("input", Coverage::PartiallyExecutable("finite numeric scalars and dense tensors execute; bool, string, JSON, and expression inputs are represented but rejected by the native adapter")),
        ("input_expr", Coverage::RepresentedUnsupported),
        ("fields", Coverage::Executable),
        ("summary", Coverage::Executable),
        ("highlight", Coverage::Executable),
        ("format", Coverage::RepresentedUnsupported),
        ("bolding", Coverage::RepresentedUnsupported),
        ("timing", Coverage::RepresentedUnsupported),
        ("offset", Coverage::Executable),
        ("limit", Coverage::Executable),
        ("page", Coverage::Executable),
        ("timeout", Coverage::Executable),
        ("language", Coverage::Executable),
        ("locale", Coverage::RepresentedUnsupported),
        ("default_index", Coverage::Executable),
        ("sources", Coverage::RepresentedUnsupported),
        ("restrict", Coverage::RepresentedUnsupported),
        ("search_path", Coverage::RepresentedUnsupported),
        ("search_group", Coverage::RepresentedUnsupported),
        ("query_string", Coverage::Executable),
        ("query_model_type", Coverage::RepresentedUnsupported),
        ("property", Coverage::PartiallyExecutable("generic properties execute for YQL requests; non-YQL properties are rejected by the native adapter")),
        ("properties", Coverage::PartiallyExecutable("generic properties execute for YQL requests; non-YQL properties are rejected by the native adapter")),
        ("query_profile", Coverage::RepresentedUnsupported),
        ("search_chain", Coverage::RepresentedUnsupported),
        ("execute", Coverage::Executable),
        ("inspect", Coverage::Executable),
    ];

    const INDEX_ENTRYPOINT_COVERAGE: &[(&str, Coverage)] = &[
        ("search", Coverage::Executable),
        ("browse", Coverage::Executable),
        ("query", Coverage::Executable),
        ("yql", Coverage::Executable),
    ];

    #[test]
    fn checked_in_search_builder_coverage_map_matches_the_normative_union() {
        let spec = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../docs/client-api-spec.md"
        ));
        let section = spec
            .split_once("## 15. Conceptual surface")
            .expect("client API spec must contain SearchBuilder conceptual surface")
            .1;
        let block = section
            .split_once("```text")
            .expect("conceptual surface must contain a text code block")
            .1
            .split_once("```")
            .expect("conceptual surface code block must be closed")
            .0;

        let expected = block
            .lines()
            .filter_map(|line| {
                let line = line.trim();
                let method = line.strip_prefix('.')?.split_once('(')?.0;
                Some(method.to_owned())
            })
            .collect::<std::collections::BTreeSet<_>>();
        let covered = SEARCH_BUILDER_COVERAGE
            .iter()
            .map(|(concept, _)| (*concept).to_owned())
            .collect::<std::collections::BTreeSet<_>>();

        assert_eq!(
            SEARCH_BUILDER_COVERAGE.len(),
            covered.len(),
            "SearchBuilder coverage rows must be unique"
        );
        assert_eq!(
            expected, covered,
            "docs/client-api-spec.md SearchBuilder surface drifted without a Rust coverage disposition"
        );

        let entrypoints = INDEX_ENTRYPOINT_COVERAGE
            .iter()
            .map(|(concept, _)| *concept)
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(INDEX_ENTRYPOINT_COVERAGE.len(), entrypoints.len());
        assert_eq!(
            entrypoints,
            ["browse", "query", "search", "yql"].into_iter().collect()
        );
    }

    #[test]
    fn coverage_dispositions_match_adapter_execution_boundaries() {
        let partial = SEARCH_BUILDER_COVERAGE
            .iter()
            .filter_map(|(concept, coverage)| match coverage {
                Coverage::PartiallyExecutable(reason) => {
                    assert!(
                        !reason.trim().is_empty(),
                        "{concept} must explain its partial execution boundary"
                    );
                    Some(*concept)
                }
                _ => None,
            })
            .collect::<std::collections::BTreeSet<_>>();

        assert_eq!(
            partial,
            ["input", "properties", "property"].into_iter().collect(),
            "mixed execution-boundary methods must not be advertised as universally executable"
        );
    }

    #[test]
    fn ergonomic_surface_lowers_to_existing_canonical_input_variants() {
        let runtime = IndexRuntime::from_backend(
            IndexDefinition::new()
                .field(Field::text("title"))
                .field(Field::float64("price").sortable(true)),
            Arc::new(RecordingBackend::new()),
        )
        .expect("test index");
        let index = runtime.index();
        let request = index
            .search("shoes")
            .search_field(crate::schema::SearchField::new("title").weight(2.0))
            .search_field(crate::schema::SearchField::new("title"))
            .sort(Sort::asc("price"))
            .sort_relevance(SortDirection::Desc)
            .sort_feature("freshness", SortDirection::Asc)
            .facet(Facet::new("brand"))
            .facets(["category"])
            .aggregate(crate::GroupingAggregation::quantiles("price", [0.5, 0.9]))
            .group(GroupingPlan::new().level(GroupingPlan::by("brand")))
            .ranking("product-search")
            .rank_property("freshness_weight", "0.25")
            .input("literal", "embed(transformer,@query)")
            .input("json", serde_json::json!({"shape": [1, 2]}))
            .input_expr("expression", "embed(transformer,@query)")
            .fields(["title"])
            .summary("search-result")
            .highlight(["title"])
            .locale("en-US")
            .default_index("title")
            .sources(["products"])
            .restrict(["products"])
            .search_path("/search")
            .search_group(2)
            .query_string("running shoes")
            .query_model_type(
                QueryModelType::default()
                    .composite("and")
                    .tokenization("linguistic")
                    .syntax("all")
                    .profile("default")
                    .yql_default(true),
            )
            .property("q", "running shoes")
            .properties([("trace.level", 2)])
            .query_profile("profile")
            .search_chain("chain")
            .format("default")
            .bolding(true)
            .timing(true)
            .page(2, 20);
        let input = to_resolver_input(&request).expect("facade lowers to resolver input");

        assert!(matches!(
            input.sorts[0].target,
            searchkernel_ir::SortTargetInput::Field(_)
        ));
        assert!(matches!(
            input.sorts[1].target,
            searchkernel_ir::SortTargetInput::Relevance
        ));
        assert!(matches!(
            input.sorts[2].target,
            searchkernel_ir::SortTargetInput::RankFeature(ref name) if name == "freshness"
        ));
        assert_eq!(input.search_fields.as_ref().expect("fields").len(), 2);
        assert!(matches!(
            input.query_inputs["literal"],
            searchkernel_ir::QueryInputIr::Literal(searchkernel_ir::RequestValueIr::String(ref value)) if value == "embed(transformer,@query)"
        ));
        assert!(matches!(
            input.query_inputs["expression"],
            searchkernel_ir::QueryInputIr::Expression(ref value) if value == "embed(transformer,@query)"
        ));
        assert!(matches!(
            input.query_inputs["json"],
            searchkernel_ir::QueryInputIr::Literal(searchkernel_ir::RequestValueIr::Object(_))
        ));
        assert_eq!(input.limit, Some(20));
        assert_eq!(input.offset, Some(20));
        assert_eq!(input.query_model.locale.as_deref(), Some("en-US"));
        assert_eq!(input.query_model.default_index.as_deref(), Some("title"));
        assert_eq!(
            input.query_model.query_string.as_deref(),
            Some("running shoes")
        );
        assert_eq!(input.query_model.sources, ["products"]);
        assert_eq!(input.query_model.restrict, ["products"]);
        assert_eq!(
            input.properties["q"],
            searchkernel_ir::RequestValueIr::String("running shoes".into())
        );
    }

    #[test]
    fn page_validation_and_mode_conflicts_are_checked_before_resolution() {
        let runtime = IndexRuntime::from_backend(
            IndexDefinition::new().field(Field::text("title")),
            Arc::new(RecordingBackend::new()),
        )
        .expect("test index");
        let index = runtime.index();

        let input = to_resolver_input(&index.browse().page(1, 0)).expect("zero page size is valid");
        assert_eq!(input.limit, Some(0));
        assert_eq!(input.offset, Some(0));
        let input = to_resolver_input(&index.browse().page(3, 20)).expect("valid page");
        assert_eq!(input.offset, Some(40));

        for request in [
            index.browse().page(2, 20).offset(20),
            index.browse().page(2, 20).limit(20),
            index.browse().offset(20).page(2, 20),
            index.browse().limit(20).page(2, 20),
            index.browse().page(1, 20).page(2, 20),
        ] {
            let error = to_resolver_input(&request).expect_err("mixed pagination must fail");
            assert_eq!(error.code(), crate::ErrorCode::InvalidArgument);
            assert_eq!(error.details()[0].code, "pagination_conflict");
        }

        let max = i64::from(u32::MAX);
        let input = to_resolver_input(&index.browse().page(1, max))
            .expect("the last portable page-end value is valid");
        assert_eq!(input.limit, Some(u32::MAX));
        assert_eq!(input.offset, Some(0));

        for (page, size) in [(0, 20), (1, -1), (i64::MAX, 2), (2, max)] {
            let error = to_resolver_input(&index.browse().page(page, size))
                .expect_err("invalid/overflowing page must fail");
            assert_eq!(error.code(), crate::ErrorCode::InvalidArgument);
        }
    }

    #[test]
    fn typed_and_yql_index_entrypoints_use_the_shared_search_input() {
        let runtime = IndexRuntime::from_backend(
            IndexDefinition::new().field(Field::text("title")),
            Arc::new(RecordingBackend::new()),
        )
        .expect("test index");
        let index = runtime.index();
        assert!(matches!(
            to_resolver_input(&index.query(Query::term("title", "boots")))
                .expect("typed query entry")
                .query,
            searchkernel_ir::QuerySourceInput::Typed(_)
        ));
        let yql = to_resolver_input(
            &index
                .yql("select * from sources * where title contains text(@q)")
                .property("q", "boots"),
        )
        .expect("YQL entry");
        assert!(matches!(
            yql.query,
            searchkernel_ir::QuerySourceInput::Yql { .. }
        ));
        assert_eq!(
            yql.properties["q"],
            searchkernel_ir::RequestValueIr::String("boots".into())
        );
    }

    #[test]
    fn aggregate_preserves_explicit_output_labels() {
        let runtime = IndexRuntime::from_backend(
            IndexDefinition::new().field(Field::float64("price").facetable(true)),
            Arc::new(RecordingBackend::new()),
        )
        .expect("test index");
        let input = to_resolver_input(
            &runtime
                .index()
                .browse()
                .aggregate(crate::GroupingAggregation::avg("price").as_("avg_price")),
        )
        .expect("aggregate intent");
        let searchkernel_ir::GroupingInput::Structured(grouping) = &input.grouping[0] else {
            panic!("root aggregate must lower to structured grouping");
        };
        let searchkernel_ir::GroupingOperationIr::Output(outputs) = &grouping.root.operations[0]
        else {
            panic!("root aggregate must lower to output operation");
        };
        assert_eq!(outputs[0].label.as_deref(), Some("avg_price"));
    }

    #[test]
    fn aggregate_without_label_matches_other_first_party_facades() {
        let runtime = IndexRuntime::from_backend(
            IndexDefinition::new().field(Field::float64("price").facetable(true)),
            Arc::new(RecordingBackend::new()),
        )
        .expect("test index");
        let input = to_resolver_input(
            &runtime
                .index()
                .browse()
                .aggregate(crate::GroupingAggregation::avg("price")),
        )
        .expect("aggregate intent");
        let searchkernel_ir::GroupingInput::Structured(grouping) = &input.grouping[0] else {
            panic!("root aggregate must lower to structured grouping");
        };
        let searchkernel_ir::GroupingOperationIr::Output(outputs) = &grouping.root.operations[0]
        else {
            panic!("root aggregate must lower to output operation");
        };
        // Python and TypeScript use an empty ergonomic placeholder but encode it
        // as an absent wire label. Canonical IR therefore keeps unlabeled output absent.
        assert_eq!(outputs[0].label, None);
    }

    #[test]
    fn inspect_rejects_unrepresentable_intent_instead_of_dropping_it() {
        let runtime = IndexRuntime::from_backend(
            IndexDefinition::new().field(Field::text("title")),
            Arc::new(RecordingBackend::new()),
        )
        .expect("test index");
        let index = runtime.index();

        for request in [
            index.browse().input("boost", 1.0),
            index.browse().summary("compact"),
            index.browse().locale("en-US"),
            index.browse().property("trace.level", 1),
            index.browse().query_profile("profile"),
        ] {
            let error = request
                .inspect()
                .expect_err("inspection must not silently erase request intent");
            assert_eq!(error.code(), crate::ErrorCode::Unsupported);
            assert_eq!(error.details()[0].code, "inspect_not_representable");
        }
    }

    #[test]
    fn inspect_keeps_canonical_validation_errors_ahead_of_fidelity_errors() {
        let runtime = IndexRuntime::from_backend(
            IndexDefinition::new().field(Field::text("title")),
            Arc::new(RecordingBackend::new()),
        )
        .expect("test index");
        let error = runtime
            .index()
            .browse()
            .limit(10)
            .property("hits", 5)
            .inspect()
            .expect_err("overlapping generic property must fail canonically");
        assert_eq!(error.code(), crate::ErrorCode::InvalidArgument);
        assert_eq!(error.details()[0].code, "property_overlaps_typed_field");
    }

    #[test]
    fn raw_grouping_reports_canonical_unsupported_error() {
        let runtime = IndexRuntime::from_backend(
            IndexDefinition::new().field(Field::keyword("brand").facetable(true)),
            Arc::new(RecordingBackend::new()),
        )
        .expect("test index");
        let error = runtime
            .index()
            .browse()
            .raw_grouping("all(group(brand))")
            .inspect()
            .expect_err("raw grouping is not normalized by the canonical resolver");
        assert_eq!(error.code(), crate::ErrorCode::Unsupported);
        assert_eq!(error.details()[0].code, "raw_grouping_not_supported");
    }
}
