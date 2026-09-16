//! Real product-path acceptance coverage. It is compiled and run by the
//! all-features integration gate when the native SDK is available.

#![cfg(feature = "native")]

use searchkernel::{
    field, ErrorCode, Facet, FacetSort, Field, IndexDefinition, IndexRuntime, Ranking, SearchField,
    Sort,
};
use serde_json::json;

#[test]
fn native_product_path_round_trips_annotated_mixed_document() {
    let path = std::env::temp_dir().join(format!(
        "searchkernel-native-e2e-{}-{}",
        std::process::id(),
        unique_suffix()
    ));
    let definition = IndexDefinition::new()
        .field(Field::text("title").stored(true))
        .field(Field::int64("price").stored(true).filterable(true));
    let runtime = IndexRuntime::create(&path, definition).expect("create native index");
    let index = runtime.index();
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
    runtime.close().expect("close native index");
    std::fs::remove_dir_all(path).expect("remove native test index");
}

#[test]
fn native_product_path_has_current_search_and_facet_parity() {
    let path = std::env::temp_dir().join(format!(
        "searchkernel-native-parity-{}-{}",
        std::process::id(),
        unique_suffix()
    ));
    let definition = IndexDefinition::new()
        .field(Field::text("title").stored(true))
        .field(Field::text("description").stored(true))
        .field(
            Field::keyword("brand")
                .stored(true)
                .filterable(true)
                .sortable(true)
                .facetable(true),
        )
        .field(
            Field::int64("price")
                .stored(true)
                .filterable(true)
                .sortable(true)
                .facetable(true),
        );
    let runtime = IndexRuntime::create(&path, definition).expect("create native parity index");
    let index = runtime.index();

    for (id, title, description, brand, price) in [
        (
            "d1",
            "Wireless Headphones",
            "portable audio",
            "Acme",
            Some(100_i64),
        ),
        (
            "d2",
            "Wireless Speaker",
            "portable audio",
            "Bravo",
            Some(50_i64),
        ),
        (
            "d3",
            "Wireless Mouse",
            "office wireless",
            "Bravo",
            Some(75_i64),
        ),
        (
            "d4",
            "Wireless Keyboard",
            "office wireless",
            "Charlie",
            Some(25_i64),
        ),
        (
            "d5",
            "Wireless Camera",
            "office wireless",
            "Charlie",
            Some(125_i64),
        ),
        ("d6", "Wireless Cable", "office wireless", "Charlie", None),
    ] {
        let mut document = json!({
            "title": title,
            "description": description,
            "brand": brand,
        });
        if let Some(price) = price {
            document["price"] = json!(price);
        }
        index
            .upsert(id, &document)
            .expect("upsert native parity document");
    }

    // An explicit field selection and an English regional language hint both
    // travel through the real analyzer/query path.
    let selected = index
        .search("wireless")
        .search_fields([SearchField::new("description")])
        .language("en-US")
        .execute::<serde_json::Value>()
        .expect("explicit search field and English language");
    assert_eq!(selected.total_hits.value, 4);
    assert_eq!(selected.hits.len(), 4);

    let empty_projection = index
        .search("wireless")
        .projection(std::iter::empty::<&str>())
        .limit(1)
        .execute::<serde_json::Value>()
        .expect("native empty projection");
    assert_eq!(empty_projection.total_hits.value, 6);
    assert_eq!(empty_projection.hits.len(), 1);
    assert_eq!(empty_projection.hits[0].document, json!({}));

    // Facets are calculated by grouping the complete filtered match set, not
    // by counting the paginated hit list. Multiple facet requests share the
    // same high-level search request.
    let faceted = index
        .search("wireless")
        .limit(1)
        .facets([
            Facet::new("brand").limit(3).sort(FacetSort::CountDesc),
            Facet::new("price").limit(2).sort(FacetSort::ValueAsc),
        ])
        .execute::<serde_json::Value>()
        .expect("multiple native facets");
    assert_eq!(faceted.total_hits.value, 6);
    assert_eq!(faceted.hits.len(), 1);
    assert_eq!(
        facet_pairs(&faceted, "brand"),
        vec![
            ("Charlie".into(), 3),
            ("Bravo".into(), 2),
            ("Acme".into(), 1)
        ]
    );
    assert_eq!(
        facet_pairs(&faceted, "price"),
        vec![("25".into(), 1), ("75".into(), 1)]
    );

    let filtered = index
        .search("wireless")
        .filter(field("brand").eq("Acme"))
        .facet(Facet::new("brand").limit(3).sort(FacetSort::ValueAsc))
        .execute::<serde_json::Value>()
        .expect("lexical query with facet filter");
    assert_eq!(filtered.total_hits.value, 1);
    assert_eq!(facet_pairs(&filtered, "brand"), vec![("Acme".into(), 1)]);

    for (sort, expected) in [
        (
            FacetSort::CountDesc,
            vec![
                ("Charlie".into(), 3),
                ("Bravo".into(), 2),
                ("Acme".into(), 1),
            ],
        ),
        (
            FacetSort::CountAsc,
            vec![
                ("Acme".into(), 1),
                ("Bravo".into(), 2),
                ("Charlie".into(), 3),
            ],
        ),
        (
            FacetSort::ValueAsc,
            vec![
                ("Acme".into(), 1),
                ("Bravo".into(), 2),
                ("Charlie".into(), 3),
            ],
        ),
        (
            FacetSort::ValueDesc,
            vec![
                ("Charlie".into(), 3),
                ("Bravo".into(), 2),
                ("Acme".into(), 1),
            ],
        ),
    ] {
        let result = index
            .search("wireless")
            .facet(Facet::new("brand").limit(3).sort(sort))
            .execute::<serde_json::Value>()
            .expect("facet ordering");
        assert_eq!(facet_pairs(&result, "brand"), expected);
    }

    // Projection, score, total hits, offset/limit, and missing-first sorting
    // are all observable in the same native result contract.
    let paged = index
        .browse()
        .sort(Sort::asc("price").missing_first())
        .offset(1)
        .limit(2)
        .projection(["title", "brand"])
        .execute::<serde_json::Value>()
        .expect("native projection and pagination");
    assert_eq!(paged.total_hits.value, 6);
    assert_eq!(paged.hits.len(), 2);
    assert_eq!(paged.hits[0].id.as_str(), "d4");
    assert_eq!(paged.hits[1].id.as_str(), "d2");
    assert!(paged.hits.iter().all(|hit| hit.score.is_some()));
    assert_eq!(
        paged.hits[0].document,
        json!({"title": "Wireless Keyboard", "brand": "Charlie"})
    );

    // These capabilities remain visible and fail at the adapter with their
    // stable details instead of being ignored.
    assert_search_detail(
        index
            .search("wireless")
            .search_fields([SearchField::new("title").weight(2.0)])
            .execute::<serde_json::Value>(),
        "ranking_weights_not_supported",
    );
    assert_search_detail(
        index
            .search("wireless")
            .ranking(Ranking::bm25())
            .execute::<serde_json::Value>(),
        "bm25_ranking_not_supported",
    );
    assert_search_detail(
        index
            .search("wireless")
            .highlight(["title"])
            .execute::<serde_json::Value>(),
        "highlighting_native_result_unavailable",
    );
    assert_search_detail(
        index
            .search("wireless")
            .language("fr")
            .execute::<serde_json::Value>(),
        "language_not_supported",
    );

    runtime.close().expect("close native parity index");
    let reopened_runtime = IndexRuntime::open(&path).expect("reopen native parity index");
    let reopened = reopened_runtime.index();
    let after_reopen = reopened
        .search("wireless")
        .limit(1)
        .facet(Facet::new("brand").limit(1).sort(FacetSort::CountDesc))
        .execute::<serde_json::Value>()
        .expect("search and facet after reopen");
    assert_eq!(after_reopen.total_hits.value, 6);
    assert_eq!(
        facet_pairs(&after_reopen, "brand"),
        vec![("Charlie".into(), 3)]
    );
    reopened_runtime
        .close()
        .expect("close reopened native parity index");
    std::fs::remove_dir_all(path).expect("remove native parity index");
}

fn facet_pairs(
    result: &searchkernel::SearchResult<serde_json::Value>,
    field: &str,
) -> Vec<(String, u64)> {
    result
        .facets
        .iter()
        .find(|facet| facet.field == field)
        .expect("facet field")
        .values
        .iter()
        .map(|value| {
            (
                match &value.value {
                    serde_json::Value::String(value) => value.clone(),
                    value => value.to_string(),
                },
                value.count,
            )
        })
        .collect()
}

fn assert_search_detail<T: std::fmt::Debug>(result: searchkernel::Result<T>, expected: &str) {
    let error = result.expect_err("unsupported search option must fail");
    assert_eq!(error.code(), ErrorCode::Unsupported);
    assert_eq!(error.details()[0].code, expected);
}

fn unique_suffix() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock after epoch")
        .as_nanos()
}
