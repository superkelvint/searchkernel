use searchkernel_sys as sys;
use std::ffi::{CStr, CString, NulError};
use std::fmt;
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::ptr;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Error {
    message: String,
}

impl Error {
    fn message(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    fn from_nul(error: NulError) -> Self {
        Self::message(format!("string contains an interior NUL: {error}"))
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for Error {}

fn native_error(error: &mut sys::sk_error, status: i32) -> Error {
    let message = if error.message.is_null() {
        format!("native operation failed with status {status}")
    } else {
        // The adapter owns a NUL-terminated diagnostic string until destroy.
        unsafe { CStr::from_ptr(error.message) }
            .to_string_lossy()
            .into_owned()
    };
    unsafe { sys::sk_error_destroy(error) };
    Error::message(message)
}

fn check(status: i32, error: &mut sys::sk_error) -> Result<()> {
    if status == 0 {
        Ok(())
    } else {
        Err(native_error(error, status))
    }
}

fn cstring(value: &str) -> Result<CString> {
    CString::new(value).map_err(Error::from_nul)
}

fn path_cstring(path: &Path) -> Result<CString> {
    #[cfg(unix)]
    {
        CString::new(path.as_os_str().as_bytes()).map_err(Error::from_nul)
    }
    #[cfg(not(unix))]
    {
        cstring(&path.to_string_lossy())
    }
}

pub struct EngineConfig {
    directory: PathBuf,
}

impl EngineConfig {
    pub fn from_directory(path: impl AsRef<Path>) -> Result<Self> {
        let directory = path.as_ref().to_path_buf();
        if !directory.is_dir() {
            return Err(Error::message(format!(
                "configuration directory does not exist: {}",
                directory.display()
            )));
        }
        Ok(Self { directory })
    }
}

pub struct Engine {
    handle: *mut sys::sk_engine,
}

impl Engine {
    pub fn open(data_dir: impl AsRef<Path>, config: EngineConfig) -> Result<Self> {
        let data_dir = path_cstring(data_dir.as_ref())?;
        let config_dir = path_cstring(&config.directory)?;
        let mut handle = ptr::null_mut();
        let mut error = sys::sk_error {
            code: 0,
            message: ptr::null_mut(),
        };
        let status = unsafe {
            sys::sk_engine_open(
                data_dir.as_ptr(),
                config_dir.as_ptr(),
                &mut handle,
                &mut error,
            )
        };
        check(status, &mut error)?;
        if handle.is_null() {
            return Err(Error::message("native engine returned a null handle"));
        }
        Ok(Self { handle })
    }

    pub fn put(&mut self, document: Document) -> Result<()> {
        let (field, value) = document
            .field
            .ok_or_else(|| Error::message("document has no fields"))?;
        let id = cstring(document.id.as_str())?;
        let field = cstring(&field)?;
        let mut native_document = ptr::null_mut();
        let mut error = sys::sk_error {
            code: 0,
            message: ptr::null_mut(),
        };
        let status = unsafe {
            sys::sk_document_create(
                id.as_ptr(),
                field.as_ptr(),
                value.as_i32(),
                &mut native_document,
                &mut error,
            )
        };
        check(status, &mut error)?;
        let mut native_document = NativeDocument(native_document);
        let mut error = sys::sk_error {
            code: 0,
            message: ptr::null_mut(),
        };
        let status = unsafe { sys::sk_engine_put(self.handle, native_document.0, &mut error) };
        let result = check(status, &mut error);
        native_document.destroy();
        result
    }

    pub fn update(&mut self, update: DocumentUpdate) -> Result<()> {
        let (field, value) = update
            .field
            .ok_or_else(|| Error::message("update has no fields"))?;
        let id = cstring(update.id.as_str())?;
        let field = cstring(&field)?;
        let mut native_update = ptr::null_mut();
        let mut error = sys::sk_error {
            code: 0,
            message: ptr::null_mut(),
        };
        let status = unsafe {
            sys::sk_document_update_create(
                id.as_ptr(),
                field.as_ptr(),
                value.as_i32(),
                &mut native_update,
                &mut error,
            )
        };
        check(status, &mut error)?;
        let mut native_update = NativeUpdate(native_update);
        let mut error = sys::sk_error {
            code: 0,
            message: ptr::null_mut(),
        };
        let status = unsafe { sys::sk_engine_update(self.handle, native_update.0, &mut error) };
        let result = check(status, &mut error);
        native_update.destroy();
        result
    }

    pub fn remove(&mut self, id: &DocumentId) -> Result<bool> {
        let id = cstring(id.as_str())?;
        let mut was_found = 0;
        let mut error = sys::sk_error {
            code: 0,
            message: ptr::null_mut(),
        };
        let status =
            unsafe { sys::sk_engine_remove(self.handle, id.as_ptr(), &mut was_found, &mut error) };
        check(status, &mut error)?;
        Ok(was_found != 0)
    }

    pub fn search(&self, request: SearchRequest) -> Result<SearchResult> {
        let limit = u32::try_from(request.limit)
            .map_err(|_| Error::message("search limit exceeds native maximum"))?;
        let field = cstring(&request.query.field)?;
        let mut native_request = ptr::null_mut();
        let mut error = sys::sk_error {
            code: 0,
            message: ptr::null_mut(),
        };
        let status = unsafe {
            sys::sk_search_request_create(
                field.as_ptr(),
                request.query.value.as_i32(),
                limit,
                &mut native_request,
                &mut error,
            )
        };
        check(status, &mut error)?;
        let mut native_request = NativeRequest(native_request);
        let mut native_result = ptr::null_mut();
        let mut error = sys::sk_error {
            code: 0,
            message: ptr::null_mut(),
        };
        let status = unsafe {
            sys::sk_engine_search(
                self.handle,
                native_request.0,
                &mut native_result,
                &mut error,
            )
        };
        let result = check(status, &mut error);
        native_request.destroy();
        result?;

        let native_result = NativeResult(native_result);
        let size = unsafe { sys::sk_search_result_size(native_result.0) } as usize;
        let mut hits = Vec::with_capacity(size);
        for index in 0..size {
            let mut id_ptr = ptr::null();
            let mut id_length = 0;
            let mut error = sys::sk_error {
                code: 0,
                message: ptr::null_mut(),
            };
            let status = unsafe {
                sys::sk_search_result_id(
                    native_result.0,
                    index as u32,
                    &mut id_ptr,
                    &mut id_length,
                    &mut error,
                )
            };
            check(status, &mut error)?;
            if id_ptr.is_null() {
                return Err(Error::message("native result returned a null document ID"));
            }
            let id = unsafe { std::slice::from_raw_parts(id_ptr.cast::<u8>(), id_length as usize) };
            let id = String::from_utf8(id.to_vec())
                .map_err(|_| Error::message("native result returned invalid UTF-8"))?;
            hits.push(Hit {
                id: DocumentId { value: id },
            });
        }
        Ok(SearchResult { hits })
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe { sys::sk_engine_destroy(self.handle) };
            self.handle = ptr::null_mut();
        }
    }
}

struct NativeDocument(*mut sys::sk_document);
impl NativeDocument {
    fn destroy(&mut self) {
        if !self.0.is_null() {
            unsafe { sys::sk_document_destroy(self.0) };
            self.0 = ptr::null_mut();
        }
    }
}
impl Drop for NativeDocument {
    fn drop(&mut self) {
        self.destroy();
    }
}

struct NativeUpdate(*mut sys::sk_document_update);
impl NativeUpdate {
    fn destroy(&mut self) {
        if !self.0.is_null() {
            unsafe { sys::sk_document_update_destroy(self.0) };
            self.0 = ptr::null_mut();
        }
    }
}
impl Drop for NativeUpdate {
    fn drop(&mut self) {
        self.destroy();
    }
}

struct NativeRequest(*mut sys::sk_search_request);
impl NativeRequest {
    fn destroy(&mut self) {
        if !self.0.is_null() {
            unsafe { sys::sk_search_request_destroy(self.0) };
            self.0 = ptr::null_mut();
        }
    }
}
impl Drop for NativeRequest {
    fn drop(&mut self) {
        self.destroy();
    }
}

struct NativeResult(*mut sys::sk_search_result);
impl Drop for NativeResult {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { sys::sk_search_result_destroy(self.0) };
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct DocumentId {
    value: String,
}

impl DocumentId {
    pub fn parse(value: impl Into<String>) -> Result<Self> {
        let value = value.into();
        if value.is_empty() || value.as_bytes().contains(&0) {
            return Err(Error::message(
                "document ID must not be empty or contain NUL",
            ));
        }
        Ok(Self { value })
    }

    pub fn as_str(&self) -> &str {
        &self.value
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    I32(i32),
}

impl Value {
    fn as_i32(&self) -> i32 {
        match self {
            Self::I32(value) => *value,
        }
    }
}

pub struct Document {
    id: DocumentId,
    field: Option<(String, Value)>,
}

impl Document {
    pub fn new(id: DocumentId) -> Self {
        Self { id, field: None }
    }

    pub fn set_field(&mut self, name: impl Into<String>, value: Value) {
        self.field = Some((name.into(), value));
    }
}

pub struct DocumentUpdate {
    id: DocumentId,
    field: Option<(String, Value)>,
}

impl DocumentUpdate {
    pub fn new(id: DocumentId) -> Self {
        Self { id, field: None }
    }

    pub fn assign(&mut self, name: impl Into<String>, value: Value) {
        self.field = Some((name.into(), value));
    }
}

pub struct Query {
    field: String,
    value: Value,
}

impl Query {
    pub fn term(field: impl Into<String>, value: Value) -> Self {
        Self {
            field: field.into(),
            value,
        }
    }
}

pub struct SearchRequest {
    query: Query,
    limit: usize,
}

impl SearchRequest {
    pub fn new(query: Query) -> Self {
        Self { query, limit: 10 }
    }

    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }
}

pub struct Hit {
    id: DocumentId,
}

impl Hit {
    pub fn id(&self) -> &DocumentId {
        &self.id
    }
}

pub struct SearchResult {
    hits: Vec<Hit>,
}

impl SearchResult {
    pub fn hits(&self) -> &[Hit] {
        &self.hits
    }
}
