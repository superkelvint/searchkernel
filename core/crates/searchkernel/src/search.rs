use crate::error::Result;
use crate::filter::Filter;
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

#[derive(Clone, Debug, PartialEq, Eq)]
/// A field and direction used to order search hits.
pub struct Sort {
    pub(crate) field: String,
    pub(crate) direction: SortDirection,
    pub(crate) missing: Option<MissingPlacement>,
}

impl Sort {
    /// Sort by `field` from the smallest value to the largest value.
    pub fn asc(field: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            direction: SortDirection::Asc,
            missing: None,
        }
    }

    /// Sort by `field` from the largest value to the smallest value.
    pub fn desc(field: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            direction: SortDirection::Desc,
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
    pub(crate) limit: Option<u32>,
    pub(crate) sort: Option<FacetSort>,
}

impl Facet {
    /// Create a facet request for `field`.
    pub fn new(field: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            limit: None,
            sort: None,
        }
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

#[derive(Clone, Debug, PartialEq)]
/// A scalar value supplied to a native Vespa query feature.
pub enum QueryInputValue {
    Int64(i64),
    UInt64(u64),
    Float64(f64),
    Tensor(crate::Tensor),
}

impl From<i64> for QueryInputValue {
    fn from(value: i64) -> Self {
        Self::Int64(value)
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
    pub(crate) query: String,
    pub(crate) search_fields: Option<Vec<crate::schema::SearchField>>,
    pub(crate) filters: Vec<Filter>,
    pub(crate) sorts: Vec<Sort>,
    pub(crate) facets: Vec<Facet>,
    pub(crate) ranking: Option<Ranking>,
    pub(crate) query_inputs: std::collections::BTreeMap<String, QueryInputValue>,
    pub(crate) rank_properties: std::collections::BTreeMap<String, String>,
    pub(crate) typed_query: Option<Query>,
    pub(crate) groupings: Vec<GroupingPlan>,
    pub(crate) projection: Option<Vec<String>>,
    pub(crate) highlighting: Option<Vec<String>>,
    pub(crate) language: Option<String>,
    pub(crate) timeout: Option<Duration>,
    pub(crate) limit: Option<u32>,
    pub(crate) offset: Option<u32>,
}

impl SearchBuilder {
    pub(crate) fn new(index: Index, query: impl Into<String>) -> Self {
        Self {
            index,
            query: query.into(),
            search_fields: None,
            filters: Vec::new(),
            sorts: Vec::new(),
            facets: Vec::new(),
            ranking: None,
            query_inputs: std::collections::BTreeMap::new(),
            rank_properties: std::collections::BTreeMap::new(),
            typed_query: None,
            groupings: Vec::new(),
            projection: None,
            highlighting: None,
            language: None,
            timeout: None,
            limit: None,
            offset: None,
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

    /// Add one filter to the search request.
    pub fn filter(mut self, filter: Filter) -> Self {
        self.filters.push(filter);
        self
    }

    /// Add one sort order to the search request.
    pub fn sort(mut self, sort: Sort) -> Self {
        self.sorts.push(sort);
        self
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
    pub fn ranking(mut self, ranking: Ranking) -> Self {
        self.ranking = Some(ranking);
        self
    }

    /// Supply a finite scalar query feature to the selected rank profile.
    pub fn query_input(
        mut self,
        name: impl Into<String>,
        value: impl Into<QueryInputValue>,
    ) -> Self {
        self.query_inputs.insert(name.into(), value.into());
        self
    }

    /// Supply a request rank property to the selected rank profile.
    pub fn rank_property(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.rank_properties.insert(name.into(), value.into());
        self
    }

    /// Execute a typed query through the canonical IR query path.
    pub fn typed_query(mut self, query: Query) -> Self {
        self.typed_query = Some(query);
        self
    }

    /// Add a general grouping plan. Facets remain available as a compatibility
    /// convenience, while plans are returned as recursive grouping results.
    pub fn grouping(mut self, grouping: GroupingPlan) -> Self {
        self.groupings.push(grouping);
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
    let sorts = request
        .sorts
        .iter()
        .map(|sort| searchkernel_ir::SortInput {
            target: searchkernel_ir::SortTargetInput::Field(sort.field.clone()),
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
        .map(|facet| searchkernel_ir::FacetInput {
            field: facet.field.clone(),
            limit: facet.limit,
            sort: facet.sort.map(|sort| match sort {
                FacetSort::CountDesc => searchkernel_ir::FacetSortInput::CountDesc,
                FacetSort::CountAsc => searchkernel_ir::FacetSortInput::CountAsc,
                FacetSort::ValueAsc => searchkernel_ir::FacetSortInput::ValueAsc,
                FacetSort::ValueDesc => searchkernel_ir::FacetSortInput::ValueDesc,
            }),
        })
        .map(searchkernel_ir::GroupingInput::Facet)
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
                QueryInputValue::Int64(value) => searchkernel_ir::RequestValueIr::Int64(*value),
                QueryInputValue::UInt64(value) => searchkernel_ir::RequestValueIr::UInt64(*value),
                QueryInputValue::Float64(value) => searchkernel_ir::RequestValueIr::Float64(*value),
                QueryInputValue::Tensor(value) => {
                    searchkernel_ir::RequestValueIr::Tensor(searchkernel_ir::RequestTensorIr {
                        type_spec: value.type_spec().to_owned(),
                        payload: searchkernel_ir::RequestTensorPayloadIr::Dense(
                            value.values().to_vec(),
                        ),
                    })
                }
            };
            (name.clone(), searchkernel_ir::QueryInputIr::Literal(value))
        })
        .collect();
    let query = request
        .typed_query
        .clone()
        .map(Query::into_ir)
        .transpose()?
        .map(searchkernel_ir::QuerySourceInput::Typed)
        .unwrap_or_else(|| searchkernel_ir::QuerySourceInput::Text(request.query.clone()));
    Ok(searchkernel_ir::SearchInput {
        query,
        search_fields,
        filters,
        textual_filters: Vec::new(),
        sorts,
        grouping,
        ranking,
        query_inputs,
        rank_properties: request.rank_properties.clone(),
        query_model: searchkernel_ir::QueryModelInput {
            language: request.language.clone(),
            ..Default::default()
        },
        projection: request.projection.clone(),
        highlighting: request.highlighting.clone(),
        presentation: searchkernel_ir::PresentationInput::default(),
        properties: std::collections::BTreeMap::new(),
        query_profile: None,
        search_chain: None,
        timeout: request.timeout,
        limit: request.limit,
        offset: request.offset,
    })
}

impl std::fmt::Debug for SearchBuilder {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SearchBuilder")
            .field("query", &self.query)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::IndexRuntime;
    use crate::schema::{Field, IndexDefinition};
    use crate::ErrorCode;
    use searchkernel_testkit::RecordingBackend;
    use std::sync::Arc;

    #[test]
    fn sort_builders_encode_direction_and_missing_placement() {
        let ascending = Sort::asc("price");
        assert_eq!(ascending.field, "price");
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
        assert_eq!(defaults.limit, None);
        assert_eq!(defaults.sort, None);

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
            .search("")
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
            .search("")
            .typed_query(Query::weighted_set(
                "price",
                [crate::WeightedQueryItem::new("not-an-integer", 10)],
            ))
            .execute::<serde_json::Value>()
            .expect_err("weighted query key type should be validated before execution");
        assert_eq!(wrong_weight_key.details()[0].code, "type_mismatch");

        let empty_weighted_query = runtime
            .index()
            .search("")
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
}
