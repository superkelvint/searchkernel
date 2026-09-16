use crate::filter::Filter;
use crate::index::Index;
use crate::result::SearchResult;
use serde::de::DeserializeOwned;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RankingKind {
    DefaultRelevanceV1,
    Bm25,
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
}

/// The normalized meaning of a search request after schema defaults are
/// applied.
#[derive(Clone, Debug, PartialEq)]
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
    pub(crate) projection: Option<Vec<String>>,
    pub(crate) highlighting: Option<Vec<String>>,
    pub(crate) language: Option<String>,
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
            projection: None,
            highlighting: None,
            language: None,
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
    use crate::index::Index;
    use crate::schema::{Field, IndexDefinition};
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
        let request = {
            let index = Index::from_backend(
                IndexDefinition::new().field(Field::text("title")),
                Arc::new(RecordingBackend::new()),
            )
            .expect("test index");
            index.search("headphones")
        };

        let inspected = request.inspect().expect("inspect search");
        assert_eq!(inspected.query, "headphones");
    }
}
