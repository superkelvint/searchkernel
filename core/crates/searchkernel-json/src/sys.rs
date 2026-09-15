use std::ffi::c_void;

unsafe extern "C" {
    pub fn sk_fbjson_json_to_flatbuffer(
        bfbs: *const u8,
        bfbs_len: usize,
        json: *const u8,
        json_len: usize,
        out_ptr: *mut *mut u8,
        out_len: *mut usize,
        error_ptr: *mut *mut std::ffi::c_char,
        error_len: *mut usize,
    ) -> i32;

    pub fn sk_fbjson_flatbuffer_to_json(
        bfbs: *const u8,
        bfbs_len: usize,
        buffer: *const u8,
        buffer_len: usize,
        out_ptr: *mut *mut u8,
        out_len: *mut usize,
        error_ptr: *mut *mut std::ffi::c_char,
        error_len: *mut usize,
    ) -> i32;

    pub fn sk_fbjson_free(ptr: *mut c_void);
}
