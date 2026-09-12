//! Safe Vespa-specific adapter for the private Search Kernel ABI.

mod linguistics;
mod schema;

pub use linguistics::{
    DocumentAnalysis, DocumentTerm, Language, Linguistics, LinguisticsOptions, QueryAnalysis,
    QueryTerm, StemMode,
};
pub use schema::{ConfigCompiler, Field, FieldType, Schema};

use searchkernel_vespa_sys as sys;
use std::ffi::{CStr, CString, NulError};
use std::path::Path;
use std::ptr::{self, NonNull};

const EXPECTED_ABI: u32 = 1;

#[derive(Debug, Clone, Copy)]
pub struct TextDocument<'a> {
    pub id: &'a str,
    pub field: &'a str,
    pub text: &'a str,
}

#[derive(Debug, Clone, Copy)]
pub enum TextQuery<'a> {
    Term { field: &'a str, text: &'a str },
    Phrase { field: &'a str, text: &'a str },
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextHit {
    pub document_id: String,
    pub score: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextSearchResult {
    pub hits: Vec<TextHit>,
}

#[derive(Debug)]
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
    let code = error.0.code;
    let message = if error.0.message.is_null() {
        format!("native operation failed with status {status}")
    } else {
        unsafe { CStr::from_ptr(error.0.message) }
            .to_string_lossy()
            .into_owned()
    };
    Error::native(code, message)
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
            )
        };
        check(status, &error)?;
        Ok(Self {
            handle: require_ptr(handle, "engine")?,
        })
    }

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
            )
        };
        check(status, &error)
    }

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
            )
        };
        check(status, &error)?;
        let update = NativeUpdate(require_ptr(update, "document update")?);
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_engine_update(self.handle.as_ptr(), update.0.as_ptr(), error.as_mut_ptr())
        };
        check(status, &error)
    }

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
            )
        };
        check(status, &error)?;
        Ok(was_found != 0)
    }

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
                occurrences.as_ptr(),
                length_u32(occurrences.len(), "text occurrence")?,
                &mut native_document,
                error.as_mut_ptr(),
            )
        };
        check(status, &error)?;
        let native_document = NativeTextDocument(require_ptr(native_document, "text document")?);
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_engine_put_text(
                self.handle.as_ptr(),
                native_document.0.as_ptr(),
                error.as_mut_ptr(),
            )
        };
        check(status, &error)
    }

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
                occurrences.as_ptr(),
                length_u32(occurrences.len(), "text occurrence")?,
                &mut native_update,
                error.as_mut_ptr(),
            )
        };
        check(status, &error)?;
        let native_update = NativeTextUpdate(require_ptr(native_update, "text update")?);
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_engine_update_text(
                self.handle.as_ptr(),
                native_update.0.as_ptr(),
                error.as_mut_ptr(),
            )
        };
        check(status, &error)
    }

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
                terms.as_ptr(),
                length_u32(terms.len(), "text query term")?,
                rank_profile.as_ptr(),
                &mut native_request,
                error.as_mut_ptr(),
            )
        };
        check(status, &error)?;
        let native_request = NativeTextRequest(require_ptr(native_request, "text request")?);

        let mut native_result = ptr::null_mut();
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_engine_search_text(
                self.handle.as_ptr(),
                native_request.0.as_ptr(),
                &mut native_result,
                error.as_mut_ptr(),
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

/// Owns the native message held by `sk_error` until the guard is dropped.
///
/// The native ABI requires the caller to initialize this value and destroy it
/// before reuse or discard. Keeping that rule in `Drop` also covers successful
/// calls and early returns, not only native failures.
struct NativeError(sys::sk_error);

impl NativeError {
    fn new() -> Self {
        Self(sys::sk_error {
            code: 0,
            message: ptr::null_mut(),
        })
    }

    fn as_mut_ptr(&mut self) -> *mut sys::sk_error {
        &mut self.0
    }
}

impl Drop for NativeError {
    fn drop(&mut self) {
        unsafe { sys::sk_error_destroy(&mut self.0) };
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

fn text_occurrences(analysis: &DocumentAnalysis) -> Result<Vec<sys::sk_text_occurrence>, Error> {
    analysis
        .terms
        .iter()
        .map(|term| {
            Ok(sys::sk_text_occurrence {
                term: term.term.as_bytes().as_ptr(),
                term_length: length_u32(term.term.len(), "text term")?,
                byte_start: length_u32(term.byte_start, "text span")?,
                byte_length: length_u32(term.byte_len, "text span")?,
                position: term.position,
            })
        })
        .collect()
}

fn text_query_terms(analysis: &QueryAnalysis) -> Result<Vec<sys::sk_text_query_term>, Error> {
    analysis
        .terms
        .iter()
        .map(|term| {
            let stem = term.stems.first().ok_or_else(|| {
                Error::message("M4 query analysis returned no stem for a query position")
            })?;
            if term.stems.len() != 1 {
                return Err(Error::message(
                    "M4 query analysis returned multiple stems for a query position",
                ));
            }
            Ok(sys::sk_text_query_term {
                term: stem.as_bytes().as_ptr(),
                term_length: length_u32(stem.len(), "text query term")?,
                position: term.position,
            })
        })
        .collect()
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
    fn document_analysis_converts_to_native_occurrences() {
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
        assert_eq!(occurrences.len(), 1);
        assert_eq!(occurrences[0].term_length, 3);
        assert_eq!(occurrences[0].byte_start, 5);
        assert_eq!(occurrences[0].byte_length, 3);
        assert_eq!(occurrences[0].position, 2);
        let term = unsafe {
            std::slice::from_raw_parts(occurrences[0].term, occurrences[0].term_length as usize)
        };
        assert_eq!(term, b"fox");
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
        assert_eq!(terms.len(), 1);
        assert_eq!(terms[0].term_length, 7);
        assert_eq!(terms[0].position, 4);
        let term =
            unsafe { std::slice::from_raw_parts(terms[0].term, terms[0].term_length as usize) };
        assert_eq!(term, b"running");
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
