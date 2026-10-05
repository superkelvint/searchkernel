#ifndef SEARCHKERNEL_NATIVE_ABI_CONTRACT_H
#define SEARCHKERNEL_NATIVE_ABI_CONTRACT_H

#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/* ABI v13 uses only opaque handles and fixed-width primitive arguments. */
typedef struct sk_engine sk_engine;

#define SK_NATIVE_ABI_VERSION 17U
#define SK_NATIVE_STATUS_TYPE_ERROR 6

/* Vespa FeatureSet::Value is DOUBLE or DATA. DATA is an opaque borrowed byte
 * view; tensor interpretation is a layer above this raw feature boundary. */
typedef struct sk_feature_value sk_feature_value;
#define SK_FEATURE_VALUE_KIND_INVALID 0U
#define SK_FEATURE_VALUE_DOUBLE 1U
#define SK_FEATURE_VALUE_DATA 2U

uint32_t sk_native_abi_version(void);

int32_t sk_engine_open(
    const char *data_dir,
    const char *config_dir,
    sk_engine **out_engine,
    uint8_t *out_error,
    uint32_t out_error_capacity);

void sk_engine_destroy(sk_engine *engine);

/* M2 operations. The opaque request/document constructors may remain private
 * to the Rust bindings crate, but these operation symbols are required so the
 * verifier can prove the final Rust executable references the real native
 * path. */
typedef struct sk_document sk_document;
typedef struct sk_document_update sk_document_update;
typedef struct sk_search_request sk_search_request;
typedef struct sk_search_result sk_search_result;

int32_t sk_engine_put(
    sk_engine *engine,
    const sk_document *document,
    uint8_t *out_error,
    uint32_t out_error_capacity);

int32_t sk_engine_update(
    sk_engine *engine,
    const sk_document_update *update,
    uint8_t *out_error,
    uint32_t out_error_capacity);

int32_t sk_engine_remove(
    sk_engine *engine,
    const char *document_id,
    int32_t *out_was_found,
    uint8_t *out_error,
    uint32_t out_error_capacity);

int32_t sk_engine_search(
    const sk_engine *engine,
    const sk_search_request *request,
    sk_search_result **out_result,
    uint8_t *out_error,
    uint32_t out_error_capacity);

typedef struct sk_generic_search_result sk_generic_search_result;

int32_t sk_generic_search_result_match_feature_value(
    const sk_generic_search_result *result,
    uint32_t hit_index,
    uint32_t feature_index,
    double *out_value,
    uint8_t *out_error,
    uint32_t out_error_capacity);

int32_t sk_generic_search_result_match_feature_value_ref(
    const sk_generic_search_result *result,
    uint32_t hit_index,
    uint32_t feature_index,
    const sk_feature_value **out_value,
    uint8_t *out_error,
    uint32_t out_error_capacity);

uint32_t sk_feature_value_kind(const sk_feature_value *value);

int32_t sk_feature_value_double(
    const sk_feature_value *value,
    double *out_value,
    uint8_t *out_error,
    uint32_t out_error_capacity);

int32_t sk_feature_value_data(
    const sk_feature_value *value,
    const uint8_t **out_data,
    size_t *out_size,
    uint8_t *out_error,
    uint32_t out_error_capacity);

/* ABI v13 structured grouping additions. The plan and result tree remain
 * opaque; requests copy plans and result tree nodes are borrowed from the
 * owning search result. */
typedef struct sk_generic_search_request sk_generic_search_request;
typedef struct sk_grouping_plan sk_grouping_plan;
typedef struct sk_grouping_tree_node sk_grouping_tree_node;

int32_t sk_generic_search_request_add_grouping_plan(
    sk_generic_search_request *request,
    const sk_grouping_plan *plan,
    uint8_t *out_error,
    uint32_t out_error_capacity);

uint32_t sk_generic_search_result_grouping_tree_count(
    const sk_generic_search_result *result);

int32_t sk_generic_search_result_grouping_tree_root(
    const sk_generic_search_result *result,
    uint32_t grouping_index,
    const sk_grouping_tree_node **out_root,
    uint8_t *out_error,
    uint32_t out_error_capacity);

#ifdef __cplusplus
}
#endif

#endif
