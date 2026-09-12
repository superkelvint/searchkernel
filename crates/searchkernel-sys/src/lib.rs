#![allow(non_camel_case_types)]

use std::os::raw::{c_char, c_int};

#[repr(C)]
pub struct sk_engine {
    _private: [u8; 0],
}

#[repr(C)]
pub struct sk_document {
    _private: [u8; 0],
}

#[repr(C)]
pub struct sk_document_update {
    _private: [u8; 0],
}

#[repr(C)]
pub struct sk_search_request {
    _private: [u8; 0],
}

#[repr(C)]
pub struct sk_search_result {
    _private: [u8; 0],
}

#[repr(C)]
pub struct sk_error {
    pub code: c_int,
    pub message: *mut c_char,
}

unsafe extern "C" {
    pub fn sk_native_abi_version() -> u32;

    pub fn sk_engine_open(
        data_dir: *const c_char,
        config_dir: *const c_char,
        out_engine: *mut *mut sk_engine,
        out_error: *mut sk_error,
    ) -> c_int;
    pub fn sk_engine_destroy(engine: *mut sk_engine);
    pub fn sk_error_destroy(error: *mut sk_error);

    pub fn sk_document_create(
        document_id: *const c_char,
        field: *const c_char,
        value: i32,
        out_document: *mut *mut sk_document,
        out_error: *mut sk_error,
    ) -> c_int;
    pub fn sk_document_destroy(document: *mut sk_document);

    pub fn sk_document_update_create(
        document_id: *const c_char,
        field: *const c_char,
        value: i32,
        out_update: *mut *mut sk_document_update,
        out_error: *mut sk_error,
    ) -> c_int;
    pub fn sk_document_update_destroy(update: *mut sk_document_update);

    pub fn sk_search_request_create(
        field: *const c_char,
        value: i32,
        limit: u32,
        out_request: *mut *mut sk_search_request,
        out_error: *mut sk_error,
    ) -> c_int;
    pub fn sk_search_request_destroy(request: *mut sk_search_request);

    pub fn sk_engine_put(
        engine: *mut sk_engine,
        document: *const sk_document,
        out_error: *mut sk_error,
    ) -> c_int;
    pub fn sk_engine_update(
        engine: *mut sk_engine,
        update: *const sk_document_update,
        out_error: *mut sk_error,
    ) -> c_int;
    pub fn sk_engine_remove(
        engine: *mut sk_engine,
        document_id: *const c_char,
        out_was_found: *mut c_int,
        out_error: *mut sk_error,
    ) -> c_int;
    pub fn sk_engine_search(
        engine: *const sk_engine,
        request: *const sk_search_request,
        out_result: *mut *mut sk_search_result,
        out_error: *mut sk_error,
    ) -> c_int;

    pub fn sk_search_result_size(result: *const sk_search_result) -> u32;
    pub fn sk_search_result_id(
        result: *const sk_search_result,
        index: u32,
        out_id: *mut *const c_char,
        out_length: *mut u32,
        out_error: *mut sk_error,
    ) -> c_int;
    pub fn sk_search_result_destroy(result: *mut sk_search_result);
}
