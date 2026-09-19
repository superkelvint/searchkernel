use searchkernel_vespa_bindings as sys;
use searchkernel_vespa_engine::{
    ConfigCompiler, Engine, Field, FieldType, MatchFeatureValue, Query, RankProfile, Schema,
    SearchRequest,
};
use std::ffi::{CStr, CString};
use std::path::{Path, PathBuf};
use std::ptr;

const ERROR_CAPACITY: u32 = sys::SK_ERROR_MESSAGE_CAPACITY;

fn cstring(value: &str) -> CString {
    CString::new(value).expect("verifier strings contain no NUL")
}

fn check(status: i32, error: &[u8]) -> Result<(), String> {
    if status == 0 {
        return Ok(());
    }
    let message = CStr::from_bytes_until_nul(error)
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|_| String::from("native operation failed"));
    Err(format!("native status {status}: {message}"))
}

struct EngineHandle(*mut sys::sk_engine);
impl Drop for EngineHandle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { sys::sk_engine_destroy(self.0) };
        }
    }
}

struct DocumentHandle(*mut sys::sk_document);
impl Drop for DocumentHandle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { sys::sk_document_destroy(self.0) };
        }
    }
}

struct QueryHandle(*mut sys::sk_query);
impl Drop for QueryHandle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { sys::sk_query_destroy(self.0) };
        }
    }
}

struct RequestHandle(*mut sys::sk_generic_search_request);
impl Drop for RequestHandle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { sys::sk_generic_search_request_destroy(self.0) };
        }
    }
}

struct ResultHandle(*mut sys::sk_generic_search_result);
impl Drop for ResultHandle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { sys::sk_generic_search_result_destroy(self.0) };
        }
    }
}

fn schema() -> Schema {
    Schema::new("typea")
        .field(Field::new("qty", FieldType::Integer).attribute().summary())
        .rank_profile(
            RankProfile::new("feature_values")
                .first_phase("attribute(qty)")
                .match_feature("attribute(qty)")
                .match_feature(r#"rankingExpression("tensor(x[2])(x)")"#),
        )
}

fn create_engine(data: &Path, config: &Path) -> Result<EngineHandle, String> {
    let data = cstring(&data.to_string_lossy());
    let config = cstring(&config.to_string_lossy());
    let mut handle = ptr::null_mut();
    let mut error = [0_u8; sys::SK_ERROR_MESSAGE_CAPACITY as usize];
    check(
        unsafe {
            sys::sk_engine_open(
                data.as_ptr(),
                config.as_ptr(),
                &mut handle,
                error.as_mut_ptr(),
                ERROR_CAPACITY,
            )
        },
        &error,
    )?;
    Ok(EngineHandle(handle))
}

fn feature_name(
    result: *const sys::sk_generic_search_result,
    index: u32,
) -> Result<String, String> {
    let mut name = ptr::null();
    let mut length = 0;
    let mut error = [0_u8; sys::SK_ERROR_MESSAGE_CAPACITY as usize];
    check(
        unsafe {
            sys::sk_generic_search_result_match_feature_name(
                result,
                index,
                &mut name,
                &mut length,
                error.as_mut_ptr(),
                ERROR_CAPACITY,
            )
        },
        &error,
    )?;
    if name.is_null() && length != 0 {
        return Err("native feature name is null".to_owned());
    }
    let bytes = if length == 0 {
        &[][..]
    } else {
        unsafe { std::slice::from_raw_parts(name.cast::<u8>(), length as usize) }
    };
    String::from_utf8(bytes.to_vec()).map_err(|_| "native feature name is not UTF-8".to_owned())
}

fn run(data: &Path, config: &Path) -> Result<(), String> {
    ConfigCompiler::default()
        .compile_to(&schema(), config)
        .map_err(|error| error.to_string())?;

    let engine = create_engine(data, config)?;
    let id = cstring("id:test:typea::one");
    let mut document = ptr::null_mut();
    let mut error = [0_u8; sys::SK_ERROR_MESSAGE_CAPACITY as usize];
    check(
        unsafe {
            sys::sk_document_create_empty(
                id.as_ptr(),
                &mut document,
                error.as_mut_ptr(),
                ERROR_CAPACITY,
            )
        },
        &error,
    )?;
    let document = DocumentHandle(document);
    let field = cstring("qty");
    check(
        unsafe {
            sys::sk_document_add_integer(
                document.0,
                field.as_ptr(),
                7,
                error.as_mut_ptr(),
                ERROR_CAPACITY,
            )
        },
        &error,
    )?;
    check(
        unsafe { sys::sk_engine_put(engine.0, document.0, error.as_mut_ptr(), ERROR_CAPACITY) },
        &error,
    )?;

    let mut query = ptr::null_mut();
    check(
        unsafe { sys::sk_query_create(engine.0, &mut query, error.as_mut_ptr(), ERROR_CAPACITY) },
        &error,
    )?;
    let query = QueryHandle(query);
    check(
        unsafe { sys::sk_query_add_true(query.0, error.as_mut_ptr(), ERROR_CAPACITY) },
        &error,
    )?;

    let profile = cstring("feature_values");
    let mut request = ptr::null_mut();
    check(
        unsafe {
            sys::sk_generic_search_request_create(
                profile.as_ptr(),
                10,
                0,
                sys::SK_SUMMARY_NONE,
                &mut request,
                error.as_mut_ptr(),
                ERROR_CAPACITY,
            )
        },
        &error,
    )?;
    let request = RequestHandle(request);
    let mut result = ptr::null_mut();
    check(
        unsafe {
            sys::sk_engine_search_generic(
                engine.0,
                query.0,
                request.0,
                &mut result,
                error.as_mut_ptr(),
                ERROR_CAPACITY,
            )
        },
        &error,
    )?;
    let result = ResultHandle(result);
    if unsafe { sys::sk_generic_search_result_hit_count(result.0) } != 1 {
        return Err("feature verifier expected one hit".to_owned());
    }
    let feature_count = unsafe { sys::sk_generic_search_result_match_feature_count(result.0) };
    if feature_count != 2 {
        return Err(format!(
            "feature verifier expected two features, got {feature_count}"
        ));
    }

    let mut scalar_name = None;
    let mut data_name = None;
    let mut raw_data = Vec::new();
    for feature_index in 0..feature_count {
        let name = feature_name(result.0, feature_index)?;
        let mut value = ptr::null();
        check(
            unsafe {
                sys::sk_generic_search_result_match_feature_value_ref(
                    result.0,
                    0,
                    feature_index,
                    &mut value,
                    error.as_mut_ptr(),
                    ERROR_CAPACITY,
                )
            },
            &error,
        )?;
        if value.is_null() {
            return Err("native feature value handle is null".to_owned());
        }
        match unsafe { sys::sk_feature_value_kind(value) } {
            sys::SK_FEATURE_VALUE_DOUBLE => {
                let mut scalar = 0.0;
                check(
                    unsafe {
                        sys::sk_feature_value_double(
                            value,
                            &mut scalar,
                            error.as_mut_ptr(),
                            ERROR_CAPACITY,
                        )
                    },
                    &error,
                )?;
                if scalar != 7.0 {
                    return Err(format!("scalar feature changed: {scalar}"));
                }
                let mut legacy = 0.0;
                check(
                    unsafe {
                        sys::sk_generic_search_result_match_feature_value(
                            result.0,
                            0,
                            feature_index,
                            &mut legacy,
                            error.as_mut_ptr(),
                            ERROR_CAPACITY,
                        )
                    },
                    &error,
                )?;
                if legacy != 7.0 {
                    return Err(format!("legacy scalar feature changed: {legacy}"));
                }
                scalar_name = Some(name);
            }
            sys::SK_FEATURE_VALUE_DATA => {
                let mut data = ptr::null();
                let mut size = 0;
                check(
                    unsafe {
                        sys::sk_feature_value_data(
                            value,
                            &mut data,
                            &mut size,
                            error.as_mut_ptr(),
                            ERROR_CAPACITY,
                        )
                    },
                    &error,
                )?;
                if size == 0 || data.is_null() {
                    return Err("data-backed feature did not return a payload".to_owned());
                }
                raw_data = unsafe { std::slice::from_raw_parts(data, size).to_vec() };

                let mut typed_as_double = 123.0;
                let typed_status = unsafe {
                    sys::sk_feature_value_double(
                        value,
                        &mut typed_as_double,
                        error.as_mut_ptr(),
                        ERROR_CAPACITY,
                    )
                };
                if typed_status != sys::SK_NATIVE_STATUS_TYPE_ERROR as i32 || typed_as_double != 0.0
                {
                    return Err("DATA was accepted or reinterpreted as DOUBLE".to_owned());
                }

                let mut legacy = 456.0;
                let legacy_status = unsafe {
                    sys::sk_generic_search_result_match_feature_value(
                        result.0,
                        0,
                        feature_index,
                        &mut legacy,
                        error.as_mut_ptr(),
                        ERROR_CAPACITY,
                    )
                };
                if legacy_status != sys::SK_NATIVE_STATUS_TYPE_ERROR as i32 || legacy != 0.0 {
                    return Err("legacy scalar getter reinterpreted DATA".to_owned());
                }
                data_name = Some(name);
            }
            kind => return Err(format!("unknown feature value kind {kind}")),
        }
    }
    let scalar_name = scalar_name.ok_or_else(|| "scalar feature is missing".to_owned())?;
    let data_name = data_name.ok_or_else(|| "DATA feature is missing".to_owned())?;
    if scalar_name == data_name {
        return Err("scalar and DATA features have the same name".to_owned());
    }

    let mut error = [0_u8; sys::SK_ERROR_MESSAGE_CAPACITY as usize];
    check(
        unsafe { sys::sk_engine_close(engine.0, error.as_mut_ptr(), ERROR_CAPACITY) },
        &error,
    )?;
    drop(result);
    drop(request);
    drop(query);
    drop(document);
    drop(engine);

    let safe_engine = Engine::open(data, config).map_err(|error| error.to_string())?;
    let safe_result = safe_engine
        .search(SearchRequest::new(Query::True).rank_profile("feature_values"))
        .map_err(|error| error.to_string())?;
    let hit = safe_result
        .hits
        .first()
        .ok_or_else(|| "safe wrapper returned no hit".to_owned())?;
    let scalar = hit
        .match_features
        .get(&scalar_name)
        .ok_or_else(|| "safe wrapper lost scalar feature".to_owned())?;
    if *scalar != MatchFeatureValue::Double(7.0) {
        return Err("safe wrapper changed scalar feature".to_owned());
    }
    let data = hit
        .match_features
        .get(&data_name)
        .and_then(MatchFeatureValue::as_data)
        .ok_or_else(|| "safe wrapper lost DATA feature".to_owned())?;
    if data != raw_data {
        return Err("safe wrapper changed DATA payload".to_owned());
    }
    safe_engine.close().map_err(|error| error.to_string())
}

fn main() {
    let mut args = std::env::args_os().skip(1);
    let config = PathBuf::from(args.next().expect("usage: driver CONFIG DATA"));
    let data = PathBuf::from(args.next().expect("usage: driver CONFIG DATA"));
    if let Err(error) = run(&data, &config) {
        eprintln!("M13 driver failed: {error}");
        std::process::exit(1);
    }
}
