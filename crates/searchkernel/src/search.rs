use crate::filter::Filter;
use crate::index::Index;
use crate::result::SearchResult;
use serde::de::DeserializeOwned;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortDirection {
    Asc,
    Desc,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MissingPlacement {
    First,
    Last,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sort {
    pub(crate) field: String,
    pub(crate) direction: SortDirection,
    pub(crate) missing: Option<MissingPlacement>,
}

impl Sort {
    pub fn asc(field: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            direction: SortDirection::Asc,
            missing: None,
        }
    }

    pub fn desc(field: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            direction: SortDirection::Desc,
            missing: None,
        }
    }

    pub fn missing_first(mut self) -> Self {
        self.missing = Some(MissingPlacement::First);
        self
    }

    pub fn missing_last(mut self) -> Self {
        self.missing = Some(MissingPlacement::Last);
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FacetSort {
    CountDesc,
    CountAsc,
    ValueAsc,
    ValueDesc,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Facet {
    pub(crate) field: String,
    pub(crate) limit: Option<u32>,
    pub(crate) sort: Option<FacetSort>,
}

impl Facet {
    pub fn new(field: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            limit: None,
            sort: None,
        }
    }

    pub fn limit(mut self, value: u32) -> Self {
        self.limit = Some(value);
        self
    }

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ranking {
    DefaultRelevanceV1,
    Bm25,
}

pub struct SearchBuilder<'a> {
    pub(crate) index: &'a Index,
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

impl<'a> SearchBuilder<'a> {
    pub(crate) fn new(index: &'a Index, query: impl Into<String>) -> Self {
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

    pub fn search_fields<I>(mut self, fields: I) -> Self
    where
        I: IntoIterator<Item = crate::schema::SearchField>,
    {
        self.search_fields = Some(fields.into_iter().collect());
        self
    }

    pub fn filter(mut self, filter: Filter) -> Self {
        self.filters.push(filter);
        self
    }

    pub fn sort(mut self, sort: Sort) -> Self {
        self.sorts.push(sort);
        self
    }

    pub fn facet(mut self, facet: Facet) -> Self {
        self.facets.push(facet);
        self
    }

    pub fn facets<I>(mut self, facets: I) -> Self
    where
        I: IntoIterator<Item = &'static str>,
    {
        self.facets.extend(facets.into_iter().map(Facet::from));
        self
    }

    pub fn ranking(mut self, ranking: Ranking) -> Self {
        self.ranking = Some(ranking);
        self
    }

    pub fn projection<I>(mut self, fields: I) -> Self
    where
        I: IntoIterator<Item = &'static str>,
    {
        self.projection = Some(fields.into_iter().map(str::to_owned).collect());
        self
    }

    pub fn highlight<I>(mut self, fields: I) -> Self
    where
        I: IntoIterator<Item = &'static str>,
    {
        self.highlighting = Some(fields.into_iter().map(str::to_owned).collect());
        self
    }

    pub fn language(mut self, language: impl Into<String>) -> Self {
        self.language = Some(language.into());
        self
    }

    pub fn limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    pub fn offset(mut self, offset: u32) -> Self {
        self.offset = Some(offset);
        self
    }

    pub fn execute<T: DeserializeOwned>(self) -> crate::Result<SearchResult<T>> {
        self.index.execute_search(&self)
    }
}

impl std::fmt::Debug for SearchBuilder<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SearchBuilder")
            .field("query", &self.query)
            .finish_non_exhaustive()
    }
}
