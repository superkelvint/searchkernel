#include "searchkernel_vespa.h"

#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

static uint8_t error_buffer[SK_ERROR_MESSAGE_CAPACITY];

static void require_ok(const char *name, int32_t status) {
    if (status != SK_NATIVE_STATUS_OK) {
        fprintf(stderr, "%s failed: %s\n", name, error_buffer);
        exit(2);
    }
}

static void require_fail(const char *name, int32_t status) {
    if (status == SK_NATIVE_STATUS_OK) {
        fprintf(stderr, "%s unexpectedly succeeded\n", name);
        exit(2);
    }
}

int main(void) {
    sk_nearest_neighbor_options *options = NULL;
    require_ok("nearest options create",
               sk_nearest_neighbor_options_create(&options, error_buffer,
                                                  sizeof(error_buffer)));
    require_ok("nearest distance threshold",
               sk_nearest_neighbor_options_set_distance_threshold(
                   options, 0.25, error_buffer, sizeof(error_buffer)));
    require_ok("nearest explore additional hits zero",
               sk_nearest_neighbor_options_set_explore_additional_hits(
                   options, 0, error_buffer, sizeof(error_buffer)));
    require_ok("nearest explore additional hits",
               sk_nearest_neighbor_options_set_explore_additional_hits(
                   options, 17, error_buffer, sizeof(error_buffer)));
    require_fail("nearest NaN threshold",
                 sk_nearest_neighbor_options_set_distance_threshold(
                     options, NAN, error_buffer, sizeof(error_buffer)));
    sk_nearest_neighbor_options_destroy(options);

    sk_document_update *update = NULL;
    require_ok("empty update",
               sk_document_update_create_empty("id:test:typea::phase9",
                                               &update, error_buffer,
                                               sizeof(error_buffer)));
    require_ok("arithmetic add",
               sk_document_update_arithmetic(update, "qty", SK_ARITHMETIC_ADD,
                                             2.0, error_buffer,
                                             sizeof(error_buffer)));
    require_ok("arithmetic subtract",
               sk_document_update_arithmetic(update, "qty", SK_ARITHMETIC_SUB,
                                             1.0, error_buffer,
                                             sizeof(error_buffer)));
    require_ok("arithmetic multiply",
               sk_document_update_arithmetic(update, "qty", SK_ARITHMETIC_MUL,
                                             3.0, error_buffer,
                                             sizeof(error_buffer)));
    require_ok("arithmetic divide",
               sk_document_update_arithmetic(update, "qty", SK_ARITHMETIC_DIV,
                                             2.0, error_buffer,
                                             sizeof(error_buffer)));
    sk_document_update_destroy(update);

    sk_generic_search_request *request = NULL;
    require_ok("generic request",
               sk_generic_search_request_create("default", 10, 0,
                                                SK_SUMMARY_NONE, &request,
                                                error_buffer,
                                                sizeof(error_buffer)));
    require_ok("raw sort",
               sk_generic_search_request_set_sort_spec(
                   request, "-relevance", error_buffer, sizeof(error_buffer)));
    require_fail("raw/structured sort collision",
                 sk_generic_search_request_add_sort(
                     request, "qty", SK_SORT_ASCENDING, SK_SORT_MISSING_DEFAULT,
                     error_buffer, sizeof(error_buffer)));
    sk_generic_search_request_destroy(request);

    sk_grouping_plan *plan = NULL;
    sk_grouping_expression *category = NULL;
    sk_grouping_expression *value = NULL;
    sk_grouping_level *level = NULL;
    sk_grouping_filter *filter = NULL;
    require_ok("grouping plan", sk_grouping_plan_create(&plan, error_buffer,
                                                         sizeof(error_buffer)));
    require_ok("category expression",
               sk_grouping_expression_attribute("category", &category,
                                                error_buffer,
                                                sizeof(error_buffer)));
    require_ok("grouping level",
               sk_grouping_plan_add_level(plan, category, &level, error_buffer,
                                          sizeof(error_buffer)));
    require_ok("max groups",
               sk_grouping_level_set_max_groups(level, 2, error_buffer,
                                                sizeof(error_buffer)));
    require_ok("precision",
               sk_grouping_level_set_precision(level, 100, error_buffer,
                                               sizeof(error_buffer)));
    require_ok("regex filter",
               sk_grouping_filter_regex("^A$", "category", &filter,
                                        error_buffer, sizeof(error_buffer)));
    require_ok("install filter",
               sk_grouping_level_set_filter(level, filter, error_buffer,
                                            sizeof(error_buffer)));
    require_ok("order expression",
               sk_grouping_level_add_order_by(level, category, 0, error_buffer,
                                              sizeof(error_buffer)));
    sk_grouping_filter_destroy(filter);

    require_ok("value expression",
               sk_grouping_expression_attribute("qty", &value, error_buffer,
                                                sizeof(error_buffer)));
    const double quantiles[] = {0.5, 0.9};
    sk_grouping_aggregation *aggregation = NULL;
    require_ok("sum", sk_grouping_aggregation_sum(value, &aggregation,
                                                   error_buffer,
                                                   sizeof(error_buffer)));
    require_ok("attach sum", sk_grouping_level_add_aggregation_handle(
                                 level, aggregation, error_buffer,
                                 sizeof(error_buffer)));
    sk_grouping_aggregation_destroy(aggregation);
    aggregation = NULL;
    require_ok("min", sk_grouping_aggregation_min(value, &aggregation,
                                                   error_buffer,
                                                   sizeof(error_buffer)));
    require_ok("attach min", sk_grouping_level_add_aggregation_handle(
                                 level, aggregation, error_buffer,
                                 sizeof(error_buffer)));
    sk_grouping_aggregation_destroy(aggregation);
    aggregation = NULL;
    require_ok("max", sk_grouping_aggregation_max(value, &aggregation,
                                                   error_buffer,
                                                   sizeof(error_buffer)));
    require_ok("attach max", sk_grouping_level_add_aggregation_handle(
                                 level, aggregation, error_buffer,
                                 sizeof(error_buffer)));
    sk_grouping_aggregation_destroy(aggregation);
    aggregation = NULL;
    require_ok("standard deviation",
               sk_grouping_aggregation_stddev(value, &aggregation, error_buffer,
                                              sizeof(error_buffer)));
    require_ok("attach standard deviation",
               sk_grouping_level_add_aggregation_handle(
                   level, aggregation, error_buffer, sizeof(error_buffer)));
    sk_grouping_aggregation_destroy(aggregation);
    aggregation = NULL;
    require_ok("xor", sk_grouping_aggregation_xor(value, &aggregation,
                                                   error_buffer,
                                                   sizeof(error_buffer)));
    require_ok("attach xor", sk_grouping_level_add_aggregation_handle(
                                 level, aggregation, error_buffer,
                                 sizeof(error_buffer)));
    sk_grouping_aggregation_destroy(aggregation);
    aggregation = NULL;
    require_ok("quantiles", sk_grouping_aggregation_quantiles(
                                 value, quantiles, 2, &aggregation, error_buffer,
                                 sizeof(error_buffer)));
    require_ok("attach quantiles", sk_grouping_level_add_aggregation_handle(
                                      level, aggregation, error_buffer,
                                      sizeof(error_buffer)));
    sk_grouping_aggregation_destroy(aggregation);
    aggregation = NULL;
    require_ok("summary", sk_grouping_aggregation_summary(
                                 "default", 3, &aggregation, error_buffer,
                                 sizeof(error_buffer)));
    require_ok("attach summary", sk_grouping_level_add_aggregation_handle(
                                      level, aggregation, error_buffer,
                                      sizeof(error_buffer)));
    sk_grouping_aggregation_destroy(aggregation);
    sk_grouping_expression_destroy(value);
    sk_grouping_expression_destroy(category);
    sk_grouping_plan_destroy(plan);

    puts("{\"id\":\"additive-handles\",\"status\":\"ok\",\"abi\":10}");
    return 0;
}
