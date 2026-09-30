#![cfg(feature = "native")]

use searchkernel::{
    ErrorCode, Field, GroupingAggregation, GroupingExpression, GroupingFilter, GroupingOrder,
    GroupingOutput, GroupingPlan, IndexDefinition, IndexRuntime, ResultNode, ResultValue,
};
use serde_json::json;

// SKC: grouping.structured/basic kinds=ergonomic runtime=native
// SKC: grouping.functions/basic kinds=ergonomic runtime=native
// SKC: grouping.aggregations/basic kinds=ergonomic runtime=native
#[test]
fn native_public_grouping_executes_nested_and_root_aggregations_after_reopen() {
    let path = std::env::temp_dir().join(format!(
        "searchkernel-native-grouping-{}-{}",
        std::process::id(),
        unique_suffix()
    ));
    let definition = IndexDefinition::new()
        .field(Field::keyword("category").stored(true).facetable(true))
        .field(Field::int64("qty").stored(true).facetable(true))
        .field(Field::float64("price").stored(true));
    let index = IndexRuntime::create(&path, definition)
        .expect("create native grouping index")
        .index();
    for (id, category, qty, price) in [
        ("audio-1", "Audio", 1_i64, 20.0),
        ("audio-2", "Audio", 2_i64, 5.0),
        ("book-1", "Book", 1_i64, 12.0),
    ] {
        index
            .upsert(
                id,
                &json!({"category": category, "qty": qty, "price": price}),
            )
            .expect("seed grouping document");
    }

    let result = run_grouping(&index, GroupFilterMode::Filter)
        .expect("execute public native grouping with filter");
    assert_grouping_result(&result);

    let error = run_filter_and_keep(&index)
        .expect_err("native grouping must not approximate simultaneous filter and keep");
    assert_eq!(error.code(), ErrorCode::Unsupported);
    assert!(error
        .details()
        .iter()
        .any(|detail| detail.code == "grouping_multiple_filters_not_supported"));

    index.close().expect("close native grouping index");
    let reopened = searchkernel::Index::open(&path).expect("reopen native grouping index");
    let reopened_result = run_grouping(&reopened, GroupFilterMode::Keep)
        .expect("execute grouping with keep after reopen");
    assert_grouping_result(&reopened_result);
    reopened.close().expect("close reopened grouping index");
    std::fs::remove_dir_all(path).expect("remove native grouping index");
}

#[derive(Clone, Copy)]
enum GroupFilterMode {
    Filter,
    Keep,
}

fn run_grouping(
    index: &searchkernel::Index,
    mode: GroupFilterMode,
) -> searchkernel::Result<searchkernel::SearchResult<serde_json::Value>> {
    let level = GroupingPlan::by_expression(GroupingExpression::function(
        "strlen",
        [GroupingExpression::attribute("category")],
    ))
    .alias("revenue", GroupingAggregation::sum("price"))
    .max_groups(10)
    .precision(20)
    .order_by([GroupingOrder::descending_alias("revenue")])
    .output(GroupingOutput::alias("revenue").label("revenue"))
    .output(GroupingOutput::aggregation(GroupingAggregation::average("price")).label("avg_price"))
    .output(GroupingOutput::aggregation(GroupingAggregation::stddev("price")).label("stddev_price"))
    .output(GroupingOutput::aggregation(GroupingAggregation::xor("qty")).label("xor_qty"))
    .output(
        GroupingOutput::aggregation(GroupingAggregation::quantiles("price", [0.5, 1.0]))
            .label("quantiles_price"),
    )
    .nested(
        GroupingPlan::by("qty")
            .output(GroupingOutput::aggregation(GroupingAggregation::count()).label("count")),
    );
    let predicate = GroupingFilter::regex("category", "^Audio$");
    let level = match mode {
        GroupFilterMode::Filter => level.filter(predicate),
        GroupFilterMode::Keep => level.keep(predicate),
    };

    index
        .browse()
        .grouping(
            GroupingPlan::new()
                .aggregate(GroupingOutput::aggregation(GroupingAggregation::count()).label("docs")),
        )
        .grouping(GroupingPlan::new().level(level))
        .execute::<serde_json::Value>()
}

fn run_filter_and_keep(
    index: &searchkernel::Index,
) -> searchkernel::Result<searchkernel::SearchResult<serde_json::Value>> {
    index
        .browse()
        .grouping(
            GroupingPlan::new().level(
                GroupingPlan::by("category")
                    .filter(GroupingFilter::regex("category", "^Audio$"))
                    .keep(GroupingFilter::regex("category", "^Audio$"))
                    .output(GroupingAggregation::count()),
            ),
        )
        .execute::<serde_json::Value>()
}

fn assert_grouping_result(result: &searchkernel::SearchResult<serde_json::Value>) {
    assert!(matches!(&result.root, ResultNode::Group(_)));
    assert_eq!(
        result.aggregations.get("docs"),
        Some(&ResultValue::Int64(3))
    );
    assert_eq!(result.groups.len(), 1);
    let grouping = &result.groups[0];
    let audio = grouping
        .groups
        .iter()
        .find(|group| group.key == Some(ResultValue::Int64(5)))
        .expect("strlen audio group");
    assert_eq!(
        audio.aggregations.get("revenue"),
        Some(&ResultValue::Float64(25.0))
    );
    assert_eq!(
        audio.aggregations.get("avg_price"),
        Some(&ResultValue::Float64(12.5))
    );
    assert!(matches!(
        audio.aggregations.get("stddev_price"),
        Some(ResultValue::Float64(value)) if value.is_finite() && *value > 0.0
    ));
    assert_eq!(
        audio.aggregations.get("xor_qty"),
        Some(&ResultValue::Int64(3))
    );
    let Some(ResultValue::List(quantiles)) = audio.aggregations.get("quantiles_price") else {
        panic!("quantiles aggregation must decode as a list");
    };
    assert_eq!(quantiles.len(), 2);
    assert!(quantiles.iter().all(|entry| matches!(
        entry,
        ResultValue::Object(values)
            if matches!(values.get("quantile"), Some(ResultValue::Float64(_)))
                && matches!(values.get("value"), Some(ResultValue::Float64(_)))
    )));
    assert_eq!(audio.groups.len(), 2);
    assert!(audio
        .groups
        .iter()
        .all(|group| group.aggregations.get("count") == Some(&ResultValue::Int64(1))));
}

fn unique_suffix() -> u128 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock after epoch")
        .as_nanos()
}
