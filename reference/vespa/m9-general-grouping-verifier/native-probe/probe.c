#include "searchkernel_vespa.h"

#include <stddef.h>
#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define ERR_CAP SK_ERROR_MESSAGE_CAPACITY

static uint8_t error_buf[ERR_CAP];

static void die_status(const char *what, int32_t status) {
    fprintf(stderr, "%s failed: status=%" PRId32 " error=%s\n",
            what, status, (const char *)error_buf);
    exit(2);
}

static void ok(const char *what, int32_t status) {
    if (status != SK_NATIVE_STATUS_OK) die_status(what, status);
}

static void put_doc(sk_engine *engine, const char *id, const char *category,
                    int64_t qty, double price, uint8_t active) {
    sk_document *doc = NULL;
    memset(error_buf, 0, sizeof(error_buf));
    ok("sk_document_create_empty",
       sk_document_create_empty(id, &doc, error_buf, ERR_CAP));
    ok("sk_document_add_string",
       sk_document_add_string(doc, "category", category, error_buf, ERR_CAP));
    ok("sk_document_add_integer",
       sk_document_add_integer(doc, "qty", qty, error_buf, ERR_CAP));
    ok("sk_document_add_float",
       sk_document_add_float(doc, "price", price, error_buf, ERR_CAP));
    ok("sk_document_add_boolean",
       sk_document_add_boolean(doc, "active", active, error_buf, ERR_CAP));
    ok("sk_engine_put", sk_engine_put(engine, doc, error_buf, ERR_CAP));
    sk_document_destroy(doc);
}

static void json_string(const char *value, uint32_t length) {
    putchar('"');
    for (uint32_t i = 0; i < length; ++i) {
        const unsigned char c = (unsigned char)value[i];
        if (c == '"' || c == '\\') {
            putchar('\\');
            putchar((char)c);
        } else if (c == '\n') {
            fputs("\\n", stdout);
        } else if (c == '\r') {
            fputs("\\r", stdout);
        } else if (c == '\t') {
            fputs("\\t", stdout);
        } else if (c < 0x20) {
            printf("\\u%04x", c);
        } else {
            putchar((char)c);
        }
    }
    putchar('"');
}

static void emit_result_value_json(const sk_grouping_result_value *value) {
    const uint32_t kind = sk_grouping_result_value_kind(value);
    if (kind == SK_GROUPING_RESULT_VALUE_STRING) {
        const char *string = NULL;
        uint32_t length = 0;
        memset(error_buf, 0, sizeof(error_buf));
        ok("sk_grouping_result_value_string",
           sk_grouping_result_value_string(value, &string, &length,
                                           error_buf,
                                           ERR_CAP));
        printf("{\"type\":\"string\",\"value\":");
        json_string(string, length);
        putchar('}');
    } else if (kind == SK_GROUPING_RESULT_VALUE_INTEGER) {
        int64_t integer = 0;
        memset(error_buf, 0, sizeof(error_buf));
        ok("sk_grouping_result_value_integer",
           sk_grouping_result_value_integer(value, &integer,
                                            error_buf,
                                            ERR_CAP));
        printf("{\"type\":\"integer\",\"value\":%" PRId64 "}",
               integer);
    } else if (kind == SK_GROUPING_RESULT_VALUE_FLOAT) {
        double floating = 0.0;
        uint64_t bits = 0;
        memset(error_buf, 0, sizeof(error_buf));
        ok("sk_grouping_result_value_float",
           sk_grouping_result_value_float(value, &floating,
                                          error_buf,
                                          ERR_CAP));
        memcpy(&bits, &floating, sizeof(bits));
        printf("{\"type\":\"float\",\"bits\":\"0x%016" PRIx64
               "\"}",
               bits);
    } else {
        fprintf(stderr, "unsupported verifier result value kind: %u\n", kind);
        exit(2);
    }
}

static void emit_node(const sk_grouping_tree_node *node, int include_key) {
    printf("{");
    if (include_key) {
        const sk_grouping_result_value *value = NULL;
        memset(error_buf, 0, sizeof(error_buf));
        ok("sk_grouping_tree_node_key",
           sk_grouping_tree_node_key(node, &value, error_buf, ERR_CAP));
        printf("\"key\":");
        emit_result_value_json(value);
        putchar(',');
    }

    printf("\"outputs\":[");
    const uint32_t output_count = sk_grouping_tree_node_output_count(node);
    for (uint32_t i = 0; i < output_count; ++i) {
        if (i) putchar(',');
        const sk_grouping_tree_output *output = NULL;
        memset(error_buf, 0, sizeof(error_buf));
        ok("sk_grouping_tree_node_output",
           sk_grouping_tree_node_output(node, i, &output, error_buf, ERR_CAP));
        if (sk_grouping_tree_output_kind(output) !=
            SK_GROUPING_TREE_OUTPUT_VALUE) {
            fprintf(stderr, "unsupported verifier output classification\n");
            exit(2);
        }
        const sk_grouping_result_value *value = NULL;
        memset(error_buf, 0, sizeof(error_buf));
        ok("sk_grouping_tree_output_value",
           sk_grouping_tree_output_value(output, &value, error_buf, ERR_CAP));
        emit_result_value_json(value);
    }
    printf("],\"groups\":[");

    const uint32_t group_count = sk_grouping_tree_node_group_count(node);
    for (uint32_t i = 0; i < group_count; ++i) {
        if (i) putchar(',');
        const sk_grouping_tree_node *child = NULL;
        memset(error_buf, 0, sizeof(error_buf));
        ok("sk_grouping_tree_node_group",
           sk_grouping_tree_node_group(node, i, &child, error_buf, ERR_CAP));
        if (!child) {
            fprintf(stderr, "group child unexpectedly null\n");
            exit(2);
        }
        emit_node(child, 1);
    }
    printf("]}");
}

int main(int argc, char **argv) {
    if (argc != 3 && argc != 4) {
        fprintf(stderr, "usage: probe DATA_DIR CONFIG_DIR [typed|convenience]\n");
        return 2;
    }
    const int convenience = argc == 4 && strcmp(argv[3], "convenience") == 0;
    if (argc == 4 && !convenience && strcmp(argv[3], "typed") != 0) {
        fprintf(stderr, "unknown grouping construction mode: %s\n", argv[3]);
        return 2;
    }

    sk_engine *engine = NULL;
    sk_query *query = NULL;
    sk_grouping_plan *plan = NULL;
    sk_grouping_expression *category_classifier = NULL;
    sk_grouping_expression *qty_classifier = NULL;
    sk_grouping_expression *price_expression = NULL;
    sk_grouping_aggregation *category_count = NULL;
    sk_grouping_aggregation *category_average = NULL;
    sk_grouping_aggregation *qty_count = NULL;
    sk_grouping_aggregation *qty_average = NULL;
    sk_grouping_level *category = NULL;
    sk_grouping_level *qty = NULL;
    sk_grouping_tree_result *result = NULL;

    memset(error_buf, 0, sizeof(error_buf));
    ok("sk_engine_open",
       sk_engine_open(argv[1], argv[2], &engine, error_buf, ERR_CAP));

    put_doc(engine, "id:test:typea::p1", "A", 1, 10.0, 1);
    put_doc(engine, "id:test:typea::p2", "A", 1, 20.0, 1);
    put_doc(engine, "id:test:typea::p3", "A", 2, 30.0, 1);
    put_doc(engine, "id:test:typea::p4", "B", 1, 40.0, 1);

    ok("sk_query_create", sk_query_create(engine, &query, error_buf, ERR_CAP));
    ok("sk_query_add_attribute_float_range",
       sk_query_add_attribute_float_range(query, "price",
                                          SK_QUERY_BOUND_INCLUDED, 0.0,
                                          SK_QUERY_BOUND_INCLUDED, 100.0,
                                          error_buf, ERR_CAP));

    ok("sk_grouping_plan_create",
       sk_grouping_plan_create(&plan, error_buf, ERR_CAP));
    ok("sk_grouping_expression_attribute(category)",
       sk_grouping_expression_attribute("category", &category_classifier,
                                        error_buf, ERR_CAP));
    ok("sk_grouping_plan_add_level(category)",
       sk_grouping_plan_add_level(plan, category_classifier, &category,
                                  error_buf, ERR_CAP));
    sk_grouping_expression_destroy(category_classifier);
    category_classifier = NULL;
    ok("sk_grouping_aggregation_count(category)",
       sk_grouping_aggregation_count(&category_count, error_buf, ERR_CAP));
    if (convenience) {
        ok("sk_grouping_level_add_count(category)",
           sk_grouping_level_add_count(category, error_buf, ERR_CAP));
    } else {
        ok("sk_grouping_level_add_aggregation_handle(category/count)",
           sk_grouping_level_add_aggregation_handle(
               category, category_count, error_buf, ERR_CAP));
    }
    sk_grouping_aggregation_destroy(category_count);
    category_count = NULL;

    ok("sk_grouping_expression_attribute(price/category)",
       sk_grouping_expression_attribute("price", &price_expression,
                                        error_buf, ERR_CAP));
    ok("sk_grouping_aggregation_average(category)",
       sk_grouping_aggregation_average(price_expression, &category_average,
                                       error_buf, ERR_CAP));
    sk_grouping_expression_destroy(price_expression);
    price_expression = NULL;
    if (convenience) {
        ok("sk_grouping_level_add_aggregation(category/average)",
           sk_grouping_level_add_aggregation(
               category, SK_AGGREGATION_AVERAGE, "price", error_buf, ERR_CAP));
    } else {
        ok("sk_grouping_level_add_aggregation_handle(category/average)",
           sk_grouping_level_add_aggregation_handle(
               category, category_average, error_buf, ERR_CAP));
    }
    sk_grouping_aggregation_destroy(category_average);
    category_average = NULL;

    ok("sk_grouping_expression_attribute(qty)",
       sk_grouping_expression_attribute("qty", &qty_classifier,
                                        error_buf, ERR_CAP));
    ok("sk_grouping_plan_add_level(qty)",
       sk_grouping_plan_add_level(plan, qty_classifier, &qty,
                                  error_buf, ERR_CAP));
    sk_grouping_expression_destroy(qty_classifier);
    qty_classifier = NULL;
    ok("sk_grouping_aggregation_count(qty)",
       sk_grouping_aggregation_count(&qty_count, error_buf, ERR_CAP));
    if (convenience) {
        ok("sk_grouping_level_add_count(qty)",
           sk_grouping_level_add_count(qty, error_buf, ERR_CAP));
    } else {
        ok("sk_grouping_level_add_aggregation_handle(qty/count)",
           sk_grouping_level_add_aggregation_handle(
               qty, qty_count, error_buf, ERR_CAP));
    }
    sk_grouping_aggregation_destroy(qty_count);
    qty_count = NULL;

    ok("sk_grouping_expression_attribute(price/qty)",
       sk_grouping_expression_attribute("price", &price_expression,
                                        error_buf, ERR_CAP));
    ok("sk_grouping_aggregation_average(qty)",
       sk_grouping_aggregation_average(price_expression, &qty_average,
                                       error_buf, ERR_CAP));
    sk_grouping_expression_destroy(price_expression);
    price_expression = NULL;
    if (convenience) {
        ok("sk_grouping_level_add_aggregation(qty/average)",
           sk_grouping_level_add_aggregation(
               qty, SK_AGGREGATION_AVERAGE, "price", error_buf, ERR_CAP));
    } else {
        ok("sk_grouping_level_add_aggregation_handle(qty/average)",
           sk_grouping_level_add_aggregation_handle(
               qty, qty_average, error_buf, ERR_CAP));
    }
    sk_grouping_aggregation_destroy(qty_average);
    qty_average = NULL;

    ok("sk_engine_group_tree",
       sk_engine_group_tree(engine, query, plan, &result, error_buf, ERR_CAP));

    const sk_grouping_tree_node *root = NULL;
    ok("sk_grouping_tree_result_root",
       sk_grouping_tree_result_root(result, &root, error_buf, ERR_CAP));
    if (!root) {
        fprintf(stderr, "grouping root unexpectedly null\n");
        return 2;
    }

    printf("{\"id\":\"recursive-two-level\",\"groups\":[");
    const uint32_t groups = sk_grouping_tree_node_group_count(root);
    for (uint32_t i = 0; i < groups; ++i) {
        if (i) putchar(',');
        const sk_grouping_tree_node *group = NULL;
        ok("sk_grouping_tree_node_group(root)",
           sk_grouping_tree_node_group(root, i, &group, error_buf, ERR_CAP));
        emit_node(group, 1);
    }
    printf("]}\n");

    sk_grouping_tree_result_destroy(result);
    sk_grouping_plan_destroy(plan);
    sk_query_destroy(query);
    ok("sk_engine_close", sk_engine_close(engine, error_buf, ERR_CAP));
    sk_engine_destroy(engine);
    return 0;
}
