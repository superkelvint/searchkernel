//! Real product-path acceptance coverage. It is ignored in lightweight
//! environments without the built Vespa SDK/native shared library.

use searchkernel::{Field, Index, IndexDefinition};
use serde_json::json;

#[test]
#[ignore = "requires the built Search Kernel native Vespa SDK"]
fn native_product_path_round_trips_annotated_mixed_document() {
    let path = std::env::temp_dir().join(format!(
        "searchkernel-native-e2e-{}-{}",
        std::process::id(),
        unique_suffix()
    ));
    let definition = IndexDefinition::new()
        .field(Field::text("title").stored(true))
        .field(Field::int64("price").stored(true).filterable(true));
    let index = Index::create(&path, definition).expect("create native index");
    index
        .upsert(
            "sku-42",
            &json!({"title": "Wireless Headphones", "price": 149_i64}),
        )
        .expect("put mixed document");
    let result = index
        .search("wireless headphones")
        .limit(20)
        .execute::<serde_json::Value>()
        .expect("search native index");
    assert_eq!(result.total_hits.value, 1);
    assert!(index.delete("sku-42").expect("delete native document"));
    std::fs::remove_dir_all(path).expect("remove native test index");
}

fn unique_suffix() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock after epoch")
        .as_nanos()
}
