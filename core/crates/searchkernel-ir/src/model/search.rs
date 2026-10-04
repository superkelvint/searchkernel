//! Search and operation requests.

use super::*;
use std::time::Duration;
#[derive(Clone, Debug, PartialEq)]
pub struct SearchRequestIr {
    pub(crate) query: QueryIr,
    pub(crate) search_fields: Vec<SearchFieldIr>,
    pub(crate) filter: Option<FilterIr>,
    pub(crate) sort: Vec<SortIr>,
    /// the canonical aggregation mechanism.
    pub(crate) grouping: Vec<GroupingRequestIr>,
    pub(crate) ranking_request: RankingRequestIr,
    pub(crate) inputs: std::collections::BTreeMap<String, QueryInputIr>,
    pub(crate) limit: u32,
    pub(crate) offset: u32,
    pub(crate) projection: ProjectionIr,
    pub(crate) highlighting: HighlightingIr,
    pub(crate) language: Option<LanguageHint>,
    pub(crate) query_model: QueryModelIr,
    pub(crate) presentation: PresentationIr,
    pub(crate) properties: std::collections::BTreeMap<String, RequestValueIr>,
    pub(crate) query_profile: Option<String>,
    pub(crate) search_chain: Option<String>,
    pub(crate) timeout: Option<Duration>,
    pub(crate) prefix_policy: PrefixPolicy,
    pub(crate) typo_policy: TypoPolicy,
    pub(crate) diagnostic_source: bool,
}

impl SearchRequestIr {
    #[cfg(any(test, feature = "test"))]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        query: QueryIr,
        search_fields: Vec<SearchFieldIr>,
        filter: Option<FilterIr>,
        sort: Vec<SortIr>,
        limit: u32,
        offset: u32,
        projection: ProjectionIr,
        highlighting: HighlightingIr,
        language: Option<LanguageHint>,
        prefix_policy: PrefixPolicy,
        typo_policy: TypoPolicy,
    ) -> Self {
        Self {
            query,
            search_fields,
            filter,
            sort,
            grouping: Vec::new(),
            ranking_request: RankingRequestIr {
                profile: "default".to_owned(),
                properties: std::collections::BTreeMap::new(),
            },
            inputs: std::collections::BTreeMap::new(),
            limit,
            offset,
            projection,
            highlighting,
            language,
            query_model: QueryModelIr::default(),
            presentation: PresentationIr::default(),
            properties: std::collections::BTreeMap::new(),
            query_profile: None,
            search_chain: None,
            timeout: None,
            prefix_policy,
            typo_policy,
            diagnostic_source: false,
        }
    }

    pub fn query(&self) -> &QueryIr {
        &self.query
    }

    pub fn diagnostic_source(&self) -> bool {
        self.diagnostic_source
    }

    pub fn search_fields(&self) -> &[SearchFieldIr] {
        &self.search_fields
    }

    pub fn filter(&self) -> Option<&FilterIr> {
        self.filter.as_ref()
    }

    pub fn sort(&self) -> &[SortIr] {
        &self.sort
    }

    pub fn grouping(&self) -> &[GroupingRequestIr] {
        &self.grouping
    }

    pub fn ranking_request(&self) -> &RankingRequestIr {
        &self.ranking_request
    }

    pub fn inputs(&self) -> &std::collections::BTreeMap<String, QueryInputIr> {
        &self.inputs
    }

    pub fn limit(&self) -> u32 {
        self.limit
    }

    pub fn offset(&self) -> u32 {
        self.offset
    }

    pub fn projection(&self) -> &ProjectionIr {
        &self.projection
    }

    pub fn highlighting(&self) -> &HighlightingIr {
        &self.highlighting
    }

    pub fn language(&self) -> Option<&LanguageHint> {
        self.language.as_ref()
    }

    pub fn query_model(&self) -> &QueryModelIr {
        &self.query_model
    }

    pub fn presentation(&self) -> &PresentationIr {
        &self.presentation
    }

    #[cfg(any(test, feature = "test"))]
    pub fn with_presentation(mut self, presentation: PresentationIr) -> Self {
        self.presentation = presentation;
        self
    }

    pub fn properties(&self) -> &std::collections::BTreeMap<String, RequestValueIr> {
        &self.properties
    }

    pub fn query_profile(&self) -> Option<&str> {
        self.query_profile.as_deref()
    }

    pub fn search_chain(&self) -> Option<&str> {
        self.search_chain.as_deref()
    }

    pub fn timeout(&self) -> Option<Duration> {
        self.timeout
    }

    #[cfg(any(test, feature = "test"))]
    pub fn with_grouping(mut self, grouping: Vec<GroupingRequestIr>) -> Self {
        self.grouping = grouping;
        self
    }

    #[cfg(any(test, feature = "test"))]
    pub fn with_timeout(mut self, timeout: Option<Duration>) -> Self {
        self.timeout = timeout;
        self
    }

    #[cfg(any(test, feature = "test"))]
    pub fn with_ranking_request(
        mut self,
        profile: impl Into<String>,
        properties: std::collections::BTreeMap<String, String>,
    ) -> Self {
        self.ranking_request = RankingRequestIr {
            profile: profile.into(),
            properties,
        };
        self
    }

    #[cfg(any(test, feature = "test"))]
    pub fn with_inputs(mut self, inputs: std::collections::BTreeMap<String, QueryInputIr>) -> Self {
        self.inputs = inputs;
        self
    }

    #[cfg(any(test, feature = "test"))]
    pub fn with_query_model(mut self, query_model: QueryModelIr) -> Self {
        self.query_model = query_model;
        self
    }

    #[cfg(any(test, feature = "test"))]
    pub fn with_query(mut self, query: QueryIr) -> Self {
        self.query = query;
        self
    }

    #[cfg(any(test, feature = "test"))]
    pub fn with_properties(
        mut self,
        properties: std::collections::BTreeMap<String, RequestValueIr>,
    ) -> Self {
        self.properties = properties;
        self
    }

    #[cfg(any(test, feature = "test"))]
    pub fn with_query_profile(mut self, query_profile: impl Into<String>) -> Self {
        self.query_profile = Some(query_profile.into());
        self
    }

    #[cfg(any(test, feature = "test"))]
    pub fn with_search_chain(mut self, search_chain: impl Into<String>) -> Self {
        self.search_chain = Some(search_chain.into());
        self
    }

    /// Exhaustive semantic surface used by adapter tests.
    ///
    /// The destructuring pattern intentionally omits `..`: adding a field to
    /// `SearchRequestIr` must fail the test build until the adapter review
    /// matrix gives that field an explicit lowered/rejected disposition.
    #[cfg(any(test, feature = "test"))]
    pub fn semantic_field_names(&self) -> &'static [&'static str] {
        let Self {
            query: _,
            search_fields: _,
            filter: _,
            sort: _,
            grouping: _,
            ranking_request: _,
            inputs: _,
            limit: _,
            offset: _,
            projection: _,
            highlighting: _,
            language: _,
            query_model: _,
            presentation: _,
            properties: _,
            query_profile: _,
            search_chain: _,
            timeout: _,
            prefix_policy: _,
            typo_policy: _,
        } = self;
        &[
            "query",
            "search_fields",
            "filter",
            "sort",
            "grouping",
            "ranking_request",
            "inputs",
            "limit",
            "offset",
            "projection",
            "highlighting",
            "language",
            "query_model",
            "presentation",
            "properties",
            "query_profile",
            "search_chain",
            "timeout",
            "prefix_policy",
            "typo_policy",
        ]
    }

    pub fn prefix_policy(&self) -> PrefixPolicy {
        self.prefix_policy
    }

    pub fn typo_policy(&self) -> TypoPolicy {
        self.typo_policy
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnalyzeRequestIr {
    pub(crate) analyzer: AnalyzerIdentity,
    pub(crate) text: String,
    pub(crate) language: Option<LanguageHint>,
}

impl AnalyzeRequestIr {
    pub fn analyzer(&self) -> AnalyzerIdentity {
        self.analyzer
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn language(&self) -> Option<&LanguageHint> {
        self.language.as_ref()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum WriteRequestIr {
    Insert(InsertRequestIr),
    Upsert(UpsertRequestIr),
    Patch(PatchRequestIr),
    Update(UpdateRequestIr),
    Delete(DeleteRequestIr),
}

#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq)]
pub enum OperationIr {
    Insert(InsertRequestIr),
    Upsert(UpsertRequestIr),
    Patch(PatchRequestIr),
    Update(UpdateRequestIr),
    Delete(DeleteRequestIr),
    Get(GetRequestIr),
    Search(SearchRequestIr),
    Analyze(AnalyzeRequestIr),
}
