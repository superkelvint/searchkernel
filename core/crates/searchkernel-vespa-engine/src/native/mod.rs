use crate::document::{
    ArithmeticOperator, ArrayValue, FieldUpdate, FieldValue, PositionValue, PredicateValue,
    ScalarValue,
};
use crate::error::{check, cstring, Error, Result};
use crate::linguistics::{DocumentAnalysis, QueryAnalysis};
use crate::query::Value;
use crate::search::{
    FacetGroup, FacetResult, GroupingBinaryOperator, GroupingOutput, GroupingResult,
    GroupingResultValue, HighlightFragment, HighlightResult, HighlightSpan, Hit, MatchFeatureValue,
    SearchResult, TotalHitsRelation,
};
use crate::tensor::TensorValue;
use searchkernel_vespa_bindings as sys;
use std::collections::BTreeMap;
use std::ptr::{self, NonNull};

#[cfg(test)]
static DENSE_DOCUMENT_FFI_CALLS: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);
#[cfg(test)]
static DENSE_UPDATE_FFI_CALLS: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

mod buffer;
mod document;
mod grouping;
mod handles;
mod query;
mod result;
mod update;

pub(crate) use buffer::{
    length_u32, native_bytes, native_f64s, text_occurrences, text_query_terms, validate_term_query,
    NativeQueryTerms,
};
pub(crate) use document::{
    add_attribute_document_value, add_document_field, add_text_annotation,
    add_text_array_annotation, attribute_document_create,
};
pub(crate) use grouping::{
    engine_group_tree, engine_group_tree_with_request, generic_search_request_add_grouping_plan,
    grouping_aggregation_average, grouping_aggregation_count, grouping_aggregation_for_expression,
    grouping_aggregation_quantiles, grouping_aggregation_summary,
    grouping_expression_aggregation_reference, grouping_expression_attribute,
    grouping_expression_binary, grouping_expression_boolean, grouping_expression_float,
    grouping_expression_function, grouping_expression_integer, grouping_expression_negative,
    grouping_expression_string, grouping_filter_is_true, grouping_filter_multi,
    grouping_filter_not, grouping_filter_range, grouping_filter_regex,
    grouping_filter_regex_expression, grouping_level_add_aggregation, grouping_level_add_order_by,
    grouping_level_set_filter, grouping_level_set_limits, grouping_plan_add_level,
    grouping_plan_add_root_aggregation, grouping_plan_create, grouping_plan_set_all,
    NativeGroupingExpression, NativeGroupingFilter, NativeGroupingPlan,
};
pub(crate) use handles::{
    require_ptr, NativeAttributeDocument, NativeAttributeUpdate, NativeDocument, NativeError,
    NativeUpdate,
};
pub(crate) use query::{nearest_neighbor_options, NativeGenericSearchRequest, NativeQuery};
pub(crate) use result::read_generic_search_result;
pub(crate) use update::{add_attribute_update_value, add_update_field, attribute_update_create};

#[cfg(test)]
mod tensor_boundary_tests {
    use super::{add_document_field, add_update_field};
    use crate::{FieldUpdate, FieldValue, TensorValue};
    use std::sync::atomic::Ordering;

    #[test]
    fn malformed_dense_tensors_are_rejected_before_document_ffi() {
        super::DENSE_DOCUMENT_FFI_CALLS.store(0, Ordering::Relaxed);
        super::DENSE_UPDATE_FFI_CALLS.store(0, Ordering::Relaxed);
        for tensor in [
            TensorValue::dense("not-a-tensor", [1.0, 0.0]),
            TensorValue::dense("tensor<float>(x[2],y[1])", [1.0, 0.0]),
            TensorValue::dense("tensor<float>(x[3])", [1.0, 0.0]),
        ] {
            let expected = match &tensor {
                TensorValue::Dense { type_spec, .. } if type_spec == "not-a-tensor" => {
                    "tensor_type_spec_invalid"
                }
                TensorValue::Dense { type_spec, .. } if type_spec.contains(',') => {
                    "tensor_shape_unsupported"
                }
                _ => "tensor_cell_count_mismatch",
            };
            let value = FieldValue::Tensor(tensor.clone());
            let put = add_document_field(std::ptr::null_mut(), "embedding", &value)
                .expect_err("invalid dense tensor must fail before native document use");
            assert_eq!(put.detail_code(), Some(expected));

            let patch = add_update_field(
                std::ptr::null_mut(),
                "embedding",
                &FieldUpdate::Assign(FieldValue::Tensor(tensor)),
            )
            .expect_err("invalid dense tensor must fail before native update use");
            assert_eq!(patch.detail_code(), Some(expected));
        }
        assert_eq!(super::DENSE_DOCUMENT_FFI_CALLS.load(Ordering::Relaxed), 0);
        assert_eq!(super::DENSE_UPDATE_FFI_CALLS.load(Ordering::Relaxed), 0);
    }
}
