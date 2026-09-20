use std::{env, error::Error, path::PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let mut arguments = env::args_os().skip(1);
    let header = PathBuf::from(arguments.next().ok_or("missing native ABI header path")?);
    let output = PathBuf::from(arguments.next().ok_or("missing generated output path")?);
    if arguments.next().is_some() {
        return Err("usage: searchkernel-native-bindings-check HEADER OUTPUT".into());
    }

    bindgen::Builder::default()
        .header(header.to_string_lossy())
        .allowlist_type(
            "sk_(engine|document|document_update|search_request|search_result|text_document|text_update|text_request|text_result|query|nearest_neighbor_options|local_search_request|local_search_result|attribute_document|attribute_update|local_grouping_request|local_grouping_result|generic_search_request|generic_search_result|generic_grouping_request|generic_grouping_result|feature_value|struct_array|grouping_plan|grouping_level|grouping_expression|grouping_aggregation|grouping_filter|grouping_result_value|grouping_tree_output|grouping_tree_result|grouping_tree_node)",
        )
        .allowlist_function("sk_.*")
        .allowlist_var(
            "SK_(NATIVE_ABI_VERSION|ERROR_MESSAGE_CAPACITY|NATIVE_STATUS_.*|ARITHMETIC_(ADD|DIV|MUL|SUB)|FEATURE_VALUE_(KIND_INVALID|DOUBLE|DATA)|DEGRADATION_(MATCH_PHASE|TIMEOUT|ANN_TIMEOUT)|GROUPING_(BINARY_(ADD|SUBTRACT|MULTIPLY|DIVIDE|MODULO)|RESULT_VALUE_(STRING|INTEGER|FLOAT|BOOLEAN|QUANTILES|HIT_LIST)|TREE_OUTPUT_(VALUE|HIT_LIST))|TEXT_QUERY_(TERM|PHRASE)|QUERY_(NODE_(AND|OR|NOT|TRUE|NEAR|ONEAR|EQUIV|WEAK_AND|RANK)|TEXT_(TERM|PHRASE)|BOUND_(UNBOUNDED|INCLUDED|EXCLUDED)|FIELD_(TEXT|INTEGER|STRING|FLOAT|BOOLEAN|ATTRIBUTE|TENSOR|ARRAY(_STRING)?|POSITION|PREDICATE))|VALUE_(STRING|INTEGER|FLOAT|BOOLEAN|TENSOR|STRING_ARRAY|POSITION|PREDICATE|STRUCT_ARRAY)|SORT_(ASCENDING|DESCENDING|MISSING_DEFAULT|MISSING_FIRST|MISSING_LAST)|SUMMARY_(NONE|DEFAULT|FIELDS)|AGGREGATION_(SUM|MIN|MAX|AVERAGE|STDDEV|XOR|QUANTILES|SUMMARY)|GROUP_ORDER_(NONE|COUNT_ASC|COUNT_DESC|VALUE_ASC|VALUE_DESC))",
        )
        .layout_tests(true)
        .generate_comments(false)
        .generate()?
        .write_to_file(output)?;
    Ok(())
}
