#![cfg(feature = "native")]

use searchkernel::{
    Field, GroupingAggregation, GroupingExpression, GroupingOrder, GroupingOutput, GroupingPlan,
    IndexDefinition, IndexRuntime, ResultNode, ResultValue,
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
        .field(Field::float64("price").stored(true).facetable(true));
    let index = IndexRuntime::create(&path, definition)
        .expect("create native grouping index")
        .index();
    for (id, category, qty, price) in [
        ("audio-1", "Audio", 1_i64, 20.0),
        ("audio-2", "Audio", 2_i64, 5.0),
        ("book-1", "Books", 1_i64, 12.0),
    ] {
        index
            .upsert(
                id,
                &json!({"category": category, "qty": qty, "price": price}),
            )
            .expect("seed grouping document");
    }

    let result = run_grouping(&index).expect("execute public native grouping");
    assert_grouping_result(&result);

    index.close().expect("close native grouping index");
    let reopened = searchkernel::Index::open(&path).expect("reopen native grouping index");
    let reopened_result = run_grouping(&reopened).expect("execute grouping after reopen");
    assert_grouping_result(&reopened_result);
    reopened.close().expect("close reopened grouping index");
    std::fs::remove_dir_all(path).expect("remove native grouping index");
}

fn run_grouping(
    index: &searchkernel::Index,
) -> searchkernel::Result<searchkernel::SearchResult<serde_json::Value>> {
    index
        .browse()
        .grouping(
            GroupingPlan::new()
                .aggregate(GroupingOutput::aggregation(GroupingAggregation::count()).label("docs")),
        )
        .grouping(
            GroupingPlan::new().level(
                GroupingPlan::by_expression(GroupingExpression::function(
                    "lowercase",
                    [GroupingExpression::attribute("category")],
                ))
                .alias("revenue", GroupingAggregation::sum("price"))
                .max_groups(10)
                .precision(20)
                .order_by([GroupingOrder::descending_alias("revenue")])
                .output(GroupingOutput::alias("revenue").label("revenue"))
                .output(
                    GroupingOutput::aggregation(GroupingAggregation::average("price"))
                        .label("avg_price"),
                )
                .nested(GroupingPlan::by("qty").output(
                    GroupingOutput::aggregation(GroupingAggregation::count()).label("count"),
                )),
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
        .find(|group| group.key == Some(ResultValue::String("audio".into())))
        .expect("lowercase audio group");
    assert_eq!(
        audio.aggregations.get("revenue"),
        Some(&ResultValue::Float64(25.0))
    );
    assert_eq!(
        audio.aggregations.get("avg_price"),
        Some(&ResultValue::Float64(12.5))
    );
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
