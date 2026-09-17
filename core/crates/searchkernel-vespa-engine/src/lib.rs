#![deny(unsafe_op_in_unsafe_fn)]
//! Safe Vespa-specific adapter for the private Search Kernel ABI.

mod document;
mod linguistics;
mod query;
mod schema;

pub use document::{
    ArrayValue, Document, DocumentUpdate, FieldUpdate, FieldValue, IntoFieldValue, IntoScalarValue,
    ScalarValue,
};
pub use linguistics::{
    DocumentAnalysis, DocumentTerm, Language, Linguistics, LinguisticsOptions, QueryAnalysis,
    QueryTerm, StemMode,
};
pub use query::{AttributeBound, Query, RangeBound, Value};
pub use schema::{ConfigCompiler, Field, FieldType, RankProfile, Schema};

#[cfg(feature = "hardening")]
pub mod hardening {
    use super::{check, cstring, Engine, NativeError, Result};
    use searchkernel_vespa_bindings as sys;

    #[derive(Debug, Clone, Copy)]
    pub enum Action {
        Error,
        StdException,
        BadAlloc,
        UnknownException,
        Block,
    }

    impl Action {
        fn native(self) -> u32 {
            match self {
                Self::Error => sys::hardening::SK_TEST_FAILPOINT_ERROR,
                Self::StdException => sys::hardening::SK_TEST_FAILPOINT_STD_EXCEPTION,
                Self::BadAlloc => sys::hardening::SK_TEST_FAILPOINT_BAD_ALLOC,
                Self::UnknownException => sys::hardening::SK_TEST_FAILPOINT_UNKNOWN_EXCEPTION,
                Self::Block => sys::hardening::SK_TEST_FAILPOINT_BLOCK,
            }
        }
    }

    #[derive(Debug, Clone, Copy)]
    #[repr(u32)]
    pub enum ResultFault {
        NullResult = sys::hardening::SK_TEST_RESULT_FAULT_NULL_RESULT,
        NullDocumentFieldName = sys::hardening::SK_TEST_RESULT_FAULT_NULL_DOCUMENT_FIELD_NAME,
        InvalidDocumentFieldName = sys::hardening::SK_TEST_RESULT_FAULT_INVALID_DOCUMENT_FIELD_NAME,
        NullDocumentString = sys::hardening::SK_TEST_RESULT_FAULT_NULL_DOCUMENT_STRING,
        InvalidDocumentString = sys::hardening::SK_TEST_RESULT_FAULT_INVALID_DOCUMENT_STRING,
        UnknownDocumentKind = sys::hardening::SK_TEST_RESULT_FAULT_UNKNOWN_DOCUMENT_KIND,
        DocumentAccessorError = sys::hardening::SK_TEST_RESULT_FAULT_DOCUMENT_ACCESSOR_ERROR,
        NullHitId = sys::hardening::SK_TEST_RESULT_FAULT_NULL_HIT_ID,
        InvalidHitId = sys::hardening::SK_TEST_RESULT_FAULT_INVALID_HIT_ID,
        NullSummaryName = sys::hardening::SK_TEST_RESULT_FAULT_NULL_SUMMARY_NAME,
        InvalidSummaryName = sys::hardening::SK_TEST_RESULT_FAULT_INVALID_SUMMARY_NAME,
        NullSummaryString = sys::hardening::SK_TEST_RESULT_FAULT_NULL_SUMMARY_STRING,
        InvalidSummaryString = sys::hardening::SK_TEST_RESULT_FAULT_INVALID_SUMMARY_STRING,
        UnknownSummaryKind = sys::hardening::SK_TEST_RESULT_FAULT_UNKNOWN_SUMMARY_KIND,
        NullMatchFeatureName = sys::hardening::SK_TEST_RESULT_FAULT_NULL_MATCH_FEATURE_NAME,
        InvalidMatchFeatureName = sys::hardening::SK_TEST_RESULT_FAULT_INVALID_MATCH_FEATURE_NAME,
        DuplicateMatchFeatureName =
            sys::hardening::SK_TEST_RESULT_FAULT_DUPLICATE_MATCH_FEATURE_NAME,
        MatchFeatureValueError = sys::hardening::SK_TEST_RESULT_FAULT_MATCH_FEATURE_VALUE_ERROR,
        UnsupportedGroupKey = sys::hardening::SK_TEST_RESULT_FAULT_UNSUPPORTED_GROUP_KEY,
        InvalidGroupKey = sys::hardening::SK_TEST_RESULT_FAULT_INVALID_GROUP_KEY,
        GroupAccessorError = sys::hardening::SK_TEST_RESULT_FAULT_GROUP_ACCESSOR_ERROR,
        UnknownAggregationKind = sys::hardening::SK_TEST_RESULT_FAULT_UNKNOWN_AGGREGATION_KIND,
        AggregationAccessorError = sys::hardening::SK_TEST_RESULT_FAULT_AGGREGATION_ACCESSOR_ERROR,
    }

    pub fn set_result_fault(engine: &Engine, fault: ResultFault) -> Result<()> {
        let mut error = NativeError::new();
        let status = unsafe {
            // SAFETY: Engine owns a live opaque handle and the temporary
            // diagnostic buffer remains valid for this call.
            sys::hardening::sk_test_result_fault_set(
                engine.handle.as_ptr(),
                fault as u32,
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)
    }

    pub fn clear_result_fault(engine: &Engine) -> Result<()> {
        let mut error = NativeError::new();
        let status = unsafe {
            // SAFETY: Engine owns a live opaque handle and the temporary
            // diagnostic buffer remains valid for this call.
            sys::hardening::sk_test_result_fault_clear(
                engine.handle.as_ptr(),
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)
    }

    pub fn result_destroy_count(engine: &Engine) -> Result<u64> {
        let mut count = 0;
        let mut error = NativeError::new();
        let status = unsafe {
            // SAFETY: Engine owns a live opaque handle and the output and
            // diagnostic buffers remain valid for this call.
            sys::hardening::sk_test_result_destroy_count(
                engine.handle.as_ptr(),
                &mut count,
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)?;
        Ok(count)
    }

    pub fn set(engine: &Engine, name: &str, hit: u32, action: Action) -> Result<()> {
        let name = cstring(name)?;
        let mut error = NativeError::new();
        let status = unsafe {
            // SAFETY: Engine owns a live opaque handle and the temporary C
            // string/error buffer remain valid for this call only.
            sys::hardening::sk_test_failpoint_set(
                engine.handle.as_ptr(),
                name.as_ptr(),
                hit,
                action.native(),
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)
    }

    pub fn set_open(name: &str, hit: u32, action: Action) -> Result<()> {
        let name = cstring(name)?;
        let mut error = NativeError::new();
        let status = unsafe {
            // SAFETY: the temporary C string/error buffer remain valid for
            // the duration of this test-control call.
            sys::hardening::sk_test_failpoint_set_open(
                name.as_ptr(),
                hit,
                action.native(),
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)
    }

    pub fn clear(engine: &Engine, name: &str) -> Result<()> {
        let name = cstring(name)?;
        let mut error = NativeError::new();
        let status = unsafe {
            // SAFETY: Engine owns a live opaque handle and the temporary C
            // string/error buffer remain valid for this call only.
            sys::hardening::sk_test_failpoint_clear(
                engine.handle.as_ptr(),
                name.as_ptr(),
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)
    }

    pub fn clear_open(name: &str) -> Result<()> {
        let name = cstring(name)?;
        let mut error = NativeError::new();
        let status = unsafe {
            // SAFETY: the temporary C string/error buffer remain valid for
            // the duration of this test-control call.
            sys::hardening::sk_test_failpoint_clear_open(
                name.as_ptr(),
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)
    }

    pub fn release(engine: &Engine, name: &str) -> Result<()> {
        let name = cstring(name)?;
        let mut error = NativeError::new();
        let status = unsafe {
            // SAFETY: Engine owns a live opaque handle and the temporary C
            // string/error buffer remain valid for this call only.
            sys::hardening::sk_test_failpoint_release(
                engine.handle.as_ptr(),
                name.as_ptr(),
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)
    }

    pub fn is_blocked(engine: &Engine, name: &str) -> Result<bool> {
        let name = cstring(name)?;
        let mut blocked = 0;
        let mut error = NativeError::new();
        let status = unsafe {
            // SAFETY: Engine owns a live opaque handle and the temporary C
            // string/error buffer remain valid for this call only.
            sys::hardening::sk_test_failpoint_is_blocked(
                engine.handle.as_ptr(),
                name.as_ptr(),
                &mut blocked,
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)?;
        Ok(blocked != 0)
    }

    #[cfg(test)]
    mod tests {
        use super::{sys, Action};

        #[test]
        fn actions_use_the_stable_native_failpoint_codes() {
            let actions = [
                (Action::Error, sys::hardening::SK_TEST_FAILPOINT_ERROR),
                (
                    Action::StdException,
                    sys::hardening::SK_TEST_FAILPOINT_STD_EXCEPTION,
                ),
                (
                    Action::BadAlloc,
                    sys::hardening::SK_TEST_FAILPOINT_BAD_ALLOC,
                ),
                (
                    Action::UnknownException,
                    sys::hardening::SK_TEST_FAILPOINT_UNKNOWN_EXCEPTION,
                ),
                (Action::Block, sys::hardening::SK_TEST_FAILPOINT_BLOCK),
            ];

            for (action, expected) in actions {
                assert_eq!(action.native(), expected);
            }
        }
    }
}

use searchkernel_vespa_bindings as sys;
use std::collections::BTreeMap;
use std::ffi::{CString, NulError};
use std::fs;
use std::path::Path;
use std::ptr::{self, NonNull};
use std::time::Duration;

const EXPECTED_ABI: u32 = sys::EXPECTED_ABI;
const RESERVED_RANK_PROPERTIES: &[&str] = &["vespa.matching.numthreadspersearch"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortOrder {
    Ascending,
    Descending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissingPlacement {
    Default,
    First,
    Last,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SortField {
    pub field: String,
    pub order: SortOrder,
    pub missing: MissingPlacement,
}

impl SortField {
    pub fn new(field: impl Into<String>, order: SortOrder) -> Self {
        Self {
            field: field.into(),
            order,
            missing: MissingPlacement::Default,
        }
    }

    pub fn ascending(field: impl Into<String>) -> Self {
        Self::new(field, SortOrder::Ascending)
    }

    pub fn descending(field: impl Into<String>) -> Self {
        Self::new(field, SortOrder::Descending)
    }

    pub fn missing_first(mut self) -> Self {
        self.missing = MissingPlacement::First;
        self
    }

    pub fn missing_last(mut self) -> Self {
        self.missing = MissingPlacement::Last;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SummaryRequest {
    None,
    Default,
    Fields(Vec<String>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// The document source selected by a search request.
pub enum Source {
    /// Search all document sources configured by the engine.
    All,
    /// Search one configured document source by name.
    Named(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TotalHitsRelation {
    Exact,
    LowerBound,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    pub document_id: String,
    pub score: f64,
    pub fields: BTreeMap<String, FieldValue>,
    pub match_features: BTreeMap<String, f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SearchResult {
    pub total_hits: u64,
    pub total_hits_relation: TotalHitsRelation,
    pub hits: Vec<Hit>,
    pub facets: Vec<GroupingResult>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SearchRequest {
    pub query: Query,
    pub source: Source,
    pub rank_profile: String,
    pub query_features: Vec<(String, f64)>,
    pub rank_properties: Vec<(String, String)>,
    pub limit: u32,
    pub offset: u32,
    pub sort: Vec<SortField>,
    pub summary: SummaryRequest,
    pub language: LinguisticsOptions,
    pub facets: Vec<FacetRequest>,
    pub timeout: Duration,
}

impl SearchRequest {
    pub fn new(query: Query) -> Self {
        Self {
            query,
            source: Source::All,
            rank_profile: "default".to_owned(),
            query_features: Vec::new(),
            rank_properties: Vec::new(),
            limit: 10,
            offset: 0,
            sort: Vec::new(),
            summary: SummaryRequest::None,
            language: LinguisticsOptions::default(),
            facets: Vec::new(),
            timeout: Duration::from_secs(60),
        }
    }

    /// Parse YQL while retaining its document-source selection.
    pub fn from_yql(yql: &str) -> Result<Self, Error> {
        let parsed = query::parse_yql(yql)?;
        query::validate_query(&parsed.query)?;
        let source = if parsed.source == "*" {
            Source::All
        } else {
            Source::Named(parsed.source)
        };
        Ok(Self::new(parsed.query).source(source))
    }

    pub fn source(mut self, source: Source) -> Self {
        self.source = source;
        self
    }

    /// Limit the time the native engine may spend executing this search.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn rank_profile(mut self, rank_profile: impl Into<String>) -> Self {
        self.rank_profile = rank_profile.into();
        self
    }

    pub fn query_feature(mut self, name: impl Into<String>, value: f64) -> Self {
        self.query_features.push((name.into(), value));
        self
    }

    pub fn rank_property(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.rank_properties.push((name.into(), value.into()));
        self
    }

    pub fn limit(mut self, limit: u32) -> Self {
        self.limit = limit;
        self
    }

    pub fn offset(mut self, offset: u32) -> Self {
        self.offset = offset;
        self
    }

    pub fn sort(mut self, sort: SortField) -> Self {
        self.sort.push(sort);
        self
    }

    pub fn fields<I, S>(mut self, fields: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.summary = SummaryRequest::Fields(fields.into_iter().map(Into::into).collect());
        self
    }

    pub fn summary(mut self, summary: SummaryRequest) -> Self {
        self.summary = summary;
        self
    }

    pub fn facet(mut self, facet: FacetRequest) -> Self {
        self.facets.push(facet);
        self
    }

    pub fn language(mut self, language: LinguisticsOptions) -> Self {
        self.language = language;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FacetRequest {
    pub field: String,
    pub max_groups: u32,
    pub order: GroupingOrder,
}

impl FacetRequest {
    pub fn new(field: impl Into<String>, max_groups: u32, order: GroupingOrder) -> Self {
        Self {
            field: field.into(),
            max_groups,
            order,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Aggregation {
    Sum(String),
    Min(String),
    Max(String),
    Average(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupingOrder {
    None,
    CountAscending,
    CountDescending,
    ValueAscending,
    ValueDescending,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GroupingRequest {
    pub query: Query,
    pub group_by: String,
    pub aggregations: Vec<Aggregation>,
    pub max_groups: Option<u32>,
    pub order: GroupingOrder,
}

impl GroupingRequest {
    pub fn new(query: Query, group_by: impl Into<String>) -> Self {
        Self {
            query,
            group_by: group_by.into(),
            aggregations: Vec::new(),
            max_groups: None,
            order: GroupingOrder::None,
        }
    }

    pub fn aggregation(mut self, aggregation: Aggregation) -> Self {
        self.aggregations.push(aggregation);
        self
    }

    pub fn max_groups(mut self, max_groups: u32) -> Self {
        self.max_groups = Some(max_groups);
        self
    }

    pub fn order(mut self, order: GroupingOrder) -> Self {
        self.order = order;
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum AggregateValue {
    Integer(i64),
    Float(f64),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Group {
    pub key: Value,
    pub count: u64,
    pub aggregations: Vec<AggregateValue>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GroupingResult {
    pub groups: Vec<Group>,
}

#[derive(Debug)]
/// An error returned while calling the native adapter.
pub struct Error {
    kind: ErrorKind,
    code: i32,
    detail_code: Option<String>,
    message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    InvalidArgument,
    ResourceExhausted,
    Unsupported,
    DeadlineExceeded,
    Internal,
}

impl Error {
    fn message(message: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::InvalidArgument,
            code: 0,
            detail_code: None,
            message: message.into(),
        }
    }

    fn native(code: i32, detail_code: Option<String>, message: impl Into<String>) -> Self {
        Self {
            kind: match code {
                1 => ErrorKind::InvalidArgument,
                3 => ErrorKind::ResourceExhausted,
                4 => ErrorKind::Unsupported,
                5 => ErrorKind::DeadlineExceeded,
                _ => ErrorKind::Internal,
            },
            code,
            detail_code,
            message: message.into(),
        }
    }

    fn from_nul(error: NulError) -> Self {
        Self::message(format!("string contains an interior NUL: {error}"))
    }

    /// Returns the native Search Kernel error code, or zero for an adapter-side error.
    pub fn code(&self) -> i32 {
        self.code
    }

    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    /// Returns the stable machine-readable native detail code, when supplied
    /// by the adapter. Human-readable diagnostics are intentionally separate.
    pub fn detail_code(&self) -> Option<&str> {
        self.detail_code.as_deref()
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for Error {}

pub type Result<T, E = Error> = std::result::Result<T, E>;

fn native_error(error: &NativeError, status: i32) -> Error {
    let message_bytes: Vec<u8> = error
        .0
        .iter()
        .take_while(|&&byte| byte != 0)
        .copied()
        .collect();
    let diagnostic = String::from_utf8_lossy(&message_bytes);
    let (detail_code, message) = match diagnostic
        .strip_prefix("SK_DETAIL_V1:")
        .and_then(|value| value.split_once(':'))
    {
        Some((code, message)) => (Some(code.to_owned()), message.to_owned()),
        None => (None, diagnostic.into_owned()),
    };
    let message = if message.is_empty() {
        format!("native operation failed with status {status}")
    } else {
        message
    };
    Error::native(status, detail_code, message)
}

fn check(status: i32, error: &NativeError) -> Result<(), Error> {
    if status == 0 {
        Ok(())
    } else {
        Err(native_error(error, status))
    }
}

fn cstring(value: &str) -> Result<CString, Error> {
    CString::new(value).map_err(Error::from_nul)
}

fn path_cstring(path: &Path) -> Result<CString, Error> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        CString::new(path.as_os_str().as_bytes()).map_err(Error::from_nul)
    }
    #[cfg(not(unix))]
    {
        cstring(&path.to_string_lossy())
    }
}

fn configured_document_type(config_dir: &Path) -> Result<String, Error> {
    let manifest_path = config_dir.join("searchkernel-schema.json");
    let manifest = fs::read_to_string(&manifest_path).map_err(|error| {
        Error::message(format!(
            "could not read generated schema manifest '{}': {error}",
            manifest_path.display()
        ))
    })?;
    let key = "\"document_type\"";
    let value = manifest
        .find(key)
        .and_then(|key_start| {
            manifest[key_start + key.len()..]
                .find(':')
                .map(|colon| key_start + key.len() + colon + 1)
        })
        .and_then(|value_start| {
            let value = manifest[value_start..].trim_start();
            value.strip_prefix('"').map(|value| value.to_owned())
        })
        .and_then(|value| value.find('"').map(|end| value[..end].to_owned()))
        .filter(|value| !value.is_empty())
        .ok_or_else(|| Error::message("generated schema manifest has no document type"))?;
    Ok(value)
}

fn require_ptr<T>(pointer: *mut T, name: &str) -> Result<NonNull<T>, Error> {
    NonNull::new(pointer)
        .ok_or_else(|| Error::message(format!("native operation returned a null {name}")))
}

fn native_bytes<'a>(
    _owner: &'a NativeDocument,
    pointer: *const std::os::raw::c_char,
    length: u32,
    name: &str,
) -> Result<&'a [u8], Error> {
    if length == 0 {
        return Ok(&[]);
    }
    if pointer.is_null() {
        return Err(Error::message(format!(
            "native operation returned a null {name}"
        )));
    }
    // SAFETY: the native result contract keeps the returned bytes alive for
    // the lifetime of its owning opaque result, which the caller holds while
    // consuming this slice.
    Ok(unsafe { std::slice::from_raw_parts(pointer.cast::<u8>(), length as usize) })
}

pub struct Engine {
    handle: NonNull<sys::sk_engine>,
    document_type: String,
}

// SAFETY: the native adapter owns the pointed-to engine and protects its
// lifetime with an exclusive lifecycle gate. Supported operations are safe to
// invoke concurrently according to the wrapper contract; the only mutable
// adapter-owned bookkeeping is protected inside the native implementation.
unsafe impl Send for Engine {}

// SAFETY: all methods borrow Engine immutably and the native adapter permits
// concurrent supported operations while close/destruction requires exclusive
// ownership. Rust's borrow rules prevent Drop from racing an outstanding
// shared borrow; Arc<Engine> can therefore safely share this owner.
unsafe impl Sync for Engine {}

impl Engine {
    /// Open a native engine using its data and generated configuration directories.
    pub fn open(data_dir: impl AsRef<Path>, config_dir: impl AsRef<Path>) -> Result<Self, Error> {
        let actual_abi = unsafe { sys::sk_native_abi_version() };
        if actual_abi != EXPECTED_ABI {
            return Err(Error::message(format!(
                "unsupported native ABI version {actual_abi}; expected {EXPECTED_ABI}"
            )));
        }

        let document_type = configured_document_type(config_dir.as_ref())?;
        let data_dir = path_cstring(data_dir.as_ref())?;
        let config_dir = path_cstring(config_dir.as_ref())?;
        let mut handle = ptr::null_mut();
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_engine_open(
                data_dir.as_ptr(),
                config_dir.as_ptr(),
                &mut handle,
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)?;
        Ok(Self {
            handle: require_ptr(handle, "engine")?,
            document_type,
        })
    }

    /// Gracefully close the engine and report any native durability failure.
    ///
    /// Dropping an engine remains a best-effort cleanup operation because Rust
    /// destructors cannot return errors. This consuming method makes graceful
    /// close observable while ensuring the native handle is still destroyed.
    pub fn close(self) -> Result<(), Error> {
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_engine_close(self.handle.as_ptr(), error.as_mut_ptr(), error.capacity())
        };
        check(status, &error)
    }

    /// Put a complete document. Each field is lowered to the native Vespa
    /// document path; subsequent fields are applied only while constructing
    /// the native document operation and are not exposed as separate API
    /// calls.
    pub fn put(&self, document: Document) -> Result<(), Error> {
        if document.fields().is_empty() {
            return Err(Error::message("document must contain at least one field"));
        }
        let id = cstring(document.id())?;
        let mut native = ptr::null_mut();
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_document_create_empty(
                id.as_ptr(),
                &mut native,
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)?;
        let native = NativeDocument(require_ptr(native, "document")?);
        for (field, value) in document.fields() {
            add_document_field(native.0.as_ptr(), field, value)?;
            let analysis = document
                .text_analysis()
                .get(field)
                .cloned()
                .or_else(|| match value {
                    FieldValue::Scalar(ScalarValue::String(value)) => {
                        Some(Linguistics.analyze_document(value, LinguisticsOptions::default()))
                    }
                    _ => None,
                });
            if let Some(analysis) = analysis {
                add_text_annotation(native.0.as_ptr(), field, &analysis, false)?;
            }
        }
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_engine_put(
                self.handle.as_ptr(),
                native.0.as_ptr(),
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)
    }

    /// Apply field-level mutations to an existing document.
    pub fn update(&self, update: DocumentUpdate) -> Result<(), Error> {
        if update.fields().is_empty() {
            return Err(Error::message(
                "document update must contain at least one field",
            ));
        }
        let id = cstring(update.id())?;
        let mut native = ptr::null_mut();
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_document_update_create_empty(
                id.as_ptr(),
                &mut native,
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)?;
        let native = NativeUpdate(require_ptr(native, "document update")?);
        for (field, mutation) in update.fields() {
            add_update_field(native.0.as_ptr(), field, mutation)?;
            let analysis = update
                .text_analysis()
                .get(field)
                .cloned()
                .or_else(|| match mutation {
                    FieldUpdate::Assign(FieldValue::Scalar(ScalarValue::String(value))) => {
                        Some(Linguistics.analyze_document(value, LinguisticsOptions::default()))
                    }
                    _ => None,
                });
            if let Some(analysis) = analysis {
                add_text_annotation(native.0.as_ptr(), field, &analysis, true)?;
            }
        }
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_engine_update(
                self.handle.as_ptr(),
                native.0.as_ptr(),
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)
    }

    /// Execute an explicit M-layer request and preserve its Vespa-shaped
    /// result without performing a second query in the adapter.
    pub fn search(&self, request: SearchRequest) -> Result<SearchResult, Error> {
        self.execute_search(&request)
    }

    /// Retrieve the complete document through Vespa's document retriever.
    pub fn get(&self, document_id: &str) -> Result<Option<Document>, Error> {
        let document_id = cstring(document_id)?;
        let mut native = ptr::null_mut();
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_engine_get(
                self.handle.as_ptr(),
                document_id.as_ptr(),
                &mut native,
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)?;
        let Some(native) = NonNull::new(native) else {
            return Ok(None);
        };
        let native = NativeDocument(native);
        let mut count = 0;
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_document_field_count(
                native.0.as_ptr(),
                &mut count,
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)?;
        let mut document = Document::new(document_id.to_string_lossy().into_owned());
        for index in 0..count {
            let mut name = ptr::null();
            let mut name_length = 0;
            let mut kind = 0;
            let mut string = ptr::null();
            let mut string_length = 0;
            let mut integer = 0;
            let mut floating = 0.0;
            let mut boolean = 0;
            let mut error = NativeError::new();
            let status = unsafe {
                sys::sk_document_field(
                    native.0.as_ptr(),
                    index,
                    &mut name,
                    &mut name_length,
                    &mut kind,
                    &mut string,
                    &mut string_length,
                    &mut integer,
                    &mut floating,
                    &mut boolean,
                    error.as_mut_ptr(),
                    error.capacity(),
                )
            };
            check(status, &error)?;
            let name = native_bytes(&native, name, name_length, "document field name")?;
            let name = std::str::from_utf8(name)
                .map_err(|_| Error::message("native document field name is invalid UTF-8"))?;
            let value = match kind {
                sys::SK_VALUE_STRING => {
                    let value =
                        native_bytes(&native, string, string_length, "document field value")?;
                    let value = std::str::from_utf8(value).map_err(|_| {
                        Error::message("native document string value is invalid UTF-8")
                    })?;
                    FieldValue::Scalar(ScalarValue::String(value.to_owned()))
                }
                sys::SK_VALUE_INTEGER => FieldValue::Scalar(ScalarValue::Integer(integer)),
                sys::SK_VALUE_FLOAT => FieldValue::Scalar(ScalarValue::Float(floating)),
                sys::SK_VALUE_BOOLEAN => FieldValue::Scalar(ScalarValue::Boolean(boolean != 0)),
                _ => {
                    return Err(Error::message(
                        "native document returned an unknown value kind",
                    ))
                }
            };
            document = document.field(name, value);
        }
        Ok(Some(document))
    }

    /// Delete a document and report Vespa's `wasFound` result.
    pub fn delete(&self, document_id: &str) -> Result<bool, Error> {
        let document_id = cstring(document_id)?;
        let mut was_found = 0;
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_engine_remove(
                self.handle.as_ptr(),
                document_id.as_ptr(),
                &mut was_found,
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)?;
        Ok(was_found != 0)
    }

    fn execute_search(&self, request: &SearchRequest) -> Result<SearchResult, Error> {
        validate_search_ranking_inputs(request)?;
        query::validate_query(&request.query)?;
        if let Source::Named(source) = &request.source {
            if source != &self.document_type {
                return Err(Error::message(format!(
                    "unsupported query source: {source}"
                )));
            }
        }
        let native_query = self.create_native_query(&request.query, request.language)?;
        let rank_profile = cstring(&request.rank_profile)?;
        let summary_kind = match request.summary {
            SummaryRequest::None => sys::SK_SUMMARY_NONE,
            SummaryRequest::Default => sys::SK_SUMMARY_DEFAULT,
            SummaryRequest::Fields(_) => sys::SK_SUMMARY_FIELDS,
        };
        let mut native_request = ptr::null_mut();
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_generic_search_request_create(
                rank_profile.as_ptr(),
                request.limit,
                request.offset,
                summary_kind,
                &mut native_request,
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)?;
        let native_request =
            NativeGenericSearchRequest(require_ptr(native_request, "generic search request")?);
        let timeout_millis = u64::try_from(request.timeout.as_millis())
            .map_err(|_| Error::message("search timeout is too large to encode"))?;
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_generic_search_request_set_timeout(
                native_request.0.as_ptr(),
                timeout_millis,
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)?;
        for sort in &request.sort {
            let field = cstring(&sort.field)?;
            let order = match sort.order {
                SortOrder::Ascending => sys::SK_SORT_ASCENDING,
                SortOrder::Descending => sys::SK_SORT_DESCENDING,
            };
            let mut error = NativeError::new();
            let status = unsafe {
                sys::sk_generic_search_request_add_sort(
                    native_request.0.as_ptr(),
                    field.as_ptr(),
                    order,
                    match sort.missing {
                        MissingPlacement::Default => sys::SK_SORT_MISSING_DEFAULT,
                        MissingPlacement::First => sys::SK_SORT_MISSING_FIRST,
                        MissingPlacement::Last => sys::SK_SORT_MISSING_LAST,
                    },
                    error.as_mut_ptr(),
                    error.capacity(),
                )
            };
            check(status, &error)?;
        }
        if let SummaryRequest::Fields(fields) = &request.summary {
            for field in fields {
                let field = cstring(field)?;
                let mut error = NativeError::new();
                let status = unsafe {
                    sys::sk_generic_search_request_add_summary_field(
                        native_request.0.as_ptr(),
                        field.as_ptr(),
                        error.as_mut_ptr(),
                        error.capacity(),
                    )
                };
                check(status, &error)?;
            }
        }
        for (name, value) in &request.query_features {
            let name = cstring(name)?;
            let mut error = NativeError::new();
            let status = unsafe {
                sys::sk_generic_search_request_add_query_feature(
                    native_request.0.as_ptr(),
                    name.as_ptr(),
                    *value,
                    error.as_mut_ptr(),
                    error.capacity(),
                )
            };
            check(status, &error)?;
        }
        for (name, value) in &request.rank_properties {
            let name = cstring(name)?;
            let value = cstring(value)?;
            let mut error = NativeError::new();
            let status = unsafe {
                sys::sk_generic_search_request_add_rank_property(
                    native_request.0.as_ptr(),
                    name.as_ptr(),
                    value.as_ptr(),
                    error.as_mut_ptr(),
                    error.capacity(),
                )
            };
            check(status, &error)?;
        }
        for facet in &request.facets {
            let field = cstring(&facet.field)?;
            let mut error = NativeError::new();
            let status = unsafe {
                sys::sk_generic_search_request_add_grouping(
                    native_request.0.as_ptr(),
                    field.as_ptr(),
                    facet.max_groups,
                    match facet.order {
                        GroupingOrder::None => sys::SK_GROUP_ORDER_NONE,
                        GroupingOrder::CountAscending => sys::SK_GROUP_ORDER_COUNT_ASC,
                        GroupingOrder::CountDescending => sys::SK_GROUP_ORDER_COUNT_DESC,
                        GroupingOrder::ValueAscending => sys::SK_GROUP_ORDER_VALUE_ASC,
                        GroupingOrder::ValueDescending => sys::SK_GROUP_ORDER_VALUE_DESC,
                    },
                    error.as_mut_ptr(),
                    error.capacity(),
                )
            };
            check(status, &error)?;
        }
        let mut native_result = ptr::null_mut();
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_engine_search_generic(
                self.handle.as_ptr(),
                native_query.0.as_ptr(),
                native_request.0.as_ptr(),
                &mut native_result,
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)?;
        read_generic_search_result(require_ptr(native_result, "generic search result")?)
    }

    pub fn group(&self, request: &GroupingRequest) -> Result<GroupingResult, Error> {
        query::validate_query(&request.query)?;
        let native_query =
            self.create_native_query(&request.query, LinguisticsOptions::default())?;
        let group_field = cstring(&request.group_by)?;
        let mut native_request = ptr::null_mut();
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_generic_grouping_request_create(
                group_field.as_ptr(),
                request.max_groups.unwrap_or(u32::MAX),
                match request.order {
                    GroupingOrder::None => sys::SK_GROUP_ORDER_NONE,
                    GroupingOrder::CountAscending => sys::SK_GROUP_ORDER_COUNT_ASC,
                    GroupingOrder::CountDescending => sys::SK_GROUP_ORDER_COUNT_DESC,
                    GroupingOrder::ValueAscending => sys::SK_GROUP_ORDER_VALUE_ASC,
                    GroupingOrder::ValueDescending => sys::SK_GROUP_ORDER_VALUE_DESC,
                },
                &mut native_request,
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)?;
        let native_request =
            NativeGenericGroupingRequest(require_ptr(native_request, "generic grouping request")?);
        for aggregation in &request.aggregations {
            let (kind, field) = match aggregation {
                Aggregation::Sum(field) => (sys::SK_AGGREGATION_SUM, field),
                Aggregation::Min(field) => (sys::SK_AGGREGATION_MIN, field),
                Aggregation::Max(field) => (sys::SK_AGGREGATION_MAX, field),
                Aggregation::Average(field) => (sys::SK_AGGREGATION_AVERAGE, field),
            };
            let field = cstring(field)?;
            let mut error = NativeError::new();
            let status = unsafe {
                sys::sk_generic_grouping_request_add_aggregation(
                    native_request.0.as_ptr(),
                    kind,
                    field.as_ptr(),
                    error.as_mut_ptr(),
                    error.capacity(),
                )
            };
            check(status, &error)?;
        }
        let mut native_result = ptr::null_mut();
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_engine_group_generic(
                self.handle.as_ptr(),
                native_query.0.as_ptr(),
                native_request.0.as_ptr(),
                &mut native_result,
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)?;
        read_generic_grouping_result(require_ptr(native_result, "generic grouping result")?)
    }

    fn create_native_query(
        &self,
        query: &Query,
        language: LinguisticsOptions,
    ) -> Result<NativeQuery, Error> {
        let mut native_query = ptr::null_mut();
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_query_create(
                self.handle.as_ptr(),
                &mut native_query,
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)?;
        let native_query = NativeQuery(require_ptr(native_query, "query")?);
        validate_query_fields(&native_query, query)?;
        lower_query(&native_query, query, language)?;
        Ok(native_query)
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        unsafe { sys::sk_engine_destroy(self.handle.as_ptr()) };
    }
}

/// Owns the caller-provided diagnostic bytes used for one native operation.
///
/// The native ABI intentionally receives only this byte pointer and capacity:
/// an error struct would duplicate C/Rust layout and ownership contracts for
/// information already returned authoritatively by the status value.
struct NativeError([u8; sys::SK_ERROR_MESSAGE_CAPACITY as usize]);

impl NativeError {
    fn new() -> Self {
        Self([0; 512])
    }

    fn as_mut_ptr(&mut self) -> *mut u8 {
        self.0.as_mut_ptr()
    }

    fn capacity(&self) -> u32 {
        self.0.len() as u32
    }
}

struct NativeDocument(NonNull<sys::sk_document>);
impl Drop for NativeDocument {
    fn drop(&mut self) {
        unsafe { sys::sk_document_destroy(self.0.as_ptr()) };
    }
}

struct NativeUpdate(NonNull<sys::sk_document_update>);
impl Drop for NativeUpdate {
    fn drop(&mut self) {
        unsafe { sys::sk_document_update_destroy(self.0.as_ptr()) };
    }
}

fn add_document_field(
    document: *mut sys::sk_document,
    field: &str,
    value: &FieldValue,
) -> Result<(), Error> {
    let field = cstring(field)?;
    let mut error = NativeError::new();
    let status = match value {
        FieldValue::Scalar(ScalarValue::String(value)) => {
            let value = cstring(value)?;
            unsafe {
                sys::sk_document_add_string(
                    document,
                    field.as_ptr(),
                    value.as_ptr(),
                    error.as_mut_ptr(),
                    error.capacity(),
                )
            }
        }
        FieldValue::Scalar(ScalarValue::Integer(value)) => unsafe {
            sys::sk_document_add_integer(
                document,
                field.as_ptr(),
                *value,
                error.as_mut_ptr(),
                error.capacity(),
            )
        },
        FieldValue::Scalar(ScalarValue::Float(value)) => unsafe {
            sys::sk_document_add_float(
                document,
                field.as_ptr(),
                *value,
                error.as_mut_ptr(),
                error.capacity(),
            )
        },
        FieldValue::Scalar(ScalarValue::Boolean(value)) => unsafe {
            sys::sk_document_add_boolean(
                document,
                field.as_ptr(),
                u8::from(*value),
                error.as_mut_ptr(),
                error.capacity(),
            )
        },
        FieldValue::Array(_) => {
            return Err(Error::message(
                "array document values require a multi-valued schema",
            ));
        }
    };
    check(status, &error)
}

fn add_update_field(
    update: *mut sys::sk_document_update,
    field: &str,
    mutation: &FieldUpdate,
) -> Result<(), Error> {
    let field = cstring(field)?;
    let mut error = NativeError::new();
    let status = match mutation {
        FieldUpdate::Clear => unsafe {
            sys::sk_document_update_clear(
                update,
                field.as_ptr(),
                error.as_mut_ptr(),
                error.capacity(),
            )
        },
        FieldUpdate::Assign(FieldValue::Scalar(ScalarValue::String(value))) => {
            let value = cstring(value)?;
            unsafe {
                sys::sk_document_update_assign_string(
                    update,
                    field.as_ptr(),
                    value.as_ptr(),
                    error.as_mut_ptr(),
                    error.capacity(),
                )
            }
        }
        FieldUpdate::Assign(FieldValue::Scalar(ScalarValue::Integer(value))) => unsafe {
            sys::sk_document_update_assign_integer(
                update,
                field.as_ptr(),
                *value,
                error.as_mut_ptr(),
                error.capacity(),
            )
        },
        FieldUpdate::Assign(FieldValue::Scalar(ScalarValue::Float(value))) => unsafe {
            sys::sk_document_update_assign_float(
                update,
                field.as_ptr(),
                *value,
                error.as_mut_ptr(),
                error.capacity(),
            )
        },
        FieldUpdate::Assign(FieldValue::Scalar(ScalarValue::Boolean(value))) => unsafe {
            sys::sk_document_update_assign_boolean(
                update,
                field.as_ptr(),
                u8::from(*value),
                error.as_mut_ptr(),
                error.capacity(),
            )
        },
        FieldUpdate::Assign(FieldValue::Array(_)) => {
            return Err(Error::message(
                "array document updates require a multi-valued schema",
            ));
        }
    };
    check(status, &error)
}

fn add_text_annotation<T>(
    handle: *mut T,
    field: &str,
    analysis: &DocumentAnalysis,
    update: bool,
) -> Result<(), Error> {
    let occurrences = text_occurrences(analysis)?;
    let field = cstring(field)?;
    let mut error = NativeError::new();
    let status = unsafe {
        if update {
            sys::sk_document_update_annotate_text(
                handle.cast(),
                field.as_ptr(),
                occurrences.term_bytes.as_ptr(),
                length_u32(occurrences.term_bytes.len(), "text term bytes")?,
                occurrences.term_lengths.as_ptr(),
                occurrences.byte_starts.as_ptr(),
                occurrences.byte_lengths.as_ptr(),
                length_u32(occurrences.term_lengths.len(), "text occurrence")?,
                error.as_mut_ptr(),
                error.capacity(),
            )
        } else {
            sys::sk_document_annotate_text(
                handle.cast(),
                field.as_ptr(),
                occurrences.term_bytes.as_ptr(),
                length_u32(occurrences.term_bytes.len(), "text term bytes")?,
                occurrences.term_lengths.as_ptr(),
                occurrences.byte_starts.as_ptr(),
                occurrences.byte_lengths.as_ptr(),
                length_u32(occurrences.term_lengths.len(), "text occurrence")?,
                error.as_mut_ptr(),
                error.capacity(),
            )
        }
    };
    check(status, &error)
}

fn length_u32(length: usize, what: &str) -> Result<u32, Error> {
    u32::try_from(length)
        .map_err(|_| Error::message(format!("{what} count exceeds native ABI capacity")))
}

struct NativeOccurrences {
    term_bytes: Vec<u8>,
    term_lengths: Vec<u32>,
    byte_starts: Vec<u32>,
    byte_lengths: Vec<u32>,
}

fn text_occurrences(analysis: &DocumentAnalysis) -> Result<NativeOccurrences, Error> {
    let mut result = NativeOccurrences {
        term_bytes: Vec::new(),
        term_lengths: Vec::with_capacity(analysis.terms.len()),
        byte_starts: Vec::with_capacity(analysis.terms.len()),
        byte_lengths: Vec::with_capacity(analysis.terms.len()),
    };
    for term in &analysis.terms {
        result
            .term_lengths
            .push(length_u32(term.term.len(), "text term")?);
        result.term_bytes.extend_from_slice(term.term.as_bytes());
        result
            .byte_starts
            .push(length_u32(term.byte_start, "text span")?);
        result
            .byte_lengths
            .push(length_u32(term.byte_len, "text span")?);
    }
    Ok(result)
}

struct NativeQueryTerms {
    term_bytes: Vec<u8>,
    term_lengths: Vec<u32>,
}

fn text_query_terms(analysis: &QueryAnalysis) -> Result<NativeQueryTerms, Error> {
    let mut result = NativeQueryTerms {
        term_bytes: Vec::new(),
        term_lengths: Vec::with_capacity(analysis.terms.len()),
    };
    for term in &analysis.terms {
        let stem = term.stems.first().ok_or_else(|| {
            Error::message("M4 query analysis returned no stem for a query position")
        })?;
        if term.stems.len() != 1 {
            return Err(Error::message(
                "M4 query analysis returned multiple stems for a query position",
            ));
        }
        result
            .term_lengths
            .push(length_u32(stem.len(), "text query term")?);
        result.term_bytes.extend_from_slice(stem.as_bytes());
    }
    Ok(result)
}

fn validate_term_query(analysis: &QueryAnalysis) -> Result<(), Error> {
    if analysis.terms.len() == 1 {
        Ok(())
    } else {
        Err(Error::message(
            "term query must analyze to exactly one term",
        ))
    }
}

fn validate_search_ranking_inputs(request: &SearchRequest) -> Result<(), Error> {
    let mut query_feature_names = std::collections::BTreeSet::new();
    for (name, value) in &request.query_features {
        if name.is_empty() {
            return Err(Error::message("query feature name must not be empty"));
        }
        if name.len() > 256 {
            return Err(Error::message("query feature name is too long"));
        }
        if RESERVED_RANK_PROPERTIES.contains(&name.as_str()) {
            return Err(Error::message(format!(
                "query feature '{name}' is reserved by Search Kernel"
            )));
        }
        if !value.is_finite() {
            return Err(Error::message("query feature value must be finite"));
        }
        if !query_feature_names.insert(name) {
            return Err(Error::message(format!("duplicate query feature '{name}'")));
        }
    }

    let mut rank_property_names = std::collections::BTreeSet::new();
    for (name, value) in &request.rank_properties {
        if name.is_empty() || name.len() > 256 {
            return Err(Error::message(
                "rank property name must not be empty or too long",
            ));
        }
        if RESERVED_RANK_PROPERTIES.contains(&name.as_str()) {
            return Err(Error::message(format!(
                "rank property '{name}' is reserved by Search Kernel"
            )));
        }
        if value.is_empty() {
            return Err(Error::message("rank property value must not be empty"));
        }
        if !rank_property_names.insert(name) {
            return Err(Error::message(format!("duplicate rank property '{name}'")));
        }
        if query_feature_names.contains(name) {
            return Err(Error::message(format!(
                "rank property collision for key '{name}'"
            )));
        }
    }
    Ok(())
}

struct NativeQuery(NonNull<sys::sk_query>);
impl Drop for NativeQuery {
    fn drop(&mut self) {
        unsafe { sys::sk_query_destroy(self.0.as_ptr()) };
    }
}

struct NativeGenericSearchRequest(NonNull<sys::sk_generic_search_request>);
impl Drop for NativeGenericSearchRequest {
    fn drop(&mut self) {
        unsafe { sys::sk_generic_search_request_destroy(self.0.as_ptr()) };
    }
}

struct NativeGenericGroupingRequest(NonNull<sys::sk_generic_grouping_request>);
impl Drop for NativeGenericGroupingRequest {
    fn drop(&mut self) {
        unsafe { sys::sk_generic_grouping_request_destroy(self.0.as_ptr()) };
    }
}

fn read_generic_search_result(
    pointer: NonNull<sys::sk_generic_search_result>,
) -> Result<SearchResult, Error> {
    let result = NativeGenericSearchResult(pointer);
    let hit_count = unsafe { sys::sk_generic_search_result_hit_count(result.0.as_ptr()) } as usize;
    let total_hit_count =
        unsafe { sys::sk_generic_search_result_total_hit_count(result.0.as_ptr()) };
    let match_feature_count =
        unsafe { sys::sk_generic_search_result_match_feature_count(result.0.as_ptr()) } as usize;
    let mut match_feature_names = Vec::with_capacity(match_feature_count);
    for feature_index in 0..match_feature_count {
        let mut name = ptr::null();
        let mut name_length = 0;
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_generic_search_result_match_feature_name(
                result.0.as_ptr(),
                feature_index as u32,
                &mut name,
                &mut name_length,
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)?;
        if name.is_null() {
            return Err(Error::message("native match feature returned null name"));
        }
        let bytes = unsafe { std::slice::from_raw_parts(name.cast::<u8>(), name_length as usize) };
        let name = String::from_utf8(bytes.to_vec())
            .map_err(|_| Error::message("native match feature returned invalid UTF-8 name"))?;
        if match_feature_names.iter().any(|previous| previous == &name) {
            return Err(Error::message(
                "native match features contain duplicate names",
            ));
        }
        match_feature_names.push(name);
    }
    let mut hits = Vec::with_capacity(hit_count);
    for index in 0..hit_count {
        let mut id = ptr::null();
        let mut id_length = 0;
        let mut score = 0.0;
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_generic_search_result_hit(
                result.0.as_ptr(),
                index as u32,
                &mut id,
                &mut id_length,
                &mut score,
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)?;
        if id.is_null() {
            return Err(Error::message(
                "native local result returned null document ID",
            ));
        }
        let id = unsafe { std::slice::from_raw_parts(id.cast::<u8>(), id_length as usize) };
        let document_id = String::from_utf8(id.to_vec())
            .map_err(|_| Error::message("native result returned invalid UTF-8"))?;
        let summaries =
            unsafe { sys::sk_generic_search_result_summary_count(result.0.as_ptr(), index as u32) }
                as usize;
        let mut summary = BTreeMap::new();
        for summary_index in 0..summaries {
            let mut name = ptr::null();
            let mut name_length = 0;
            let mut error = NativeError::new();
            let status = unsafe {
                sys::sk_generic_search_result_summary_name(
                    result.0.as_ptr(),
                    index as u32,
                    summary_index as u32,
                    &mut name,
                    &mut name_length,
                    error.as_mut_ptr(),
                    error.capacity(),
                )
            };
            check(status, &error)?;
            if name.is_null() {
                return Err(Error::message("native summary returned null field name"));
            }
            let name_bytes =
                unsafe { std::slice::from_raw_parts(name.cast::<u8>(), name_length as usize) };
            let name = String::from_utf8(name_bytes.to_vec())
                .map_err(|_| Error::message("native summary returned invalid UTF-8 field name"))?;
            let mut kind = 0;
            let mut string = ptr::null();
            let mut string_length = 0;
            let mut integer = 0;
            let mut float = 0.0;
            let mut boolean = 0;
            let mut error = NativeError::new();
            let status = unsafe {
                sys::sk_generic_search_result_summary_value(
                    result.0.as_ptr(),
                    index as u32,
                    summary_index as u32,
                    &mut kind,
                    &mut string,
                    &mut string_length,
                    &mut integer,
                    &mut float,
                    &mut boolean,
                    error.as_mut_ptr(),
                    error.capacity(),
                )
            };
            check(status, &error)?;
            let value = match kind {
                sys::SK_VALUE_STRING => {
                    if string.is_null() {
                        return Err(Error::message("native summary returned null string"));
                    }
                    let bytes = unsafe {
                        std::slice::from_raw_parts(string.cast::<u8>(), string_length as usize)
                    };
                    Value::String(
                        String::from_utf8(bytes.to_vec())
                            .map_err(|_| Error::message("native summary returned invalid UTF-8"))?,
                    )
                }
                sys::SK_VALUE_INTEGER => Value::Integer(integer),
                sys::SK_VALUE_FLOAT => Value::Float(float),
                sys::SK_VALUE_BOOLEAN => Value::Boolean(boolean != 0),
                _ => return Err(Error::message("native summary returned unknown value kind")),
            };
            summary.insert(name, value);
        }
        let mut match_features = BTreeMap::new();
        for (feature_index, feature_name) in match_feature_names.iter().enumerate() {
            let mut value = 0.0;
            let mut error = NativeError::new();
            let status = unsafe {
                sys::sk_generic_search_result_match_feature_value(
                    result.0.as_ptr(),
                    index as u32,
                    feature_index as u32,
                    &mut value,
                    error.as_mut_ptr(),
                    error.capacity(),
                )
            };
            check(status, &error)?;
            match_features.insert(feature_name.clone(), value);
        }
        hits.push(Hit {
            document_id,
            score,
            match_features,
            fields: summary
                .into_iter()
                .map(|(name, value)| (name, FieldValue::Scalar(value)))
                .collect(),
        });
    }
    let grouping_count = unsafe { sys::sk_generic_search_result_grouping_count(result.0.as_ptr()) };
    let mut facets = Vec::with_capacity(grouping_count as usize);
    for grouping_index in 0..grouping_count {
        facets.push(read_search_facet(result.0.as_ptr(), grouping_index)?);
    }
    Ok(SearchResult {
        total_hits: total_hit_count,
        total_hits_relation: TotalHitsRelation::Exact,
        hits,
        facets,
    })
}

struct NativeGenericSearchResult(NonNull<sys::sk_generic_search_result>);
impl Drop for NativeGenericSearchResult {
    fn drop(&mut self) {
        unsafe { sys::sk_generic_search_result_destroy(self.0.as_ptr()) };
    }
}

fn read_search_facet(
    result: *const sys::sk_generic_search_result,
    grouping_index: u32,
) -> Result<GroupingResult, Error> {
    let group_count =
        unsafe { sys::sk_generic_search_result_group_count(result, grouping_index) } as usize;
    let mut groups = Vec::with_capacity(group_count);
    for group_index in 0..group_count {
        let mut key_kind = 0;
        let mut key_string = ptr::null();
        let mut key_string_length = 0;
        let mut key_integer = 0;
        let mut count = 0;
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_generic_search_result_group(
                result,
                grouping_index,
                group_index as u32,
                &mut key_kind,
                &mut key_string,
                &mut key_string_length,
                &mut key_integer,
                &mut count,
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)?;
        let key = match key_kind {
            sys::SK_VALUE_STRING => {
                if key_string.is_null() {
                    return Err(Error::message("native search returned null facet key"));
                }
                let bytes = unsafe {
                    std::slice::from_raw_parts(key_string.cast::<u8>(), key_string_length as usize)
                };
                Value::String(
                    String::from_utf8(bytes.to_vec())
                        .map_err(|_| Error::message("native search returned invalid facet key"))?,
                )
            }
            sys::SK_VALUE_INTEGER => Value::Integer(key_integer),
            sys::SK_VALUE_BOOLEAN => Value::Boolean(key_integer != 0),
            _ => {
                return Err(Error::message(
                    "native search returned unsupported facet key kind",
                ))
            }
        };
        groups.push(Group {
            key,
            count,
            aggregations: Vec::new(),
        });
    }
    Ok(GroupingResult { groups })
}

fn read_generic_grouping_result(
    pointer: NonNull<sys::sk_generic_grouping_result>,
) -> Result<GroupingResult, Error> {
    let result = NativeGenericGroupingResult(pointer);
    let group_count =
        unsafe { sys::sk_generic_grouping_result_group_count(result.0.as_ptr()) } as usize;
    let mut groups = Vec::with_capacity(group_count);
    for group_index in 0..group_count {
        let mut key_kind = 0;
        let mut key_string = ptr::null();
        let mut key_string_length = 0;
        let mut key_integer = 0;
        let mut count = 0;
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_generic_grouping_result_group(
                result.0.as_ptr(),
                group_index as u32,
                &mut key_kind,
                &mut key_string,
                &mut key_string_length,
                &mut key_integer,
                &mut count,
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)?;
        let key = match key_kind {
            sys::SK_VALUE_STRING => {
                if key_string.is_null() {
                    return Err(Error::message("native grouping returned null string key"));
                }
                let bytes = unsafe {
                    std::slice::from_raw_parts(key_string.cast::<u8>(), key_string_length as usize)
                };
                Value::String(
                    String::from_utf8(bytes.to_vec())
                        .map_err(|_| Error::message("native grouping returned invalid UTF-8"))?,
                )
            }
            sys::SK_VALUE_INTEGER => Value::Integer(key_integer),
            _ => {
                return Err(Error::message(
                    "native grouping returned unsupported key kind",
                ));
            }
        };
        let aggregation_count = unsafe {
            sys::sk_generic_grouping_result_aggregation_count(result.0.as_ptr(), group_index as u32)
        } as usize;
        let mut aggregations = Vec::with_capacity(aggregation_count);
        for aggregation_index in 0..aggregation_count {
            let mut kind = 0;
            let mut integer = 0;
            let mut float = 0.0;
            let mut error = NativeError::new();
            let status = unsafe {
                sys::sk_generic_grouping_result_aggregation(
                    result.0.as_ptr(),
                    group_index as u32,
                    aggregation_index as u32,
                    &mut kind,
                    &mut integer,
                    &mut float,
                    error.as_mut_ptr(),
                    error.capacity(),
                )
            };
            check(status, &error)?;
            aggregations.push(match kind {
                sys::SK_VALUE_INTEGER => AggregateValue::Integer(integer),
                sys::SK_VALUE_FLOAT => AggregateValue::Float(float),
                _ => {
                    return Err(Error::message(
                        "native grouping returned unknown aggregate kind",
                    ));
                }
            });
        }
        groups.push(Group {
            key,
            count,
            aggregations,
        });
    }
    Ok(GroupingResult { groups })
}

struct NativeGenericGroupingResult(NonNull<sys::sk_generic_grouping_result>);
impl Drop for NativeGenericGroupingResult {
    fn drop(&mut self) {
        unsafe { sys::sk_generic_grouping_result_destroy(self.0.as_ptr()) };
    }
}

fn lower_query(
    native: &NativeQuery,
    query: &Query,
    language: LinguisticsOptions,
) -> Result<(), Error> {
    match query {
        Query::True => {
            let mut error = NativeError::new();
            let status = unsafe {
                sys::sk_query_add_true(native.0.as_ptr(), error.as_mut_ptr(), error.capacity())
            };
            check(status, &error)
        }
        Query::Term { field, text } => {
            let analysis = Linguistics.analyze_query(text, language);
            validate_term_query(&analysis)
                .map_err(|error| query::invalid_query(format!("term analysis failed: {error}")))?;
            let terms = text_query_terms(&analysis)?;
            add_native_text(native, sys::SK_QUERY_TEXT_TERM, field, &terms)
        }
        Query::Phrase { field, text } => {
            let analysis = Linguistics.analyze_query(text, language);
            if analysis.terms.is_empty() {
                return Err(query::invalid_query(
                    "phrase must analyze to at least one term",
                ));
            }
            let terms = text_query_terms(&analysis)?;
            add_native_text(native, sys::SK_QUERY_TEXT_PHRASE, field, &terms)
        }
        Query::And(children) => {
            add_native_operator(native, sys::SK_QUERY_NODE_AND, children.len())?;
            for child in children {
                lower_query(native, child, language)?;
            }
            Ok(())
        }
        Query::Or(children) => {
            add_native_operator(native, sys::SK_QUERY_NODE_OR, children.len())?;
            for child in children {
                lower_query(native, child, language)?;
            }
            Ok(())
        }
        Query::Not(child) => {
            add_native_operator(native, sys::SK_QUERY_NODE_NOT, 1)?;
            lower_query(native, child, language)
        }
        Query::Equals { field, value } => {
            let field = cstring(field)?;
            let mut error = NativeError::new();
            let status = unsafe {
                sys::sk_query_add_equals(
                    native.0.as_ptr(),
                    field.as_ptr(),
                    *value,
                    error.as_mut_ptr(),
                    error.capacity(),
                )
            };
            check(status, &error)
        }
        Query::Range {
            field,
            lower,
            upper,
        } => {
            let field = cstring(field)?;
            let (lower_kind, lower_value) = native_bound(lower);
            let (upper_kind, upper_value) = native_bound(upper);
            let mut error = NativeError::new();
            let status = unsafe {
                sys::sk_query_add_range(
                    native.0.as_ptr(),
                    field.as_ptr(),
                    lower_kind,
                    lower_value,
                    upper_kind,
                    upper_value,
                    error.as_mut_ptr(),
                    error.capacity(),
                )
            };
            check(status, &error)
        }
        Query::Exists { field } => {
            let field = cstring(field)?;
            let mut error = NativeError::new();
            let status = unsafe {
                sys::sk_query_add_range(
                    native.0.as_ptr(),
                    field.as_ptr(),
                    sys::SK_QUERY_BOUND_UNBOUNDED,
                    0,
                    sys::SK_QUERY_BOUND_UNBOUNDED,
                    0,
                    error.as_mut_ptr(),
                    error.capacity(),
                )
            };
            check(status, &error)
        }
        Query::AttributeEquals { field, value } => {
            add_native_attribute_equals(native, field, value)
        }
        Query::AttributeRange {
            field,
            lower,
            upper,
        } => {
            let value_kind = attribute_range_kind(lower).or_else(|| attribute_range_kind(upper));
            match value_kind {
                None => {
                    let field = cstring(field)?;
                    let mut error = NativeError::new();
                    let status = unsafe {
                        sys::sk_query_add_attribute_exists(
                            native.0.as_ptr(),
                            field.as_ptr(),
                            error.as_mut_ptr(),
                            error.capacity(),
                        )
                    };
                    check(status, &error)
                }
                Some(AttributeKind::Integer) => {
                    let (lower_kind, lower_value) = integer_attribute_bound(lower)?;
                    let (upper_kind, upper_value) = integer_attribute_bound(upper)?;
                    let field = cstring(field)?;
                    let mut error = NativeError::new();
                    let status = unsafe {
                        sys::sk_query_add_attribute_integer_range(
                            native.0.as_ptr(),
                            field.as_ptr(),
                            lower_kind,
                            lower_value,
                            upper_kind,
                            upper_value,
                            error.as_mut_ptr(),
                            error.capacity(),
                        )
                    };
                    check(status, &error)
                }
                Some(AttributeKind::Float) => {
                    let (lower_kind, lower_value) = float_attribute_bound(lower)?;
                    let (upper_kind, upper_value) = float_attribute_bound(upper)?;
                    let field = cstring(field)?;
                    let mut error = NativeError::new();
                    let status = unsafe {
                        sys::sk_query_add_attribute_float_range(
                            native.0.as_ptr(),
                            field.as_ptr(),
                            lower_kind,
                            lower_value,
                            upper_kind,
                            upper_value,
                            error.as_mut_ptr(),
                            error.capacity(),
                        )
                    };
                    check(status, &error)
                }
                Some(AttributeKind::String | AttributeKind::Boolean) => {
                    Err(Error::message("unsupported attribute range field type"))
                }
            }
        }
    }
}

fn validate_query_fields(native: &NativeQuery, query: &Query) -> Result<(), Error> {
    match query {
        Query::True => Ok(()),
        Query::Term { field, .. } | Query::Phrase { field, .. } => {
            validate_native_field(native, sys::SK_QUERY_FIELD_TEXT, field)
        }
        Query::And(children) | Query::Or(children) => {
            for child in children {
                validate_query_fields(native, child)?;
            }
            Ok(())
        }
        Query::Not(child) => validate_query_fields(native, child),
        Query::Equals { field, .. } | Query::Range { field, .. } | Query::Exists { field } => {
            validate_native_field(native, sys::SK_QUERY_FIELD_INTEGER, field)
        }
        Query::AttributeEquals { field, value } => validate_native_field(
            native,
            match value {
                Value::String(_) => sys::SK_QUERY_FIELD_STRING,
                Value::Integer(_) => sys::SK_QUERY_FIELD_INTEGER,
                Value::Float(_) => sys::SK_QUERY_FIELD_FLOAT,
                Value::Boolean(_) => sys::SK_QUERY_FIELD_BOOLEAN,
            },
            field,
        ),
        Query::AttributeRange {
            field,
            lower,
            upper,
        } => {
            let kind = attribute_range_kind(lower).or_else(|| attribute_range_kind(upper));
            if let Some(kind) = kind {
                validate_native_field(
                    native,
                    match kind {
                        AttributeKind::Integer => sys::SK_QUERY_FIELD_INTEGER,
                        AttributeKind::Float => sys::SK_QUERY_FIELD_FLOAT,
                        AttributeKind::String => sys::SK_QUERY_FIELD_STRING,
                        AttributeKind::Boolean => sys::SK_QUERY_FIELD_BOOLEAN,
                    },
                    field,
                )
            } else {
                validate_native_field(native, sys::SK_QUERY_FIELD_ATTRIBUTE, field)
            }
        }
    }
}

#[derive(Clone, Copy)]
enum AttributeKind {
    String,
    Integer,
    Float,
    Boolean,
}

fn attribute_range_kind(bound: &AttributeBound) -> Option<AttributeKind> {
    match bound {
        AttributeBound::Unbounded => None,
        AttributeBound::Included(Value::String(_)) | AttributeBound::Excluded(Value::String(_)) => {
            Some(AttributeKind::String)
        }
        AttributeBound::Included(Value::Integer(_))
        | AttributeBound::Excluded(Value::Integer(_)) => Some(AttributeKind::Integer),
        AttributeBound::Included(Value::Float(_)) | AttributeBound::Excluded(Value::Float(_)) => {
            Some(AttributeKind::Float)
        }
        AttributeBound::Included(Value::Boolean(_))
        | AttributeBound::Excluded(Value::Boolean(_)) => Some(AttributeKind::Boolean),
    }
}

fn integer_attribute_bound(bound: &AttributeBound) -> Result<(u32, i64), Error> {
    match bound {
        AttributeBound::Unbounded => Ok((sys::SK_QUERY_BOUND_UNBOUNDED, 0)),
        AttributeBound::Included(Value::Integer(value)) => {
            Ok((sys::SK_QUERY_BOUND_INCLUDED, *value))
        }
        AttributeBound::Excluded(Value::Integer(value)) => {
            Ok((sys::SK_QUERY_BOUND_EXCLUDED, *value))
        }
        _ => Err(Error::message(
            "wrong value type: attribute range bounds must have matching integer types",
        )),
    }
}

fn float_attribute_bound(bound: &AttributeBound) -> Result<(u32, f64), Error> {
    match bound {
        AttributeBound::Unbounded => Ok((sys::SK_QUERY_BOUND_UNBOUNDED, 0.0)),
        AttributeBound::Included(Value::Float(value)) => {
            if !value.is_finite() {
                return Err(Error::message("attribute float bound must be finite"));
            }
            Ok((sys::SK_QUERY_BOUND_INCLUDED, *value))
        }
        AttributeBound::Excluded(Value::Float(value)) => {
            if !value.is_finite() {
                return Err(Error::message("attribute float bound must be finite"));
            }
            Ok((sys::SK_QUERY_BOUND_EXCLUDED, *value))
        }
        _ => Err(Error::message(
            "wrong value type: attribute range bounds must have matching float types",
        )),
    }
}

fn add_native_attribute_equals(
    native: &NativeQuery,
    field: &str,
    value: &Value,
) -> Result<(), Error> {
    let field = cstring(field)?;
    let mut error = NativeError::new();
    let status = unsafe {
        match value {
            Value::String(value) => {
                let value = cstring(value)?;
                sys::sk_query_add_attribute_string_equals(
                    native.0.as_ptr(),
                    field.as_ptr(),
                    value.as_ptr(),
                    error.as_mut_ptr(),
                    error.capacity(),
                )
            }
            Value::Integer(value) => sys::sk_query_add_equals(
                native.0.as_ptr(),
                field.as_ptr(),
                *value,
                error.as_mut_ptr(),
                error.capacity(),
            ),
            Value::Float(value) => sys::sk_query_add_attribute_float_equals(
                native.0.as_ptr(),
                field.as_ptr(),
                *value,
                error.as_mut_ptr(),
                error.capacity(),
            ),
            Value::Boolean(value) => sys::sk_query_add_attribute_boolean_equals(
                native.0.as_ptr(),
                field.as_ptr(),
                u8::from(*value),
                error.as_mut_ptr(),
                error.capacity(),
            ),
        }
    };
    check(status, &error)
}

fn validate_native_field(native: &NativeQuery, kind: u32, field: &str) -> Result<(), Error> {
    let field = cstring(field)?;
    let mut error = NativeError::new();
    let status = unsafe {
        sys::sk_query_validate_field(
            native.0.as_ptr(),
            kind,
            field.as_ptr(),
            error.as_mut_ptr(),
            error.capacity(),
        )
    };
    check(status, &error)
}

fn add_native_operator(native: &NativeQuery, kind: u32, count: usize) -> Result<(), Error> {
    let count = length_u32(count, "query child")?;
    let mut error = NativeError::new();
    let status = unsafe {
        sys::sk_query_add_operator(
            native.0.as_ptr(),
            kind,
            count,
            error.as_mut_ptr(),
            error.capacity(),
        )
    };
    check(status, &error)
}

fn add_native_text(
    native: &NativeQuery,
    kind: u32,
    field: &str,
    terms: &NativeQueryTerms,
) -> Result<(), Error> {
    let field = cstring(field)?;
    let mut error = NativeError::new();
    let status = unsafe {
        sys::sk_query_add_text(
            native.0.as_ptr(),
            kind,
            field.as_ptr(),
            terms.term_bytes.as_ptr(),
            length_u32(terms.term_bytes.len(), "query text term bytes")?,
            terms.term_lengths.as_ptr(),
            length_u32(terms.term_lengths.len(), "query text term")?,
            error.as_mut_ptr(),
            error.capacity(),
        )
    };
    check(status, &error)
}

fn native_bound(bound: &RangeBound) -> (u32, i64) {
    match bound {
        RangeBound::Unbounded => (sys::SK_QUERY_BOUND_UNBOUNDED, 0),
        RangeBound::Included(value) => (sys::SK_QUERY_BOUND_INCLUDED, *value),
        RangeBound::Excluded(value) => (sys::SK_QUERY_BOUND_EXCLUDED, *value),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        check, cstring, length_u32, native_error, path_cstring, require_ptr, text_occurrences,
        text_query_terms, validate_term_query, Document, DocumentAnalysis, DocumentTerm, Duration,
        Error, ErrorKind, NativeError, Query, QueryAnalysis, QueryTerm, SearchRequest, SortField,
        SortOrder, Source, SummaryRequest,
    };

    #[test]
    fn boundary_helpers_preserve_native_error_information() {
        let error = NativeError::new();
        assert!(check(0, &error).is_ok());
        assert_eq!(
            native_error(&error, 17).to_string(),
            "native operation failed with status 17"
        );
        assert_eq!(native_error(&error, 17).code(), 17);
        assert_eq!(native_error(&error, 17).kind(), ErrorKind::Internal);

        let mut error = NativeError::new();
        error.0[..15].copy_from_slice(b"native failure\0");
        let checked = check(3, &error).expect_err("nonzero status must fail");
        assert_eq!(checked.to_string(), "native failure");
        assert_eq!(checked.code(), 3);
        assert_eq!(checked.kind(), ErrorKind::ResourceExhausted);

        let valid = cstring("field").expect("ordinary strings are portable");
        assert_eq!(valid.to_bytes(), b"field");
        let nul = cstring("bad\0field").expect_err("interior NUL must be rejected");
        assert!(nul.to_string().contains("interior NUL"));

        let valid_path =
            path_cstring(std::path::Path::new("config")).expect("ordinary paths are portable");
        assert_eq!(valid_path.to_bytes(), b"config");
        let invalid_path = path_cstring(std::path::Path::new("bad\0path"))
            .expect_err("interior NUL in a path must be rejected");
        assert!(invalid_path.to_string().contains("interior NUL"));
    }

    #[test]
    fn boundary_helpers_enforce_native_size_and_pointer_contracts() {
        assert_eq!(length_u32(0, "items").unwrap(), 0);
        assert_eq!(length_u32(u32::MAX as usize, "items").unwrap(), u32::MAX);
        let overflow = length_u32(u32::MAX as usize + 1, "items")
            .expect_err("values above u32 must be rejected");
        assert!(overflow.to_string().contains("items count"));

        assert!(require_ptr::<u8>(std::ptr::null_mut(), "result").is_err());
        let mut value = 1_u8;
        assert!(require_ptr(&mut value, "result").is_ok());
    }

    #[test]
    fn errors_preserve_codes_kinds_and_messages() {
        for (code, kind) in [
            (1, ErrorKind::InvalidArgument),
            (3, ErrorKind::ResourceExhausted),
            (77, ErrorKind::Internal),
        ] {
            let error = Error::native(code, None, "native failure");
            assert_eq!(error.code(), code);
            assert_eq!(error.kind(), kind);
            assert_eq!(error.to_string(), "native failure");
        }

        let local = Error::message("wrapper failure");
        assert_eq!(local.code(), 0);
        assert_eq!(local.kind(), ErrorKind::InvalidArgument);
        assert_eq!(local.to_string(), "wrapper failure");

        let nul = std::ffi::CString::new(b"bad\0value".as_slice()).expect_err("interior NUL");
        let nul_error = Error::from_nul(nul);
        assert_eq!(nul_error.kind(), ErrorKind::InvalidArgument);
        assert!(nul_error.to_string().contains("interior NUL"));
    }

    #[test]
    fn native_error_decodes_stable_detail_code_without_using_message_text() {
        let mut error = NativeError::new();
        let diagnostic = b"SK_DETAIL_V1:facet_key_type_not_supported:unsupported";
        error.0[..diagnostic.len()].copy_from_slice(diagnostic);
        let decoded = check(4, &error).expect_err("native unsupported status must fail");
        assert_eq!(decoded.kind(), ErrorKind::Unsupported);
        assert_eq!(decoded.detail_code(), Some("facet_key_type_not_supported"));
        assert_eq!(decoded.to_string(), "unsupported");
    }

    #[test]
    fn fluent_requests_and_attribute_builders_preserve_configuration() {
        let ascending = SortField::ascending("title");
        let descending = SortField::new("score", SortOrder::Descending);
        assert_eq!(ascending.order, SortOrder::Ascending);
        assert_eq!(descending.field, "score");

        let search = SearchRequest::new(Query::True)
            .rank_profile("title_rank")
            .limit(4)
            .offset(2)
            .sort(ascending.clone())
            .fields(["title", "body"])
            .timeout(Duration::from_millis(200));
        assert_eq!(search.rank_profile, "title_rank");
        assert_eq!(search.limit, 4);
        assert_eq!(search.offset, 2);
        assert_eq!(search.sort, vec![ascending]);
        assert_eq!(
            search.summary,
            SummaryRequest::Fields(vec!["title".into(), "body".into()])
        );
        assert_eq!(search.timeout, Duration::from_millis(200));

        let search = SearchRequest::new(Query::True)
            .rank_profile("local_rank")
            .limit(3)
            .offset(1)
            .sort(descending.clone())
            .summary(SummaryRequest::Default);
        assert_eq!(search.rank_profile, "local_rank");
        assert_eq!(search.limit, 3);
        assert_eq!(search.offset, 1);
        assert_eq!(search.sort, vec![descending]);
        assert_eq!(search.summary, SummaryRequest::Default);

        let yql = SearchRequest::from_yql("select * from typea where title contains \"fox\"")
            .expect("supported YQL should parse");
        assert_eq!(yql.source, Source::Named("typea".into()));
        assert_eq!(yql.query, Query::term("title", "fox"));

        let document = Document::new("id:ns:type::1").field("rating", 4_i64);
        assert_eq!(document.id(), "id:ns:type::1");
        assert_eq!(document.fields().len(), 1);

        let update = crate::DocumentUpdate::new("id:ns:type::1").assign("rating", 5_i64);
        assert_eq!(update.id(), "id:ns:type::1");
        assert_eq!(update.fields().len(), 1);
    }

    #[test]
    fn ranking_request_inputs_are_validated_before_native_calls() {
        let query_feature = SearchRequest::new(Query::True).query_feature("boost", 2.0);
        super::validate_search_ranking_inputs(&query_feature).expect("finite query feature");

        let rank_property = SearchRequest::new(Query::True).rank_property("model", "v1");
        super::validate_search_ranking_inputs(&rank_property).expect("non-empty rank property");

        let collision = SearchRequest::new(Query::True)
            .query_feature("boost", 2.0)
            .rank_property("boost", "3");
        let error = super::validate_search_ranking_inputs(&collision)
            .expect_err("query-feature/rank-property collisions must be rejected");
        assert!(error.to_string().contains("collision"));

        let reserved = SearchRequest::new(Query::True)
            .rank_property("vespa.matching.numthreadspersearch", "2");
        let error = super::validate_search_ranking_inputs(&reserved)
            .expect_err("Search Kernel-owned rank properties must be rejected");
        assert!(error.to_string().contains("reserved"));

        let reserved = SearchRequest::new(Query::True)
            .query_feature("vespa.matching.numthreadspersearch", 2.0);
        let error = super::validate_search_ranking_inputs(&reserved)
            .expect_err("Search Kernel-owned query features must be rejected");
        assert!(error.to_string().contains("reserved"));
    }

    #[test]
    fn document_analysis_converts_to_native_soa_occurrences() {
        let analysis = DocumentAnalysis {
            terms: vec![DocumentTerm {
                original: "fox".to_owned(),
                term: "fox".to_owned(),
                byte_start: 5,
                byte_len: 3,
                position: 2,
            }],
        };

        let occurrences = text_occurrences(&analysis).expect("valid occurrence");
        assert_eq!(occurrences.term_bytes, b"fox");
        assert_eq!(occurrences.term_lengths, [3]);
        assert_eq!(occurrences.byte_starts, [5]);
        assert_eq!(occurrences.byte_lengths, [3]);
    }

    #[test]
    fn query_analysis_converts_one_stem_per_native_term() {
        let analysis = QueryAnalysis {
            terms: vec![QueryTerm {
                stems: vec!["running".to_owned()],
                position: 4,
            }],
        };

        let terms = text_query_terms(&analysis).expect("valid query term");
        assert_eq!(terms.term_bytes, b"running");
        assert_eq!(terms.term_lengths, [7]);
    }

    #[test]
    fn native_lengths_reject_u32_overflow() {
        if usize::BITS > 32 {
            assert!(length_u32(u32::MAX as usize + 1, "test").is_err());
        }
    }

    #[test]
    fn query_conversion_rejects_zero_stems() {
        let analysis = QueryAnalysis {
            terms: vec![QueryTerm {
                stems: Vec::new(),
                position: 0,
            }],
        };

        assert!(text_query_terms(&analysis).is_err());
    }

    #[test]
    fn query_conversion_rejects_multiple_stems() {
        let analysis = QueryAnalysis {
            terms: vec![QueryTerm {
                stems: vec!["first".to_owned(), "second".to_owned()],
                position: 0,
            }],
        };

        assert!(text_query_terms(&analysis).is_err());
    }

    #[test]
    fn term_queries_require_one_analyzed_position() {
        let no_terms = QueryAnalysis { terms: Vec::new() };
        assert!(validate_term_query(&no_terms).is_err());

        let multiple = QueryAnalysis {
            terms: vec![
                QueryTerm {
                    stems: vec!["one".to_owned()],
                    position: 0,
                },
                QueryTerm {
                    stems: vec!["two".to_owned()],
                    position: 1,
                },
            ],
        };
        assert!(validate_term_query(&multiple).is_err());

        let one = QueryAnalysis {
            terms: vec![QueryTerm {
                stems: vec!["one".to_owned()],
                position: 0,
            }],
        };
        assert!(validate_term_query(&one).is_ok());
    }
}
