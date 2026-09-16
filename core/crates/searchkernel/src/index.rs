use crate::analyze::{AnalyzeBuilder, AnalyzeResult, AnalyzedToken};
use crate::document::{serialize, DocumentId};
use crate::error::{Error, Result};
use crate::patch::Patch;
use crate::result;
use crate::schema::{Analyzer, Field, IndexDefinition, SearchField};
use crate::search::{Facet, FacetSort, Ranking, SearchBuilder, Sort};
use crate::wire;
use searchkernel_ir::{api, Backend, BackendSession, IndexContext};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::sync::Arc;

#[cfg(feature = "native")]
const MANIFEST_FORMAT_VERSION: u32 = 3;
#[cfg(feature = "native")]
const LEGACY_MANIFEST_FORMAT_VERSION: u32 = 2;
#[cfg(feature = "native")]
const STORAGE_COMPATIBILITY_GENERATION: u32 = 1;
#[cfg(feature = "native")]
const NATIVE_ABI_VERSION: u32 = 5;
#[cfg(feature = "native")]
const VESPA_REVISION: &str = "2300ef0412bdb76f227ae3b497c8ca5e73c5da4e";

#[cfg(feature = "native")]
fn create_directory_transaction<T>(
    root: &std::path::Path,
    initialize: impl FnOnce(&std::path::Path) -> Result<T>,
) -> Result<T> {
    if root.exists() {
        return Err(Error::already_exists(root.display().to_string()));
    }
    if let Some(parent) = root.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| Error::Infrastructure(error.to_string()))?;
    }
    std::fs::create_dir(root).map_err(|error| {
        if error.kind() == std::io::ErrorKind::AlreadyExists {
            Error::already_exists(root.display().to_string())
        } else {
            Error::Infrastructure(error.to_string())
        }
    })?;

    match initialize(root) {
        Ok(value) => Ok(value),
        Err(error) => {
            if let Err(cleanup_error) = std::fs::remove_dir_all(root) {
                return Err(Error::Infrastructure(format!(
                    "{error}; failed to clean up partial index {}: {cleanup_error}",
                    root.display()
                )));
            }
            Err(error)
        }
    }
}

struct IndexInner {
    context: Arc<IndexContext>,
    definition: Arc<IndexDefinition>,
    session: Arc<BackendSession>,
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

impl Index {
    /// Construct an index from the canonical FlatBuffers index-definition
    /// payload used by the native bootstrap. This is intentionally hidden from
    /// the ergonomic Rust API; the portable wire definition is validated by
    /// `searchkernel-ir` before it reaches the product lifecycle.
    #[doc(hidden)]
    #[cfg(feature = "native")]
    pub fn create_from_definition_bytes(
        path: impl AsRef<std::path::Path>,
        bytes: &[u8],
    ) -> Result<Self> {
        let root = searchkernel_ir::__flatbuffers::root::<api::IndexDefinition>(bytes)
            .map_err(|error| Error::Serialization(format!("invalid index definition: {error}")))?;
        let ir = searchkernel_ir::index_definition_to_ir(root).map_err(Error::from_ir)?;
        Self::create(path, definition_from_ir(&ir))
    }

    /// Create a new on-disk index and persist all information required to
    /// reopen it safely.
    #[cfg(feature = "native")]
    pub fn create(path: impl AsRef<std::path::Path>, definition: IndexDefinition) -> Result<Self> {
        let root = path.as_ref();
        create_directory_transaction(root, |root| {
            let resolved_ir = definition_to_ir(&definition)?;
            searchkernel_ir_adapter::validate_index_definition(&resolved_ir)
                .map_err(Error::from_ir)?;
            let resolved_definition = definition_from_ir(&resolved_ir);
            let data = root.join("data");
            let config = root.join("config");
            std::fs::create_dir(&data).map_err(|error| Error::Infrastructure(error.to_string()))?;
            searchkernel_ir_adapter::compile_index_definition(&resolved_ir, &config)
                .map_err(Error::from_ir)?;
            let manifest = IndexManifest::for_definition(&definition, &config)?;
            manifest.write(root)?;
            let backend = searchkernel_ir_adapter::VespaBackend::open(&data, &config)
                .map_err(|error| Error::Infrastructure(error.to_string()))?;
            Self::from_owned_backend(resolved_definition, BackendSession::new(Box::new(backend)))
        })
    }

    /// Reopen an existing index using its durable manifest.
    #[cfg(feature = "native")]
    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self> {
        let root = path.as_ref();
        if !root.is_dir() {
            return Err(Error::not_found(root.display().to_string()));
        }
        let manifest = IndexManifest::read(root)?;
        let definition = manifest.definition()?;
        let config = root.join("config");
        if !root.join("data").is_dir() || !config.is_dir() {
            return Err(Error::corrupt_index(
                "index data/config directories are missing",
            ));
        }
        manifest.validate_config(&config)?;
        let backend = searchkernel_ir_adapter::VespaBackend::open(root.join("data"), &config)
            .map_err(|error| Error::Infrastructure(error.to_string()))?;
        Self::from_owned_backend(definition, BackendSession::new(Box::new(backend)))
    }

    /// Construct an index around a backend for tests and infrastructure.
    ///
    /// This is hidden from normal API documentation because production
    /// lifecycle is supplied by a later integration milestone.
    #[doc(hidden)]
    pub fn from_backend(definition: IndexDefinition, backend: Arc<dyn Backend>) -> Result<Self> {
        Self::from_owned_backend(definition, BackendSession::from_backend(backend))
    }

    fn from_owned_backend(
        definition: IndexDefinition,
        session: Arc<BackendSession>,
    ) -> Result<Self> {
        let bytes = wire::definition(&definition);
        let root = searchkernel_ir::__flatbuffers::root::<api::IndexDefinition>(&bytes).map_err(
            |error| Error::infrastructure(format!("invalid generated index definition: {error}")),
        )?;
        let context = IndexContext::from_api(root).map_err(Error::from_ir)?;
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

    /// Return a cloneable operation handle.
    pub fn index(&self) -> Index {
        self.clone()
    }

    /// Gracefully close this index and all of its clones after draining active
    /// operations.
    pub fn close(&self) -> Result<()> {
        self.inner.session.close().map_err(Error::from_ir)
    }

    /// Transfer the shared lifecycle state to the native handle registry.
    #[doc(hidden)]
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
            let request_bytes = wire::insert(document_id.as_str(), &document_value)?;
            let request = self.root_insert(&request_bytes)?;
            let ir_request = self
                .inner
                .context
                .insert_to_ir(request)
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
            let request_bytes = wire::upsert(document_id.as_str(), &document_value)?;
            let request = self.root_upsert(&request_bytes)?;
            let ir_request = self
                .inner
                .context
                .upsert_to_ir(request)
                .map_err(Error::from_ir)?;
            backend.upsert(ir_request).map_err(Error::from_ir)?;
            Ok(())
        })
    }

    /// Apply field updates to an existing document.
    pub fn patch(&self, id: impl Into<DocumentId>, patch: Patch) -> Result<()> {
        self.with_backend(|backend| {
            let document_id = id.into();
            let request_bytes = wire::patch(document_id.as_str(), &patch)?;
            let request = self.root_patch(&request_bytes)?;
            let ir_request = self
                .inner
                .context
                .patch_to_ir(request)
                .map_err(Error::from_ir)?;
            backend.patch(ir_request).map_err(Error::from_ir)?;
            Ok(())
        })
    }

    /// Delete a document and return whether it existed.
    pub fn delete(&self, id: impl Into<DocumentId>) -> Result<bool> {
        self.with_backend(|backend| {
            let document_id = id.into();
            let request_bytes = wire::delete(document_id.as_str());
            let request = self.root_delete(&request_bytes)?;
            let ir_request = self
                .inner
                .context
                .delete_to_ir(request)
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
            let request_bytes = wire::get(document_id.as_str(), projection.as_deref());
            let request = self.root_get(&request_bytes)?;
            let get_result = backend
                .get(
                    self.inner
                        .context
                        .get_to_ir(request)
                        .map_err(Error::from_ir)?,
                )
                .map_err(Error::from_ir)?;
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
        self.search("")
    }

    /// Return the effective schema and search defaults for this index.
    pub fn describe(&self) -> Result<IndexDefinition> {
        self.with_backend(|_| Ok((*self.inner.definition).clone()))
    }

    /// Start a linguistic analysis request.
    pub fn analyze(&self, text: impl Into<String>) -> AnalyzeBuilder<'_> {
        AnalyzeBuilder::new(self, text)
    }

    pub(crate) fn execute_analyze(&self, request: &AnalyzeBuilder<'_>) -> Result<AnalyzeResult> {
        self.with_backend(|backend| {
            let request_bytes = wire::analyze(request)?;
            let wire_request =
                searchkernel_ir::__flatbuffers::root::<api::AnalyzeRequest>(&request_bytes)
                    .map_err(|error| {
                        Error::infrastructure(format!("invalid generated analyze request: {error}"))
                    })?;
            let ir_request = self
                .inner
                .context
                .analyze_to_ir(wire_request)
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
            let request_bytes = wire::search(request)?;
            let wire_request =
                searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&request_bytes)
                    .map_err(|error| {
                        Error::infrastructure(format!("invalid generated search request: {error}"))
                    })?;
            let normalized = self
                .inner
                .context
                .inspect_query(wire_request)
                .map_err(Error::from_ir)?;
            let query = match normalized.query() {
                searchkernel_ir::QueryIr::MatchAll => String::new(),
                searchkernel_ir::QueryIr::Text(value) => value.clone(),
                _ => format!("{:?}", normalized.query()),
            };
            let highlighting = match normalized.highlighting() {
                searchkernel_ir::HighlightingIr::Disabled => None,
                searchkernel_ir::HighlightingIr::Requested { fields } => Some(
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
                        let value = match sort.direction() {
                            searchkernel_ir::SortDirection::Asc => Sort::asc(sort.field().as_str()),
                            searchkernel_ir::SortDirection::Desc => {
                                Sort::desc(sort.field().as_str())
                            }
                        };
                        match sort.missing() {
                            searchkernel_ir::MissingPlacement::First => value.missing_first(),
                            searchkernel_ir::MissingPlacement::Last => value.missing_last(),
                        }
                    })
                    .collect(),
                facets: normalized
                    .facets()
                    .iter()
                    .map(|facet| {
                        let value = Facet::new(facet.field().as_str()).limit(facet.limit());
                        value.sort(match facet.sort() {
                            searchkernel_ir::FacetSort::CountDesc => FacetSort::CountDesc,
                            searchkernel_ir::FacetSort::CountAsc => FacetSort::CountAsc,
                            searchkernel_ir::FacetSort::ValueAsc => FacetSort::ValueAsc,
                            searchkernel_ir::FacetSort::ValueDesc => FacetSort::ValueDesc,
                        })
                    })
                    .collect(),
                ranking: match normalized.ranking() {
                    searchkernel_ir::RankingPolicyIr::DefaultRelevanceV1 => {
                        Ranking::default_relevance_v1()
                    }
                    searchkernel_ir::RankingPolicyIr::Bm25 => Ranking::bm25(),
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
            let request_bytes = wire::search(request)?;
            let wire_request =
                searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&request_bytes)
                    .map_err(|error| {
                        Error::infrastructure(format!("invalid generated search request: {error}"))
                    })?;
            let ir_request = self
                .inner
                .context
                .search_to_ir(wire_request)
                .map_err(Error::from_ir)?;
            let backend_result = backend.search(ir_request).map_err(Error::from_ir)?;
            result::search_result(backend_result)
        })
    }

    fn root_insert<'a>(&self, bytes: &'a [u8]) -> Result<api::InsertRequest<'a>> {
        searchkernel_ir::__flatbuffers::root::<api::InsertRequest>(bytes).map_err(|error| {
            Error::infrastructure(format!("invalid generated insert request: {error}"))
        })
    }

    fn root_upsert<'a>(&self, bytes: &'a [u8]) -> Result<api::UpsertRequest<'a>> {
        searchkernel_ir::__flatbuffers::root::<api::UpsertRequest>(bytes).map_err(|error| {
            Error::infrastructure(format!("invalid generated upsert request: {error}"))
        })
    }

    fn root_patch<'a>(&self, bytes: &'a [u8]) -> Result<api::PatchRequest<'a>> {
        searchkernel_ir::__flatbuffers::root::<api::PatchRequest>(bytes).map_err(|error| {
            Error::infrastructure(format!("invalid generated patch request: {error}"))
        })
    }

    fn root_delete<'a>(&self, bytes: &'a [u8]) -> Result<api::DeleteRequest<'a>> {
        searchkernel_ir::__flatbuffers::root::<api::DeleteRequest>(bytes).map_err(|error| {
            Error::infrastructure(format!("invalid generated delete request: {error}"))
        })
    }

    fn root_get<'a>(&self, bytes: &'a [u8]) -> Result<api::GetRequest<'a>> {
        searchkernel_ir::__flatbuffers::root::<api::GetRequest>(bytes).map_err(|error| {
            Error::infrastructure(format!("invalid generated get request: {error}"))
        })
    }
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
            if let Some(value) = range.lower() {
                bounds = bounds.lower(scalar_to_json(value)?);
            }
            if let Some(value) = range.upper() {
                bounds = bounds.upper(scalar_to_json(value)?);
            }
            Ok(crate::field(range.field().as_str()).range(bounds))
        }
        FilterIr::Exists { field } => Ok(crate::field(field.as_str()).exists()),
    }
}

#[cfg(feature = "native")]
#[derive(Debug, Clone)]
struct IndexManifest {
    value: serde_json::Value,
    definition_bytes: Vec<u8>,
}

#[cfg(feature = "native")]
impl IndexManifest {
    fn for_definition(
        portable_definition: &IndexDefinition,
        config: &std::path::Path,
    ) -> Result<Self> {
        Ok(Self {
            value: serde_json::json!({
                "manifest_format_version": MANIFEST_FORMAT_VERSION,
                "storage_compatibility_generation": STORAGE_COMPATIBILITY_GENERATION,
                "provenance": {
                    "created_by_searchkernel_version": env!("CARGO_PKG_VERSION"),
                    "created_by_native_abi": NATIVE_ABI_VERSION,
                    "created_by_vespa_revision": VESPA_REVISION,
                },
                "canonical_index_definition": "index-definition.bfbs",
                "resolved_engine_config_identity": config_identity(config)?,
            }),
            definition_bytes: wire::definition(portable_definition),
        })
    }

    fn write(&self, root: &std::path::Path) -> Result<()> {
        let directory = root.join("manifest");
        std::fs::create_dir_all(&directory)
            .map_err(|error| Error::Infrastructure(error.to_string()))?;
        let bytes = serde_json::to_vec_pretty(&self.value)
            .map_err(|error| Error::Infrastructure(error.to_string()))?;
        std::fs::write(
            directory.join("index-definition.bfbs"),
            &self.definition_bytes,
        )
        .map_err(|error| Error::Infrastructure(error.to_string()))?;
        std::fs::write(directory.join("index.json"), bytes)
            .map_err(|error| Error::Infrastructure(error.to_string()))
    }

    fn read(root: &std::path::Path) -> Result<Self> {
        let path = root.join("manifest/index.json");
        let bytes = std::fs::read(&path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                Error::corrupt_index(path.display().to_string())
            } else {
                Error::Infrastructure(error.to_string())
            }
        })?;
        let value: serde_json::Value = serde_json::from_slice(&bytes)
            .map_err(|error| Error::corrupt_index(error.to_string()))?;
        let version = value
            .get("manifest_format_version")
            .or_else(|| value.get("format_version"))
            .and_then(serde_json::Value::as_u64);
        let definition_bytes = if version == Some(u64::from(MANIFEST_FORMAT_VERSION)) {
            let path = root.join("manifest/index-definition.bfbs");
            std::fs::read(&path).map_err(|error| {
                if error.kind() == std::io::ErrorKind::NotFound {
                    Error::corrupt_index(path.display().to_string())
                } else {
                    Error::Infrastructure(error.to_string())
                }
            })?
        } else {
            Vec::new()
        };
        let manifest = Self {
            value,
            definition_bytes,
        };
        manifest.validate_identity()?;
        manifest.definition()?;
        Ok(manifest)
    }

    fn validate_identity(&self) -> Result<()> {
        let object = self
            .value
            .as_object()
            .ok_or_else(|| Error::corrupt_index("manifest root is not an object"))?;
        let version = object
            .get("manifest_format_version")
            .or_else(|| object.get("format_version"))
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| Error::corrupt_index("manifest format version is missing"))?;
        match version {
            value if value == u64::from(MANIFEST_FORMAT_VERSION) => {
                let generation = object
                    .get("storage_compatibility_generation")
                    .and_then(serde_json::Value::as_u64)
                    .and_then(|value| u32::try_from(value).ok())
                    .ok_or_else(|| {
                        Error::corrupt_index("manifest storage compatibility generation is missing")
                    })?;
                if generation != STORAGE_COMPATIBILITY_GENERATION {
                    return Err(Error::incompatible_index(format!(
                        "storage compatibility generation {generation}"
                    )));
                }
                let provenance = object
                    .get("provenance")
                    .and_then(serde_json::Value::as_object)
                    .ok_or_else(|| Error::corrupt_index("manifest provenance is invalid"))?;
                if !provenance
                    .get("created_by_searchkernel_version")
                    .is_some_and(serde_json::Value::is_string)
                    || !provenance
                        .get("created_by_native_abi")
                        .and_then(serde_json::Value::as_u64)
                        .and_then(|value| u32::try_from(value).ok())
                        .is_some()
                    || !provenance
                        .get("created_by_vespa_revision")
                        .is_some_and(serde_json::Value::is_string)
                {
                    return Err(Error::corrupt_index(
                        "manifest provenance has invalid fields",
                    ));
                }
                if !object
                    .get("canonical_index_definition")
                    .is_some_and(serde_json::Value::is_string)
                {
                    return Err(Error::corrupt_index(
                        "manifest canonical_index_definition is invalid",
                    ));
                }
            }
            value if value == u64::from(LEGACY_MANIFEST_FORMAT_VERSION) => {
                for key in [
                    "searchkernel_version",
                    "native_abi_version",
                    "vespa_revision",
                ] {
                    if !object.contains_key(key) {
                        return Err(Error::corrupt_index(format!(
                            "legacy manifest {key} is missing"
                        )));
                    }
                }
                if !object
                    .get("portable_index_definition")
                    .is_some_and(serde_json::Value::is_object)
                    || !object
                        .get("resolved_index_definition")
                        .is_some_and(serde_json::Value::is_object)
                {
                    return Err(Error::corrupt_index(
                        "legacy manifest index definition is invalid",
                    ));
                }
            }
            _ => {
                return Err(Error::incompatible_index(format!(
                    "manifest format version {version}"
                )));
            }
        }
        if !object
            .get("resolved_engine_config_identity")
            .is_some_and(serde_json::Value::is_object)
        {
            return Err(Error::corrupt_index(
                "manifest resolved_engine_config_identity is invalid",
            ));
        }
        Ok(())
    }

    fn validate_config(&self, config: &std::path::Path) -> Result<()> {
        for name in [
            "searchkernel-schema.json",
            "indexschema.cfg",
            "attributes.cfg",
            "summary.cfg",
            "rank-profiles.cfg",
            "juniperrc.cfg",
            "proton.cfg",
        ] {
            if !config.join(name).is_file() {
                return Err(Error::corrupt_index(format!(
                    "resolved engine configuration is missing {name}"
                )));
            }
        }
        let expected = self
            .value
            .get("resolved_engine_config_identity")
            .ok_or_else(|| Error::corrupt_index("manifest config identity is missing"))?;
        let actual = config_identity(config)?;
        if expected != &actual {
            return Err(Error::incompatible_index(
                "resolved engine configuration differs",
            ));
        }
        Ok(())
    }

    fn definition(&self) -> Result<IndexDefinition> {
        let version = self
            .value
            .get("manifest_format_version")
            .or_else(|| self.value.get("format_version"))
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| Error::corrupt_index("manifest format version is missing"))?;
        if version == u64::from(MANIFEST_FORMAT_VERSION) {
            let root = searchkernel_ir::__flatbuffers::root::<api::IndexDefinition>(
                &self.definition_bytes,
            )
            .map_err(|error| Error::corrupt_index(format!("invalid index definition: {error}")))?;
            let context = IndexContext::from_api(root).map_err(Error::from_ir)?;
            Ok(definition_from_ir(context.definition()))
        } else {
            definition_from_json(
                self.value
                    .get("resolved_index_definition")
                    .ok_or_else(|| Error::corrupt_index("resolved definition is missing"))?,
            )
        }
    }
}

#[cfg(feature = "native")]
fn definition_to_ir(definition: &IndexDefinition) -> Result<searchkernel_ir::IndexDefinitionIr> {
    let bytes = wire::definition(definition);
    let root =
        searchkernel_ir::__flatbuffers::root::<api::IndexDefinition>(&bytes).map_err(|error| {
            Error::infrastructure(format!("invalid generated index definition: {error}"))
        })?;
    searchkernel_ir::index_definition_to_ir(root).map_err(Error::from_ir)
}

#[cfg(feature = "native")]
fn definition_from_ir(definition: &searchkernel_ir::IndexDefinitionIr) -> IndexDefinition {
    let mut result = IndexDefinition::new();
    if let Some(name) = definition.name() {
        result = result.name(name.as_str());
    }
    for field in definition.fields() {
        let mut value = match field.kind() {
            searchkernel_ir::FieldKind::Text => Field::text(field.name().as_str()),
            searchkernel_ir::FieldKind::Keyword => Field::keyword(field.name().as_str()),
            searchkernel_ir::FieldKind::Int64 => Field::int64(field.name().as_str()),
            searchkernel_ir::FieldKind::Float64 => Field::float64(field.name().as_str()),
            searchkernel_ir::FieldKind::Bool => Field::bool(field.name().as_str()),
        };
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
        if matches!(
            field.analyzer(),
            Some(searchkernel_ir::AnalyzerIdentity::StandardV1)
        ) {
            value = value.analyzer(Analyzer::standard_v1());
        }
        result = result.field(value);
    }
    result.default_search_fields(
        definition
            .default_search_fields()
            .iter()
            .map(|field| SearchField::new(field.field().as_str()).weight(field.weight().get())),
    )
}

#[cfg(feature = "native")]
fn definition_from_json(value: &serde_json::Value) -> Result<IndexDefinition> {
    let object = value
        .as_object()
        .ok_or_else(|| Error::corrupt_index("portable definition is not an object"))?;
    let mut definition = IndexDefinition::new();
    if let Some(name) = object.get("name").and_then(serde_json::Value::as_str) {
        definition = definition.name(name);
    }
    let fields = object
        .get("fields")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| Error::corrupt_index("portable definition fields are missing"))?;
    for field in fields {
        let metadata = field
            .as_object()
            .ok_or_else(|| Error::corrupt_index("manifest field is not an object"))?;
        let name = metadata
            .get("name")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| Error::corrupt_index("manifest field name is missing"))?;
        let kind = match metadata.get("kind").and_then(serde_json::Value::as_str) {
            Some("Text") => Field::text(name),
            Some("Keyword") => Field::keyword(name),
            Some("Int64") => Field::int64(name),
            Some("Float64") => Field::float64(name),
            Some("Bool") => Field::bool(name),
            _ => return Err(Error::corrupt_index("manifest field kind is invalid")),
        };
        let mut field = kind;
        if let Some(analyzer) = metadata.get("analyzer") {
            if !analyzer.is_null() {
                if let Some(analyzer) = analyzer.as_str() {
                    field = match analyzer {
                        "StandardV1" => field.analyzer(Analyzer::standard_v1()),
                        _ => {
                            return Err(Error::corrupt_index("manifest field analyzer is invalid"))
                        }
                    };
                } else {
                    let analyzer = analyzer.as_object().ok_or_else(|| {
                        Error::corrupt_index("manifest field analyzer is invalid")
                    })?;
                    let name = analyzer
                        .get("name")
                        .and_then(serde_json::Value::as_str)
                        .ok_or_else(|| {
                            Error::corrupt_index("manifest field analyzer name is missing")
                        })?;
                    let version = analyzer
                        .get("version")
                        .and_then(serde_json::Value::as_u64)
                        .and_then(|value| u32::try_from(value).ok())
                        .ok_or_else(|| {
                            Error::corrupt_index("manifest field analyzer version is invalid")
                        })?;
                    let options = analyzer
                        .get("options")
                        .map(|options| {
                            options
                                .as_object()
                                .ok_or_else(|| {
                                    Error::corrupt_index(
                                        "manifest field analyzer options are invalid",
                                    )
                                })
                                .map(|options| {
                                    options
                                        .iter()
                                        .map(|(key, value)| (key.clone(), value.clone()))
                                        .collect()
                                })
                        })
                        .transpose()?
                        .unwrap_or_default();
                    field = field.analyzer(Analyzer {
                        name: name.to_owned(),
                        version,
                        options,
                    });
                }
            }
        }
        if metadata
            .get("multi")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
        {
            field = field.multi();
        }
        for (key, setter) in [
            ("stored", 0_u8),
            ("searchable", 1),
            ("filterable", 2),
            ("sortable", 3),
            ("facetable", 4),
        ] {
            if let Some(value) = metadata.get(key).and_then(serde_json::Value::as_bool) {
                field = match setter {
                    0 => field.stored(value),
                    1 => field.searchable(value),
                    2 => field.filterable(value),
                    3 => field.sortable(value),
                    _ => field.facetable(value),
                };
            }
        }
        definition = definition.field(field);
    }
    if let Some(search_fields) = object
        .get("default_search_fields")
        .and_then(serde_json::Value::as_array)
    {
        let mut fields = Vec::with_capacity(search_fields.len());
        for search_field in search_fields {
            let search_field = search_field
                .as_object()
                .ok_or_else(|| Error::corrupt_index("manifest search field is not an object"))?;
            let name = search_field
                .get("field")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| Error::corrupt_index("manifest search field name is missing"))?;
            let mut field = SearchField::new(name);
            if let Some(weight) = search_field
                .get("weight")
                .and_then(serde_json::Value::as_f64)
            {
                field = field.weight(weight);
            }
            fields.push(field);
        }
        definition = definition.default_search_fields(fields);
    }
    Ok(definition)
}

#[cfg(feature = "native")]
fn config_identity(config: &std::path::Path) -> Result<serde_json::Value> {
    let mut files = serde_json::Map::new();
    for name in [
        "searchkernel-schema.json",
        "indexschema.cfg",
        "attributes.cfg",
        "summary.cfg",
        "rank-profiles.cfg",
        "juniperrc.cfg",
        "proton.cfg",
    ] {
        let path = config.join(name);
        let contents = std::fs::read_to_string(&path)
            .map_err(|error| Error::Infrastructure(error.to_string()))?;
        files.insert(name.into(), serde_json::Value::String(contents));
    }
    Ok(serde_json::Value::Object(files))
}

#[cfg(all(test, feature = "native"))]
mod tests {
    use super::*;
    use crate::{Analyzer, Field, SearchField};
    use searchkernel_ir::{AnalyzerIdentity, FieldCardinality, FieldKind};
    use searchkernel_testkit::RecordingBackend;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temporary_path(label: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("searchkernel-index-test-{label}-{nonce}"))
    }

    fn write_config(config: &std::path::Path) {
        std::fs::create_dir_all(config).unwrap();
        for name in [
            "searchkernel-schema.json",
            "indexschema.cfg",
            "attributes.cfg",
            "summary.cfg",
            "rank-profiles.cfg",
            "juniperrc.cfg",
            "proton.cfg",
        ] {
            std::fs::write(config.join(name), format!("{name}\n")).unwrap();
        }
    }

    fn manifest_definition() -> IndexDefinition {
        IndexDefinition::new()
            .name("catalog")
            .field(
                Field::text("title")
                    .multi()
                    .stored(true)
                    .searchable(true)
                    .filterable(false)
                    .sortable(false)
                    .facetable(false)
                    .analyzer(Analyzer::standard_v1()),
            )
            .field(
                Field::keyword("brand")
                    .stored(false)
                    .searchable(false)
                    .filterable(true)
                    .sortable(true)
                    .facetable(true),
            )
            .field(Field::int64("quantity"))
            .field(Field::float64("price"))
            .field(Field::bool("enabled"))
            .default_search_fields([SearchField::new("title")])
    }

    #[test]
    fn schema_options_and_default_search_fields_reach_core() {
        let definition = IndexDefinition::new()
            .name("catalog")
            .field(
                Field::text("title")
                    .stored(false)
                    .searchable(true)
                    .filterable(false)
                    .sortable(false)
                    .facetable(false)
                    .analyzer(Analyzer::standard_v1()),
            )
            .field(
                Field::keyword("brand")
                    .stored(false)
                    .searchable(false)
                    .filterable(true)
                    .sortable(true)
                    .facetable(true),
            )
            .field(
                Field::int64("quantity")
                    .stored(true)
                    .searchable(false)
                    .filterable(true)
                    .sortable(true)
                    .facetable(true),
            )
            .field(
                Field::float64("price")
                    .stored(false)
                    .searchable(false)
                    .filterable(true)
                    .sortable(false)
                    .facetable(false),
            )
            .field(
                Field::bool("enabled")
                    .multi()
                    .stored(true)
                    .searchable(false)
                    .filterable(true)
                    .sortable(false)
                    .facetable(true),
            )
            .default_search_fields([SearchField::new("title").weight(0.0)]);
        let index = IndexRuntime::from_backend(definition, Arc::new(RecordingBackend::new()))
            .unwrap()
            .index();
        let description = index.inner.context.describe();

        assert_eq!(description.name().unwrap().as_str(), "catalog");
        assert_eq!(description.fields().len(), 5);
        assert_eq!(description.default_search_fields().len(), 1);
        assert_eq!(description.default_search_fields()[0].weight().get(), 0.0);

        let fields = description.fields();
        assert_eq!(fields[0].name().as_str(), "title");
        assert_eq!(fields[0].kind(), FieldKind::Text);
        assert_eq!(fields[0].cardinality(), FieldCardinality::Single);
        assert_eq!(fields[0].analyzer(), Some(AnalyzerIdentity::StandardV1));
        assert!(!fields[0].capabilities().stored());
        assert!(fields[0].capabilities().searchable());

        assert_eq!(fields[1].kind(), FieldKind::Keyword);
        assert!(!fields[1].capabilities().stored());
        assert!(fields[1].capabilities().filterable());
        assert!(fields[1].capabilities().sortable());
        assert!(fields[1].capabilities().facetable());

        assert_eq!(fields[2].kind(), FieldKind::Int64);
        assert!(fields[2].capabilities().facetable());
        assert_eq!(fields[3].kind(), FieldKind::Float64);
        assert!(!fields[3].capabilities().stored());
        assert_eq!(fields[4].kind(), FieldKind::Bool);
        assert_eq!(fields[4].cardinality(), FieldCardinality::Multi);
        assert!(!fields[4].capabilities().sortable());
    }

    #[test]
    fn manifest_round_trips_definition_and_config_identity() {
        let root = temporary_path("manifest-round-trip");
        let config = root.join("config");
        write_config(&config);
        let definition = manifest_definition();

        let manifest = IndexManifest::for_definition(&definition, &config).unwrap();
        manifest.write(&root).unwrap();
        let loaded = IndexManifest::read(&root).unwrap();
        loaded.validate_config(&config).unwrap();

        let expected = definition_from_ir(&definition_to_ir(&definition).unwrap());
        assert_eq!(loaded.definition().unwrap(), expected);
        assert_eq!(
            loaded.value["manifest_format_version"],
            MANIFEST_FORMAT_VERSION
        );
        assert_eq!(
            loaded.value["storage_compatibility_generation"],
            STORAGE_COMPATIBILITY_GENERATION
        );
        assert!(root.join("manifest/index-definition.bfbs").is_file());

        std::fs::write(config.join("proton.cfg"), "changed\n").unwrap();
        assert!(matches!(
            loaded.validate_config(&config),
            Err(Error::Search { code: crate::ErrorCode::IncompatibleIndex, message, .. })
                if message == "incompatible index: resolved engine configuration differs"
        ));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn legacy_manifest_with_old_abi_is_still_readable() {
        let root = temporary_path("legacy-manifest");
        let config = root.join("config");
        write_config(&config);
        let definition = serde_json::json!({
            "name": "legacy",
            "fields": [{"name": "title", "kind": "Text"}],
            "default_search_fields": [{"field": "title", "weight": 1.0}]
        });
        let value = serde_json::json!({
            "format_version": LEGACY_MANIFEST_FORMAT_VERSION,
            "searchkernel_version": "0.1.0",
            "native_abi_version": 4,
            "vespa_revision": "2300ef0412bdb76f227ae3b497c8ca5e73c5da4e",
            "portable_index_definition": definition.clone(),
            "resolved_index_definition": definition,
            "resolved_engine_config_identity": config_identity(&config).unwrap(),
        });
        std::fs::create_dir_all(root.join("manifest")).unwrap();
        std::fs::write(
            root.join("manifest/index.json"),
            serde_json::to_vec_pretty(&value).unwrap(),
        )
        .unwrap();

        let loaded = IndexManifest::read(&root).unwrap();
        assert_eq!(loaded.definition().unwrap().index_name(), Some("legacy"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn canonical_definition_corruption_is_rejected() {
        let root = temporary_path("canonical-definition-corruption");
        let config = root.join("config");
        write_config(&config);
        let definition = manifest_definition();
        let manifest = IndexManifest::for_definition(&definition, &config).unwrap();
        manifest.write(&root).unwrap();
        std::fs::write(root.join("manifest/index-definition.bfbs"), b"invalid").unwrap();

        assert!(matches!(
            IndexManifest::read(&root),
            Err(Error::Search {
                code: crate::ErrorCode::CorruptIndex,
                ..
            })
        ));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn manifest_validation_rejects_identity_and_structure_corruption() {
        let root = temporary_path("manifest-validation");
        let config = root.join("config");
        write_config(&config);
        let definition = manifest_definition();
        let manifest = IndexManifest::for_definition(&definition, &config).unwrap();

        let mut provenance = manifest.clone();
        provenance.value["provenance"]["created_by_native_abi"] = serde_json::json!(4);
        provenance.value["provenance"]["created_by_vespa_revision"] =
            serde_json::json!("older-revision");
        assert!(provenance.validate_identity().is_ok());

        let mut generation = manifest.clone();
        generation.value["storage_compatibility_generation"] = serde_json::json!(999);
        assert!(matches!(
            generation.validate_identity(),
            Err(Error::Search {
                code: crate::ErrorCode::IncompatibleIndex,
                ..
            })
        ));

        let mut malformed_config = manifest.clone();
        malformed_config.value["resolved_engine_config_identity"] = serde_json::json!("invalid");
        assert!(matches!(
            malformed_config.validate_identity(),
            Err(Error::Search {
                code: crate::ErrorCode::CorruptIndex,
                ..
            })
        ));

        let mut malformed_provenance = manifest.clone();
        malformed_provenance.value["provenance"]["created_by_native_abi"] = serde_json::json!("5");
        assert!(matches!(
            malformed_provenance.validate_identity(),
            Err(Error::Search {
                code: crate::ErrorCode::CorruptIndex,
                ..
            })
        ));

        for value in [serde_json::json!(null), serde_json::json!({})] {
            let result = IndexManifest {
                value,
                definition_bytes: Vec::new(),
            }
            .validate_identity();
            assert!(matches!(
                result,
                Err(Error::Search {
                    code: crate::ErrorCode::CorruptIndex,
                    ..
                })
            ));
        }

        let malformed = root.join("malformed");
        std::fs::create_dir_all(malformed.join("manifest")).unwrap();
        std::fs::write(malformed.join("manifest/index.json"), b"not-json").unwrap();
        assert!(matches!(
            IndexManifest::read(&malformed),
            Err(Error::Search {
                code: crate::ErrorCode::CorruptIndex,
                ..
            })
        ));

        let missing = root.join("missing");
        std::fs::create_dir_all(&missing).unwrap();
        assert!(matches!(
            IndexManifest::read(&missing),
            Err(Error::Search {
                code: crate::ErrorCode::CorruptIndex,
                ..
            })
        ));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn manifest_definition_decoder_rejects_malformed_fields() {
        let invalid_definitions = [
            serde_json::json!(null),
            serde_json::json!({}),
            serde_json::json!({"fields": [null]}),
            serde_json::json!({"fields": [{"name": "title"}]}),
            serde_json::json!({"fields": [{"name": "title", "kind": "Unknown"}]}),
            serde_json::json!({
                "fields": [{"name": "title", "kind": "Text", "analyzer": 1}]
            }),
            serde_json::json!({
                "fields": [{"name": "title", "kind": "Text", "analyzer": "Unknown"}]
            }),
            serde_json::json!({
                "fields": [{"name": "title", "kind": "Text"}],
                "default_search_fields": [null]
            }),
            serde_json::json!({
                "fields": [{"name": "title", "kind": "Text"}],
                "default_search_fields": [{}]
            }),
        ];

        for value in invalid_definitions {
            assert!(matches!(
                definition_from_json(&value),
                Err(Error::Search {
                    code: crate::ErrorCode::CorruptIndex,
                    ..
                })
            ));
        }
    }

    #[test]
    fn failed_creation_removes_the_claimed_index_directory() {
        let root = temporary_path("failed-create-cleanup");
        let error = create_directory_transaction(&root, |root| {
            std::fs::write(root.join("partial-state"), b"incomplete")
                .map_err(|error| Error::Infrastructure(error.to_string()))?;
            Err::<(), _>(Error::Infrastructure(
                "forced initialization failure".into(),
            ))
        })
        .unwrap_err();

        assert!(
            matches!(error, Error::Infrastructure(message) if message == "forced initialization failure")
        );
        assert!(!root.exists());
    }
}

#[cfg(test)]
mod api_tests {
    use super::*;
    use crate::{field, Analyzer, Facet, Filter, Ranking, SearchBuilder, Sort};
    use searchkernel_ir::{api, IndexContext, OperationIr};
    use searchkernel_testkit::{ExpectedCall, RecordingBackend, ScriptedBackend, ScriptedResult};
    use serde_json::json;
    use std::collections::BTreeMap;
    use std::sync::Arc;

    fn test_index() -> (IndexRuntime, Index, Arc<RecordingBackend>) {
        let backend = Arc::new(RecordingBackend::new());
        let definition = IndexDefinition::new()
            .field(Field::text("title").stored(true))
            .field(Field::float64("price"));
        let runtime =
            IndexRuntime::from_backend(definition, backend.clone()).expect("valid test index");
        let index = runtime.index();
        (runtime, index, backend)
    }

    fn runtime_index(
        definition: IndexDefinition,
        backend: Arc<dyn Backend>,
    ) -> (IndexRuntime, Index) {
        let runtime = IndexRuntime::from_backend(definition, backend).expect("valid test index");
        let index = runtime.index();
        (runtime, index)
    }

    fn context_for(definition: &IndexDefinition) -> Arc<IndexContext> {
        let (_probe_runtime, probe) =
            runtime_index(definition.clone(), Arc::new(RecordingBackend::new()));
        Arc::clone(&probe.inner.context)
    }

    fn expected_get(
        context: &IndexContext,
        id: &str,
        projection: Option<&[String]>,
    ) -> searchkernel_ir::GetRequestIr {
        let bytes = wire::get(id, projection);
        let request = searchkernel_ir::__flatbuffers::root::<api::GetRequest>(&bytes)
            .expect("valid generated get request");
        context.get_to_ir(request).expect("valid get request IR")
    }

    fn expected_search(
        context: &IndexContext,
        request: &SearchBuilder,
    ) -> searchkernel_ir::SearchRequestIr {
        let bytes = wire::search(request).expect("valid generated search request");
        let request = searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&bytes)
            .expect("valid generated search request");
        context
            .search_to_ir(request)
            .expect("valid search request IR")
    }

    fn expected_analyze(
        context: &IndexContext,
        request: &AnalyzeBuilder<'_>,
    ) -> searchkernel_ir::AnalyzeRequestIr {
        let bytes = wire::analyze(request).expect("valid generated analyze request");
        let request = searchkernel_ir::__flatbuffers::root::<api::AnalyzeRequest>(&bytes)
            .expect("valid generated analyze request");
        context
            .analyze_to_ir(request)
            .expect("valid analyze request IR")
    }

    fn scripted_definition() -> IndexDefinition {
        IndexDefinition::new()
            .name("catalog")
            .field(
                Field::text("title")
                    .stored(true)
                    .searchable(true)
                    .analyzer(Analyzer::standard_v1()),
            )
            .field(
                Field::keyword("brand")
                    .stored(true)
                    .filterable(true)
                    .sortable(true)
                    .facetable(true),
            )
            .field(
                Field::keyword("category")
                    .stored(false)
                    .filterable(true)
                    .sortable(true)
                    .facetable(true),
            )
            .field(
                Field::keyword("region")
                    .stored(false)
                    .filterable(true)
                    .sortable(true)
                    .facetable(true),
            )
            .field(
                Field::keyword("channel")
                    .stored(false)
                    .filterable(true)
                    .sortable(true)
                    .facetable(true),
            )
    }

    fn normalized_search(index: &Index) -> SearchBuilder {
        index
            .search("  fox  ")
            .search_fields(["title"])
            .filter(field("brand").eq("acme"))
            .filter(field("category").eq("books"))
            .sort(Sort::asc("brand").missing_last())
            .sort(Sort::desc("category").missing_first())
            .facets([
                Facet::new("brand")
                    .limit(3)
                    .sort(crate::FacetSort::CountDesc),
                Facet::new("category")
                    .limit(4)
                    .sort(crate::FacetSort::CountAsc),
                Facet::new("region")
                    .limit(5)
                    .sort(crate::FacetSort::ValueAsc),
                Facet::new("channel")
                    .limit(6)
                    .sort(crate::FacetSort::ValueDesc),
            ])
            .ranking(Ranking::bm25())
            .limit(7)
            .offset(2)
            .projection(["title", "brand"])
            .highlight(["title"])
            .language("EN-us")
    }

    #[test]
    fn inspect_exposes_normalized_filter_defaults() {
        let definition = IndexDefinition::new()
            .field(Field::text("title"))
            .field(Field::float64("price"));
        let (_runtime, index) = runtime_index(definition, Arc::new(RecordingBackend::new()));

        let inspected = index
            .search("")
            .filter(field("price").range(crate::Range::new().lower(100.0)))
            .inspect()
            .unwrap();
        let Some(crate::Filter(crate::filter::FilterNode::Range { bounds, .. })) = inspected.filter
        else {
            panic!("expected normalized range filter");
        };
        assert_eq!(bounds.include_lower, Some(true));
        assert_eq!(bounds.include_upper, Some(true));
    }

    #[test]
    fn inspect_exposes_canonical_filter_structure() {
        let definition = scripted_definition();
        let (_runtime, index) = runtime_index(definition, Arc::new(RecordingBackend::new()));

        let inspected = index
            .search("")
            .filter(Filter::all([
                field("brand").eq("acme"),
                Filter::all([field("category").eq("books"), field("region").eq("eu")]),
            ]))
            .inspect()
            .unwrap();

        assert_eq!(
            inspected.filter,
            Some(Filter::all([
                field("brand").eq("acme"),
                field("category").eq("books"),
                field("region").eq("eu"),
            ]))
        );
    }

    #[test]
    fn scripted_backend_get_converts_documents_and_projections() {
        let definition = scripted_definition();
        let context = context_for(&definition);
        let projection = vec!["title".to_owned()];
        let document = searchkernel_ir::DocumentResult {
            id: "doc-1".to_owned(),
            fields: BTreeMap::from([
                (
                    "title".to_owned(),
                    searchkernel_ir::Value::String("fox".to_owned()),
                ),
                (
                    "brand".to_owned(),
                    searchkernel_ir::Value::String("acme".to_owned()),
                ),
            ]),
        };
        let backend = Arc::new(ScriptedBackend::new([
            ExpectedCall::success(
                OperationIr::Get(expected_get(&context, "doc-1", None)),
                ScriptedResult::Get(searchkernel_ir::GetResult {
                    document: Some(document),
                }),
            ),
            ExpectedCall::success(
                OperationIr::Get(expected_get(&context, "doc-1", Some(&projection))),
                ScriptedResult::Get(searchkernel_ir::GetResult {
                    document: Some(searchkernel_ir::DocumentResult {
                        id: "doc-1".to_owned(),
                        fields: BTreeMap::from([(
                            "title".to_owned(),
                            searchkernel_ir::Value::String("fox".to_owned()),
                        )]),
                    }),
                }),
            ),
            ExpectedCall::success(
                OperationIr::Get(expected_get(&context, "missing", None)),
                ScriptedResult::Get(searchkernel_ir::GetResult { document: None }),
            ),
        ]));
        let (_runtime, index) = runtime_index(definition, backend.clone());

        let full: Option<serde_json::Value> = index.get("doc-1").unwrap();
        assert_eq!(full, Some(json!({"title": "fox", "brand": "acme"})));
        let projected: Option<serde_json::Value> =
            index.get_with_projection("doc-1", ["title"]).unwrap();
        assert_eq!(projected, Some(json!({"title": "fox"})));
        let missing: Option<serde_json::Value> = index.get("missing").unwrap();
        assert_eq!(missing, None);
        backend.finish().unwrap();
    }

    #[test]
    fn scripted_backend_executes_analyze_and_converts_tokens() {
        let definition = scripted_definition();
        let context = context_for(&definition);
        let (_probe_runtime, probe) =
            runtime_index(definition.clone(), Arc::new(RecordingBackend::new()));
        let probe_request = probe.analyze("Running").field("title").language("EN-us");
        let expected = expected_analyze(&context, &probe_request);
        let backend = Arc::new(ScriptedBackend::new([ExpectedCall::success(
            OperationIr::Analyze(expected),
            ScriptedResult::Analyze(searchkernel_ir::AnalyzeResult {
                tokens: vec![searchkernel_ir::AnalyzedToken {
                    surface: "Running".to_owned(),
                    term: "run".to_owned(),
                    position: 1,
                    start_byte: 0,
                    end_byte: 7,
                }],
            }),
        )]));
        let (_runtime, index) = runtime_index(definition, backend.clone());

        let result = index
            .analyze("Running")
            .field("title")
            .language("EN-us")
            .execute()
            .unwrap();
        assert_eq!(result.tokens[0].term, "run");
        assert_eq!(result.tokens[0].start_byte, 0);
        assert_eq!(result.tokens[0].end_byte, 7);
        backend.finish().unwrap();
    }

    #[test]
    fn browse_executes_match_all_and_describe_returns_schema() {
        let definition = scripted_definition();
        let context = context_for(&definition);
        let (_probe_runtime, probe) =
            runtime_index(definition.clone(), Arc::new(RecordingBackend::new()));
        let expected = expected_search(&context, &probe.browse());
        let backend = Arc::new(ScriptedBackend::new([ExpectedCall::success(
            OperationIr::Search(expected),
            ScriptedResult::Search(searchkernel_ir::SearchResult {
                hits: Vec::new(),
                total_hits: searchkernel_ir::TotalHits {
                    value: 0,
                    relation: searchkernel_ir::TotalHitsRelation::Exact,
                },
                facets: Vec::new(),
            }),
        )]));
        let (_runtime, index) = runtime_index(definition, backend.clone());

        let described = index.describe().unwrap();
        assert_eq!(described.index_name(), Some("catalog"));
        assert_eq!(described.fields().len(), 5);
        assert_eq!(described.fields()[0].name(), "title");
        assert_eq!(described.fields()[1].name(), "brand");

        let result: crate::SearchResult<serde_json::Value> = index.browse().execute().unwrap();
        assert!(result.hits.is_empty());
        assert_eq!(result.total_hits.value, 0);
        backend.finish().unwrap();
    }

    #[test]
    fn search_normalization_covers_explicit_options_and_defaults() {
        let definition = scripted_definition();
        let context = context_for(&definition);
        let (_probe_runtime, probe) =
            runtime_index(definition.clone(), Arc::new(RecordingBackend::new()));
        let expected = expected_search(&context, &normalized_search(&probe));
        let backend = Arc::new(ScriptedBackend::new([ExpectedCall::success(
            OperationIr::Search(expected),
            ScriptedResult::Search(searchkernel_ir::SearchResult {
                hits: Vec::new(),
                total_hits: searchkernel_ir::TotalHits {
                    value: 0,
                    relation: searchkernel_ir::TotalHitsRelation::Exact,
                },
                facets: Vec::new(),
            }),
        )]));
        let (_runtime, index) = runtime_index(definition, backend.clone());

        let inspected = normalized_search(&index).inspect().unwrap();
        assert_eq!(inspected.query, "  fox  ");
        assert_eq!(inspected.search_fields.len(), 1);
        assert_eq!(inspected.search_fields[0].field(), "title");
        assert_eq!(
            inspected.filter,
            Some(Filter::all([
                field("brand").eq("acme"),
                field("category").eq("books"),
            ]))
        );
        assert_eq!(inspected.sorts.len(), 2);
        assert_eq!(inspected.sorts[0].direction, crate::SortDirection::Asc);
        assert_eq!(
            inspected.sorts[0].missing,
            Some(crate::MissingPlacement::Last)
        );
        assert_eq!(inspected.sorts[1].direction, crate::SortDirection::Desc);
        assert_eq!(
            inspected.sorts[1].missing,
            Some(crate::MissingPlacement::First)
        );
        assert_eq!(inspected.facets.len(), 4);
        assert_eq!(inspected.facets[0].limit, Some(3));
        assert_eq!(inspected.facets[1].sort, Some(crate::FacetSort::CountAsc));
        assert_eq!(inspected.facets[2].sort, Some(crate::FacetSort::ValueAsc));
        assert_eq!(inspected.facets[3].sort, Some(crate::FacetSort::ValueDesc));
        assert_eq!(inspected.ranking.kind, crate::search::RankingKind::Bm25);
        assert_eq!(inspected.limit, 7);
        assert_eq!(inspected.offset, 2);
        assert_eq!(inspected.projection, ["title", "brand"]);
        assert_eq!(inspected.highlighting, Some(vec!["title".to_owned()]));
        assert_eq!(inspected.language.as_deref(), Some("en-us"));

        let result: crate::SearchResult<serde_json::Value> =
            normalized_search(&index).execute().unwrap();
        assert!(result.hits.is_empty());
        backend.finish().unwrap();

        let single_filter = index
            .search("fox")
            .filter(field("brand").eq("acme"))
            .inspect()
            .unwrap();
        assert_eq!(single_filter.query, "fox");
        assert_eq!(single_filter.filter, Some(field("brand").eq("acme")));

        let defaults = index.browse().inspect().unwrap();
        assert_eq!(defaults.query, "");
        assert!(defaults.filter.is_none());
        assert!(defaults.sorts.is_empty());
        assert!(defaults.facets.is_empty());
        assert_eq!(
            defaults.ranking.kind,
            crate::search::RankingKind::DefaultRelevanceV1
        );
        assert_eq!(defaults.limit, 20);
        assert_eq!(defaults.offset, 0);
        assert_eq!(defaults.projection, ["title", "brand"]);
        assert!(defaults.highlighting.is_none());
        assert!(defaults.language.is_none());
    }

    #[test]
    fn scripted_backend_errors_preserve_code_message_and_details() {
        let definition = scripted_definition();
        let context = context_for(&definition);
        let (_probe_runtime, probe) =
            runtime_index(definition.clone(), Arc::new(RecordingBackend::new()));
        let mut backend_error = searchkernel_ir::PortableError::new(
            searchkernel_ir::ErrorCode::Unavailable,
            "backend unavailable",
        )
        .with_detail("backend_timeout", Some("search"), "request timed out");
        backend_error.retryable = true;
        let backend = Arc::new(ScriptedBackend::new([ExpectedCall::error(
            OperationIr::Search(expected_search(&context, &probe.search("fox"))),
            backend_error,
        )]));
        let (_runtime, index) = runtime_index(definition, backend.clone());

        let error = index
            .search("fox")
            .execute::<serde_json::Value>()
            .expect_err("scripted backend should fail the search");
        assert_eq!(error.code(), crate::ErrorCode::Unavailable);
        assert!(error.retryable());
        assert_eq!(error.details()[0].code, "backend_timeout");
        assert_eq!(error.details()[0].path.as_deref(), Some("search"));
        assert_eq!(error.details()[0].message, "request timed out");
        assert_eq!(error.to_string(), "backend unavailable");
        backend.finish().unwrap();
    }

    #[test]
    fn invalid_values_do_not_reach_the_backend() {
        let (_runtime, index, backend) = test_index();

        let error = index
            .insert("nan", &f64::NAN)
            .expect_err("non-finite JSON values must be rejected");
        assert!(
            matches!(error, Error::Serialization(_)),
            "unexpected error: {error}"
        );
        assert!(backend.calls().is_empty());

        let error = index
            .patch("nan", Patch::new().set("price", f64::NAN))
            .expect_err("invalid patch values must be rejected");
        assert_eq!(error.code(), crate::ErrorCode::InvalidArgument);
        assert!(backend.calls().is_empty());
    }

    #[test]
    fn backend_errors_are_returned_without_being_rewritten() {
        let (_runtime, index, backend) = test_index();
        let error = index
            .insert("doc-1", &json!({"title": "fox"}))
            .expect_err("recording backend intentionally rejects writes");
        assert_eq!(error.code(), crate::ErrorCode::Unsupported);
        assert_eq!(backend.calls().len(), 1);

        let error = index
            .search("fox")
            .limit(5)
            .execute::<serde_json::Value>()
            .expect_err("recording backend intentionally rejects searches");
        assert_eq!(error.code(), crate::ErrorCode::Unsupported);
        assert_eq!(backend.calls().len(), 2);
    }
}
