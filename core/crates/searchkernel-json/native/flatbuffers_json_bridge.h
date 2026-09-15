#ifndef SEARCHKERNEL_FLATBUFFERS_JSON_BRIDGE_H_
#define SEARCHKERNEL_FLATBUFFERS_JSON_BRIDGE_H_

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

int sk_fbjson_json_to_flatbuffer(const uint8_t* bfbs, size_t bfbs_len,
                                 const uint8_t* json, size_t json_len,
                                 uint8_t** out_ptr, size_t* out_len,
                                 char** error_ptr, size_t* error_len);

int sk_fbjson_flatbuffer_to_json(const uint8_t* bfbs, size_t bfbs_len,
                                 const uint8_t* buffer, size_t buffer_len,
                                 uint8_t** out_ptr, size_t* out_len,
                                 char** error_ptr, size_t* error_len);

void sk_fbjson_free(void* ptr);

#ifdef __cplusplus
}
#endif

#endif
