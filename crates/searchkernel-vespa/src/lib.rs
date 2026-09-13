//! Safe Vespa-specific adapter for the private Search Kernel ABI.

mod linguistics;
mod schema;

pub use linguistics::{
    DocumentAnalysis, DocumentTerm, Language, Linguistics, LinguisticsOptions, QueryAnalysis,
    QueryTerm, StemMode,
};
pub use schema::{ConfigCompiler, Field, FieldType, Schema};

use searchkernel_vespa_sys as sys;
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

pub struct Engine {
    handle: NonNull<sys::sk_engine>,
}

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

    /// Insert one integer-valued document.
    pub fn put(&self, document_id: &str, field: &str, value: i32) -> Result<(), Error> {
        let document_id = cstring(document_id)?;
        let field = cstring(field)?;
        let mut document = ptr::null_mut();
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_document_create(
                document_id.as_ptr(),
                field.as_ptr(),
                value,
                &mut document,
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)?;
        let document = NativeDocument(require_ptr(document, "document")?);
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_engine_put(
                self.handle.as_ptr(),
                document.0.as_ptr(),
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)
    }

    /// Update one integer field in an existing document.
    pub fn update(&self, document_id: &str, field: &str, value: i32) -> Result<(), Error> {
        let document_id = cstring(document_id)?;
        let field = cstring(field)?;
        let mut update = ptr::null_mut();
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_document_update_create(
                document_id.as_ptr(),
                field.as_ptr(),
                value,
                &mut update,
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)?;
        let update = NativeUpdate(require_ptr(update, "document update")?);
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_engine_update(
                self.handle.as_ptr(),
                update.0.as_ptr(),
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)
    }

    /// Remove a document and report whether it existed.
    pub fn remove(&self, document_id: &str) -> Result<bool, Error> {
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

    /// Search an integer field and return matching document IDs.
    pub fn search(&self, field: &str, value: i32, limit: u32) -> Result<Vec<String>, Error> {
        let field = cstring(field)?;
        let mut request = ptr::null_mut();
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_search_request_create(
                field.as_ptr(),
                value,
                limit,
                &mut request,
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)?;
        let request = NativeRequest(require_ptr(request, "search request")?);

        let mut result = ptr::null_mut();
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_engine_search(
                self.handle.as_ptr(),
                request.0.as_ptr(),
                &mut result,
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)?;
        let result = NativeResult(require_ptr(result, "search result")?);
        let size = unsafe { sys::sk_search_result_size(result.0.as_ptr()) } as usize;
        let mut ids = Vec::with_capacity(size);
        for index in 0..size {
            let mut id_ptr = ptr::null();
            let mut id_length = 0;
            let mut error = NativeError::new();
            let status = unsafe {
                sys::sk_search_result_id(
                    result.0.as_ptr(),
                    index as u32,
                    &mut id_ptr,
                    &mut id_length,
                    error.as_mut_ptr(),
                    error.capacity(),
                )
            };
            check(status, &error)?;
            let id_ptr = if id_ptr.is_null() {
                return Err(Error::message("native result returned a null document ID"));
            } else {
                id_ptr
            };
            let bytes =
                unsafe { std::slice::from_raw_parts(id_ptr.cast::<u8>(), id_length as usize) };
            ids.push(
                String::from_utf8(bytes.to_vec())
                    .map_err(|_| Error::message("native result returned invalid UTF-8"))?,
            );
        }
        Ok(ids)
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
            TextQuery::Term { field, text } => (1, field, text),
            TextQuery::Phrase { field, text } => (2, field, text),
        };
        let analysis = Linguistics.analyze_query(text, LinguisticsOptions::default());
        if kind == 1 {
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

struct NativeRequest(NonNull<sys::sk_search_request>);
impl Drop for NativeRequest {
    fn drop(&mut self) {
        unsafe { sys::sk_search_request_destroy(self.0.as_ptr()) };
    }
}

struct NativeResult(NonNull<sys::sk_search_result>);
impl Drop for NativeResult {
    fn drop(&mut self) {
        unsafe { sys::sk_search_result_destroy(self.0.as_ptr()) };
    }
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
