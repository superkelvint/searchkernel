#define _GNU_SOURCE
#include <dlfcn.h>
#include <limits.h>
#include <stdatomic.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include "searchkernel_vespa.h"
#include "searchkernel_vespa_test.h"

static void *sk_loader_handle(void) {
    static _Atomic(void *) handle = NULL;
    void *loaded = atomic_load_explicit(&handle, memory_order_acquire);
    if (loaded != NULL) return loaded;
    char exe[PATH_MAX];
    ssize_t size = readlink("/proc/self/exe", exe, sizeof(exe) - 1);
    if (size <= 0 || (size_t)size >= sizeof(exe) - 1) {
        fputs("Search Kernel native loader could not resolve /proc/self/exe\n", stderr); abort();
    }
    exe[size] = '\0';
    char *slash = strrchr(exe, '/');
    if (slash == NULL) abort();
    *slash = '\0';
    char path[PATH_MAX];
    int written = snprintf(path, sizeof(path), "%s/.searchkernel-native/libsearchkernel_vespa.so.2", exe);
    if (written < 0 || (size_t)written >= sizeof(path)) abort();
    loaded = dlopen(path, RTLD_NOW | RTLD_LOCAL);
    if (loaded == NULL) {
        fprintf(stderr, "Search Kernel native loader could not load %s: %s\n", path, dlerror()); abort();
    }
    void *expected = NULL;
    if (!atomic_compare_exchange_strong_explicit(&handle, &expected, loaded, memory_order_release, memory_order_acquire)) {
        dlclose(loaded); loaded = expected;
    }
    return loaded;
}
static void *sk_loader_symbol(const char *name) {
    dlerror();
    void *symbol = dlsym(sk_loader_handle(), name);
    const char *error = dlerror();
    if (error != NULL || symbol == NULL) {
        fprintf(stderr, "Search Kernel native loader could not resolve %s: %s\n", name, error ? error : "missing symbol"); abort();
    }
    return symbol;
}

uint32_t sk_native_abi_version(void) {
    typedef uint32_t (*fn_type)(void);
    fn_type fn = (fn_type)sk_loader_symbol("sk_native_abi_version");
    return fn();
}

int32_t sk_engine_open(const char *data_dir, const char *config_dir, sk_engine **out_engine, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)(const char *data_dir, const char *config_dir, sk_engine **out_engine, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_engine_open");
    return fn(data_dir, config_dir, out_engine, out_error, out_error_capacity);
}

int32_t sk_engine_close(sk_engine *engine, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)(sk_engine *engine, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_engine_close");
    return fn(engine, out_error, out_error_capacity);
}

void sk_engine_destroy(sk_engine *engine) {
    typedef void (*fn_type)(sk_engine *engine);
    fn_type fn = (fn_type)sk_loader_symbol("sk_engine_destroy");
    fn(engine);
}

int32_t sk_document_create(const char *document_id, const char *field, int64_t value, sk_document **out_document, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)(const char *document_id, const char *field, int64_t value, sk_document **out_document, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_document_create");
    return fn(document_id, field, value, out_document, out_error, out_error_capacity);
}

void sk_document_destroy(sk_document *document) {
    typedef void (*fn_type)(sk_document *document);
    fn_type fn = (fn_type)sk_loader_symbol("sk_document_destroy");
    fn(document);
}

int32_t sk_document_create_empty(const char *document_id, sk_document **out_document, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)(const char *document_id, sk_document **out_document, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_document_create_empty");
    return fn(document_id, out_document, out_error, out_error_capacity);
}

int32_t sk_document_add_string(sk_document *document, const char *field, const char *value, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)(sk_document *document, const char *field, const char *value, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_document_add_string");
    return fn(document, field, value, out_error, out_error_capacity);
}

int32_t sk_document_add_integer(sk_document *document, const char *field, int64_t value, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)(sk_document *document, const char *field, int64_t value, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_document_add_integer");
    return fn(document, field, value, out_error, out_error_capacity);
}

int32_t sk_document_add_float(sk_document *document, const char *field, double value, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)(sk_document *document, const char *field, double value, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_document_add_float");
    return fn(document, field, value, out_error, out_error_capacity);
}

int32_t sk_document_add_boolean(sk_document *document, const char *field, uint8_t value, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)(sk_document *document, const char *field, uint8_t value, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_document_add_boolean");
    return fn(document, field, value, out_error, out_error_capacity);
}

int32_t sk_document_add_tensor_dense( sk_document *document, const char *field, const char *type_spec, const double *values, uint32_t value_count, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_document *document, const char *field, const char *type_spec, const double *values, uint32_t value_count, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_document_add_tensor_dense");
    return fn(document, field, type_spec, values, value_count, out_error, out_error_capacity);
}

int32_t sk_document_add_string_array( sk_document *document, const char *field, const uint8_t *value_bytes, const uint32_t *value_lengths, uint32_t value_count, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_document *document, const char *field, const uint8_t *value_bytes, const uint32_t *value_lengths, uint32_t value_count, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_document_add_string_array");
    return fn(document, field, value_bytes, value_lengths, value_count, out_error, out_error_capacity);
}

int32_t sk_document_add_position( sk_document *document, const char *field, double latitude, double longitude, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_document *document, const char *field, double latitude, double longitude, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_document_add_position");
    return fn(document, field, latitude, longitude, out_error, out_error_capacity);
}

int32_t sk_document_add_predicate( sk_document *document, const char *field, const uint8_t *feature_key_bytes, const uint32_t *feature_key_lengths, const uint8_t *feature_value_bytes, const uint32_t *feature_value_lengths, uint32_t feature_count, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_document *document, const char *field, const uint8_t *feature_key_bytes, const uint32_t *feature_key_lengths, const uint8_t *feature_value_bytes, const uint32_t *feature_value_lengths, uint32_t feature_count, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_document_add_predicate");
    return fn(document, field, feature_key_bytes, feature_key_lengths, feature_value_bytes, feature_value_lengths, feature_count, out_error, out_error_capacity);
}

int32_t sk_document_add_predicate_json( sk_document *document, const char *field, const char *expression_json, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_document *document, const char *field, const char *expression_json, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_document_add_predicate_json");
    return fn(document, field, expression_json, out_error, out_error_capacity);
}

int32_t sk_struct_array_create( sk_struct_array **out_array, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_struct_array **out_array, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_struct_array_create");
    return fn(out_array, out_error, out_error_capacity);
}

int32_t sk_struct_array_add_element( sk_struct_array *array, uint32_t *out_element_index, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_struct_array *array, uint32_t *out_element_index, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_struct_array_add_element");
    return fn(array, out_element_index, out_error, out_error_capacity);
}

int32_t sk_struct_array_set_string( sk_struct_array *array, uint32_t element_index, const char *field, const char *value, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_struct_array *array, uint32_t element_index, const char *field, const char *value, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_struct_array_set_string");
    return fn(array, element_index, field, value, out_error, out_error_capacity);
}

int32_t sk_struct_array_set_integer( sk_struct_array *array, uint32_t element_index, const char *field, int64_t value, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_struct_array *array, uint32_t element_index, const char *field, int64_t value, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_struct_array_set_integer");
    return fn(array, element_index, field, value, out_error, out_error_capacity);
}

int32_t sk_struct_array_set_float( sk_struct_array *array, uint32_t element_index, const char *field, double value, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_struct_array *array, uint32_t element_index, const char *field, double value, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_struct_array_set_float");
    return fn(array, element_index, field, value, out_error, out_error_capacity);
}

int32_t sk_struct_array_set_boolean( sk_struct_array *array, uint32_t element_index, const char *field, uint8_t value, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_struct_array *array, uint32_t element_index, const char *field, uint8_t value, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_struct_array_set_boolean");
    return fn(array, element_index, field, value, out_error, out_error_capacity);
}

void sk_struct_array_destroy(sk_struct_array *array) {
    typedef void (*fn_type)(sk_struct_array *array);
    fn_type fn = (fn_type)sk_loader_symbol("sk_struct_array_destroy");
    fn(array);
}

int32_t sk_document_add_struct_array( sk_document *document, const char *field, const sk_struct_array *array, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_document *document, const char *field, const sk_struct_array *array, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_document_add_struct_array");
    return fn(document, field, array, out_error, out_error_capacity);
}

int32_t sk_document_annotate_text( sk_document *document, const char *field, const uint8_t *term_bytes, uint32_t term_bytes_length, const uint32_t *term_lengths, const uint32_t *byte_starts, const uint32_t *byte_lengths, uint32_t occurrence_count, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_document *document, const char *field, const uint8_t *term_bytes, uint32_t term_bytes_length, const uint32_t *term_lengths, const uint32_t *byte_starts, const uint32_t *byte_lengths, uint32_t occurrence_count, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_document_annotate_text");
    return fn(document, field, term_bytes, term_bytes_length, term_lengths, byte_starts, byte_lengths, occurrence_count, out_error, out_error_capacity);
}

int32_t sk_document_annotate_string_array( sk_document *document, const char *field, const uint8_t *term_bytes, uint32_t term_bytes_length, const uint32_t *term_lengths, const uint32_t *byte_starts, const uint32_t *byte_lengths, const uint32_t *element_offsets, uint32_t element_count, uint32_t occurrence_count, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_document *document, const char *field, const uint8_t *term_bytes, uint32_t term_bytes_length, const uint32_t *term_lengths, const uint32_t *byte_starts, const uint32_t *byte_lengths, const uint32_t *element_offsets, uint32_t element_count, uint32_t occurrence_count, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_document_annotate_string_array");
    return fn(document, field, term_bytes, term_bytes_length, term_lengths, byte_starts, byte_lengths, element_offsets, element_count, occurrence_count, out_error, out_error_capacity);
}

int32_t sk_document_update_create(const char *document_id, const char *field, int64_t value, sk_document_update **out_update, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)(const char *document_id, const char *field, int64_t value, sk_document_update **out_update, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_document_update_create");
    return fn(document_id, field, value, out_update, out_error, out_error_capacity);
}

void sk_document_update_destroy(sk_document_update *update) {
    typedef void (*fn_type)(sk_document_update *update);
    fn_type fn = (fn_type)sk_loader_symbol("sk_document_update_destroy");
    fn(update);
}

int32_t sk_document_update_create_empty(const char *document_id, sk_document_update **out_update, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)(const char *document_id, sk_document_update **out_update, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_document_update_create_empty");
    return fn(document_id, out_update, out_error, out_error_capacity);
}

int32_t sk_document_update_set_create_if_nonexistent( sk_document_update *update, uint8_t create_if_nonexistent, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_document_update *update, uint8_t create_if_nonexistent, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_document_update_set_create_if_nonexistent");
    return fn(update, create_if_nonexistent, out_error, out_error_capacity);
}

int32_t sk_document_update_assign_string(sk_document_update *update, const char *field, const char *value, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)(sk_document_update *update, const char *field, const char *value, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_document_update_assign_string");
    return fn(update, field, value, out_error, out_error_capacity);
}

int32_t sk_document_update_assign_integer(sk_document_update *update, const char *field, int64_t value, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)(sk_document_update *update, const char *field, int64_t value, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_document_update_assign_integer");
    return fn(update, field, value, out_error, out_error_capacity);
}

int32_t sk_document_update_assign_float(sk_document_update *update, const char *field, double value, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)(sk_document_update *update, const char *field, double value, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_document_update_assign_float");
    return fn(update, field, value, out_error, out_error_capacity);
}

int32_t sk_document_update_assign_boolean(sk_document_update *update, const char *field, uint8_t value, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)(sk_document_update *update, const char *field, uint8_t value, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_document_update_assign_boolean");
    return fn(update, field, value, out_error, out_error_capacity);
}

int32_t sk_document_update_assign_tensor_dense( sk_document_update *update, const char *field, const char *type_spec, const double *values, uint32_t value_count, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_document_update *update, const char *field, const char *type_spec, const double *values, uint32_t value_count, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_document_update_assign_tensor_dense");
    return fn(update, field, type_spec, values, value_count, out_error, out_error_capacity);
}

int32_t sk_document_update_assign_string_array( sk_document_update *update, const char *field, const uint8_t *value_bytes, const uint32_t *value_lengths, uint32_t value_count, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_document_update *update, const char *field, const uint8_t *value_bytes, const uint32_t *value_lengths, uint32_t value_count, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_document_update_assign_string_array");
    return fn(update, field, value_bytes, value_lengths, value_count, out_error, out_error_capacity);
}

int32_t sk_document_update_assign_position( sk_document_update *update, const char *field, double latitude, double longitude, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_document_update *update, const char *field, double latitude, double longitude, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_document_update_assign_position");
    return fn(update, field, latitude, longitude, out_error, out_error_capacity);
}

int32_t sk_document_update_assign_predicate( sk_document_update *update, const char *field, const uint8_t *feature_key_bytes, const uint32_t *feature_key_lengths, const uint8_t *feature_value_bytes, const uint32_t *feature_value_lengths, uint32_t feature_count, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_document_update *update, const char *field, const uint8_t *feature_key_bytes, const uint32_t *feature_key_lengths, const uint8_t *feature_value_bytes, const uint32_t *feature_value_lengths, uint32_t feature_count, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_document_update_assign_predicate");
    return fn(update, field, feature_key_bytes, feature_key_lengths, feature_value_bytes, feature_value_lengths, feature_count, out_error, out_error_capacity);
}

int32_t sk_document_update_assign_predicate_json( sk_document_update *update, const char *field, const char *expression_json, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_document_update *update, const char *field, const char *expression_json, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_document_update_assign_predicate_json");
    return fn(update, field, expression_json, out_error, out_error_capacity);
}

int32_t sk_document_update_clear(sk_document_update *update, const char *field, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)(sk_document_update *update, const char *field, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_document_update_clear");
    return fn(update, field, out_error, out_error_capacity);
}

int32_t sk_document_update_arithmetic( sk_document_update *update, const char *field, uint32_t arithmetic_operator, double operand, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_document_update *update, const char *field, uint32_t arithmetic_operator, double operand, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_document_update_arithmetic");
    return fn(update, field, arithmetic_operator, operand, out_error, out_error_capacity);
}

int32_t sk_document_update_annotate_text( sk_document_update *update, const char *field, const uint8_t *term_bytes, uint32_t term_bytes_length, const uint32_t *term_lengths, const uint32_t *byte_starts, const uint32_t *byte_lengths, uint32_t occurrence_count, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_document_update *update, const char *field, const uint8_t *term_bytes, uint32_t term_bytes_length, const uint32_t *term_lengths, const uint32_t *byte_starts, const uint32_t *byte_lengths, uint32_t occurrence_count, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_document_update_annotate_text");
    return fn(update, field, term_bytes, term_bytes_length, term_lengths, byte_starts, byte_lengths, occurrence_count, out_error, out_error_capacity);
}

int32_t sk_document_update_annotate_string_array( sk_document_update *update, const char *field, const uint8_t *term_bytes, uint32_t term_bytes_length, const uint32_t *term_lengths, const uint32_t *byte_starts, const uint32_t *byte_lengths, const uint32_t *element_offsets, uint32_t element_count, uint32_t occurrence_count, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_document_update *update, const char *field, const uint8_t *term_bytes, uint32_t term_bytes_length, const uint32_t *term_lengths, const uint32_t *byte_starts, const uint32_t *byte_lengths, const uint32_t *element_offsets, uint32_t element_count, uint32_t occurrence_count, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_document_update_annotate_string_array");
    return fn(update, field, term_bytes, term_bytes_length, term_lengths, byte_starts, byte_lengths, element_offsets, element_count, occurrence_count, out_error, out_error_capacity);
}

int32_t sk_search_request_create(const char *field, int64_t value, uint32_t limit, sk_search_request **out_request, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)(const char *field, int64_t value, uint32_t limit, sk_search_request **out_request, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_search_request_create");
    return fn(field, value, limit, out_request, out_error, out_error_capacity);
}

void sk_search_request_destroy(sk_search_request *request) {
    typedef void (*fn_type)(sk_search_request *request);
    fn_type fn = (fn_type)sk_loader_symbol("sk_search_request_destroy");
    fn(request);
}

int32_t sk_engine_put(sk_engine *engine, const sk_document *document, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)(sk_engine *engine, const sk_document *document, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_engine_put");
    return fn(engine, document, out_error, out_error_capacity);
}

int32_t sk_engine_update(sk_engine *engine, const sk_document_update *update, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)(sk_engine *engine, const sk_document_update *update, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_engine_update");
    return fn(engine, update, out_error, out_error_capacity);
}

int32_t sk_engine_get(const sk_engine *engine, const char *document_id, sk_document **out_document, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)(const sk_engine *engine, const char *document_id, sk_document **out_document, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_engine_get");
    return fn(engine, document_id, out_document, out_error, out_error_capacity);
}

int32_t sk_document_field_count(const sk_document *document, uint32_t *out_count, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)(const sk_document *document, uint32_t *out_count, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_document_field_count");
    return fn(document, out_count, out_error, out_error_capacity);
}

int32_t sk_document_field(const sk_document *document, uint32_t index, const char **out_name, uint32_t *out_name_length, uint32_t *out_kind, const char **out_string, uint32_t *out_string_length, int64_t *out_integer, double *out_float, uint8_t *out_boolean, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)(const sk_document *document, uint32_t index, const char **out_name, uint32_t *out_name_length, uint32_t *out_kind, const char **out_string, uint32_t *out_string_length, int64_t *out_integer, double *out_float, uint8_t *out_boolean, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_document_field");
    return fn(document, index, out_name, out_name_length, out_kind, out_string, out_string_length, out_integer, out_float, out_boolean, out_error, out_error_capacity);
}

int32_t sk_document_field_tensor_dense( const sk_document *document, uint32_t index, const char **out_type_spec, uint32_t *out_type_spec_length, const double **out_cells, uint32_t *out_cell_count, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const sk_document *document, uint32_t index, const char **out_type_spec, uint32_t *out_type_spec_length, const double **out_cells, uint32_t *out_cell_count, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_document_field_tensor_dense");
    return fn(document, index, out_type_spec, out_type_spec_length, out_cells, out_cell_count, out_error, out_error_capacity);
}

int32_t sk_engine_remove(sk_engine *engine, const char *document_id, int32_t *out_was_found, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)(sk_engine *engine, const char *document_id, int32_t *out_was_found, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_engine_remove");
    return fn(engine, document_id, out_was_found, out_error, out_error_capacity);
}

int32_t sk_engine_search(const sk_engine *engine, const sk_search_request *request, sk_search_result **out_result, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)(const sk_engine *engine, const sk_search_request *request, sk_search_result **out_result, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_engine_search");
    return fn(engine, request, out_result, out_error, out_error_capacity);
}

uint32_t sk_search_result_size(const sk_search_result *result) {
    typedef uint32_t (*fn_type)(const sk_search_result *result);
    fn_type fn = (fn_type)sk_loader_symbol("sk_search_result_size");
    return fn(result);
}

int32_t sk_search_result_id(const sk_search_result *result, uint32_t index, const char **out_id, uint32_t *out_length, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)(const sk_search_result *result, uint32_t index, const char **out_id, uint32_t *out_length, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_search_result_id");
    return fn(result, index, out_id, out_length, out_error, out_error_capacity);
}

void sk_search_result_destroy(sk_search_result *result) {
    typedef void (*fn_type)(sk_search_result *result);
    fn_type fn = (fn_type)sk_loader_symbol("sk_search_result_destroy");
    fn(result);
}

int32_t sk_text_document_create(const char *document_id, const char *field, const uint8_t *text, uint32_t text_length, sk_text_document **out_document, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)(const char *document_id, const char *field, const uint8_t *text, uint32_t text_length, sk_text_document **out_document, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_text_document_create");
    return fn(document_id, field, text, text_length, out_document, out_error, out_error_capacity);
}

int32_t sk_text_document_add_occurrences( sk_text_document *document, const uint8_t *term_bytes, uint32_t term_bytes_length, const uint32_t *term_lengths, const uint32_t *byte_starts, const uint32_t *byte_lengths, uint32_t occurrence_count, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_text_document *document, const uint8_t *term_bytes, uint32_t term_bytes_length, const uint32_t *term_lengths, const uint32_t *byte_starts, const uint32_t *byte_lengths, uint32_t occurrence_count, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_text_document_add_occurrences");
    return fn(document, term_bytes, term_bytes_length, term_lengths, byte_starts, byte_lengths, occurrence_count, out_error, out_error_capacity);
}

void sk_text_document_destroy(sk_text_document *document) {
    typedef void (*fn_type)(sk_text_document *document);
    fn_type fn = (fn_type)sk_loader_symbol("sk_text_document_destroy");
    fn(document);
}

int32_t sk_text_update_create(const char *document_id, const char *field, const uint8_t *text, uint32_t text_length, sk_text_update **out_update, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)(const char *document_id, const char *field, const uint8_t *text, uint32_t text_length, sk_text_update **out_update, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_text_update_create");
    return fn(document_id, field, text, text_length, out_update, out_error, out_error_capacity);
}

int32_t sk_text_update_add_occurrences( sk_text_update *update, const uint8_t *term_bytes, uint32_t term_bytes_length, const uint32_t *term_lengths, const uint32_t *byte_starts, const uint32_t *byte_lengths, uint32_t occurrence_count, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_text_update *update, const uint8_t *term_bytes, uint32_t term_bytes_length, const uint32_t *term_lengths, const uint32_t *byte_starts, const uint32_t *byte_lengths, uint32_t occurrence_count, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_text_update_add_occurrences");
    return fn(update, term_bytes, term_bytes_length, term_lengths, byte_starts, byte_lengths, occurrence_count, out_error, out_error_capacity);
}

void sk_text_update_destroy(sk_text_update *update) {
    typedef void (*fn_type)(sk_text_update *update);
    fn_type fn = (fn_type)sk_loader_symbol("sk_text_update_destroy");
    fn(update);
}

int32_t sk_text_request_create(uint32_t kind, const char *field, const char *rank_profile, sk_text_request **out_request, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)(uint32_t kind, const char *field, const char *rank_profile, sk_text_request **out_request, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_text_request_create");
    return fn(kind, field, rank_profile, out_request, out_error, out_error_capacity);
}

int32_t sk_text_request_add_terms( sk_text_request *request, const uint8_t *term_bytes, uint32_t term_bytes_length, const uint32_t *term_lengths, uint32_t term_count, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_text_request *request, const uint8_t *term_bytes, uint32_t term_bytes_length, const uint32_t *term_lengths, uint32_t term_count, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_text_request_add_terms");
    return fn(request, term_bytes, term_bytes_length, term_lengths, term_count, out_error, out_error_capacity);
}

void sk_text_request_destroy(sk_text_request *request) {
    typedef void (*fn_type)(sk_text_request *request);
    fn_type fn = (fn_type)sk_loader_symbol("sk_text_request_destroy");
    fn(request);
}

int32_t sk_engine_put_text(sk_engine *engine, const sk_text_document *document, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)(sk_engine *engine, const sk_text_document *document, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_engine_put_text");
    return fn(engine, document, out_error, out_error_capacity);
}

int32_t sk_engine_update_text(sk_engine *engine, const sk_text_update *update, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)(sk_engine *engine, const sk_text_update *update, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_engine_update_text");
    return fn(engine, update, out_error, out_error_capacity);
}

int32_t sk_engine_search_text(const sk_engine *engine, const sk_text_request *request, sk_text_result **out_result, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)(const sk_engine *engine, const sk_text_request *request, sk_text_result **out_result, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_engine_search_text");
    return fn(engine, request, out_result, out_error, out_error_capacity);
}

uint32_t sk_text_result_size(const sk_text_result *result) {
    typedef uint32_t (*fn_type)(const sk_text_result *result);
    fn_type fn = (fn_type)sk_loader_symbol("sk_text_result_size");
    return fn(result);
}

int32_t sk_text_result_hit(const sk_text_result *result, uint32_t index, const char **out_id, uint32_t *out_length, double *out_score, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)(const sk_text_result *result, uint32_t index, const char **out_id, uint32_t *out_length, double *out_score, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_text_result_hit");
    return fn(result, index, out_id, out_length, out_score, out_error, out_error_capacity);
}

void sk_text_result_destroy(sk_text_result *result) {
    typedef void (*fn_type)(sk_text_result *result);
    fn_type fn = (fn_type)sk_loader_symbol("sk_text_result_destroy");
    fn(result);
}

int32_t sk_query_create(const sk_engine *engine, sk_query **out_query, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)(const sk_engine *engine, sk_query **out_query, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_query_create");
    return fn(engine, out_query, out_error, out_error_capacity);
}

int32_t sk_query_add_operator(sk_query *query, uint32_t kind, uint32_t child_count, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)(sk_query *query, uint32_t kind, uint32_t child_count, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_query_add_operator");
    return fn(query, kind, child_count, out_error, out_error_capacity);
}

int32_t sk_query_validate_field(const sk_query *query, uint32_t kind, const char *field, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)(const sk_query *query, uint32_t kind, const char *field, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_query_validate_field");
    return fn(query, kind, field, out_error, out_error_capacity);
}

int32_t sk_query_add_true(sk_query *query, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)(sk_query *query, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_query_add_true");
    return fn(query, out_error, out_error_capacity);
}

int32_t sk_query_add_text(sk_query *query, uint32_t kind, const char *field, const uint8_t *term_bytes, uint32_t term_bytes_length, const uint32_t *term_lengths, uint32_t term_count, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)(sk_query *query, uint32_t kind, const char *field, const uint8_t *term_bytes, uint32_t term_bytes_length, const uint32_t *term_lengths, uint32_t term_count, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_query_add_text");
    return fn(query, kind, field, term_bytes, term_bytes_length, term_lengths, term_count, out_error, out_error_capacity);
}

int32_t sk_query_add_in_strings( sk_query *query, const char *field, const uint8_t *value_bytes, uint32_t value_bytes_length, const uint32_t *value_lengths, uint32_t value_count, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_query *query, const char *field, const uint8_t *value_bytes, uint32_t value_bytes_length, const uint32_t *value_lengths, uint32_t value_count, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_query_add_in_strings");
    return fn(query, field, value_bytes, value_bytes_length, value_lengths, value_count, out_error, out_error_capacity);
}

int32_t sk_query_add_in_integers( sk_query *query, const char *field, const int64_t *values, uint32_t value_count, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_query *query, const char *field, const int64_t *values, uint32_t value_count, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_query_add_in_integers");
    return fn(query, field, values, value_count, out_error, out_error_capacity);
}

int32_t sk_query_add_proximity( sk_query *query, uint8_t ordered, const char *field, uint32_t child_count, uint32_t distance, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_query *query, uint8_t ordered, const char *field, uint32_t child_count, uint32_t distance, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_query_add_proximity");
    return fn(query, ordered, field, child_count, distance, out_error, out_error_capacity);
}

int32_t sk_query_add_equiv( sk_query *query, uint32_t child_count, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_query *query, uint32_t child_count, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_query_add_equiv");
    return fn(query, child_count, out_error, out_error_capacity);
}

int32_t sk_query_add_weak_and( sk_query *query, const char *field, uint32_t child_count, uint32_t target_num_hits, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_query *query, const char *field, uint32_t child_count, uint32_t target_num_hits, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_query_add_weak_and");
    return fn(query, field, child_count, target_num_hits, out_error, out_error_capacity);
}

int32_t sk_query_add_rank( sk_query *query, uint32_t child_count, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_query *query, uint32_t child_count, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_query_add_rank");
    return fn(query, child_count, out_error, out_error_capacity);
}

int32_t sk_query_add_fuzzy( sk_query *query, const char *field, const char *term, uint32_t max_edit_distance, uint32_t prefix_lock_length, uint8_t prefix_match, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_query *query, const char *field, const char *term, uint32_t max_edit_distance, uint32_t prefix_lock_length, uint8_t prefix_match, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_query_add_fuzzy");
    return fn(query, field, term, max_edit_distance, prefix_lock_length, prefix_match, out_error, out_error_capacity);
}

int32_t sk_query_add_alternatives( sk_query *query, const char *field, const uint8_t *term_bytes, uint32_t term_bytes_length, const uint32_t *term_lengths, const int64_t *exactness_weights, uint32_t term_count, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_query *query, const char *field, const uint8_t *term_bytes, uint32_t term_bytes_length, const uint32_t *term_lengths, const int64_t *exactness_weights, uint32_t term_count, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_query_add_alternatives");
    return fn(query, field, term_bytes, term_bytes_length, term_lengths, exactness_weights, term_count, out_error, out_error_capacity);
}

int32_t sk_query_add_matches( sk_query *query, const char *field, const char *pattern, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_query *query, const char *field, const char *pattern, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_query_add_matches");
    return fn(query, field, pattern, out_error, out_error_capacity);
}

int32_t sk_query_add_labeled( sk_query *query, int32_t unique_id, double score, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_query *query, int32_t unique_id, double score, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_query_add_labeled");
    return fn(query, unique_id, score, out_error, out_error_capacity);
}

int32_t sk_query_add_weighted_set_strings( sk_query *query, const char *field, const uint8_t *key_bytes, uint32_t key_bytes_length, const uint32_t *key_lengths, const int64_t *weights, uint32_t item_count, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_query *query, const char *field, const uint8_t *key_bytes, uint32_t key_bytes_length, const uint32_t *key_lengths, const int64_t *weights, uint32_t item_count, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_query_add_weighted_set_strings");
    return fn(query, field, key_bytes, key_bytes_length, key_lengths, weights, item_count, out_error, out_error_capacity);
}

int32_t sk_query_add_weighted_set_integers( sk_query *query, const char *field, const int64_t *keys, const int64_t *weights, uint32_t item_count, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_query *query, const char *field, const int64_t *keys, const int64_t *weights, uint32_t item_count, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_query_add_weighted_set_integers");
    return fn(query, field, keys, weights, item_count, out_error, out_error_capacity);
}

int32_t sk_query_add_dot_product_strings( sk_query *query, const char *field, const uint8_t *key_bytes, uint32_t key_bytes_length, const uint32_t *key_lengths, const int64_t *weights, uint32_t item_count, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_query *query, const char *field, const uint8_t *key_bytes, uint32_t key_bytes_length, const uint32_t *key_lengths, const int64_t *weights, uint32_t item_count, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_query_add_dot_product_strings");
    return fn(query, field, key_bytes, key_bytes_length, key_lengths, weights, item_count, out_error, out_error_capacity);
}

int32_t sk_query_add_dot_product_integers( sk_query *query, const char *field, const int64_t *keys, const int64_t *weights, uint32_t item_count, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_query *query, const char *field, const int64_t *keys, const int64_t *weights, uint32_t item_count, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_query_add_dot_product_integers");
    return fn(query, field, keys, weights, item_count, out_error, out_error_capacity);
}

int32_t sk_query_add_wand_strings( sk_query *query, const char *field, const uint8_t *key_bytes, uint32_t key_bytes_length, const uint32_t *key_lengths, const int64_t *weights, uint32_t item_count, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_query *query, const char *field, const uint8_t *key_bytes, uint32_t key_bytes_length, const uint32_t *key_lengths, const int64_t *weights, uint32_t item_count, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_query_add_wand_strings");
    return fn(query, field, key_bytes, key_bytes_length, key_lengths, weights, item_count, out_error, out_error_capacity);
}

int32_t sk_query_add_wand_integers( sk_query *query, const char *field, const int64_t *keys, const int64_t *weights, uint32_t item_count, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_query *query, const char *field, const int64_t *keys, const int64_t *weights, uint32_t item_count, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_query_add_wand_integers");
    return fn(query, field, keys, weights, item_count, out_error, out_error_capacity);
}

int32_t sk_query_add_wand_strings_with_options( sk_query *query, const char *field, const uint8_t *key_bytes, uint32_t key_bytes_length, const uint32_t *key_lengths, const int64_t *weights, uint32_t item_count, uint32_t target_num_hits, int64_t score_threshold, double threshold_boost_factor, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_query *query, const char *field, const uint8_t *key_bytes, uint32_t key_bytes_length, const uint32_t *key_lengths, const int64_t *weights, uint32_t item_count, uint32_t target_num_hits, int64_t score_threshold, double threshold_boost_factor, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_query_add_wand_strings_with_options");
    return fn(query, field, key_bytes, key_bytes_length, key_lengths, weights, item_count, target_num_hits, score_threshold, threshold_boost_factor, out_error, out_error_capacity);
}

int32_t sk_query_add_wand_integers_with_options( sk_query *query, const char *field, const int64_t *keys, const int64_t *weights, uint32_t item_count, uint32_t target_num_hits, int64_t score_threshold, double threshold_boost_factor, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_query *query, const char *field, const int64_t *keys, const int64_t *weights, uint32_t item_count, uint32_t target_num_hits, int64_t score_threshold, double threshold_boost_factor, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_query_add_wand_integers_with_options");
    return fn(query, field, keys, weights, item_count, target_num_hits, score_threshold, threshold_boost_factor, out_error, out_error_capacity);
}

int32_t sk_query_add_equals(sk_query *query, const char *field, int64_t value, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)(sk_query *query, const char *field, int64_t value, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_query_add_equals");
    return fn(query, field, value, out_error, out_error_capacity);
}

int32_t sk_query_add_range(sk_query *query, const char *field, uint32_t lower_kind, int64_t lower_value, uint32_t upper_kind, int64_t upper_value, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)(sk_query *query, const char *field, uint32_t lower_kind, int64_t lower_value, uint32_t upper_kind, int64_t upper_value, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_query_add_range");
    return fn(query, field, lower_kind, lower_value, upper_kind, upper_value, out_error, out_error_capacity);
}

int32_t sk_query_add_nearest_neighbor( sk_query *query, const char *document_field, const char *query_tensor, uint32_t target_num_hits, uint8_t allow_approximate, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_query *query, const char *document_field, const char *query_tensor, uint32_t target_num_hits, uint8_t allow_approximate, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_query_add_nearest_neighbor");
    return fn(query, document_field, query_tensor, target_num_hits, allow_approximate, out_error, out_error_capacity);
}

int32_t sk_nearest_neighbor_options_create( sk_nearest_neighbor_options **out_options, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_nearest_neighbor_options **out_options, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_nearest_neighbor_options_create");
    return fn(out_options, out_error, out_error_capacity);
}

void sk_nearest_neighbor_options_destroy( sk_nearest_neighbor_options *options) {
    typedef void (*fn_type)( sk_nearest_neighbor_options *options);
    fn_type fn = (fn_type)sk_loader_symbol("sk_nearest_neighbor_options_destroy");
    fn(options);
}

int32_t sk_nearest_neighbor_options_set_distance_threshold( sk_nearest_neighbor_options *options, double distance_threshold, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_nearest_neighbor_options *options, double distance_threshold, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_nearest_neighbor_options_set_distance_threshold");
    return fn(options, distance_threshold, out_error, out_error_capacity);
}

int32_t sk_nearest_neighbor_options_set_explore_additional_hits( sk_nearest_neighbor_options *options, uint32_t explore_additional_hits, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_nearest_neighbor_options *options, uint32_t explore_additional_hits, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_nearest_neighbor_options_set_explore_additional_hits");
    return fn(options, explore_additional_hits, out_error, out_error_capacity);
}

int32_t sk_query_add_nearest_neighbor_with_options_handle( sk_query *query, const char *document_field, const char *query_tensor, uint32_t target_num_hits, uint8_t allow_approximate, const sk_nearest_neighbor_options *options, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_query *query, const char *document_field, const char *query_tensor, uint32_t target_num_hits, uint8_t allow_approximate, const sk_nearest_neighbor_options *options, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_query_add_nearest_neighbor_with_options_handle");
    return fn(query, document_field, query_tensor, target_num_hits, allow_approximate, options, out_error, out_error_capacity);
}

int32_t sk_query_add_same_element( sk_query *query, const char *field, uint32_t child_count, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_query *query, const char *field, uint32_t child_count, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_query_add_same_element");
    return fn(query, field, child_count, out_error, out_error_capacity);
}

int32_t sk_query_add_same_element_with_filter( sk_query *query, const char *field, uint32_t child_count, const uint32_t *element_filter, uint32_t element_filter_count, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_query *query, const char *field, uint32_t child_count, const uint32_t *element_filter, uint32_t element_filter_count, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_query_add_same_element_with_filter");
    return fn(query, field, child_count, element_filter, element_filter_count, out_error, out_error_capacity);
}

int32_t sk_query_add_geo_location( sk_query *query, const char *field, double latitude, double longitude, const char *radius, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_query *query, const char *field, double latitude, double longitude, const char *radius, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_query_add_geo_location");
    return fn(query, field, latitude, longitude, radius, out_error, out_error_capacity);
}

int32_t sk_query_add_geo_bounding_box( sk_query *query, const char *field, double south, double west, double north, double east, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_query *query, const char *field, double south, double west, double north, double east, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_query_add_geo_bounding_box");
    return fn(query, field, south, west, north, east, out_error, out_error_capacity);
}

int32_t sk_query_add_predicate( sk_query *query, const char *field, const uint8_t *feature_key_bytes, const uint32_t *feature_key_lengths, const uint8_t *feature_value_bytes, const uint32_t *feature_value_lengths, const uint64_t *feature_masks, uint32_t feature_count, const uint8_t *range_key_bytes, const uint32_t *range_key_lengths, const uint64_t *range_values, const uint64_t *range_masks, uint32_t range_count, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_query *query, const char *field, const uint8_t *feature_key_bytes, const uint32_t *feature_key_lengths, const uint8_t *feature_value_bytes, const uint32_t *feature_value_lengths, const uint64_t *feature_masks, uint32_t feature_count, const uint8_t *range_key_bytes, const uint32_t *range_key_lengths, const uint64_t *range_values, const uint64_t *range_masks, uint32_t range_count, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_query_add_predicate");
    return fn(query, field, feature_key_bytes, feature_key_lengths, feature_value_bytes, feature_value_lengths, feature_masks, feature_count, range_key_bytes, range_key_lengths, range_values, range_masks, range_count, out_error, out_error_capacity);
}

void sk_query_destroy(sk_query *query) {
    typedef void (*fn_type)(sk_query *query);
    fn_type fn = (fn_type)sk_loader_symbol("sk_query_destroy");
    fn(query);
}

int32_t sk_engine_search_query(const sk_engine *engine, const sk_query *query, const char *rank_profile, const char *source, sk_text_result **out_result, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)(const sk_engine *engine, const sk_query *query, const char *rank_profile, const char *source, sk_text_result **out_result, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_engine_search_query");
    return fn(engine, query, rank_profile, source, out_result, out_error, out_error_capacity);
}

int32_t sk_local_search_request_create( const char *rank_profile, uint32_t limit, uint32_t offset, uint32_t summary_kind, sk_local_search_request **out_request, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const char *rank_profile, uint32_t limit, uint32_t offset, uint32_t summary_kind, sk_local_search_request **out_request, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_local_search_request_create");
    return fn(rank_profile, limit, offset, summary_kind, out_request, out_error, out_error_capacity);
}

int32_t sk_local_search_request_add_sort( sk_local_search_request *request, const char *field, uint32_t order, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_local_search_request *request, const char *field, uint32_t order, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_local_search_request_add_sort");
    return fn(request, field, order, out_error, out_error_capacity);
}

int32_t sk_local_search_request_add_summary_field( sk_local_search_request *request, const char *field, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_local_search_request *request, const char *field, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_local_search_request_add_summary_field");
    return fn(request, field, out_error, out_error_capacity);
}

int32_t sk_local_search_request_add_highlight_field( sk_local_search_request *request, const char *field, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_local_search_request *request, const char *field, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_local_search_request_add_highlight_field");
    return fn(request, field, out_error, out_error_capacity);
}

int32_t sk_local_search_request_set_summary_class( sk_local_search_request *request, const char *name, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_local_search_request *request, const char *name, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_local_search_request_set_summary_class");
    return fn(request, name, out_error, out_error_capacity);
}

int32_t sk_local_search_request_add_query_feature( sk_local_search_request *request, const char *name, double value, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_local_search_request *request, const char *name, double value, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_local_search_request_add_query_feature");
    return fn(request, name, value, out_error, out_error_capacity);
}

int32_t sk_local_search_request_add_rank_property( sk_local_search_request *request, const char *name, const char *value, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_local_search_request *request, const char *name, const char *value, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_local_search_request_add_rank_property");
    return fn(request, name, value, out_error, out_error_capacity);
}

void sk_local_search_request_destroy(sk_local_search_request *request) {
    typedef void (*fn_type)(sk_local_search_request *request);
    fn_type fn = (fn_type)sk_loader_symbol("sk_local_search_request_destroy");
    fn(request);
}

int32_t sk_engine_search_local( const sk_engine *engine, const sk_query *query, const sk_local_search_request *request, sk_local_search_result **out_result, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const sk_engine *engine, const sk_query *query, const sk_local_search_request *request, sk_local_search_result **out_result, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_engine_search_local");
    return fn(engine, query, request, out_result, out_error, out_error_capacity);
}

uint64_t sk_local_search_result_total_hit_count( const sk_local_search_result *result) {
    typedef uint64_t (*fn_type)( const sk_local_search_result *result);
    fn_type fn = (fn_type)sk_loader_symbol("sk_local_search_result_total_hit_count");
    return fn(result);
}

uint32_t sk_local_search_result_hit_count( const sk_local_search_result *result) {
    typedef uint32_t (*fn_type)( const sk_local_search_result *result);
    fn_type fn = (fn_type)sk_loader_symbol("sk_local_search_result_hit_count");
    return fn(result);
}

int32_t sk_local_search_result_hit( const sk_local_search_result *result, uint32_t index, const char **out_id, uint32_t *out_id_length, double *out_score, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const sk_local_search_result *result, uint32_t index, const char **out_id, uint32_t *out_id_length, double *out_score, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_local_search_result_hit");
    return fn(result, index, out_id, out_id_length, out_score, out_error, out_error_capacity);
}

uint32_t sk_local_search_result_summary_count( const sk_local_search_result *result, uint32_t hit_index) {
    typedef uint32_t (*fn_type)( const sk_local_search_result *result, uint32_t hit_index);
    fn_type fn = (fn_type)sk_loader_symbol("sk_local_search_result_summary_count");
    return fn(result, hit_index);
}

int32_t sk_local_search_result_summary_name( const sk_local_search_result *result, uint32_t hit_index, uint32_t summary_index, const char **out_name, uint32_t *out_length, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const sk_local_search_result *result, uint32_t hit_index, uint32_t summary_index, const char **out_name, uint32_t *out_length, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_local_search_result_summary_name");
    return fn(result, hit_index, summary_index, out_name, out_length, out_error, out_error_capacity);
}

int32_t sk_local_search_result_summary_value( const sk_local_search_result *result, uint32_t hit_index, uint32_t summary_index, uint32_t *out_kind, const char **out_string, uint32_t *out_string_length, int64_t *out_integer, double *out_float, uint8_t *out_boolean, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const sk_local_search_result *result, uint32_t hit_index, uint32_t summary_index, uint32_t *out_kind, const char **out_string, uint32_t *out_string_length, int64_t *out_integer, double *out_float, uint8_t *out_boolean, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_local_search_result_summary_value");
    return fn(result, hit_index, summary_index, out_kind, out_string, out_string_length, out_integer, out_float, out_boolean, out_error, out_error_capacity);
}

uint32_t sk_local_search_result_match_feature_count( const sk_local_search_result *result) {
    typedef uint32_t (*fn_type)( const sk_local_search_result *result);
    fn_type fn = (fn_type)sk_loader_symbol("sk_local_search_result_match_feature_count");
    return fn(result);
}

int32_t sk_local_search_result_match_feature_name( const sk_local_search_result *result, uint32_t feature_index, const char **out_name, uint32_t *out_length, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const sk_local_search_result *result, uint32_t feature_index, const char **out_name, uint32_t *out_length, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_local_search_result_match_feature_name");
    return fn(result, feature_index, out_name, out_length, out_error, out_error_capacity);
}

int32_t sk_local_search_result_match_feature_value( const sk_local_search_result *result, uint32_t hit_index, uint32_t feature_index, double *out_value, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const sk_local_search_result *result, uint32_t hit_index, uint32_t feature_index, double *out_value, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_local_search_result_match_feature_value");
    return fn(result, hit_index, feature_index, out_value, out_error, out_error_capacity);
}

int32_t sk_local_search_result_match_feature_value_ref( const sk_local_search_result *result, uint32_t hit_index, uint32_t feature_index, const sk_feature_value **out_value, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const sk_local_search_result *result, uint32_t hit_index, uint32_t feature_index, const sk_feature_value **out_value, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_local_search_result_match_feature_value_ref");
    return fn(result, hit_index, feature_index, out_value, out_error, out_error_capacity);
}

void sk_local_search_result_destroy(sk_local_search_result *result) {
    typedef void (*fn_type)(sk_local_search_result *result);
    fn_type fn = (fn_type)sk_loader_symbol("sk_local_search_result_destroy");
    fn(result);
}

uint32_t sk_feature_value_kind(const sk_feature_value *value) {
    typedef uint32_t (*fn_type)(const sk_feature_value *value);
    fn_type fn = (fn_type)sk_loader_symbol("sk_feature_value_kind");
    return fn(value);
}

int32_t sk_feature_value_double(const sk_feature_value *value, double *out_value, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)(const sk_feature_value *value, double *out_value, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_feature_value_double");
    return fn(value, out_value, out_error, out_error_capacity);
}

int32_t sk_feature_value_data(const sk_feature_value *value, const uint8_t **out_data, size_t *out_size, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)(const sk_feature_value *value, const uint8_t **out_data, size_t *out_size, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_feature_value_data");
    return fn(value, out_data, out_size, out_error, out_error_capacity);
}

uint32_t sk_local_search_result_highlight_count( const sk_local_search_result *result, uint32_t hit_index) {
    typedef uint32_t (*fn_type)( const sk_local_search_result *result, uint32_t hit_index);
    fn_type fn = (fn_type)sk_loader_symbol("sk_local_search_result_highlight_count");
    return fn(result, hit_index);
}

int32_t sk_local_search_result_highlight_field( const sk_local_search_result *result, uint32_t hit_index, uint32_t highlight_index, const char **out_field, uint32_t *out_length, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const sk_local_search_result *result, uint32_t hit_index, uint32_t highlight_index, const char **out_field, uint32_t *out_length, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_local_search_result_highlight_field");
    return fn(result, hit_index, highlight_index, out_field, out_length, out_error, out_error_capacity);
}

uint32_t sk_local_search_result_highlight_fragment_count( const sk_local_search_result *result, uint32_t hit_index, uint32_t highlight_index) {
    typedef uint32_t (*fn_type)( const sk_local_search_result *result, uint32_t hit_index, uint32_t highlight_index);
    fn_type fn = (fn_type)sk_loader_symbol("sk_local_search_result_highlight_fragment_count");
    return fn(result, hit_index, highlight_index);
}

int32_t sk_local_search_result_highlight_fragment_text( const sk_local_search_result *result, uint32_t hit_index, uint32_t highlight_index, uint32_t fragment_index, const char **out_text, uint32_t *out_length, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const sk_local_search_result *result, uint32_t hit_index, uint32_t highlight_index, uint32_t fragment_index, const char **out_text, uint32_t *out_length, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_local_search_result_highlight_fragment_text");
    return fn(result, hit_index, highlight_index, fragment_index, out_text, out_length, out_error, out_error_capacity);
}

uint32_t sk_local_search_result_highlight_match_count( const sk_local_search_result *result, uint32_t hit_index, uint32_t highlight_index, uint32_t fragment_index) {
    typedef uint32_t (*fn_type)( const sk_local_search_result *result, uint32_t hit_index, uint32_t highlight_index, uint32_t fragment_index);
    fn_type fn = (fn_type)sk_loader_symbol("sk_local_search_result_highlight_match_count");
    return fn(result, hit_index, highlight_index, fragment_index);
}

int32_t sk_local_search_result_highlight_match( const sk_local_search_result *result, uint32_t hit_index, uint32_t highlight_index, uint32_t fragment_index, uint32_t match_index, uint32_t *out_start_byte, uint32_t *out_end_byte, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const sk_local_search_result *result, uint32_t hit_index, uint32_t highlight_index, uint32_t fragment_index, uint32_t match_index, uint32_t *out_start_byte, uint32_t *out_end_byte, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_local_search_result_highlight_match");
    return fn(result, hit_index, highlight_index, fragment_index, match_index, out_start_byte, out_end_byte, out_error, out_error_capacity);
}

int32_t sk_generic_search_request_create( const char *rank_profile, uint32_t limit, uint32_t offset, uint32_t summary_kind, sk_generic_search_request **out_request, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const char *rank_profile, uint32_t limit, uint32_t offset, uint32_t summary_kind, sk_generic_search_request **out_request, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_generic_search_request_create");
    return fn(rank_profile, limit, offset, summary_kind, out_request, out_error, out_error_capacity);
}

int32_t sk_generic_search_request_set_timeout( sk_generic_search_request *request, uint64_t timeout_millis, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_generic_search_request *request, uint64_t timeout_millis, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_generic_search_request_set_timeout");
    return fn(request, timeout_millis, out_error, out_error_capacity);
}

int32_t sk_generic_search_request_add_sort( sk_generic_search_request *request, const char *field, uint32_t order, uint32_t missing, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_generic_search_request *request, const char *field, uint32_t order, uint32_t missing, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_generic_search_request_add_sort");
    return fn(request, field, order, missing, out_error, out_error_capacity);
}

int32_t sk_generic_search_request_set_sort_spec( sk_generic_search_request *request, const char *sort_spec, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_generic_search_request *request, const char *sort_spec, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_generic_search_request_set_sort_spec");
    return fn(request, sort_spec, out_error, out_error_capacity);
}

int32_t sk_generic_search_request_add_summary_field( sk_generic_search_request *request, const char *field, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_generic_search_request *request, const char *field, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_generic_search_request_add_summary_field");
    return fn(request, field, out_error, out_error_capacity);
}

int32_t sk_generic_search_request_add_highlight_field( sk_generic_search_request *request, const char *field, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_generic_search_request *request, const char *field, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_generic_search_request_add_highlight_field");
    return fn(request, field, out_error, out_error_capacity);
}

int32_t sk_generic_search_request_set_summary_class( sk_generic_search_request *request, const char *name, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_generic_search_request *request, const char *name, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_generic_search_request_set_summary_class");
    return fn(request, name, out_error, out_error_capacity);
}

int32_t sk_generic_search_request_add_query_feature( sk_generic_search_request *request, const char *name, double value, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_generic_search_request *request, const char *name, double value, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_generic_search_request_add_query_feature");
    return fn(request, name, value, out_error, out_error_capacity);
}

int32_t sk_generic_search_request_add_query_tensor_dense( sk_generic_search_request *request, const char *name, const char *type_spec, const double *values, uint32_t value_count, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_generic_search_request *request, const char *name, const char *type_spec, const double *values, uint32_t value_count, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_generic_search_request_add_query_tensor_dense");
    return fn(request, name, type_spec, values, value_count, out_error, out_error_capacity);
}

int32_t sk_generic_search_request_add_rank_property( sk_generic_search_request *request, const char *name, const char *value, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_generic_search_request *request, const char *name, const char *value, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_generic_search_request_add_rank_property");
    return fn(request, name, value, out_error, out_error_capacity);
}

int32_t sk_generic_search_request_add_grouping( sk_generic_search_request *request, const char *group_field, uint32_t max_groups, uint32_t order, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_generic_search_request *request, const char *group_field, uint32_t max_groups, uint32_t order, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_generic_search_request_add_grouping");
    return fn(request, group_field, max_groups, order, out_error, out_error_capacity);
}

int32_t sk_generic_search_request_add_grouping_plan( sk_generic_search_request *request, const sk_grouping_plan *plan, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_generic_search_request *request, const sk_grouping_plan *plan, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_generic_search_request_add_grouping_plan");
    return fn(request, plan, out_error, out_error_capacity);
}

void sk_generic_search_request_destroy(sk_generic_search_request *request) {
    typedef void (*fn_type)(sk_generic_search_request *request);
    fn_type fn = (fn_type)sk_loader_symbol("sk_generic_search_request_destroy");
    fn(request);
}

int32_t sk_engine_search_generic( const sk_engine *engine, const sk_query *query, const sk_generic_search_request *request, sk_generic_search_result **out_result, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const sk_engine *engine, const sk_query *query, const sk_generic_search_request *request, sk_generic_search_result **out_result, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_engine_search_generic");
    return fn(engine, query, request, out_result, out_error, out_error_capacity);
}

uint64_t sk_generic_search_result_total_hit_count( const sk_generic_search_result *result) {
    typedef uint64_t (*fn_type)( const sk_generic_search_result *result);
    fn_type fn = (fn_type)sk_loader_symbol("sk_generic_search_result_total_hit_count");
    return fn(result);
}

uint32_t sk_generic_search_result_hit_count( const sk_generic_search_result *result) {
    typedef uint32_t (*fn_type)( const sk_generic_search_result *result);
    fn_type fn = (fn_type)sk_loader_symbol("sk_generic_search_result_hit_count");
    return fn(result);
}

uint64_t sk_generic_search_result_coverage_active( const sk_generic_search_result *result) {
    typedef uint64_t (*fn_type)( const sk_generic_search_result *result);
    fn_type fn = (fn_type)sk_loader_symbol("sk_generic_search_result_coverage_active");
    return fn(result);
}

uint64_t sk_generic_search_result_coverage_covered( const sk_generic_search_result *result) {
    typedef uint64_t (*fn_type)( const sk_generic_search_result *result);
    fn_type fn = (fn_type)sk_loader_symbol("sk_generic_search_result_coverage_covered");
    return fn(result);
}

uint64_t sk_generic_search_result_coverage_target_active( const sk_generic_search_result *result) {
    typedef uint64_t (*fn_type)( const sk_generic_search_result *result);
    fn_type fn = (fn_type)sk_loader_symbol("sk_generic_search_result_coverage_target_active");
    return fn(result);
}

uint32_t sk_generic_search_result_degradation_flags( const sk_generic_search_result *result) {
    typedef uint32_t (*fn_type)( const sk_generic_search_result *result);
    fn_type fn = (fn_type)sk_loader_symbol("sk_generic_search_result_degradation_flags");
    return fn(result);
}

uint32_t sk_generic_search_result_issue_count( const sk_generic_search_result *result) {
    typedef uint32_t (*fn_type)( const sk_generic_search_result *result);
    fn_type fn = (fn_type)sk_loader_symbol("sk_generic_search_result_issue_count");
    return fn(result);
}

int32_t sk_generic_search_result_issue_message( const sk_generic_search_result *result, uint32_t index, const char **out_message, uint32_t *out_length, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const sk_generic_search_result *result, uint32_t index, const char **out_message, uint32_t *out_length, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_generic_search_result_issue_message");
    return fn(result, index, out_message, out_length, out_error, out_error_capacity);
}

int32_t sk_generic_search_result_hit( const sk_generic_search_result *result, uint32_t index, const char **out_id, uint32_t *out_id_length, double *out_score, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const sk_generic_search_result *result, uint32_t index, const char **out_id, uint32_t *out_id_length, double *out_score, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_generic_search_result_hit");
    return fn(result, index, out_id, out_id_length, out_score, out_error, out_error_capacity);
}

uint32_t sk_generic_search_result_summary_count( const sk_generic_search_result *result, uint32_t hit_index) {
    typedef uint32_t (*fn_type)( const sk_generic_search_result *result, uint32_t hit_index);
    fn_type fn = (fn_type)sk_loader_symbol("sk_generic_search_result_summary_count");
    return fn(result, hit_index);
}

int32_t sk_generic_search_result_summary_name( const sk_generic_search_result *result, uint32_t hit_index, uint32_t summary_index, const char **out_name, uint32_t *out_length, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const sk_generic_search_result *result, uint32_t hit_index, uint32_t summary_index, const char **out_name, uint32_t *out_length, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_generic_search_result_summary_name");
    return fn(result, hit_index, summary_index, out_name, out_length, out_error, out_error_capacity);
}

int32_t sk_generic_search_result_summary_value( const sk_generic_search_result *result, uint32_t hit_index, uint32_t summary_index, uint32_t *out_kind, const char **out_string, uint32_t *out_string_length, int64_t *out_integer, double *out_float, uint8_t *out_boolean, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const sk_generic_search_result *result, uint32_t hit_index, uint32_t summary_index, uint32_t *out_kind, const char **out_string, uint32_t *out_string_length, int64_t *out_integer, double *out_float, uint8_t *out_boolean, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_generic_search_result_summary_value");
    return fn(result, hit_index, summary_index, out_kind, out_string, out_string_length, out_integer, out_float, out_boolean, out_error, out_error_capacity);
}

uint32_t sk_generic_search_result_highlight_count( const sk_generic_search_result *result, uint32_t hit_index) {
    typedef uint32_t (*fn_type)( const sk_generic_search_result *result, uint32_t hit_index);
    fn_type fn = (fn_type)sk_loader_symbol("sk_generic_search_result_highlight_count");
    return fn(result, hit_index);
}

int32_t sk_generic_search_result_highlight_field( const sk_generic_search_result *result, uint32_t hit_index, uint32_t highlight_index, const char **out_field, uint32_t *out_length, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const sk_generic_search_result *result, uint32_t hit_index, uint32_t highlight_index, const char **out_field, uint32_t *out_length, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_generic_search_result_highlight_field");
    return fn(result, hit_index, highlight_index, out_field, out_length, out_error, out_error_capacity);
}

uint32_t sk_generic_search_result_highlight_fragment_count( const sk_generic_search_result *result, uint32_t hit_index, uint32_t highlight_index) {
    typedef uint32_t (*fn_type)( const sk_generic_search_result *result, uint32_t hit_index, uint32_t highlight_index);
    fn_type fn = (fn_type)sk_loader_symbol("sk_generic_search_result_highlight_fragment_count");
    return fn(result, hit_index, highlight_index);
}

int32_t sk_generic_search_result_highlight_fragment_text( const sk_generic_search_result *result, uint32_t hit_index, uint32_t highlight_index, uint32_t fragment_index, const char **out_text, uint32_t *out_length, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const sk_generic_search_result *result, uint32_t hit_index, uint32_t highlight_index, uint32_t fragment_index, const char **out_text, uint32_t *out_length, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_generic_search_result_highlight_fragment_text");
    return fn(result, hit_index, highlight_index, fragment_index, out_text, out_length, out_error, out_error_capacity);
}

uint32_t sk_generic_search_result_highlight_match_count( const sk_generic_search_result *result, uint32_t hit_index, uint32_t highlight_index, uint32_t fragment_index) {
    typedef uint32_t (*fn_type)( const sk_generic_search_result *result, uint32_t hit_index, uint32_t highlight_index, uint32_t fragment_index);
    fn_type fn = (fn_type)sk_loader_symbol("sk_generic_search_result_highlight_match_count");
    return fn(result, hit_index, highlight_index, fragment_index);
}

int32_t sk_generic_search_result_highlight_match( const sk_generic_search_result *result, uint32_t hit_index, uint32_t highlight_index, uint32_t fragment_index, uint32_t match_index, uint32_t *out_start_byte, uint32_t *out_end_byte, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const sk_generic_search_result *result, uint32_t hit_index, uint32_t highlight_index, uint32_t fragment_index, uint32_t match_index, uint32_t *out_start_byte, uint32_t *out_end_byte, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_generic_search_result_highlight_match");
    return fn(result, hit_index, highlight_index, fragment_index, match_index, out_start_byte, out_end_byte, out_error, out_error_capacity);
}

uint32_t sk_generic_search_result_match_feature_count( const sk_generic_search_result *result) {
    typedef uint32_t (*fn_type)( const sk_generic_search_result *result);
    fn_type fn = (fn_type)sk_loader_symbol("sk_generic_search_result_match_feature_count");
    return fn(result);
}

int32_t sk_generic_search_result_match_feature_name( const sk_generic_search_result *result, uint32_t feature_index, const char **out_name, uint32_t *out_length, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const sk_generic_search_result *result, uint32_t feature_index, const char **out_name, uint32_t *out_length, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_generic_search_result_match_feature_name");
    return fn(result, feature_index, out_name, out_length, out_error, out_error_capacity);
}

int32_t sk_generic_search_result_match_feature_value( const sk_generic_search_result *result, uint32_t hit_index, uint32_t feature_index, double *out_value, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const sk_generic_search_result *result, uint32_t hit_index, uint32_t feature_index, double *out_value, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_generic_search_result_match_feature_value");
    return fn(result, hit_index, feature_index, out_value, out_error, out_error_capacity);
}

int32_t sk_generic_search_result_match_feature_value_ref( const sk_generic_search_result *result, uint32_t hit_index, uint32_t feature_index, const sk_feature_value **out_value, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const sk_generic_search_result *result, uint32_t hit_index, uint32_t feature_index, const sk_feature_value **out_value, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_generic_search_result_match_feature_value_ref");
    return fn(result, hit_index, feature_index, out_value, out_error, out_error_capacity);
}

uint32_t sk_generic_search_result_grouping_count( const sk_generic_search_result *result) {
    typedef uint32_t (*fn_type)( const sk_generic_search_result *result);
    fn_type fn = (fn_type)sk_loader_symbol("sk_generic_search_result_grouping_count");
    return fn(result);
}

uint32_t sk_generic_search_result_grouping_tree_count( const sk_generic_search_result *result) {
    typedef uint32_t (*fn_type)( const sk_generic_search_result *result);
    fn_type fn = (fn_type)sk_loader_symbol("sk_generic_search_result_grouping_tree_count");
    return fn(result);
}

int32_t sk_generic_search_result_grouping_tree_root( const sk_generic_search_result *result, uint32_t grouping_index, const sk_grouping_tree_node **out_root, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const sk_generic_search_result *result, uint32_t grouping_index, const sk_grouping_tree_node **out_root, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_generic_search_result_grouping_tree_root");
    return fn(result, grouping_index, out_root, out_error, out_error_capacity);
}

uint32_t sk_generic_search_result_group_count( const sk_generic_search_result *result, uint32_t grouping_index) {
    typedef uint32_t (*fn_type)( const sk_generic_search_result *result, uint32_t grouping_index);
    fn_type fn = (fn_type)sk_loader_symbol("sk_generic_search_result_group_count");
    return fn(result, grouping_index);
}

int32_t sk_generic_search_result_group( const sk_generic_search_result *result, uint32_t grouping_index, uint32_t group_index, uint32_t *out_key_kind, const char **out_key_string, uint32_t *out_key_string_length, int64_t *out_key_integer, uint64_t *out_count, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const sk_generic_search_result *result, uint32_t grouping_index, uint32_t group_index, uint32_t *out_key_kind, const char **out_key_string, uint32_t *out_key_string_length, int64_t *out_key_integer, uint64_t *out_count, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_generic_search_result_group");
    return fn(result, grouping_index, group_index, out_key_kind, out_key_string, out_key_string_length, out_key_integer, out_count, out_error, out_error_capacity);
}

void sk_generic_search_result_destroy(sk_generic_search_result *result) {
    typedef void (*fn_type)(sk_generic_search_result *result);
    fn_type fn = (fn_type)sk_loader_symbol("sk_generic_search_result_destroy");
    fn(result);
}

int32_t sk_query_add_attribute_string_equals( sk_query *query, const char *field, const char *value, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_query *query, const char *field, const char *value, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_query_add_attribute_string_equals");
    return fn(query, field, value, out_error, out_error_capacity);
}

int32_t sk_query_add_attribute_float_equals( sk_query *query, const char *field, double value, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_query *query, const char *field, double value, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_query_add_attribute_float_equals");
    return fn(query, field, value, out_error, out_error_capacity);
}

int32_t sk_query_add_attribute_boolean_equals( sk_query *query, const char *field, uint8_t value, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_query *query, const char *field, uint8_t value, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_query_add_attribute_boolean_equals");
    return fn(query, field, value, out_error, out_error_capacity);
}

int32_t sk_query_add_attribute_exists( sk_query *query, const char *field, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_query *query, const char *field, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_query_add_attribute_exists");
    return fn(query, field, out_error, out_error_capacity);
}

int32_t sk_query_add_attribute_integer_range( sk_query *query, const char *field, uint32_t lower_kind, int64_t lower_value, uint32_t upper_kind, int64_t upper_value, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_query *query, const char *field, uint32_t lower_kind, int64_t lower_value, uint32_t upper_kind, int64_t upper_value, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_query_add_attribute_integer_range");
    return fn(query, field, lower_kind, lower_value, upper_kind, upper_value, out_error, out_error_capacity);
}

int32_t sk_query_add_attribute_float_range( sk_query *query, const char *field, uint32_t lower_kind, double lower_value, uint32_t upper_kind, double upper_value, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_query *query, const char *field, uint32_t lower_kind, double lower_value, uint32_t upper_kind, double upper_value, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_query_add_attribute_float_range");
    return fn(query, field, lower_kind, lower_value, upper_kind, upper_value, out_error, out_error_capacity);
}

int32_t sk_attribute_document_create( const char *document_id, sk_attribute_document **out_document, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const char *document_id, sk_attribute_document **out_document, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_attribute_document_create");
    return fn(document_id, out_document, out_error, out_error_capacity);
}

int32_t sk_attribute_document_add_string( sk_attribute_document *document, const char *field, const char *value, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_attribute_document *document, const char *field, const char *value, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_attribute_document_add_string");
    return fn(document, field, value, out_error, out_error_capacity);
}

int32_t sk_attribute_document_add_integer( sk_attribute_document *document, const char *field, int64_t value, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_attribute_document *document, const char *field, int64_t value, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_attribute_document_add_integer");
    return fn(document, field, value, out_error, out_error_capacity);
}

int32_t sk_attribute_document_add_float( sk_attribute_document *document, const char *field, double value, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_attribute_document *document, const char *field, double value, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_attribute_document_add_float");
    return fn(document, field, value, out_error, out_error_capacity);
}

int32_t sk_attribute_document_add_boolean( sk_attribute_document *document, const char *field, uint8_t value, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_attribute_document *document, const char *field, uint8_t value, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_attribute_document_add_boolean");
    return fn(document, field, value, out_error, out_error_capacity);
}

void sk_attribute_document_destroy(sk_attribute_document *document) {
    typedef void (*fn_type)(sk_attribute_document *document);
    fn_type fn = (fn_type)sk_loader_symbol("sk_attribute_document_destroy");
    fn(document);
}

int32_t sk_attribute_update_create( const char *document_id, sk_attribute_update **out_update, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const char *document_id, sk_attribute_update **out_update, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_attribute_update_create");
    return fn(document_id, out_update, out_error, out_error_capacity);
}

int32_t sk_attribute_update_add_string( sk_attribute_update *update, const char *field, const char *value, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_attribute_update *update, const char *field, const char *value, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_attribute_update_add_string");
    return fn(update, field, value, out_error, out_error_capacity);
}

int32_t sk_attribute_update_add_integer( sk_attribute_update *update, const char *field, int64_t value, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_attribute_update *update, const char *field, int64_t value, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_attribute_update_add_integer");
    return fn(update, field, value, out_error, out_error_capacity);
}

int32_t sk_attribute_update_add_float( sk_attribute_update *update, const char *field, double value, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_attribute_update *update, const char *field, double value, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_attribute_update_add_float");
    return fn(update, field, value, out_error, out_error_capacity);
}

int32_t sk_attribute_update_add_boolean( sk_attribute_update *update, const char *field, uint8_t value, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_attribute_update *update, const char *field, uint8_t value, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_attribute_update_add_boolean");
    return fn(update, field, value, out_error, out_error_capacity);
}

void sk_attribute_update_destroy(sk_attribute_update *update) {
    typedef void (*fn_type)(sk_attribute_update *update);
    fn_type fn = (fn_type)sk_loader_symbol("sk_attribute_update_destroy");
    fn(update);
}

int32_t sk_engine_put_attributes( sk_engine *engine, const sk_attribute_document *document, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_engine *engine, const sk_attribute_document *document, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_engine_put_attributes");
    return fn(engine, document, out_error, out_error_capacity);
}

int32_t sk_engine_update_attributes( sk_engine *engine, const sk_attribute_update *update, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_engine *engine, const sk_attribute_update *update, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_engine_update_attributes");
    return fn(engine, update, out_error, out_error_capacity);
}

int32_t sk_grouping_plan_create( sk_grouping_plan **out_plan, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_grouping_plan **out_plan, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_grouping_plan_create");
    return fn(out_plan, out_error, out_error_capacity);
}

int32_t sk_grouping_plan_set_all( sk_grouping_plan *plan, uint8_t all, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_grouping_plan *plan, uint8_t all, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_grouping_plan_set_all");
    return fn(plan, all, out_error, out_error_capacity);
}

int32_t sk_grouping_expression_attribute( const char *field, sk_grouping_expression **out_expression, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const char *field, sk_grouping_expression **out_expression, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_grouping_expression_attribute");
    return fn(field, out_expression, out_error, out_error_capacity);
}

int32_t sk_grouping_expression_string( const char *value, sk_grouping_expression **out_expression, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const char *value, sk_grouping_expression **out_expression, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_grouping_expression_string");
    return fn(value, out_expression, out_error, out_error_capacity);
}

int32_t sk_grouping_expression_integer( int64_t value, sk_grouping_expression **out_expression, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( int64_t value, sk_grouping_expression **out_expression, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_grouping_expression_integer");
    return fn(value, out_expression, out_error, out_error_capacity);
}

int32_t sk_grouping_expression_float( double value, sk_grouping_expression **out_expression, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( double value, sk_grouping_expression **out_expression, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_grouping_expression_float");
    return fn(value, out_expression, out_error, out_error_capacity);
}

int32_t sk_grouping_expression_boolean( uint8_t value, sk_grouping_expression **out_expression, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( uint8_t value, sk_grouping_expression **out_expression, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_grouping_expression_boolean");
    return fn(value, out_expression, out_error, out_error_capacity);
}

int32_t sk_grouping_expression_negative( const sk_grouping_expression *input, sk_grouping_expression **out_expression, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const sk_grouping_expression *input, sk_grouping_expression **out_expression, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_grouping_expression_negative");
    return fn(input, out_expression, out_error, out_error_capacity);
}

int32_t sk_grouping_expression_binary( uint32_t operator_kind, const sk_grouping_expression *left, const sk_grouping_expression *right, sk_grouping_expression **out_expression, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( uint32_t operator_kind, const sk_grouping_expression *left, const sk_grouping_expression *right, sk_grouping_expression **out_expression, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_grouping_expression_binary");
    return fn(operator_kind, left, right, out_expression, out_error, out_error_capacity);
}

int32_t sk_grouping_expression_function( const char *name, const sk_grouping_expression *const *arguments, uint32_t argument_count, sk_grouping_expression **out_expression, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const char *name, const sk_grouping_expression *const *arguments, uint32_t argument_count, sk_grouping_expression **out_expression, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_grouping_expression_function");
    return fn(name, arguments, argument_count, out_expression, out_error, out_error_capacity);
}

int32_t sk_grouping_expression_aggregation_reference( uint32_t index, sk_grouping_expression **out_expression, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( uint32_t index, sk_grouping_expression **out_expression, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_grouping_expression_aggregation_reference");
    return fn(index, out_expression, out_error, out_error_capacity);
}

void sk_grouping_expression_destroy( sk_grouping_expression *expression) {
    typedef void (*fn_type)( sk_grouping_expression *expression);
    fn_type fn = (fn_type)sk_loader_symbol("sk_grouping_expression_destroy");
    fn(expression);
}

int32_t sk_grouping_aggregation_count( sk_grouping_aggregation **out, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_grouping_aggregation **out, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_grouping_aggregation_count");
    return fn(out, out_error, out_error_capacity);
}

int32_t sk_grouping_aggregation_average( const sk_grouping_expression *expression, sk_grouping_aggregation **out, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const sk_grouping_expression *expression, sk_grouping_aggregation **out, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_grouping_aggregation_average");
    return fn(expression, out, out_error, out_error_capacity);
}

int32_t sk_grouping_aggregation_sum( const sk_grouping_expression *expression, sk_grouping_aggregation **out, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const sk_grouping_expression *expression, sk_grouping_aggregation **out, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_grouping_aggregation_sum");
    return fn(expression, out, out_error, out_error_capacity);
}

int32_t sk_grouping_aggregation_min( const sk_grouping_expression *expression, sk_grouping_aggregation **out, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const sk_grouping_expression *expression, sk_grouping_aggregation **out, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_grouping_aggregation_min");
    return fn(expression, out, out_error, out_error_capacity);
}

int32_t sk_grouping_aggregation_max( const sk_grouping_expression *expression, sk_grouping_aggregation **out, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const sk_grouping_expression *expression, sk_grouping_aggregation **out, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_grouping_aggregation_max");
    return fn(expression, out, out_error, out_error_capacity);
}

int32_t sk_grouping_aggregation_stddev( const sk_grouping_expression *expression, sk_grouping_aggregation **out, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const sk_grouping_expression *expression, sk_grouping_aggregation **out, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_grouping_aggregation_stddev");
    return fn(expression, out, out_error, out_error_capacity);
}

int32_t sk_grouping_aggregation_xor( const sk_grouping_expression *expression, sk_grouping_aggregation **out, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const sk_grouping_expression *expression, sk_grouping_aggregation **out, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_grouping_aggregation_xor");
    return fn(expression, out, out_error, out_error_capacity);
}

int32_t sk_grouping_aggregation_quantiles( const sk_grouping_expression *expression, const double *quantiles, uint32_t quantile_count, sk_grouping_aggregation **out, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const sk_grouping_expression *expression, const double *quantiles, uint32_t quantile_count, sk_grouping_aggregation **out, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_grouping_aggregation_quantiles");
    return fn(expression, quantiles, quantile_count, out, out_error, out_error_capacity);
}

int32_t sk_grouping_aggregation_summary( const char *summary_class, uint32_t max_hits, sk_grouping_aggregation **out, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const char *summary_class, uint32_t max_hits, sk_grouping_aggregation **out, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_grouping_aggregation_summary");
    return fn(summary_class, max_hits, out, out_error, out_error_capacity);
}

void sk_grouping_aggregation_destroy( sk_grouping_aggregation *aggregation) {
    typedef void (*fn_type)( sk_grouping_aggregation *aggregation);
    fn_type fn = (fn_type)sk_loader_symbol("sk_grouping_aggregation_destroy");
    fn(aggregation);
}

int32_t sk_grouping_plan_add_level( sk_grouping_plan *plan, const sk_grouping_expression *classifier, sk_grouping_level **out_level, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_grouping_plan *plan, const sk_grouping_expression *classifier, sk_grouping_level **out_level, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_grouping_plan_add_level");
    return fn(plan, classifier, out_level, out_error, out_error_capacity);
}

int32_t sk_grouping_level_set_max_groups( sk_grouping_level *level, int64_t max_groups, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_grouping_level *level, int64_t max_groups, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_grouping_level_set_max_groups");
    return fn(level, max_groups, out_error, out_error_capacity);
}

int32_t sk_grouping_level_set_precision( sk_grouping_level *level, int64_t precision, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_grouping_level *level, int64_t precision, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_grouping_level_set_precision");
    return fn(level, precision, out_error, out_error_capacity);
}

int32_t sk_grouping_filter_regex( const char *pattern, const char *field, sk_grouping_filter **out_filter, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const char *pattern, const char *field, sk_grouping_filter **out_filter, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_grouping_filter_regex");
    return fn(pattern, field, out_filter, out_error, out_error_capacity);
}

int32_t sk_grouping_filter_regex_expression( const char *pattern, const sk_grouping_expression *expression, sk_grouping_filter **out_filter, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const char *pattern, const sk_grouping_expression *expression, sk_grouping_filter **out_filter, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_grouping_filter_regex_expression");
    return fn(pattern, expression, out_filter, out_error, out_error_capacity);
}

int32_t sk_grouping_filter_and( const sk_grouping_filter *const *filters, uint32_t filter_count, sk_grouping_filter **out_filter, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const sk_grouping_filter *const *filters, uint32_t filter_count, sk_grouping_filter **out_filter, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_grouping_filter_and");
    return fn(filters, filter_count, out_filter, out_error, out_error_capacity);
}

int32_t sk_grouping_filter_or( const sk_grouping_filter *const *filters, uint32_t filter_count, sk_grouping_filter **out_filter, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const sk_grouping_filter *const *filters, uint32_t filter_count, sk_grouping_filter **out_filter, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_grouping_filter_or");
    return fn(filters, filter_count, out_filter, out_error, out_error_capacity);
}

int32_t sk_grouping_filter_not( const sk_grouping_filter *filter, sk_grouping_filter **out_filter, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const sk_grouping_filter *filter, sk_grouping_filter **out_filter, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_grouping_filter_not");
    return fn(filter, out_filter, out_error, out_error_capacity);
}

int32_t sk_grouping_filter_range( double lower, double upper, const sk_grouping_expression *expression, uint8_t include_lower, uint8_t include_upper, sk_grouping_filter **out_filter, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( double lower, double upper, const sk_grouping_expression *expression, uint8_t include_lower, uint8_t include_upper, sk_grouping_filter **out_filter, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_grouping_filter_range");
    return fn(lower, upper, expression, include_lower, include_upper, out_filter, out_error, out_error_capacity);
}

int32_t sk_grouping_filter_is_true( const sk_grouping_expression *expression, sk_grouping_filter **out_filter, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const sk_grouping_expression *expression, sk_grouping_filter **out_filter, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_grouping_filter_is_true");
    return fn(expression, out_filter, out_error, out_error_capacity);
}

void sk_grouping_filter_destroy(sk_grouping_filter *filter) {
    typedef void (*fn_type)(sk_grouping_filter *filter);
    fn_type fn = (fn_type)sk_loader_symbol("sk_grouping_filter_destroy");
    fn(filter);
}

int32_t sk_grouping_level_set_filter( sk_grouping_level *level, const sk_grouping_filter *filter, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_grouping_level *level, const sk_grouping_filter *filter, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_grouping_level_set_filter");
    return fn(level, filter, out_error, out_error_capacity);
}

int32_t sk_grouping_level_add_order_by( sk_grouping_level *level, const sk_grouping_expression *expression, uint8_t ascending, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_grouping_level *level, const sk_grouping_expression *expression, uint8_t ascending, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_grouping_level_add_order_by");
    return fn(level, expression, ascending, out_error, out_error_capacity);
}

int32_t sk_grouping_level_add_count( sk_grouping_level *level, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_grouping_level *level, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_grouping_level_add_count");
    return fn(level, out_error, out_error_capacity);
}

int32_t sk_grouping_level_add_aggregation( sk_grouping_level *level, uint32_t kind, const char *field, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_grouping_level *level, uint32_t kind, const char *field, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_grouping_level_add_aggregation");
    return fn(level, kind, field, out_error, out_error_capacity);
}

int32_t sk_grouping_level_add_aggregation_handle( sk_grouping_level *level, const sk_grouping_aggregation *aggregation, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_grouping_level *level, const sk_grouping_aggregation *aggregation, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_grouping_level_add_aggregation_handle");
    return fn(level, aggregation, out_error, out_error_capacity);
}

int32_t sk_grouping_plan_add_root_aggregation_handle( sk_grouping_plan *plan, const sk_grouping_aggregation *aggregation, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_grouping_plan *plan, const sk_grouping_aggregation *aggregation, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_grouping_plan_add_root_aggregation_handle");
    return fn(plan, aggregation, out_error, out_error_capacity);
}

void sk_grouping_plan_destroy(sk_grouping_plan *plan) {
    typedef void (*fn_type)(sk_grouping_plan *plan);
    fn_type fn = (fn_type)sk_loader_symbol("sk_grouping_plan_destroy");
    fn(plan);
}

int32_t sk_engine_group_tree( const sk_engine *engine, const sk_query *query, const sk_grouping_plan *plan, sk_grouping_tree_result **out_result, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const sk_engine *engine, const sk_query *query, const sk_grouping_plan *plan, sk_grouping_tree_result **out_result, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_engine_group_tree");
    return fn(engine, query, plan, out_result, out_error, out_error_capacity);
}

int32_t sk_engine_group_tree_with_request( const sk_engine *engine, const sk_query *query, const sk_generic_search_request *request, const sk_grouping_plan *plan, sk_grouping_tree_result **out_result, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const sk_engine *engine, const sk_query *query, const sk_generic_search_request *request, const sk_grouping_plan *plan, sk_grouping_tree_result **out_result, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_engine_group_tree_with_request");
    return fn(engine, query, request, plan, out_result, out_error, out_error_capacity);
}

int32_t sk_grouping_tree_result_root( const sk_grouping_tree_result *result, const sk_grouping_tree_node **out_root, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const sk_grouping_tree_result *result, const sk_grouping_tree_node **out_root, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_grouping_tree_result_root");
    return fn(result, out_root, out_error, out_error_capacity);
}

uint32_t sk_grouping_tree_node_group_count( const sk_grouping_tree_node *node) {
    typedef uint32_t (*fn_type)( const sk_grouping_tree_node *node);
    fn_type fn = (fn_type)sk_loader_symbol("sk_grouping_tree_node_group_count");
    return fn(node);
}

int32_t sk_grouping_tree_node_group( const sk_grouping_tree_node *node, uint32_t group_index, const sk_grouping_tree_node **out_group, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const sk_grouping_tree_node *node, uint32_t group_index, const sk_grouping_tree_node **out_group, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_grouping_tree_node_group");
    return fn(node, group_index, out_group, out_error, out_error_capacity);
}

int32_t sk_grouping_tree_node_key( const sk_grouping_tree_node *node, const sk_grouping_result_value **out_value, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const sk_grouping_tree_node *node, const sk_grouping_result_value **out_value, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_grouping_tree_node_key");
    return fn(node, out_value, out_error, out_error_capacity);
}

uint32_t sk_grouping_result_value_kind( const sk_grouping_result_value *value) {
    typedef uint32_t (*fn_type)( const sk_grouping_result_value *value);
    fn_type fn = (fn_type)sk_loader_symbol("sk_grouping_result_value_kind");
    return fn(value);
}

int32_t sk_grouping_result_value_string( const sk_grouping_result_value *value, const char **out_string, uint32_t *out_length, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const sk_grouping_result_value *value, const char **out_string, uint32_t *out_length, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_grouping_result_value_string");
    return fn(value, out_string, out_length, out_error, out_error_capacity);
}

int32_t sk_grouping_result_value_integer( const sk_grouping_result_value *value, int64_t *out_integer, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const sk_grouping_result_value *value, int64_t *out_integer, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_grouping_result_value_integer");
    return fn(value, out_integer, out_error, out_error_capacity);
}

int32_t sk_grouping_result_value_float( const sk_grouping_result_value *value, double *out_float, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const sk_grouping_result_value *value, double *out_float, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_grouping_result_value_float");
    return fn(value, out_float, out_error, out_error_capacity);
}

int32_t sk_grouping_result_value_boolean( const sk_grouping_result_value *value, uint8_t *out_boolean, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const sk_grouping_result_value *value, uint8_t *out_boolean, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_grouping_result_value_boolean");
    return fn(value, out_boolean, out_error, out_error_capacity);
}

uint32_t sk_grouping_result_value_quantile_count( const sk_grouping_result_value *value) {
    typedef uint32_t (*fn_type)( const sk_grouping_result_value *value);
    fn_type fn = (fn_type)sk_loader_symbol("sk_grouping_result_value_quantile_count");
    return fn(value);
}

int32_t sk_grouping_result_value_quantile( const sk_grouping_result_value *value, uint32_t index, double *out_quantile, double *out_value, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const sk_grouping_result_value *value, uint32_t index, double *out_quantile, double *out_value, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_grouping_result_value_quantile");
    return fn(value, index, out_quantile, out_value, out_error, out_error_capacity);
}

uint32_t sk_grouping_tree_node_output_count( const sk_grouping_tree_node *node) {
    typedef uint32_t (*fn_type)( const sk_grouping_tree_node *node);
    fn_type fn = (fn_type)sk_loader_symbol("sk_grouping_tree_node_output_count");
    return fn(node);
}

int32_t sk_grouping_tree_node_output( const sk_grouping_tree_node *node, uint32_t output_index, const sk_grouping_tree_output **out_output, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const sk_grouping_tree_node *node, uint32_t output_index, const sk_grouping_tree_output **out_output, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_grouping_tree_node_output");
    return fn(node, output_index, out_output, out_error, out_error_capacity);
}

uint32_t sk_grouping_tree_output_kind( const sk_grouping_tree_output *output) {
    typedef uint32_t (*fn_type)( const sk_grouping_tree_output *output);
    fn_type fn = (fn_type)sk_loader_symbol("sk_grouping_tree_output_kind");
    return fn(output);
}

int32_t sk_grouping_tree_output_value( const sk_grouping_tree_output *output, const sk_grouping_result_value **out_value, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const sk_grouping_tree_output *output, const sk_grouping_result_value **out_value, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_grouping_tree_output_value");
    return fn(output, out_value, out_error, out_error_capacity);
}

uint32_t sk_grouping_tree_output_hit_count( const sk_grouping_tree_output *output) {
    typedef uint32_t (*fn_type)( const sk_grouping_tree_output *output);
    fn_type fn = (fn_type)sk_loader_symbol("sk_grouping_tree_output_hit_count");
    return fn(output);
}

int32_t sk_grouping_tree_output_hit_id( const sk_grouping_tree_output *output, uint32_t hit_index, const char **out_id, uint32_t *out_id_length, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const sk_grouping_tree_output *output, uint32_t hit_index, const char **out_id, uint32_t *out_id_length, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_grouping_tree_output_hit_id");
    return fn(output, hit_index, out_id, out_id_length, out_error, out_error_capacity);
}

int32_t sk_grouping_tree_output_hit_score( const sk_grouping_tree_output *output, uint32_t hit_index, double *out_score, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const sk_grouping_tree_output *output, uint32_t hit_index, double *out_score, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_grouping_tree_output_hit_score");
    return fn(output, hit_index, out_score, out_error, out_error_capacity);
}

uint32_t sk_grouping_tree_output_hit_summary_count( const sk_grouping_tree_output *output, uint32_t hit_index) {
    typedef uint32_t (*fn_type)( const sk_grouping_tree_output *output, uint32_t hit_index);
    fn_type fn = (fn_type)sk_loader_symbol("sk_grouping_tree_output_hit_summary_count");
    return fn(output, hit_index);
}

int32_t sk_grouping_tree_output_hit_summary_name( const sk_grouping_tree_output *output, uint32_t hit_index, uint32_t summary_index, const char **out_name, uint32_t *out_length, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const sk_grouping_tree_output *output, uint32_t hit_index, uint32_t summary_index, const char **out_name, uint32_t *out_length, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_grouping_tree_output_hit_summary_name");
    return fn(output, hit_index, summary_index, out_name, out_length, out_error, out_error_capacity);
}

int32_t sk_grouping_tree_output_hit_summary_value( const sk_grouping_tree_output *output, uint32_t hit_index, uint32_t summary_index, uint32_t *out_kind, const char **out_string, uint32_t *out_string_length, int64_t *out_integer, double *out_float, uint8_t *out_boolean, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const sk_grouping_tree_output *output, uint32_t hit_index, uint32_t summary_index, uint32_t *out_kind, const char **out_string, uint32_t *out_string_length, int64_t *out_integer, double *out_float, uint8_t *out_boolean, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_grouping_tree_output_hit_summary_value");
    return fn(output, hit_index, summary_index, out_kind, out_string, out_string_length, out_integer, out_float, out_boolean, out_error, out_error_capacity);
}

void sk_grouping_tree_result_destroy( sk_grouping_tree_result *result) {
    typedef void (*fn_type)( sk_grouping_tree_result *result);
    fn_type fn = (fn_type)sk_loader_symbol("sk_grouping_tree_result_destroy");
    fn(result);
}

int32_t sk_local_grouping_request_create( const char *group_field, sk_local_grouping_request **out_request, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const char *group_field, sk_local_grouping_request **out_request, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_local_grouping_request_create");
    return fn(group_field, out_request, out_error, out_error_capacity);
}

int32_t sk_local_grouping_request_add_aggregation( sk_local_grouping_request *request, uint32_t kind, const char *field, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_local_grouping_request *request, uint32_t kind, const char *field, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_local_grouping_request_add_aggregation");
    return fn(request, kind, field, out_error, out_error_capacity);
}

void sk_local_grouping_request_destroy(sk_local_grouping_request *request) {
    typedef void (*fn_type)(sk_local_grouping_request *request);
    fn_type fn = (fn_type)sk_loader_symbol("sk_local_grouping_request_destroy");
    fn(request);
}

int32_t sk_engine_group_local( const sk_engine *engine, const sk_query *query, const sk_local_grouping_request *request, sk_local_grouping_result **out_result, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const sk_engine *engine, const sk_query *query, const sk_local_grouping_request *request, sk_local_grouping_result **out_result, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_engine_group_local");
    return fn(engine, query, request, out_result, out_error, out_error_capacity);
}

uint32_t sk_local_grouping_result_group_count( const sk_local_grouping_result *result) {
    typedef uint32_t (*fn_type)( const sk_local_grouping_result *result);
    fn_type fn = (fn_type)sk_loader_symbol("sk_local_grouping_result_group_count");
    return fn(result);
}

int32_t sk_local_grouping_result_group( const sk_local_grouping_result *result, uint32_t group_index, uint32_t *out_key_kind, const char **out_key_string, uint32_t *out_key_string_length, int64_t *out_key_integer, uint64_t *out_count, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const sk_local_grouping_result *result, uint32_t group_index, uint32_t *out_key_kind, const char **out_key_string, uint32_t *out_key_string_length, int64_t *out_key_integer, uint64_t *out_count, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_local_grouping_result_group");
    return fn(result, group_index, out_key_kind, out_key_string, out_key_string_length, out_key_integer, out_count, out_error, out_error_capacity);
}

uint32_t sk_local_grouping_result_aggregation_count( const sk_local_grouping_result *result, uint32_t group_index) {
    typedef uint32_t (*fn_type)( const sk_local_grouping_result *result, uint32_t group_index);
    fn_type fn = (fn_type)sk_loader_symbol("sk_local_grouping_result_aggregation_count");
    return fn(result, group_index);
}

int32_t sk_local_grouping_result_aggregation( const sk_local_grouping_result *result, uint32_t group_index, uint32_t aggregation_index, uint32_t *out_kind, int64_t *out_integer, double *out_float, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const sk_local_grouping_result *result, uint32_t group_index, uint32_t aggregation_index, uint32_t *out_kind, int64_t *out_integer, double *out_float, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_local_grouping_result_aggregation");
    return fn(result, group_index, aggregation_index, out_kind, out_integer, out_float, out_error, out_error_capacity);
}

void sk_local_grouping_result_destroy(sk_local_grouping_result *result) {
    typedef void (*fn_type)(sk_local_grouping_result *result);
    fn_type fn = (fn_type)sk_loader_symbol("sk_local_grouping_result_destroy");
    fn(result);
}

int32_t sk_generic_grouping_request_create( const char *group_field, uint32_t max_groups, uint32_t order, sk_generic_grouping_request **out_request, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const char *group_field, uint32_t max_groups, uint32_t order, sk_generic_grouping_request **out_request, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_generic_grouping_request_create");
    return fn(group_field, max_groups, order, out_request, out_error, out_error_capacity);
}

int32_t sk_generic_grouping_request_add_aggregation( sk_generic_grouping_request *request, uint32_t kind, const char *field, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( sk_generic_grouping_request *request, uint32_t kind, const char *field, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_generic_grouping_request_add_aggregation");
    return fn(request, kind, field, out_error, out_error_capacity);
}

void sk_generic_grouping_request_destroy(sk_generic_grouping_request *request) {
    typedef void (*fn_type)(sk_generic_grouping_request *request);
    fn_type fn = (fn_type)sk_loader_symbol("sk_generic_grouping_request_destroy");
    fn(request);
}

int32_t sk_engine_group_generic( const sk_engine *engine, const sk_query *query, const sk_generic_grouping_request *request, sk_generic_grouping_result **out_result, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const sk_engine *engine, const sk_query *query, const sk_generic_grouping_request *request, sk_generic_grouping_result **out_result, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_engine_group_generic");
    return fn(engine, query, request, out_result, out_error, out_error_capacity);
}

uint32_t sk_generic_grouping_result_group_count( const sk_generic_grouping_result *result) {
    typedef uint32_t (*fn_type)( const sk_generic_grouping_result *result);
    fn_type fn = (fn_type)sk_loader_symbol("sk_generic_grouping_result_group_count");
    return fn(result);
}

int32_t sk_generic_grouping_result_group( const sk_generic_grouping_result *result, uint32_t group_index, uint32_t *out_key_kind, const char **out_key_string, uint32_t *out_key_string_length, int64_t *out_key_integer, uint64_t *out_count, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const sk_generic_grouping_result *result, uint32_t group_index, uint32_t *out_key_kind, const char **out_key_string, uint32_t *out_key_string_length, int64_t *out_key_integer, uint64_t *out_count, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_generic_grouping_result_group");
    return fn(result, group_index, out_key_kind, out_key_string, out_key_string_length, out_key_integer, out_count, out_error, out_error_capacity);
}

uint32_t sk_generic_grouping_result_aggregation_count( const sk_generic_grouping_result *result, uint32_t group_index) {
    typedef uint32_t (*fn_type)( const sk_generic_grouping_result *result, uint32_t group_index);
    fn_type fn = (fn_type)sk_loader_symbol("sk_generic_grouping_result_aggregation_count");
    return fn(result, group_index);
}

int32_t sk_generic_grouping_result_aggregation( const sk_generic_grouping_result *result, uint32_t group_index, uint32_t aggregation_index, uint32_t *out_kind, int64_t *out_integer, double *out_float, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( const sk_generic_grouping_result *result, uint32_t group_index, uint32_t aggregation_index, uint32_t *out_kind, int64_t *out_integer, double *out_float, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_generic_grouping_result_aggregation");
    return fn(result, group_index, aggregation_index, out_kind, out_integer, out_float, out_error, out_error_capacity);
}

void sk_generic_grouping_result_destroy(sk_generic_grouping_result *result) {
    typedef void (*fn_type)(sk_generic_grouping_result *result);
    fn_type fn = (fn_type)sk_loader_symbol("sk_generic_grouping_result_destroy");
    fn(result);
}

int32_t sk_test_failpoint_set(sk_engine *engine, const char *name, uint32_t hit, uint32_t action, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)(sk_engine *engine, const char *name, uint32_t hit, uint32_t action, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_test_failpoint_set");
    return fn(engine, name, hit, action, out_error, out_error_capacity);
}

int32_t sk_test_failpoint_set_open(const char *name, uint32_t hit, uint32_t action, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)(const char *name, uint32_t hit, uint32_t action, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_test_failpoint_set_open");
    return fn(name, hit, action, out_error, out_error_capacity);
}

int32_t sk_test_failpoint_clear(sk_engine *engine, const char *name, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)(sk_engine *engine, const char *name, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_test_failpoint_clear");
    return fn(engine, name, out_error, out_error_capacity);
}

int32_t sk_test_failpoint_clear_open(const char *name, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)(const char *name, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_test_failpoint_clear_open");
    return fn(name, out_error, out_error_capacity);
}

int32_t sk_test_failpoint_release(sk_engine *engine, const char *name, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)(sk_engine *engine, const char *name, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_test_failpoint_release");
    return fn(engine, name, out_error, out_error_capacity);
}

int32_t sk_test_failpoint_is_blocked(const sk_engine *engine, const char *name, uint8_t *out_blocked, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)(const sk_engine *engine, const char *name, uint8_t *out_blocked, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_test_failpoint_is_blocked");
    return fn(engine, name, out_blocked, out_error, out_error_capacity);
}

int32_t sk_test_result_fault_set(sk_engine *engine, uint32_t fault, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)(sk_engine *engine, uint32_t fault, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_test_result_fault_set");
    return fn(engine, fault, out_error, out_error_capacity);
}

int32_t sk_test_result_fault_clear(sk_engine *engine, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)(sk_engine *engine, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_test_result_fault_clear");
    return fn(engine, out_error, out_error_capacity);
}

int32_t sk_test_search_reply_metadata( uint32_t degradation_flags, const char *const *issue_messages, uint32_t issue_count, sk_generic_search_result **out_result, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)( uint32_t degradation_flags, const char *const *issue_messages, uint32_t issue_count, sk_generic_search_result **out_result, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_test_search_reply_metadata");
    return fn(degradation_flags, issue_messages, issue_count, out_result, out_error, out_error_capacity);
}

int32_t sk_test_result_destroy_count(const sk_engine *engine, uint64_t *out_count, uint8_t *out_error, uint32_t out_error_capacity) {
    typedef int32_t (*fn_type)(const sk_engine *engine, uint64_t *out_count, uint8_t *out_error, uint32_t out_error_capacity);
    fn_type fn = (fn_type)sk_loader_symbol("sk_test_result_destroy_count");
    return fn(engine, out_count, out_error, out_error_capacity);
}
