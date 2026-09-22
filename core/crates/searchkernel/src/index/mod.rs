use crate::analyze::{to_resolver_input, AnalyzeBuilder, AnalyzeResult, AnalyzedToken};
use crate::document::{serialize, DocumentId};
use crate::error::{Error, Result};
use crate::patch::{to_resolver_operations, Patch};
use crate::result;
use crate::schema::{
    Analyzer, ConstantDefinition, Field, FieldCapabilities, IndexDefinition, RankProfile,
    SearchField,
};
use crate::search::{
    to_resolver_input as search_to_resolver_input, Facet, FacetSort, Ranking, SearchBuilder, Sort,
};
#[cfg(all(test, feature = "native"))]
use searchkernel_ir::api;
use searchkernel_ir::{Backend, BackendSession, IndexContext};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::collections::BTreeMap;
use std::sync::Arc;

#[cfg(feature = "native")]
mod manifest;
#[cfg(feature = "native")]
mod native;

#[cfg(all(test, feature = "native"))]
pub(crate) use manifest::{
    config_identity, definition_from_json, IndexManifest, LEGACY_MANIFEST_FORMAT_VERSION,
    MANIFEST_FORMAT_VERSION, RESOLVED_SCHEMA_SNAPSHOT_VERSION, STORAGE_COMPATIBILITY_GENERATION,
};
#[cfg(all(test, feature = "native"))]
use native::{
    create_directory_transaction, create_staging_directory, publish_staging_directory,
    FAIL_NEXT_BACKEND_OPEN, FAIL_NEXT_PARENT_FSYNC, FAIL_NEXT_PUBLISHED_CLEANUP,
};

#[cfg(test)]
#[path = "../api_tests.rs"]
mod api_tests;
#[cfg(test)]
#[path = "model_state_machine_tests.rs"]
mod model_state_machine_tests;
#[cfg(all(test, feature = "native"))]
#[path = "../tests.rs"]
mod tests;

struct IndexInner {
    context: Arc<IndexContext>,
    definition: Arc<IndexDefinition>,
    session: Arc<BackendSession>,
    #[cfg(feature = "native")]
    lifecycle_lock: std::sync::Mutex<Option<native::LifecycleLock>>,
}

#[derive(Clone)]
pub struct Index {
    inner: Arc<IndexInner>,
}

/// Compatibility alias for the provisional runtime/handle split.
///
/// There is no separate lifecycle owner: `IndexRuntime` and `Index` now share
/// the same close state and all operations can use `Index::close` directly.
#[doc(hidden)]
pub type IndexRuntime = Index;

#[cfg(not(feature = "native"))]
impl Index {
    /// Construct an index around a backend for tests and infrastructure.
    #[doc(hidden)]
    pub fn from_backend(definition: IndexDefinition, backend: Arc<dyn Backend>) -> Result<Self> {
        let context = IndexContext::from_definition_ir(definition_to_ir(&definition)?);
        Self::from_owned_backend(context, BackendSession::from_backend(backend), None)
    }

    fn from_owned_backend(
        context: IndexContext,
        session: Arc<BackendSession>,
        _lifecycle_lock: Option<()>,
    ) -> Result<Self> {
        let effective_definition = definition_from_ir(context.definition());
        let context = Arc::new(context);
        let definition = Arc::new(effective_definition);
        Ok(Self {
            inner: Arc::new(IndexInner {
                session,
                context,
                definition,
            }),
        })
    }

    pub fn close(&self) -> Result<()> {
        self.inner.session.close().map_err(Error::from_ir)
    }
}

impl Index {
    /// Return a cloneable operation handle.
    pub fn index(&self) -> Index {
        self.clone()
    }

    /// Transfer the shared lifecycle state to the native handle registry.
    #[doc(hidden)]
    #[cfg(feature = "internal-api")]
    pub fn into_session_parts(self) -> (Arc<IndexContext>, Arc<BackendSession>) {
        (
            Arc::clone(&self.inner.context),
            Arc::clone(&self.inner.session),
        )
    }
}

impl Index {
    fn with_backend<T>(&self, callback: impl FnOnce(&dyn Backend) -> Result<T>) -> Result<T> {
        self.inner
            .session
            .execute(callback)
            .map_err(|_| Error::index_closed())?
    }

    /// Insert a new document.
    pub fn insert<T: Serialize>(&self, id: impl Into<DocumentId>, document: &T) -> Result<()> {
        self.with_backend(|backend| {
            let document_id = id.into();
            let document_value =
                serialize(document).map_err(|error| Error::Serialization(error.to_string()))?;
            let input = document_input(document_id.as_str(), &document_value)?;
            let ir_request = searchkernel_ir::resolve_insert(&self.inner.context, input)
                .map_err(Error::from_ir)?;
            backend.insert(ir_request).map_err(Error::from_ir)?;
            Ok(())
        })
    }

    /// Insert a document or replace the existing document with the same ID.
    pub fn upsert<T: Serialize>(&self, id: impl Into<DocumentId>, document: &T) -> Result<()> {
        self.with_backend(|backend| {
            let document_id = id.into();
            let document_value =
                serialize(document).map_err(|error| Error::Serialization(error.to_string()))?;
            let input = document_input(document_id.as_str(), &document_value)?;
            let ir_request = searchkernel_ir::resolve_upsert(&self.inner.context, input, None)
                .map_err(Error::from_ir)?;
            backend.upsert(ir_request).map_err(Error::from_ir)?;
            Ok(())
        })
    }

    /// Apply field updates to an existing document.
    pub fn patch(&self, id: impl Into<DocumentId>, patch: Patch) -> Result<()> {
        self.with_backend(|backend| {
            let document_id = id.into();
            let operations = to_resolver_operations(&patch)?;
            let ir_request = searchkernel_ir::resolve_patch(
                &self.inner.context,
                document_id.as_str(),
                &operations,
            )
            .map_err(Error::from_ir)?;
            backend.patch(ir_request).map_err(Error::from_ir)?;
            Ok(())
        })
    }

    /// Delete a document and return whether it existed.
    pub fn delete(&self, id: impl Into<DocumentId>) -> Result<bool> {
        self.with_backend(|backend| {
            let document_id = id.into();
            let ir_request = searchkernel_ir::resolve_delete(document_id.as_str(), None)
                .map_err(Error::from_ir)?;
            let delete_result = backend.delete(ir_request).map_err(Error::from_ir)?;
            Ok(delete_result.deleted)
        })
    }

    /// Read a document and deserialize it into `T`.
    pub fn get<T: DeserializeOwned>(&self, id: impl Into<DocumentId>) -> Result<Option<T>> {
        self.get_inner(id.into(), None)
    }

    /// Read a document while requesting only the selected fields.
    pub fn get_with_projection<T: DeserializeOwned>(
        &self,
        id: impl Into<DocumentId>,
        fields: impl IntoIterator<Item = impl AsRef<str>>,
    ) -> Result<Option<T>> {
        let fields = fields
            .into_iter()
            .map(|field| field.as_ref().to_owned())
            .collect();
        self.get_inner(id.into(), Some(fields))
    }

    fn get_inner<T: DeserializeOwned>(
        &self,
        document_id: DocumentId,
        projection: Option<Vec<String>>,
    ) -> Result<Option<T>> {
        self.with_backend(|backend| {
            let ir_request = searchkernel_ir::resolve_get(
                &self.inner.context,
                searchkernel_ir::GetInput {
                    id: document_id.to_string(),
                    projection,
                },
            )
            .map_err(Error::from_ir)?;
            let get_result = backend.get(ir_request).map_err(Error::from_ir)?;
            get_result
                .document
                .map(result::document)
                .transpose()
                .map(|value| value.map(|(_, document)| document))
        })
    }

    /// Start a search for `query`.
    pub fn search(&self, query: impl Into<String>) -> SearchBuilder {
        SearchBuilder::new(self.clone(), query)
    }

    /// Start a match-all search intended for browsing or administration.
    pub fn browse(&self) -> SearchBuilder {
        SearchBuilder::match_all(self.clone())
    }

    /// Return the effective schema and search defaults for this index.
    pub fn describe(&self) -> Result<IndexDefinition> {
        self.with_backend(|_| Ok((*self.inner.definition).clone()))
    }

    /// Start a linguistic analysis request.
    pub fn analyze(&self, text: impl Into<String>) -> AnalyzeBuilder {
        AnalyzeBuilder::new(self.clone(), text)
    }

    pub(crate) fn execute_analyze(&self, request: &AnalyzeBuilder) -> Result<AnalyzeResult> {
        self.with_backend(|backend| {
            let ir_request =
                searchkernel_ir::resolve_analyze(&self.inner.context, to_resolver_input(request)?)
                    .map_err(Error::from_ir)?;
            let result = backend.analyze(ir_request).map_err(Error::from_ir)?;
            Ok(AnalyzeResult {
                tokens: result
                    .tokens
                    .into_iter()
                    .map(|token| AnalyzedToken {
                        surface: token.surface,
                        term: token.term,
                        position: token.position,
                        start_byte: token.start_byte,
                        end_byte: token.end_byte,
                    })
                    .collect(),
            })
        })
    }

    pub(crate) fn inspect_search(
        &self,
        request: &SearchBuilder,
    ) -> Result<crate::search::InspectedQuery> {
        self.with_backend(|_| {
            let normalized = searchkernel_ir::resolve_search(
                &self.inner.context,
                search_to_resolver_input(request)?,
            )
            .map_err(Error::from_ir)?;
            let query = match normalized.query() {
                searchkernel_ir::QueryIr::MatchAll => String::new(),
                searchkernel_ir::QueryIr::Text(value) => value.clone(),
                searchkernel_ir::QueryIr::Yql { .. }
                | searchkernel_ir::QueryIr::Term { .. }
                | searchkernel_ir::QueryIr::And(_)
                | searchkernel_ir::QueryIr::Or(_)
                | searchkernel_ir::QueryIr::Not(_)
                | searchkernel_ir::QueryIr::Typed { .. } => {
                    return Err(Error::search(
                        crate::error::ErrorCode::Unsupported,
                        "inspect cannot represent this query source in the legacy query string",
                    ));
                }
            };
            let highlighting = match normalized.highlighting() {
                searchkernel_ir::HighlightingIr::Disabled => None,
                searchkernel_ir::HighlightingIr::Requested { fields, .. } => Some(
                    fields
                        .iter()
                        .map(|field| field.as_str().to_owned())
                        .collect(),
                ),
            };
            Ok(crate::search::InspectedQuery {
                query,
                search_fields: normalized
                    .search_fields()
                    .iter()
                    .map(|field| {
                        SearchField::new(field.field().as_str()).weight(field.weight().get())
                    })
                    .collect(),
                filter: normalized.filter().map(filter_from_ir).transpose()?,
                sorts: normalized
                    .sort()
                    .iter()
                    .map(|sort| {
                        if !sort.annotations().is_empty() {
                            return Err(Error::search(
                                crate::error::ErrorCode::Unsupported,
                                "annotated sort expressions are not supported by the public search API",
                            ));
                        }
                        let field = match sort.target() {
                            searchkernel_ir::SortTargetIr::Field(field) => field,
                            searchkernel_ir::SortTargetIr::Relevance
                            | searchkernel_ir::SortTargetIr::RankFeature(_) => {
                                return Err(Error::search(
                                    crate::error::ErrorCode::Unsupported,
                                    "the public search API supports only field sort targets",
                                ));
                            }
                        };
                        let value = match sort.direction() {
                            searchkernel_ir::SortDirection::Asc => Sort::asc(field.as_str()),
                            searchkernel_ir::SortDirection::Desc => Sort::desc(field.as_str()),
                        };
                        match sort.missing() {
                            searchkernel_ir::MissingPlacement::First => Ok(value.missing_first()),
                            searchkernel_ir::MissingPlacement::Last => Ok(value.missing_last()),
                            searchkernel_ir::MissingPlacement::As(_) => Err(Error::search(
                                crate::error::ErrorCode::Unsupported,
                                "missing-as sort policies are not supported by the public search API",
                            )),
                        }
                    })
                    .collect::<Result<Vec<_>>>()?,
                facets: normalized
                    .grouping()
                    .iter()
                    .filter_map(facet_from_grouping)
                    .collect(),
                ranking: match normalized.ranking_request().profile.as_str() {
                    "default" => Ranking::default_relevance_v1(),
                    "bm25" => Ranking::bm25(),
                    profile => Ranking::named(profile),
                },
                limit: normalized.limit(),
                offset: normalized.offset(),
                projection: normalized
                    .projection()
                    .fields()
                    .iter()
                    .map(|field| field.as_str().to_owned())
                    .collect(),
                highlighting,
                language: normalized
                    .language()
                    .map(|language| language.as_str().to_owned()),
                timeout: normalized.timeout(),
            })
        })
    }

    pub(crate) fn execute_search<T: DeserializeOwned>(
        &self,
        request: &SearchBuilder,
    ) -> Result<result::SearchResult<T>> {
        self.with_backend(|backend| {
            let ir_request = searchkernel_ir::resolve_search(
                &self.inner.context,
                search_to_resolver_input(request)?,
            )
            .map_err(Error::from_ir)?;
            let backend_result = backend.search(ir_request).map_err(Error::from_ir)?;
            let requested_facets = request
                .facets
                .iter()
                .map(|facet| {
                    (
                        facet.field.as_str(),
                        facet.name.as_deref().unwrap_or(facet.field.as_str()),
                    )
                })
                .collect::<Vec<_>>();
            result::search_result(backend_result, &requested_facets)
        })
    }
}

fn document_input(id: &str, value: &serde_json::Value) -> Result<searchkernel_ir::DocumentInput> {
    let serde_json::Value::Object(fields) = value else {
        return Err(Error::Serialization(
            "document must serialize as an object".to_owned(),
        ));
    };
    let fields = fields
        .iter()
        .map(|(name, value)| {
            searchkernel_ir::request_value_from_json(value, format!("document.fields.{name}"))
                .map(|value| (name.clone(), value))
                .map_err(Error::from_ir)
        })
        .collect::<Result<BTreeMap<_, _>>>()?;
    Ok(searchkernel_ir::DocumentInput {
        id: id.to_owned(),
        fields,
    })
}

fn facet_from_grouping(grouping: &searchkernel_ir::GroupingRequestIr) -> Option<Facet> {
    use searchkernel_ir::{GroupingExpressionIr as Expr, GroupingOperationIr as Op};
    let field = match grouping.root.operations.first()? {
        Op::Group(Expr::Attribute(field)) => field,
        _ => return None,
    };
    let each = match grouping.root.operations.get(1)? {
        Op::Each(each) => each,
        _ => return None,
    };
    let limit = each
        .operations
        .iter()
        .find_map(|operation| match operation {
            Op::Max(Some(limit)) => Some(*limit),
            _ => None,
        })?;
    let order = each
        .operations
        .iter()
        .find_map(|operation| match operation {
            Op::Order(expressions) => expressions.first(),
            _ => None,
        })?;
    let sort = match order {
        Expr::Negative(value) if matches!(value.as_ref(), Expr::Aggregation(value) if matches!(value.as_ref(), searchkernel_ir::GroupingAggregationIr::Count)) => {
            FacetSort::CountDesc
        }
        Expr::Aggregation(value)
            if matches!(
                value.as_ref(),
                searchkernel_ir::GroupingAggregationIr::Count
            ) =>
        {
            FacetSort::CountAsc
        }
        Expr::Negative(value) if matches!(value.as_ref(), Expr::Attribute(_)) => {
            FacetSort::ValueDesc
        }
        Expr::Attribute(_) => FacetSort::ValueAsc,
        _ => return None,
    };
    let facet = Facet::new(field).limit(limit).sort(sort);
    Some(match grouping.root.label.as_deref() {
        Some(name) if name != field => facet.name(name),
        _ => facet,
    })
}

fn filter_from_ir(filter: &searchkernel_ir::FilterIr) -> Result<crate::Filter> {
    use searchkernel_ir::{ComparisonOp, FilterIr, ScalarValueIr};

    fn scalar_to_json(value: &ScalarValueIr) -> Result<serde_json::Value> {
        match value {
            ScalarValueIr::Bool(value) => Ok(serde_json::Value::Bool(*value)),
            ScalarValueIr::Int64(value) => Ok(serde_json::Value::Number((*value).into())),
            ScalarValueIr::Float64(value) => serde_json::Number::from_f64(*value)
                .map(serde_json::Value::Number)
                .ok_or_else(|| {
                    Error::infrastructure("normalized filter contains a non-finite float")
                }),
            ScalarValueIr::String(value) => Ok(serde_json::Value::String(value.clone())),
        }
    }

    match filter {
        FilterIr::And(filters) => filters
            .iter()
            .map(filter_from_ir)
            .collect::<Result<Vec<_>>>()
            .map(crate::Filter::all),
        FilterIr::Or(filters) => filters
            .iter()
            .map(filter_from_ir)
            .collect::<Result<Vec<_>>>()
            .map(crate::Filter::any),
        FilterIr::Not(filter) => Ok(crate::Filter::not(filter_from_ir(filter)?)),
        FilterIr::Comparison(comparison) => {
            let field = crate::field(comparison.field().as_str());
            let value = scalar_to_json(comparison.value())?;
            Ok(match comparison.op() {
                ComparisonOp::Eq => field.eq(value),
                ComparisonOp::Ne => field.ne(value),
                ComparisonOp::Lt => field.lt(value),
                ComparisonOp::Lte => field.lte(value),
                ComparisonOp::Gt => field.gt(value),
                ComparisonOp::Gte => field.gte(value),
            })
        }
        FilterIr::Range(range) => {
            let mut bounds = crate::Range::new()
                .include_lower(range.include_lower())
                .include_upper(range.include_upper());
            match range.lower_bound() {
                searchkernel_ir::FilterRangeBoundIr::Finite(value) => {
                    bounds = bounds.lower(scalar_to_json(value)?);
                }
                searchkernel_ir::FilterRangeBoundIr::Unbounded => {}
                searchkernel_ir::FilterRangeBoundIr::NegativeInfinity
                | searchkernel_ir::FilterRangeBoundIr::PositiveInfinity => {
                    return Err(Error::search(
                        crate::error::ErrorCode::Unsupported,
                        "infinite filter bounds are not supported by the public filter API",
                    ));
                }
            }
            match range.upper_bound() {
                searchkernel_ir::FilterRangeBoundIr::Finite(value) => {
                    bounds = bounds.upper(scalar_to_json(value)?);
                }
                searchkernel_ir::FilterRangeBoundIr::Unbounded => {}
                searchkernel_ir::FilterRangeBoundIr::NegativeInfinity
                | searchkernel_ir::FilterRangeBoundIr::PositiveInfinity => {
                    return Err(Error::search(
                        crate::error::ErrorCode::Unsupported,
                        "infinite filter bounds are not supported by the public filter API",
                    ));
                }
            }
            Ok(crate::field(range.field().as_str()).range(bounds))
        }
        FilterIr::In { field, values } => values
            .iter()
            .map(|value| scalar_to_json(value).map(|value| crate::field(field.as_str()).eq(value)))
            .collect::<Result<Vec<_>>>()
            .map(crate::Filter::any),
        FilterIr::Exists { field } => Ok(crate::field(field.as_str()).exists()),
    }
}

fn definition_to_ir(definition: &IndexDefinition) -> Result<searchkernel_ir::IndexDefinitionIr> {
    let input = crate::schema::index_definition_to_input(definition)?;
    searchkernel_ir::resolve_index_definition(input).map_err(Error::from_ir)
}

fn field_from_ir(field: &searchkernel_ir::FieldIr) -> Field {
    let mut value = match field.kind() {
        searchkernel_ir::FieldKind::Text => Field::text(field.name().as_str()),
        searchkernel_ir::FieldKind::Keyword => Field::keyword(field.name().as_str()),
        searchkernel_ir::FieldKind::Int64 => Field::int64(field.name().as_str()),
        searchkernel_ir::FieldKind::Float64 => Field::float64(field.name().as_str()),
        searchkernel_ir::FieldKind::Bool => Field::bool(field.name().as_str()),
    };
    if matches!(field.data_type(), searchkernel_ir::DataTypeIr::Position) {
        value = Field::position(field.name().as_str());
    }
    if let searchkernel_ir::DataTypeIr::Tensor { type_spec } = field.data_type() {
        value = Field::tensor(field.name().as_str(), type_spec);
    }
    value.analyzer_config = field.analyzer().cloned();
    value.matching_config = field.matching().cloned();
    value.data_type = field.data_type().clone();
    value.struct_fields = field.struct_fields().to_vec();
    value.summary_config = field.summary().cloned();
    value.indexing_script = field.indexing_script().map(ToOwned::to_owned);
    value.properties = field.properties().clone();
    if matches!(
        field.cardinality(),
        searchkernel_ir::FieldCardinality::Multi
    ) {
        value = value.multi();
    }
    let capabilities = field.capabilities();
    value = value
        .stored(capabilities.stored())
        .searchable(capabilities.searchable())
        .filterable(capabilities.filterable())
        .sortable(capabilities.sortable())
        .facetable(capabilities.facetable());
    value.capabilities = Some(FieldCapabilities::from_ir(capabilities));
    value.mark_capabilities_resolved();
    if field.analyzer().is_some_and(|analyzer| {
        analyzer.name == "standard" && analyzer.version == 1 && analyzer.options.is_empty()
    }) {
        value = value.analyzer(Analyzer::standard_v1());
    }
    if let Some(attribute) = field.attribute() {
        if attribute.distance_metric != searchkernel_ir::DistanceMetricIr::Unspecified {
            value = value.distance_metric(match attribute.distance_metric {
                searchkernel_ir::DistanceMetricIr::Euclidean => {
                    crate::schema::DistanceMetric::Euclidean
                }
                searchkernel_ir::DistanceMetricIr::Angular => {
                    crate::schema::DistanceMetric::Angular
                }
                searchkernel_ir::DistanceMetricIr::DotProduct => {
                    crate::schema::DistanceMetric::DotProduct
                }
                searchkernel_ir::DistanceMetricIr::PrenormalizedAngular => {
                    crate::schema::DistanceMetric::PrenormalizedAngular
                }
                searchkernel_ir::DistanceMetricIr::GeoDegrees => {
                    crate::schema::DistanceMetric::GeoDegrees
                }
                searchkernel_ir::DistanceMetricIr::Hamming => {
                    crate::schema::DistanceMetric::Hamming
                }
                searchkernel_ir::DistanceMetricIr::Unspecified => unreachable!(),
            });
        }
    }
    if let Some(config) = field.index().and_then(|value| value.hnsw.as_ref()) {
        let mut hnsw = crate::schema::HnswIndexConfig::new();
        if let Some(value) = config.max_links_per_node {
            hnsw = hnsw.max_links_per_node(value);
        }
        if let Some(value) = config.neighbors_to_explore_at_insert {
            hnsw = hnsw.neighbors_to_explore_at_insert(value);
        }
        value = value.hnsw(hnsw);
    }
    value
}

fn definition_from_ir(definition: &searchkernel_ir::IndexDefinitionIr) -> IndexDefinition {
    let mut result = IndexDefinition::new();
    result.properties = definition.properties().clone();
    if let Some(name) = definition.name() {
        result = result.name(name.as_str());
    }
    for field in definition.fields() {
        let value = field_from_ir(field);
        result = result.field(value);
        let described = result.fields.last_mut().expect("field was just appended");
        described.index_config = field.index().cloned();
        described.attribute_config = field.attribute().cloned();
        described.canonical = Some(field.clone());
    }
    for field in definition.schema_fields() {
        let mut value = field_from_ir(field);
        value.index_config = field.index().cloned();
        value.attribute_config = field.attribute().cloned();
        value.canonical = Some(field.clone());
        result.schema_fields.push(value);
    }
    result = result.default_search_fields(
        definition
            .default_search_fields()
            .iter()
            .map(|field| SearchField::new(field.field().as_str()).weight(field.weight().get())),
    );
    for profile in definition.rank_profiles() {
        let mut value = RankProfile::new(profile.name.clone());
        if let Some(expression) = profile
            .first_phase
            .as_ref()
            .and_then(|phase| phase.expression.as_ref())
        {
            value = value.first_phase(expression.clone());
        }
        if let Some(phase) = profile.second_phase.as_ref() {
            if let Some(expression) = phase.expression.as_ref() {
                value = value.second_phase(expression.clone());
            }
            if let Some(rerank_count) = phase.rerank_count {
                value = value.rerank_count(rerank_count);
            }
        }
        value.first_phase = profile.first_phase.clone();
        value.second_phase = profile.second_phase.clone();
        value.global_phase = profile.global_phase.clone();
        for feature in &profile.match_features {
            value = value.match_feature(feature.clone());
        }
        for input in &profile.inputs {
            if let searchkernel_ir::DataTypeIr::Tensor { type_spec } = &input.data_type {
                value = value.query_input(input.name.clone(), type_spec.clone());
            }
        }
        value.rank_features = profile.rank_features.clone();
        value.summary_features = profile.summary_features.clone();
        value.sort_features = profile.sort_features.clone();
        value.rank_properties = profile.rank_properties.clone();
        value.strict = profile.strict;
        value.properties = profile.properties.clone();
        result = result.rank_profile(value);
        let described = result
            .rank_profiles
            .last_mut()
            .expect("rank profile was just appended");
        described.inherits = profile.inherits.clone();
        described.functions = profile.functions.clone();
        described.inputs = profile.inputs.clone();
        described.constants = profile.constants.clone();
        described.onnx_models = profile.onnx_models.clone();
        described.canonical = Some(profile.clone());
    }
    result.constants = definition
        .constants()
        .iter()
        .map(ConstantDefinition::from_ir)
        .collect();
    result.fieldsets = definition.fieldsets().to_vec();
    result.onnx_models = definition.onnx_models().to_vec();
    result.imported_fields = definition.imported_fields().to_vec();
    result.document_summaries = definition.document_summaries().to_vec();
    result.document_type = definition.document_type().map(str::to_owned);
    result.schema_inherits = definition.schema_inherits().to_vec();
    result.document_inherits = definition.document_inherits().to_vec();
    result.structs = definition.structs().to_vec();
    result.canonical = Some(definition.clone());
    result
}
