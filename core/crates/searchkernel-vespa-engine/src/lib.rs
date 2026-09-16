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
pub use schema::{ConfigCompiler, Field, FieldType, Schema};

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
use std::path::Path;
use std::ptr::{self, NonNull};

const EXPECTED_ABI: u32 = sys::EXPECTED_ABI;

#[derive(Debug, Clone, Copy)]
/// A text document passed to the native Vespa adapter.
pub struct TextDocument<'a> {
    pub id: &'a str,
    pub field: &'a str,
    pub text: &'a str,
}

#[derive(Debug, Clone, Copy)]
/// A text query supported by the native Vespa adapter.
pub enum TextQuery<'a> {
    Term { field: &'a str, text: &'a str },
    Phrase { field: &'a str, text: &'a str },
}

#[derive(Debug, Clone, PartialEq)]
/// One hit returned by a text search.
pub struct TextHit {
    pub document_id: String,
    pub score: f64,
}

#[derive(Debug, Clone, PartialEq)]
/// The result of a text search.
pub struct TextSearchResult {
    pub hits: Vec<TextHit>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortOrder {
    Ascending,
    Descending,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SortField {
    pub field: String,
    pub order: SortOrder,
}

impl SortField {
    pub fn new(field: impl Into<String>, order: SortOrder) -> Self {
        Self {
            field: field.into(),
            order,
        }
    }

    pub fn ascending(field: impl Into<String>) -> Self {
        Self::new(field, SortOrder::Ascending)
    }

    pub fn descending(field: impl Into<String>) -> Self {
        Self::new(field, SortOrder::Descending)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SummaryRequest {
    None,
    Default,
    Fields(Vec<String>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct LocalSearchRequest {
    pub query: Query,
    pub rank_profile: String,
    pub limit: u32,
    pub offset: u32,
    pub sort: Vec<SortField>,
    pub summary: SummaryRequest,
}

impl LocalSearchRequest {
    pub fn new(query: Query) -> Self {
        Self {
            query,
            rank_profile: "default".to_owned(),
            limit: 10,
            offset: 0,
            sort: Vec::new(),
            summary: SummaryRequest::None,
        }
    }

    pub fn rank_profile(mut self, rank_profile: impl Into<String>) -> Self {
        self.rank_profile = rank_profile.into();
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

    pub fn sort_by(mut self, sort: SortField) -> Self {
        self.sort.push(sort);
        self
    }

    pub fn summary(mut self, summary: SummaryRequest) -> Self {
        self.summary = summary;
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LocalHit {
    pub document_id: String,
    pub score: f64,
    pub summary: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LocalSearchResult {
    pub total_hit_count: u64,
    pub hits: Vec<LocalHit>,
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
}

#[derive(Debug, Clone, PartialEq)]
pub struct SearchResult {
    pub total_hits: u64,
    pub total_hits_relation: TotalHitsRelation,
    pub hits: Vec<Hit>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SearchRequest {
    pub query: Query,
    pub rank_profile: String,
    pub limit: u32,
    pub offset: u32,
    pub sort: Vec<SortField>,
    pub summary: SummaryRequest,
}

impl SearchRequest {
    pub fn new(query: Query) -> Self {
        Self {
            query,
            rank_profile: "default".to_owned(),
            limit: 10,
            offset: 0,
            sort: Vec::new(),
            summary: SummaryRequest::None,
        }
    }

    pub fn rank_profile(mut self, rank_profile: impl Into<String>) -> Self {
        self.rank_profile = rank_profile.into();
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
}

#[derive(Debug, Clone, PartialEq)]
pub struct AttributeDocument {
    pub document_id: String,
    pub fields: Vec<(String, Value)>,
}

impl AttributeDocument {
    pub fn new(document_id: impl Into<String>) -> Self {
        Self {
            document_id: document_id.into(),
            fields: Vec::new(),
        }
    }

    pub fn field(mut self, field: impl Into<String>, value: Value) -> Self {
        self.fields.push((field.into(), value));
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AttributeUpdate {
    pub document_id: String,
    pub assignments: Vec<(String, Value)>,
}

impl AttributeUpdate {
    pub fn new(document_id: impl Into<String>) -> Self {
        Self {
            document_id: document_id.into(),
            assignments: Vec::new(),
        }
    }

    pub fn assign(mut self, field: impl Into<String>, value: Value) -> Self {
        self.assignments.push((field.into(), value));
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Aggregation {
    Sum(String),
    Min(String),
    Max(String),
    Average(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct LocalGroupingRequest {
    pub query: Query,
    pub group_by: String,
    pub aggregations: Vec<Aggregation>,
}

impl LocalGroupingRequest {
    pub fn new(query: Query, group_by: impl Into<String>) -> Self {
        Self {
            query,
            group_by: group_by.into(),
            aggregations: Vec::new(),
        }
    }

    pub fn aggregation(mut self, aggregation: Aggregation) -> Self {
        self.aggregations.push(aggregation);
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum AggregateValue {
    Integer(i64),
    Float(f64),
}

#[derive(Debug, Clone, PartialEq)]
pub struct LocalGroup {
    pub key: Value,
    pub count: u64,
    pub aggregations: Vec<AggregateValue>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LocalGroupingResult {
    pub groups: Vec<LocalGroup>,
}

#[derive(Debug)]
/// An error returned while calling the native adapter.
pub struct Error {
    code: i32,
    message: String,
}

impl Error {
    fn message(message: impl Into<String>) -> Self {
        Self {
            code: 0,
            message: message.into(),
        }
    }

    fn native(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
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
    let message = String::from_utf8_lossy(&message_bytes).into_owned();
    let message = if message.is_empty() {
        format!("native operation failed with status {status}")
    } else {
        message
    };
    Error::native(status, message)
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
        let local = LocalSearchRequest {
            query: request.query,
            rank_profile: request.rank_profile,
            limit: request.limit,
            offset: request.offset,
            sort: request.sort,
            summary: request.summary,
        };
        let result = self.search_local(&local)?;
        Ok(SearchResult {
            total_hits: result.total_hit_count,
            total_hits_relation: TotalHitsRelation::Exact,
            hits: result
                .hits
                .into_iter()
                .map(|hit| Hit {
                    document_id: hit.document_id,
                    score: hit.score,
                    fields: hit
                        .summary
                        .into_iter()
                        .map(|(name, value)| (name, FieldValue::Scalar(scalar_value(value))))
                        .collect(),
                })
                .collect(),
        })
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

    /// Insert a text document after applying Vespa-compatible linguistics.
    pub fn put_text(&self, document: TextDocument<'_>) -> Result<(), Error> {
        let analysis = Linguistics.analyze_document(document.text, LinguisticsOptions::default());
        let occurrences = text_occurrences(&analysis)?;
        let id = cstring(document.id)?;
        let field = cstring(document.field)?;
        let mut native_document = ptr::null_mut();
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_text_document_create(
                id.as_ptr(),
                field.as_ptr(),
                document.text.as_bytes().as_ptr(),
                length_u32(document.text.len(), "text")?,
                &mut native_document,
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)?;
        let native_document = NativeTextDocument(require_ptr(native_document, "text document")?);
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_text_document_add_occurrences(
                native_document.0.as_ptr(),
                occurrences.term_bytes.as_ptr(),
                length_u32(occurrences.term_bytes.len(), "text term bytes")?,
                occurrences.term_lengths.as_ptr(),
                occurrences.byte_starts.as_ptr(),
                occurrences.byte_lengths.as_ptr(),
                length_u32(occurrences.term_lengths.len(), "text occurrence")?,
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)?;
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_engine_put_text(
                self.handle.as_ptr(),
                native_document.0.as_ptr(),
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)
    }

    /// Update a text field after applying Vespa-compatible linguistics.
    pub fn update_text(&self, document: TextDocument<'_>) -> Result<(), Error> {
        let analysis = Linguistics.analyze_document(document.text, LinguisticsOptions::default());
        let occurrences = text_occurrences(&analysis)?;
        let id = cstring(document.id)?;
        let field = cstring(document.field)?;
        let mut native_update = ptr::null_mut();
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_text_update_create(
                id.as_ptr(),
                field.as_ptr(),
                document.text.as_bytes().as_ptr(),
                length_u32(document.text.len(), "text")?,
                &mut native_update,
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)?;
        let native_update = NativeTextUpdate(require_ptr(native_update, "text update")?);
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_text_update_add_occurrences(
                native_update.0.as_ptr(),
                occurrences.term_bytes.as_ptr(),
                length_u32(occurrences.term_bytes.len(), "text term bytes")?,
                occurrences.term_lengths.as_ptr(),
                occurrences.byte_starts.as_ptr(),
                occurrences.byte_lengths.as_ptr(),
                length_u32(occurrences.term_lengths.len(), "text occurrence")?,
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)?;
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_engine_update_text(
                self.handle.as_ptr(),
                native_update.0.as_ptr(),
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)
    }

    /// Search a text field using the requested query form.
    pub fn search_text(
        &self,
        query: TextQuery<'_>,
        rank_profile: &str,
    ) -> Result<TextSearchResult, Error> {
        let (kind, field, text) = match query {
            TextQuery::Term { field, text } => (sys::SK_TEXT_QUERY_TERM, field, text),
            TextQuery::Phrase { field, text } => (sys::SK_TEXT_QUERY_PHRASE, field, text),
        };
        let analysis = Linguistics.analyze_query(text, LinguisticsOptions::default());
        if kind == sys::SK_TEXT_QUERY_TERM {
            validate_term_query(&analysis)?;
        }
        let terms = text_query_terms(&analysis)?;
        let field = cstring(field)?;
        let rank_profile = cstring(rank_profile)?;
        let mut native_request = ptr::null_mut();
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_text_request_create(
                kind,
                field.as_ptr(),
                rank_profile.as_ptr(),
                &mut native_request,
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)?;
        let native_request = NativeTextRequest(require_ptr(native_request, "text request")?);
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_text_request_add_terms(
                native_request.0.as_ptr(),
                terms.term_bytes.as_ptr(),
                length_u32(terms.term_bytes.len(), "text query term bytes")?,
                terms.term_lengths.as_ptr(),
                length_u32(terms.term_lengths.len(), "text query term")?,
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)?;

        let mut native_result = ptr::null_mut();
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_engine_search_text(
                self.handle.as_ptr(),
                native_request.0.as_ptr(),
                &mut native_result,
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)?;
        let native_result = NativeTextResult(require_ptr(native_result, "text result")?);
        let size = unsafe { sys::sk_text_result_size(native_result.0.as_ptr()) } as usize;
        let mut hits = Vec::with_capacity(size);
        for index in 0..size {
            let mut id_ptr = ptr::null();
            let mut id_length = 0;
            let mut score = 0.0;
            let mut error = NativeError::new();
            let status = unsafe {
                sys::sk_text_result_hit(
                    native_result.0.as_ptr(),
                    index as u32,
                    &mut id_ptr,
                    &mut id_length,
                    &mut score,
                    error.as_mut_ptr(),
                    error.capacity(),
                )
            };
            check(status, &error)?;
            if id_ptr.is_null() {
                return Err(Error::message(
                    "native text result returned a null document ID",
                ));
            }
            let bytes =
                unsafe { std::slice::from_raw_parts(id_ptr.cast::<u8>(), id_length as usize) };
            hits.push(TextHit {
                document_id: String::from_utf8(bytes.to_vec())
                    .map_err(|_| Error::message("native text result returned invalid UTF-8"))?,
                score,
            });
        }
        Ok(TextSearchResult { hits })
    }

    /// Search using the typed low-level Vespa query representation.
    pub fn search_query(
        &self,
        query: &Query,
        rank_profile: &str,
    ) -> Result<TextSearchResult, Error> {
        self.search_query_with_source(query, rank_profile, "*")
    }

    pub fn search_local(&self, request: &LocalSearchRequest) -> Result<LocalSearchResult, Error> {
        query::validate_query(&request.query)?;
        let native_query = self.create_native_query(&request.query)?;
        let rank_profile = cstring(&request.rank_profile)?;
        let summary_kind = match request.summary {
            SummaryRequest::None => sys::SK_SUMMARY_NONE,
            SummaryRequest::Default => sys::SK_SUMMARY_DEFAULT,
            SummaryRequest::Fields(_) => sys::SK_SUMMARY_FIELDS,
        };
        let mut native_request = ptr::null_mut();
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_local_search_request_create(
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
            NativeLocalSearchRequest(require_ptr(native_request, "local search request")?);
        for sort in &request.sort {
            let field = cstring(&sort.field)?;
            let order = match sort.order {
                SortOrder::Ascending => sys::SK_SORT_ASCENDING,
                SortOrder::Descending => sys::SK_SORT_DESCENDING,
            };
            let mut error = NativeError::new();
            let status = unsafe {
                sys::sk_local_search_request_add_sort(
                    native_request.0.as_ptr(),
                    field.as_ptr(),
                    order,
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
                    sys::sk_local_search_request_add_summary_field(
                        native_request.0.as_ptr(),
                        field.as_ptr(),
                        error.as_mut_ptr(),
                        error.capacity(),
                    )
                };
                check(status, &error)?;
            }
        }
        let mut native_result = ptr::null_mut();
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_engine_search_local(
                self.handle.as_ptr(),
                native_query.0.as_ptr(),
                native_request.0.as_ptr(),
                &mut native_result,
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)?;
        read_local_search_result(require_ptr(native_result, "local search result")?)
    }

    pub fn put_attributes(&self, document: &AttributeDocument) -> Result<(), Error> {
        if document.fields.is_empty() {
            return Err(Error::message(
                "request must contain at least one document field",
            ));
        }
        let id = cstring(&document.document_id)?;
        let mut native_document = ptr::null_mut();
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_attribute_document_create(
                id.as_ptr(),
                &mut native_document,
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)?;
        let native_document =
            NativeAttributeDocument(require_ptr(native_document, "attribute document")?);
        for (field, value) in &document.fields {
            add_attribute_value(native_document.0.as_ptr(), field, value, false)?;
        }
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_engine_put_attributes(
                self.handle.as_ptr(),
                native_document.0.as_ptr(),
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)
    }

    pub fn update_attributes(&self, update: &AttributeUpdate) -> Result<(), Error> {
        if update.assignments.is_empty() {
            return Err(Error::message(
                "request must contain at least one update assignment",
            ));
        }
        let id = cstring(&update.document_id)?;
        let mut native_update = ptr::null_mut();
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_attribute_update_create(
                id.as_ptr(),
                &mut native_update,
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)?;
        let native_update = NativeAttributeUpdate(require_ptr(native_update, "attribute update")?);
        for (field, value) in &update.assignments {
            add_attribute_value(native_update.0.as_ptr(), field, value, true)?;
        }
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_engine_update_attributes(
                self.handle.as_ptr(),
                native_update.0.as_ptr(),
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)
    }

    pub fn group_local(
        &self,
        request: &LocalGroupingRequest,
    ) -> Result<LocalGroupingResult, Error> {
        query::validate_query(&request.query)?;
        let native_query = self.create_native_query(&request.query)?;
        let group_field = cstring(&request.group_by)?;
        let mut native_request = ptr::null_mut();
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_local_grouping_request_create(
                group_field.as_ptr(),
                &mut native_request,
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)?;
        let native_request =
            NativeGroupingRequest(require_ptr(native_request, "grouping request")?);
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
                sys::sk_local_grouping_request_add_aggregation(
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
            sys::sk_engine_group_local(
                self.handle.as_ptr(),
                native_query.0.as_ptr(),
                native_request.0.as_ptr(),
                &mut native_result,
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)?;
        read_grouping_result(require_ptr(native_result, "grouping result")?)
    }

    fn create_native_query(&self, query: &Query) -> Result<NativeQuery, Error> {
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
        lower_query(&native_query, query)?;
        Ok(native_query)
    }

    /// Parse and search a supported YQL query through the typed query compiler.
    pub fn search_yql(&self, yql: &str, rank_profile: &str) -> Result<TextSearchResult, Error> {
        let parsed = query::parse_yql(yql)?;
        self.search_query_with_source(&parsed.query, rank_profile, &parsed.source)
    }

    fn search_query_with_source(
        &self,
        query: &Query,
        rank_profile: &str,
        source: &str,
    ) -> Result<TextSearchResult, Error> {
        query::validate_query(query)?;
        let rank_profile = cstring(rank_profile)?;
        let source = cstring(source)?;
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
        lower_query(&native_query, query)?;

        let mut native_result = ptr::null_mut();
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_engine_search_query(
                self.handle.as_ptr(),
                native_query.0.as_ptr(),
                rank_profile.as_ptr(),
                source.as_ptr(),
                &mut native_result,
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)?;
        read_text_result(native_result)
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

struct NativeTextDocument(NonNull<sys::sk_text_document>);
impl Drop for NativeTextDocument {
    fn drop(&mut self) {
        unsafe { sys::sk_text_document_destroy(self.0.as_ptr()) };
    }
}

struct NativeTextUpdate(NonNull<sys::sk_text_update>);
impl Drop for NativeTextUpdate {
    fn drop(&mut self) {
        unsafe { sys::sk_text_update_destroy(self.0.as_ptr()) };
    }
}

struct NativeTextRequest(NonNull<sys::sk_text_request>);
impl Drop for NativeTextRequest {
    fn drop(&mut self) {
        unsafe { sys::sk_text_request_destroy(self.0.as_ptr()) };
    }
}

struct NativeQuery(NonNull<sys::sk_query>);
impl Drop for NativeQuery {
    fn drop(&mut self) {
        unsafe { sys::sk_query_destroy(self.0.as_ptr()) };
    }
}

struct NativeLocalSearchRequest(NonNull<sys::sk_local_search_request>);
impl Drop for NativeLocalSearchRequest {
    fn drop(&mut self) {
        unsafe { sys::sk_local_search_request_destroy(self.0.as_ptr()) };
    }
}

struct NativeAttributeDocument(NonNull<sys::sk_attribute_document>);
impl Drop for NativeAttributeDocument {
    fn drop(&mut self) {
        unsafe { sys::sk_attribute_document_destroy(self.0.as_ptr()) };
    }
}

struct NativeAttributeUpdate(NonNull<sys::sk_attribute_update>);
impl Drop for NativeAttributeUpdate {
    fn drop(&mut self) {
        unsafe { sys::sk_attribute_update_destroy(self.0.as_ptr()) };
    }
}

struct NativeGroupingRequest(NonNull<sys::sk_local_grouping_request>);
impl Drop for NativeGroupingRequest {
    fn drop(&mut self) {
        unsafe { sys::sk_local_grouping_request_destroy(self.0.as_ptr()) };
    }
}

fn scalar_value(value: Value) -> ScalarValue {
    match value {
        Value::String(value) => ScalarValue::String(value),
        Value::Integer(value) => ScalarValue::Integer(value),
        Value::Float(value) => ScalarValue::Float(value),
        Value::Boolean(value) => ScalarValue::Boolean(value),
    }
}

fn add_attribute_value<T>(
    handle: *mut T,
    field: &str,
    value: &Value,
    update: bool,
) -> Result<(), Error> {
    let field = cstring(field)?;
    let mut error = NativeError::new();
    let status = unsafe {
        match (update, value) {
            (false, Value::String(value)) => {
                let value = cstring(value)?;
                sys::sk_attribute_document_add_string(
                    handle.cast(),
                    field.as_ptr(),
                    value.as_ptr(),
                    error.as_mut_ptr(),
                    error.capacity(),
                )
            }
            (false, Value::Integer(value)) => sys::sk_attribute_document_add_integer(
                handle.cast(),
                field.as_ptr(),
                *value,
                error.as_mut_ptr(),
                error.capacity(),
            ),
            (false, Value::Float(value)) => sys::sk_attribute_document_add_float(
                handle.cast(),
                field.as_ptr(),
                *value,
                error.as_mut_ptr(),
                error.capacity(),
            ),
            (false, Value::Boolean(value)) => sys::sk_attribute_document_add_boolean(
                handle.cast(),
                field.as_ptr(),
                u8::from(*value),
                error.as_mut_ptr(),
                error.capacity(),
            ),
            (true, Value::String(value)) => {
                let value = cstring(value)?;
                sys::sk_attribute_update_add_string(
                    handle.cast(),
                    field.as_ptr(),
                    value.as_ptr(),
                    error.as_mut_ptr(),
                    error.capacity(),
                )
            }
            (true, Value::Integer(value)) => sys::sk_attribute_update_add_integer(
                handle.cast(),
                field.as_ptr(),
                *value,
                error.as_mut_ptr(),
                error.capacity(),
            ),
            (true, Value::Float(value)) => sys::sk_attribute_update_add_float(
                handle.cast(),
                field.as_ptr(),
                *value,
                error.as_mut_ptr(),
                error.capacity(),
            ),
            (true, Value::Boolean(value)) => sys::sk_attribute_update_add_boolean(
                handle.cast(),
                field.as_ptr(),
                u8::from(*value),
                error.as_mut_ptr(),
                error.capacity(),
            ),
        }
    };
    check(status, &error)
}

fn read_local_search_result(
    pointer: NonNull<sys::sk_local_search_result>,
) -> Result<LocalSearchResult, Error> {
    let result = NativeLocalSearchResult(pointer);
    let hit_count = unsafe { sys::sk_local_search_result_hit_count(result.0.as_ptr()) } as usize;
    let total_hit_count = unsafe { sys::sk_local_search_result_total_hit_count(result.0.as_ptr()) };
    let mut hits = Vec::with_capacity(hit_count);
    for index in 0..hit_count {
        let mut id = ptr::null();
        let mut id_length = 0;
        let mut score = 0.0;
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_local_search_result_hit(
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
            unsafe { sys::sk_local_search_result_summary_count(result.0.as_ptr(), index as u32) }
                as usize;
        let mut summary = BTreeMap::new();
        for summary_index in 0..summaries {
            let mut name = ptr::null();
            let mut name_length = 0;
            let mut error = NativeError::new();
            let status = unsafe {
                sys::sk_local_search_result_summary_name(
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
                sys::sk_local_search_result_summary_value(
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
        hits.push(LocalHit {
            document_id,
            score,
            summary,
        });
    }
    Ok(LocalSearchResult {
        total_hit_count,
        hits,
    })
}

struct NativeLocalSearchResult(NonNull<sys::sk_local_search_result>);
impl Drop for NativeLocalSearchResult {
    fn drop(&mut self) {
        unsafe { sys::sk_local_search_result_destroy(self.0.as_ptr()) };
    }
}

fn read_grouping_result(
    pointer: NonNull<sys::sk_local_grouping_result>,
) -> Result<LocalGroupingResult, Error> {
    let result = NativeGroupingResult(pointer);
    let group_count =
        unsafe { sys::sk_local_grouping_result_group_count(result.0.as_ptr()) } as usize;
    let mut groups = Vec::with_capacity(group_count);
    for group_index in 0..group_count {
        let mut key_kind = 0;
        let mut key_string = ptr::null();
        let mut key_string_length = 0;
        let mut key_integer = 0;
        let mut count = 0;
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_local_grouping_result_group(
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
            sys::sk_local_grouping_result_aggregation_count(result.0.as_ptr(), group_index as u32)
        } as usize;
        let mut aggregations = Vec::with_capacity(aggregation_count);
        for aggregation_index in 0..aggregation_count {
            let mut kind = 0;
            let mut integer = 0;
            let mut float = 0.0;
            let mut error = NativeError::new();
            let status = unsafe {
                sys::sk_local_grouping_result_aggregation(
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
        groups.push(LocalGroup {
            key,
            count,
            aggregations,
        });
    }
    Ok(LocalGroupingResult { groups })
}

struct NativeGroupingResult(NonNull<sys::sk_local_grouping_result>);
impl Drop for NativeGroupingResult {
    fn drop(&mut self) {
        unsafe { sys::sk_local_grouping_result_destroy(self.0.as_ptr()) };
    }
}

fn lower_query(native: &NativeQuery, query: &Query) -> Result<(), Error> {
    match query {
        Query::True => {
            let mut error = NativeError::new();
            let status = unsafe {
                sys::sk_query_add_true(native.0.as_ptr(), error.as_mut_ptr(), error.capacity())
            };
            check(status, &error)
        }
        Query::Term { field, text } => {
            let analysis = Linguistics.analyze_query(text, LinguisticsOptions::default());
            validate_term_query(&analysis)
                .map_err(|error| query::invalid_query(format!("term analysis failed: {error}")))?;
            let terms = text_query_terms(&analysis)?;
            add_native_text(native, sys::SK_QUERY_TEXT_TERM, field, &terms)
        }
        Query::Phrase { field, text } => {
            let analysis = Linguistics.analyze_query(text, LinguisticsOptions::default());
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
                lower_query(native, child)?;
            }
            Ok(())
        }
        Query::Or(children) => {
            add_native_operator(native, sys::SK_QUERY_NODE_OR, children.len())?;
            for child in children {
                lower_query(native, child)?;
            }
            Ok(())
        }
        Query::Not(child) => {
            add_native_operator(native, sys::SK_QUERY_NODE_NOT, 1)?;
            lower_query(native, child)
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

fn read_text_result(pointer: *mut sys::sk_text_result) -> Result<TextSearchResult, Error> {
    let result = NativeTextResult(require_ptr(pointer, "text result")?);
    let size = unsafe { sys::sk_text_result_size(result.0.as_ptr()) } as usize;
    let mut hits = Vec::with_capacity(size);
    for index in 0..size {
        let mut id_ptr = ptr::null();
        let mut id_length = 0;
        let mut score = 0.0;
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_text_result_hit(
                result.0.as_ptr(),
                index as u32,
                &mut id_ptr,
                &mut id_length,
                &mut score,
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)?;
        if id_ptr.is_null() {
            return Err(Error::message(
                "native text result returned a null document ID",
            ));
        }
        let bytes = unsafe { std::slice::from_raw_parts(id_ptr.cast::<u8>(), id_length as usize) };
        hits.push(TextHit {
            document_id: String::from_utf8(bytes.to_vec())
                .map_err(|_| Error::message("native text result returned invalid UTF-8"))?,
            score,
        });
    }
    Ok(TextSearchResult { hits })
}

struct NativeTextResult(NonNull<sys::sk_text_result>);
impl Drop for NativeTextResult {
    fn drop(&mut self) {
        unsafe { sys::sk_text_result_destroy(self.0.as_ptr()) };
    }
}

#[cfg(test)]
mod tests {
    use super::{
        length_u32, text_occurrences, text_query_terms, validate_term_query, DocumentAnalysis,
        DocumentTerm, QueryAnalysis, QueryTerm,
    };

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
