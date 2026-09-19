use crate::analyze::{to_resolver_input, AnalyzeBuilder, AnalyzeResult, AnalyzedToken};
use crate::document::{serialize, DocumentId};
use crate::error::{Error, Result};
use crate::patch::{to_resolver_operations, Patch};
#[cfg(feature = "native")]
use crate::portable_definition::encode_portable_definition;
use crate::result;
use crate::schema::{Analyzer, Field, IndexDefinition, SearchField};
use crate::search::{
    to_resolver_input as search_to_resolver_input, Facet, FacetSort, Ranking, SearchBuilder, Sort,
};
#[cfg(feature = "internal-api")]
use searchkernel_ir::api;
use searchkernel_ir::{Backend, BackendSession, IndexContext};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::collections::BTreeMap;
use std::sync::Arc;

#[cfg(test)]
#[path = "api_tests.rs"]
mod api_tests;
#[cfg(all(test, feature = "native"))]
#[path = "tests.rs"]
mod tests;

#[cfg(feature = "native")]
use std::io::Write;

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

#[cfg(all(feature = "native", test))]
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

#[cfg(feature = "native")]
fn create_staging_directory(root: &std::path::Path) -> Result<std::path::PathBuf> {
    if root.exists() {
        return Err(Error::already_exists(root.display().to_string()));
    }
    let parent = root.parent().unwrap_or_else(|| std::path::Path::new("."));
    std::fs::create_dir_all(parent).map_err(|error| Error::Infrastructure(error.to_string()))?;
    let name = root
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| Error::infrastructure("index path has an invalid final component"))?;
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| Error::infrastructure(error.to_string()))?
        .as_nanos();
    let staging = parent.join(format!(
        ".{name}.searchkernel-creating-{}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir(&staging).map_err(|error| {
        if error.kind() == std::io::ErrorKind::AlreadyExists {
            Error::already_exists(root.display().to_string())
        } else {
            Error::Infrastructure(error.to_string())
        }
    })?;
    Ok(staging)
}

#[cfg(feature = "native")]
fn publish_staging_directory(staging: &std::path::Path, root: &std::path::Path) -> Result<()> {
    std::fs::rename(staging, root).map_err(|error| {
        if error.kind() == std::io::ErrorKind::AlreadyExists {
            Error::already_exists(root.display().to_string())
        } else {
            Error::Infrastructure(error.to_string())
        }
    })?;
    let parent = root.parent().unwrap_or_else(|| std::path::Path::new("."));
    // The rename is the publication point. A failure to sync the parent is a
    // durability warning, not a failed create: the index is already visible
    // at `root` and must not be reported as though publication was rolled
    // back.
    let _ = sync_published_parent(parent);
    Ok(())
}

#[cfg(feature = "native")]
fn sync_published_parent(parent: &std::path::Path) -> std::io::Result<()> {
    #[cfg(test)]
    if FAIL_NEXT_PARENT_FSYNC.with(|flag| flag.replace(false)) {
        return Err(std::io::Error::other("injected parent fsync failure"));
    }
    std::fs::File::open(parent).and_then(|directory| directory.sync_all())
}

#[cfg(feature = "native")]
fn remove_published_root(root: &std::path::Path) -> std::io::Result<()> {
    #[cfg(test)]
    if FAIL_NEXT_PUBLISHED_CLEANUP.with(|flag| flag.replace(false)) {
        return Err(std::io::Error::other(
            "injected published-root cleanup failure",
        ));
    }
    std::fs::remove_dir_all(root)
}

#[cfg(feature = "native")]
fn open_native_backend(
    data: &std::path::Path,
    config: &std::path::Path,
) -> std::result::Result<searchkernel_ir_adapter::VespaBackend, searchkernel_ir::PortableError> {
    #[cfg(test)]
    if FAIL_NEXT_BACKEND_OPEN.with(|flag| flag.replace(false)) {
        return Err(searchkernel_ir::PortableError::new(
            searchkernel_ir::ErrorCode::Unavailable,
            "injected backend open failure",
        ));
    }
    searchkernel_ir_adapter::VespaBackend::open(data, config)
}

#[cfg(all(feature = "native", test))]
thread_local! {
    static FAIL_NEXT_PARENT_FSYNC: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FAIL_NEXT_PUBLISHED_CLEANUP: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FAIL_NEXT_BACKEND_OPEN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(feature = "native")]
struct LifecycleLock {
    _file: std::fs::File,
}

#[cfg(feature = "native")]
impl LifecycleLock {
    fn acquire(root: &std::path::Path) -> Result<Self> {
        use std::os::fd::AsRawFd;

        let parent = root.parent().unwrap_or_else(|| std::path::Path::new("."));
        std::fs::create_dir_all(parent)
            .map_err(|error| Error::Infrastructure(error.to_string()))?;
        let name = root
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| Error::infrastructure("index path has an invalid final component"))?;
        let path = parent.join(format!(".{name}.searchkernel.lock"));
        let file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&path)
            .map_err(|error| Error::Infrastructure(error.to_string()))?;
        // This lock covers the interval in which a complete index is being
        // assembled and published, when its final data directory is absent.
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            let error = std::io::Error::last_os_error();
            if matches!(error.raw_os_error(), Some(code) if code == libc::EWOULDBLOCK || code == libc::EAGAIN)
            {
                return Err(Error::already_exists(root.display().to_string()));
            }
            return Err(Error::Infrastructure(format!(
                "could not lock index lifecycle path '{}': {error}",
                path.display()
            )));
        }
        Ok(Self { _file: file })
    }
}

struct IndexInner {
    context: Arc<IndexContext>,
    definition: Arc<IndexDefinition>,
    session: Arc<BackendSession>,
    #[cfg(feature = "native")]
    lifecycle_lock: std::sync::Mutex<Option<LifecycleLock>>,
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
    #[cfg(all(feature = "native", feature = "internal-api"))]
    pub fn create_from_definition_bytes(
        path: impl AsRef<std::path::Path>,
        bytes: &[u8],
    ) -> Result<Self> {
        let root = searchkernel_ir::__flatbuffers::root::<api::IndexDefinition>(bytes)
            .map_err(|error| Error::Serialization(format!("invalid index definition: {error}")))?;
        let ir = searchkernel_ir::index_definition_to_ir(root).map_err(Error::from_ir)?;
        searchkernel_ir_adapter::validate_index_definition(&ir).map_err(Error::from_ir)?;
        let resolved_definition = definition_from_ir(&ir);
        Self::create_with_definitions(path.as_ref(), bytes.to_vec(), ir, resolved_definition)
    }

    /// Create a new on-disk index and persist all information required to
    /// reopen it safely.
    #[cfg(feature = "native")]
    pub fn create(path: impl AsRef<std::path::Path>, definition: IndexDefinition) -> Result<Self> {
        let root = path.as_ref();
        let portable_definition_bytes = encode_portable_definition(&definition);
        let resolved_ir = definition_to_ir(&definition)?;
        searchkernel_ir_adapter::validate_index_definition(&resolved_ir).map_err(Error::from_ir)?;
        let resolved_definition = definition_from_ir(&resolved_ir);
        Self::create_with_definitions(
            root,
            portable_definition_bytes,
            resolved_ir,
            resolved_definition,
        )
    }

    #[cfg(feature = "native")]
    fn create_with_definitions(
        root: &std::path::Path,
        portable_definition_bytes: Vec<u8>,
        resolved_ir: searchkernel_ir::IndexDefinitionIr,
        resolved_definition: IndexDefinition,
    ) -> Result<Self> {
        // Create and open can arrive in either order. If an opener briefly
        // owns the lock while observing an absent root, retrying here avoids
        // turning that harmless ordering into an AlreadyExists failure.
        let lifecycle_lock = loop {
            match LifecycleLock::acquire(root) {
                Ok(lock) => break lock,
                Err(error)
                    if error.code() == crate::error::ErrorCode::AlreadyExists && !root.is_dir() =>
                {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(error) => return Err(error),
            }
        };
        let staging = create_staging_directory(root)?;
        let initialize = (|| {
            let staging_data = staging.join("data");
            let staging_config = staging.join("config");
            std::fs::create_dir(&staging_data)
                .map_err(|error| Error::Infrastructure(error.to_string()))?;
            searchkernel_ir_adapter::compile_index_definition(&resolved_ir, &staging_config)
                .map_err(Error::from_ir)?;
            let manifest = IndexManifest::for_definitions(
                &portable_definition_bytes,
                &resolved_definition,
                &staging_config,
            )?;
            manifest.write(&staging)?;
            Ok::<(), Error>(())
        })();
        if let Err(error) = initialize {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(error);
        }
        // Publish the complete on-disk description before opening Vespa. The
        // native backend retains its data-directory path for asynchronous
        // flushes, so it must be opened at the final path rather than at the
        // temporary staging path.
        if let Err(error) = publish_staging_directory(&staging, root) {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(error);
        }
        let backend = match open_native_backend(&root.join("data"), &root.join("config")) {
            Ok(backend) => backend,
            Err(error) => {
                return match remove_published_root(root) {
                    Ok(()) => Err(Error::Infrastructure(error.to_string())),
                    Err(cleanup_error) => Err(Error::search(
                        crate::ErrorCode::Unavailable,
                        format!("index was published but backend opening failed: {error}; cleanup failed: {cleanup_error}"),
                    )
                    .with_detail(
                        "index_published",
                        Some(root.display().to_string()),
                        "published index remains available for recovery",
                    )),
                };
            }
        };
        Self::from_owned_backend(
            resolved_definition,
            BackendSession::new(Box::new(backend)),
            Some(lifecycle_lock),
        )
    }

    /// Reopen an existing index using its durable manifest.
    #[cfg(feature = "native")]
    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self> {
        let root = path.as_ref();
        // A creator holds the lifecycle lock while it assembles the staging
        // tree, before atomically publishing `root`. An opener that arrives in
        // that interval must wait for publication; reporting AlreadyExists
        // would turn a successful create/open race into two failures.
        let lifecycle_lock = loop {
            match LifecycleLock::acquire(root) {
                Ok(lock) => break lock,
                Err(error)
                    if error.code() == crate::error::ErrorCode::AlreadyExists && !root.is_dir() =>
                {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(error) => return Err(error),
            }
        };
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
        Self::from_owned_backend(
            definition,
            BackendSession::new(Box::new(backend)),
            Some(lifecycle_lock),
        )
    }

    /// Construct an index around a backend for tests and infrastructure.
    ///
    /// This is hidden from normal API documentation because production
    /// lifecycle is supplied by a later integration milestone.
    #[doc(hidden)]
    #[cfg(feature = "native")]
    pub fn from_backend(definition: IndexDefinition, backend: Arc<dyn Backend>) -> Result<Self> {
        Self::from_owned_backend(definition, BackendSession::from_backend(backend), None)
    }

    /// Construct an index around a backend for tests and infrastructure.
    #[doc(hidden)]
    #[cfg(not(feature = "native"))]
    pub fn from_backend(definition: IndexDefinition, backend: Arc<dyn Backend>) -> Result<Self> {
        Self::from_owned_backend(definition, BackendSession::from_backend(backend))
    }

    #[cfg(feature = "native")]
    fn from_owned_backend(
        definition: IndexDefinition,
        session: Arc<BackendSession>,
        lifecycle_lock: Option<LifecycleLock>,
    ) -> Result<Self> {
        let resolved = definition_to_ir(&definition)?;
        let context = IndexContext::from_definition_ir(resolved);
        let effective_definition = definition_from_ir(context.definition());
        let context = Arc::new(context);
        let definition = Arc::new(effective_definition);
        Ok(Self {
            inner: Arc::new(IndexInner {
                session,
                context,
                definition,
                lifecycle_lock: std::sync::Mutex::new(lifecycle_lock),
            }),
        })
    }

    #[cfg(not(feature = "native"))]
    fn from_owned_backend(
        definition: IndexDefinition,
        session: Arc<BackendSession>,
    ) -> Result<Self> {
        let resolved = definition_to_ir(&definition)?;
        let context = IndexContext::from_definition_ir(resolved);
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
        let result = self.inner.session.close().map_err(Error::from_ir);
        #[cfg(feature = "native")]
        {
            // Release the publication lock only after the backend has drained
            // and shut down. All cloned handles share this same mutex.
            let _ = self
                .inner
                .lifecycle_lock
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .take();
        }
        result
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
        self.search("")
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
            let ir_request = searchkernel_ir::resolve_search(
                &self.inner.context,
                search_to_resolver_input(request)?,
            )
            .map_err(Error::from_ir)?;
            let backend_result = backend.search(ir_request).map_err(Error::from_ir)?;
            let requested_facets = request
                .facets
                .iter()
                .map(|facet| facet.field.as_str())
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
    Some(Facet::new(field).limit(limit).sort(sort))
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

#[cfg(feature = "native")]
#[derive(Debug, Clone)]
struct IndexManifest {
    value: serde_json::Value,
    portable_definition_bytes: Vec<u8>,
    resolved_definition_bytes: Vec<u8>,
}

#[cfg(feature = "native")]
fn atomic_write(path: &std::path::Path, bytes: &[u8]) -> Result<()> {
    let directory = path
        .parent()
        .ok_or_else(|| Error::infrastructure("manifest path has no parent directory"))?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| Error::infrastructure("manifest path has an invalid file name"))?;
    let temporary = directory.join(format!(".{file_name}.tmp"));

    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&temporary)
        .map_err(|error| Error::Infrastructure(error.to_string()))?;
    file.write_all(bytes)
        .map_err(|error| Error::Infrastructure(error.to_string()))?;
    file.sync_all()
        .map_err(|error| Error::Infrastructure(error.to_string()))?;
    drop(file);

    std::fs::rename(&temporary, path).map_err(|error| Error::Infrastructure(error.to_string()))?;
    std::fs::File::open(directory)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| Error::Infrastructure(error.to_string()))
}

#[cfg(feature = "native")]
impl IndexManifest {
    fn for_definitions(
        portable_definition_bytes: &[u8],
        resolved_definition: &IndexDefinition,
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
                "portable_index_definition": "portable-index-definition.bfbs",
                "canonical_index_definition": "index-definition.bfbs",
                "resolved_engine_config_identity": config_identity(config)?,
            }),
            portable_definition_bytes: portable_definition_bytes.to_vec(),
            resolved_definition_bytes: searchkernel_ir::index_definition_ir_to_api(
                &definition_to_ir(resolved_definition)?,
            ),
        })
    }

    fn write(&self, root: &std::path::Path) -> Result<()> {
        let directory = root.join("manifest");
        std::fs::create_dir_all(&directory)
            .map_err(|error| Error::Infrastructure(error.to_string()))?;
        let bytes = serde_json::to_vec_pretty(&self.value)
            .map_err(|error| Error::Infrastructure(error.to_string()))?;
        atomic_write(
            &directory.join("index-definition.bfbs"),
            &self.resolved_definition_bytes,
        )?;
        atomic_write(
            &directory.join("portable-index-definition.bfbs"),
            &self.portable_definition_bytes,
        )?;
        atomic_write(&directory.join("index.json"), &bytes)
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
        let (resolved_definition_bytes, portable_definition_bytes) =
            if version == Some(u64::from(MANIFEST_FORMAT_VERSION)) {
                let read_definition = |name: &str| {
                    let path = root.join("manifest").join(name);
                    std::fs::read(&path).map_err(|error| {
                        if error.kind() == std::io::ErrorKind::NotFound {
                            Error::corrupt_index(path.display().to_string())
                        } else {
                            Error::Infrastructure(error.to_string())
                        }
                    })
                };
                (
                    read_definition("index-definition.bfbs")?,
                    read_definition("portable-index-definition.bfbs")?,
                )
            } else {
                (Vec::new(), Vec::new())
            };
        let manifest = Self {
            value,
            portable_definition_bytes,
            resolved_definition_bytes,
        };
        manifest.validate_identity()?;
        if version == Some(u64::from(MANIFEST_FORMAT_VERSION)) {
            // The portable definition is provenance. Its raw FlatBuffers
            // envelope must remain readable, but its semantics were resolved
            // under the creator's defaults and must not be reinterpreted on
            // reopen.
            let _portable_root = searchkernel_ir::__flatbuffers::root::<api::IndexDefinition>(
                &manifest.portable_definition_bytes,
            )
            .map_err(|error| {
                Error::corrupt_index(format!("invalid portable index definition: {error}"))
            })?;
            let resolved_root = searchkernel_ir::__flatbuffers::root::<api::IndexDefinition>(
                &manifest.resolved_definition_bytes,
            )
            .map_err(|error| {
                Error::corrupt_index(format!("invalid resolved index definition: {error}"))
            })?;
            let resolved =
                searchkernel_ir::index_definition_to_ir(resolved_root).map_err(Error::from_ir)?;
            let _ = resolved;
        }
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
                if object
                    .get("portable_index_definition")
                    .and_then(serde_json::Value::as_str)
                    != Some("portable-index-definition.bfbs")
                    || object
                        .get("canonical_index_definition")
                        .and_then(serde_json::Value::as_str)
                        != Some("index-definition.bfbs")
                {
                    return Err(Error::corrupt_index(
                        "manifest index definition references are invalid",
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
                &self.resolved_definition_bytes,
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

fn definition_to_ir(definition: &IndexDefinition) -> Result<searchkernel_ir::IndexDefinitionIr> {
    let fields = definition
        .fields
        .iter()
        .map(|field| {
            let analyzer = field
                .analyzer
                .as_ref()
                .map(|analyzer| {
                    analyzer
                        .options
                        .iter()
                        .map(|(key, value)| {
                            searchkernel_ir::request_value_from_json(
                                value,
                                format!("fields.{}.analyzer.options.{key}", field.name),
                            )
                            .map(|value| (key.clone(), value))
                            .map_err(Error::from_ir)
                        })
                        .collect::<Result<searchkernel_ir::SchemaPropertiesIr>>()
                        .map(|options| searchkernel_ir::RustAnalyzerInput {
                            name: analyzer.name.clone(),
                            version: analyzer.version,
                            options,
                        })
                })
                .transpose()?;
            Ok(searchkernel_ir::RustFieldInput {
                name: field.name.clone(),
                kind: match field.kind {
                    crate::schema::FieldKind::Text => searchkernel_ir::RustFieldKind::Text,
                    crate::schema::FieldKind::Keyword => searchkernel_ir::RustFieldKind::Keyword,
                    crate::schema::FieldKind::Int64 => searchkernel_ir::RustFieldKind::Int64,
                    crate::schema::FieldKind::Float64 => searchkernel_ir::RustFieldKind::Float64,
                    crate::schema::FieldKind::Bool => searchkernel_ir::RustFieldKind::Bool,
                },
                multi: field.multi,
                stored: field.stored,
                searchable: field.searchable,
                filterable: field.filterable,
                sortable: field.sortable,
                facetable: field.facetable,
                analyzer,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let default_search_fields = definition.default_search_fields.as_ref().map(|fields| {
        fields
            .iter()
            .map(|field| searchkernel_ir::RustSearchFieldInput {
                field: field.field.clone(),
                weight: field.weight,
            })
            .collect()
    });
    searchkernel_ir::resolve_rust_index_definition(searchkernel_ir::RustIndexDefinitionInput {
        name: definition.name.clone(),
        fields,
        default_search_fields,
    })
    .map_err(Error::from_ir)
}

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
        if field.analyzer().is_some_and(|analyzer| {
            analyzer.name == "standard" && analyzer.version == 1 && analyzer.options.is_empty()
        }) {
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
            Some("text" | "Text") => Field::text(name),
            Some("keyword" | "Keyword") => Field::keyword(name),
            Some("int64" | "Int64") => Field::int64(name),
            Some("float64" | "Float64") => Field::float64(name),
            Some("bool" | "Bool") => Field::bool(name),
            _ => return Err(Error::corrupt_index("manifest field kind is invalid")),
        };
        let mut field = kind;
        if let Some(analyzer) = metadata.get("analyzer") {
            if !analyzer.is_null() {
                if let Some(analyzer) = analyzer.as_str() {
                    field = match analyzer {
                        "standard-v1" | "StandardV1" => field.analyzer(Analyzer::standard_v1()),
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
