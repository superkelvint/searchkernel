//! Real product-path acceptance coverage. It is compiled and run by the
//! all-features integration gate when the native SDK is available.

#![cfg(feature = "native")]

use searchkernel::{
    field, ErrorCode, Field, IndexDefinition, IndexRuntime, NearestNeighborOptions, Query,
    RankProfile, Ranking, SearchField, Sort, Tensor, WeightedQueryItem,
};
use serde_json::json;

#[test]
fn native_long_annotated_teaser_remains_valid() {
    let path = std::env::temp_dir().join(format!(
        "searchkernel-long-teaser-{}-{}",
        std::process::id(),
        unique_suffix()
    ));
    let runtime = IndexRuntime::create(
        &path,
        IndexDefinition::new()
            .field(Field::text("title").stored(true))
            .field(Field::text("description").stored(true)),
    )
    .expect("create native index");
    let index = runtime.index();
    index.upsert("speaker", &json!({
        "title":"Wired Speaker 19",
        "description":format!("{} wired speaker {}", "An introductory description. ".repeat(40), "Long trailing text. ".repeat(40))
    })).expect("index annotated description");
    let result = index
        .search("speaker")
        .highlight(["title", "description"])
        .execute::<serde_json::Value>();
    runtime.close().expect("close index");
    std::fs::remove_dir_all(path).expect("remove fixture");
    let result = result.expect("backend-selected annotated teaser must decode");
    assert_eq!(result.total_hits.value, 1);
    for field in ["title", "description"] {
        let highlight = result.hits[0]
            .highlights
            .iter()
            .find(|highlight| highlight.field == field)
            .expect("requested highlight");
        assert!(highlight
            .fragments
            .iter()
            .any(|fragment| fragment.matches.iter().any(|span| fragment.text
                [span.start_byte as usize..span.end_byte as usize]
                .eq_ignore_ascii_case("speaker"))));
        assert!(highlight
            .fragments
            .iter()
            .all(|fragment| !fragment.text.contains(['\u{fff9}', '\u{fffa}', '\u{fffb}'])));
    }
}
use std::collections::BTreeSet;
use std::sync::{Arc, Barrier};

// SKC: update.document_crud/basic kinds=ergonomic runtime=native
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
fn native_product_path_supports_int64_values_outside_legacy_i32_range() {
    let path = std::env::temp_dir().join(format!(
        "searchkernel-native-int64-{}-{}",
        std::process::id(),
        unique_suffix()
    ));
    let definition = IndexDefinition::new().field(
        Field::int64("value")
            .stored(true)
            .filterable(true)
            .sortable(true),
    );
    let index = IndexRuntime::create(&path, definition)
        .expect("create native int64 index")
        .index();
    let values = [
        i32::MIN as i64 - 1,
        i32::MIN as i64,
        i32::MAX as i64,
        i32::MAX as i64 + 1,
        i64::MAX,
    ];
    for (slot, value) in values.into_iter().enumerate() {
        index
            .upsert(format!("v-{slot}"), &json!({"value": value}))
            .unwrap_or_else(|error| panic!("write full int64 value {value}: {error:?}"));
        let document = index
            .get::<serde_json::Value>(format!("v-{slot}"))
            .expect("get full int64 value")
            .expect("int64 document exists");
        assert_eq!(document["value"], json!(value));
    }
    let minimum_error = index
        .upsert("v-minimum", &json!({"value": i64::MIN}))
        .expect_err("native Vespa LONG minimum sentinel must be rejected");
    assert!(format!("{minimum_error:?}").contains("INT64_MIN"));
    let minimum_query_error = index
        .browse()
        .filter(field("value").eq(i64::MIN))
        .execute::<serde_json::Value>()
        .expect_err("native query must reject the LONG minimum sentinel");
    assert!(format!("{minimum_query_error:?}").contains("INT64_MIN"));
    let filtered = index
        .browse()
        .filter(field("value").eq(i64::MAX))
        .execute::<serde_json::Value>()
        .expect("filter full int64 value");
    assert_eq!(filtered.total_hits.value, 1);
    assert_eq!(filtered.hits[0].id.as_str(), "v-4");

    index
        .patch("v-0", searchkernel::Patch::new().set("value", i64::MAX))
        .expect("patch full int64 value");
    index.close().expect("close native int64 index");
    let reopened = searchkernel::Index::open(&path).expect("reopen native int64 index");
    let patched = reopened
        .get::<serde_json::Value>("v-0")
        .expect("get patched int64 value")
        .expect("patched document exists");
    assert_eq!(patched["value"], json!(i64::MAX));
    reopened.close().expect("close reopened native int64 index");
    std::fs::remove_dir_all(path).expect("remove native int64 index");
}

#[test]
fn native_product_path_preserves_vespa_boolean_clear_semantics() {
    let path = std::env::temp_dir().join(format!(
        "searchkernel-native-bool-{}-{}",
        std::process::id(),
        unique_suffix()
    ));
    let definition = IndexDefinition::new()
        .field(Field::text("title").stored(true))
        .field(
            Field::bool("active")
                .stored(true)
                .filterable(true)
                .sortable(false)
                .facetable(false),
        );
    let index = IndexRuntime::create(&path, definition)
        .expect("create native bool index")
        .index();
    index
        .upsert("absent", &json!({"title": "absent"}))
        .expect("put absent bool document");
    index
        .upsert("false", &json!({"title": "false", "active": false}))
        .expect("put false bool document");
    index
        .upsert("true", &json!({"title": "true", "active": true}))
        .expect("put true bool document");

    let false_hits = index
        .browse()
        .filter(field("active").eq(false))
        .execute::<serde_json::Value>()
        .expect("search false bool values");
    assert_eq!(false_hits.total_hits.value, 2);
    assert_eq!(
        index
            .get::<serde_json::Value>("absent")
            .expect("get absent bool document")
            .expect("absent bool document exists")["active"],
        json!(false)
    );

    index
        .patch("true", searchkernel::Patch::new().unset("active"))
        .expect("clear bool attribute");
    assert_eq!(
        index
            .get::<serde_json::Value>("true")
            .expect("get cleared bool document")
            .expect("cleared bool document exists")["active"],
        json!(false)
    );
    index.close().expect("close native bool index");
    let reopened = searchkernel::Index::open(&path).expect("reopen native bool index");
    let reopened_false_hits = reopened
        .browse()
        .filter(field("active").eq(false))
        .execute::<serde_json::Value>()
        .expect("search cleared bool values after reopen");
    assert_eq!(reopened_false_hits.total_hits.value, 3);
    reopened.close().expect("close reopened native bool index");
    std::fs::remove_dir_all(path).expect("remove native bool index");
}

// SKC: update.arithmetic/basic kinds=ergonomic,e2e runtime=native
// SKC: update.create_if_nonexistent/basic kinds=ergonomic,e2e runtime=native
#[test]
fn native_update_builder_executes_arithmetic_create_and_persists() {
    let path = std::env::temp_dir().join(format!(
        "searchkernel-native-update-builder-{}-{}",
        std::process::id(),
        unique_suffix()
    ));
    let definition = IndexDefinition::new()
        .field(Field::text("title").stored(true))
        .field(Field::keyword("brand").stored(true))
        .field(Field::int64("inventory").stored(true))
        .field(Field::float64("ratio").stored(true));
    let index = IndexRuntime::create(&path, definition).expect("create native update index");

    index
        .upsert(
            "sku-42",
            &json!({
                "title": "Original",
                "brand": "Acme",
                "inventory": 10_i64,
                "ratio": 8.0,
            }),
        )
        .expect("seed update document");
    index
        .update_builder("sku-42")
        .assign("title", "Updated")
        .clear("brand")
        .increment("inventory", 5_i64)
        .multiply("ratio", 2.0)
        .execute()
        .expect("execute mixed advanced update");
    index
        .update_builder("sku-42")
        .decrement("inventory", 3_i64)
        .execute()
        .expect("decrement inventory");
    index
        .update_builder("sku-42")
        .divide("ratio", 4.0)
        .execute()
        .expect("divide ratio");

    index
        .update_builder("created-by-update")
        .set("title", "Created")
        .set("inventory", 7_i64)
        .set("ratio", 1.5)
        .create(true)
        .execute()
        .expect("create missing document through update");

    let updated = index
        .get::<serde_json::Value>("sku-42")
        .expect("read updated document")
        .expect("updated document exists");
    assert_eq!(updated["title"], json!("Updated"));
    assert_eq!(updated["brand"], serde_json::Value::Null);
    assert_eq!(updated["inventory"], json!(12_i64));
    assert_eq!(updated["ratio"], json!(4.0));

    let created = index
        .get::<serde_json::Value>("created-by-update")
        .expect("read create-if-missing document")
        .expect("create-if-missing document exists");
    assert_eq!(created["title"], json!("Created"));
    assert_eq!(created["inventory"], json!(7_i64));
    assert_eq!(created["ratio"], json!(1.5));

    index.close().expect("close native update index");
    let reopened = searchkernel::Index::open(&path).expect("reopen native update index");
    let persisted = reopened
        .get::<serde_json::Value>("sku-42")
        .expect("read persisted updated document")
        .expect("persisted updated document exists");
    assert_eq!(persisted["title"], json!("Updated"));
    assert_eq!(persisted["brand"], serde_json::Value::Null);
    assert_eq!(persisted["inventory"], json!(12_i64));
    assert_eq!(persisted["ratio"], json!(4.0));
    let persisted_created = reopened
        .get::<serde_json::Value>("created-by-update")
        .expect("read persisted create-if-missing document")
        .expect("persisted create-if-missing document exists");
    assert_eq!(persisted_created["inventory"], json!(7_i64));
    reopened.close().expect("close reopened update index");
    std::fs::remove_dir_all(path).expect("remove native update index");
}

#[test]
fn native_create_and_open_race_never_removes_a_published_index() {
    let path = std::env::temp_dir().join(format!(
        "searchkernel-native-create-open-race-{}-{}",
        std::process::id(),
        unique_suffix()
    ));
    let _ = std::fs::remove_dir_all(&path);
    let definition = IndexDefinition::new().field(Field::text("title").stored(true));
    let start = Arc::new(Barrier::new(2));

    let create_path = path.clone();
    let create_start = Arc::clone(&start);
    let creator = std::thread::spawn(move || {
        create_start.wait();
        searchkernel::Index::create(create_path, definition)
    });
    let open_path = path.clone();
    let open_start = Arc::clone(&start);
    let opener = std::thread::spawn(move || {
        open_start.wait();
        searchkernel::Index::open(open_path)
    });

    let created = creator.join().expect("creator thread");
    let opened = opener.join().expect("opener thread");
    match (created, opened) {
        (Ok(created), Err(_)) => created.close().expect("close created index"),
        (Err(_), Ok(opened)) => opened.close().expect("close opened index"),
        (Ok(created), Ok(opened)) => {
            created.close().expect("close created index");
            opened.close().expect("close opened index");
        }
        (Err(create_error), Err(open_error)) => panic!(
            "create/open race did not publish an index: create={create_error:?}, open={open_error:?}"
        ),
    }

    let reopened = searchkernel::Index::open(&path).expect("published index remains usable");
    reopened.close().expect("close reopened index");
    std::fs::remove_dir_all(path).expect("remove create/open race index");
}

// SKC: search.sorting/basic kinds=ergonomic runtime=native
// SKC: search.pagination/basic kinds=ergonomic runtime=native
// SKC: result.docsum_field_projection/basic kinds=ergonomic runtime=native
// SKC: result.highlighting/basic kinds=ergonomic runtime=native
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
    assert_eq!(empty_projection.hits[0].document, Some(json!({})));

    let brands_present = index
        .browse()
        .filter(field("brand").exists())
        .execute::<serde_json::Value>()
        .expect("keyword exists filter");
    assert_eq!(brands_present.total_hits.value, 6);
    let prices_present = index
        .browse()
        .filter(field("price").exists())
        .execute::<serde_json::Value>()
        .expect("numeric exists filter");
    assert_eq!(prices_present.total_hits.value, 5);

    let filtered = index
        .search("wireless")
        .filter(field("brand").eq("Acme"))
        .execute::<serde_json::Value>()
        .expect("lexical query with filter");
    assert_eq!(filtered.total_hits.value, 1);

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
        paged.hits[0].document.as_ref().unwrap(),
        &json!({"title": "Wireless Keyboard", "brand": "Charlie"})
    );

    // Unsupported controls still fail at the adapter with stable details
    // instead of being ignored. Highlighting is a real docsums capability.
    assert_search_detail(
        index
            .search("wireless")
            .search_fields([SearchField::new("title").weight(2.0)])
            .execute::<serde_json::Value>(),
        "ranking_weights_not_supported",
    );
    let bm25 = index
        .search("wireless")
        .ranking(Ranking::bm25())
        .execute::<serde_json::Value>()
        .expect("native BM25 ranking");
    assert_eq!(bm25.total_hits.value, 6);
    let highlighted = index
        .search("wireless")
        .summary("default")
        .projection(["brand"])
        .highlight(["title"])
        .execute::<serde_json::Value>()
        .expect("native Vespa highlighting");
    assert!(highlighted.hits.iter().any(|hit| {
        hit.document.as_ref() == Some(&json!({"brand": "Charlie"}))
            && hit.highlights.iter().any(|highlight| {
                highlight.field == "title"
                    && highlight.fragments[0]
                        .matches
                        .first()
                        .is_some_and(|span| span.start_byte == 0 && span.end_byte == 8)
            })
    }));
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
        .execute::<serde_json::Value>()
        .expect("search after reopen");
    assert_eq!(after_reopen.total_hits.value, 6);
    reopened_runtime
        .close()
        .expect("close reopened native parity index");
    std::fs::remove_dir_all(path).expect("remove native parity index");
}

// SKC: query.phrase/basic kinds=ergonomic runtime=native
// SKC: query.proximity/basic kinds=ergonomic runtime=native
// SKC: query.fuzzy/basic kinds=ergonomic runtime=native
// SKC: query.weighted_and_wand/basic kinds=ergonomic runtime=native
#[test]
fn native_product_path_supports_typed_query_families() {
    let path = std::env::temp_dir().join(format!(
        "searchkernel-native-typed-query-e2e-{}-{}",
        std::process::id(),
        unique_suffix()
    ));
    let definition = IndexDefinition::new()
        .field(Field::text("title").stored(true))
        .field(Field::keyword("brand").stored(true).filterable(true));
    let runtime = IndexRuntime::create(&path, definition).expect("create typed-query index");
    let index = runtime.index();
    for (id, title, brand) in [
        ("d1", "quick brown fox", "brown"),
        ("d2", "brown quick fox", "brown"),
        ("d3", "quick fox", "black"),
        ("d4", "fox", "black"),
    ] {
        index
            .upsert(id, &json!({"title": title, "brand": brand}))
            .expect("upsert typed-query document");
    }

    let ids = |query| {
        index
            .browse()
            .typed_query(query)
            .execute::<serde_json::Value>()
            .expect("execute typed query")
            .hits
            .into_iter()
            .map(|hit| hit.id.to_string())
            .collect::<Vec<_>>()
    };

    assert_eq!(ids(Query::phrase("title", "quick brown")), ["d1"]);
    assert_eq!(
        ids(Query::near(
            "title",
            [Query::term("title", "quick"), Query::term("title", "brown"),],
        ))
        .into_iter()
        .collect::<BTreeSet<_>>(),
        BTreeSet::from(["d1".to_owned(), "d2".to_owned()])
    );
    assert_eq!(
        ids(Query::onear(
            "title",
            [Query::term("title", "quick"), Query::term("title", "brown"),],
        )),
        ["d1"]
    );
    assert_eq!(
        ids(Query::term("title", "brwn"))
            .into_iter()
            .collect::<BTreeSet<_>>(),
        BTreeSet::new()
    );
    assert_eq!(
        ids(Query::fuzzy("brand", "brwn"))
            .into_iter()
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["d1".to_owned(), "d2".to_owned()])
    );
    assert_eq!(
        ids(Query::rank(
            Query::term("title", "fox"),
            [Query::term("title", "quick")],
        ))
        .into_iter()
        .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "d1".to_owned(),
            "d2".to_owned(),
            "d3".to_owned(),
            "d4".to_owned(),
        ])
    );
    assert_eq!(
        ids(Query::wand(
            "title",
            [
                WeightedQueryItem::new("quick", 10),
                WeightedQueryItem::new("fox", 1),
            ],
        ))
        .into_iter()
        .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "d1".to_owned(),
            "d2".to_owned(),
            "d3".to_owned(),
            "d4".to_owned(),
        ])
    );

    runtime.close().expect("close typed-query index");
    std::fs::remove_dir_all(path).expect("remove typed-query index");
}

#[test]
fn native_product_path_supports_geo_queries() {
    let path = std::env::temp_dir().join(format!(
        "searchkernel-native-geo-e2e-{}-{}",
        std::process::id(),
        unique_suffix()
    ));
    let definition = IndexDefinition::new().field(Field::position("location"));
    let runtime = IndexRuntime::create(&path, definition).expect("create geo index");
    let index = runtime.index();
    index
        .upsert(
            "inside",
            &json!({"location": {"latitude": 37.3, "longitude": -121.9}}),
        )
        .expect("upsert inside geo document");
    index
        .upsert(
            "outside",
            &json!({"location": {"latitude": 40.7, "longitude": -74.0}}),
        )
        .expect("upsert outside geo document");

    let location = index
        .browse()
        .typed_query(Query::geo_location("location", 37.3, -121.9, "10km"))
        .execute::<serde_json::Value>()
        .expect("execute geo-location query");
    let bounding_box = index
        .browse()
        .typed_query(Query::geo_bounding_box(
            "location", 37.0, -122.2, 37.6, -121.5,
        ))
        .execute::<serde_json::Value>()
        .expect("execute geo-bounding-box query");
    assert_eq!(location.hits[0].id.as_str(), "inside");
    assert_eq!(bounding_box.hits[0].id.as_str(), "inside");
    assert_eq!(location.total_hits.value, 1);
    assert_eq!(bounding_box.total_hits.value, 1);

    runtime.close().expect("close geo index");
    std::fs::remove_dir_all(path).expect("remove geo index");
}

#[test]
fn native_product_path_supports_8d_vector_and_hybrid_search() {
    let path = std::env::temp_dir().join(format!(
        "searchkernel-native-vector-e2e-{}-{}",
        std::process::id(),
        unique_suffix()
    ));
    let tensor_type = "tensor<float>(x[8])";
    let definition = IndexDefinition::new()
        .field(Field::text("title").stored(true))
        .field(Field::tensor("embedding", tensor_type))
        .rank_profile(
            RankProfile::new("hybrid")
                .query_input("query_embedding", tensor_type)
                .first_phase("bm25(title) + 10 * closeness(embedding)")
                .match_feature("bm25(title)")
                .match_feature("closeness(embedding)"),
        );
    let runtime = IndexRuntime::create(&path, definition).expect("create vector index");
    let index = runtime.index();
    let document = |title: &str, values: [f64; 8]| {
        let embedding = serde_json::to_value(Tensor::dense(tensor_type, values))
            .expect("serialize document tensor");
        json!({"title": title, "embedding": embedding})
    };
    for (id, title, values) in [
        (
            "shoe-1",
            "red running shoes",
            [0.92, 0.14, 0.05, 0.74, 0.76, 0.42, 0.20, 0.78],
        ),
        (
            "shoe-2",
            "blue running shoes",
            [0.74, 0.55, 0.08, 0.58, 0.42, 0.86, 0.18, 0.64],
        ),
        (
            "shoe-3",
            "trail shoes",
            [0.10, 0.16, 0.78, 0.12, 0.24, 0.28, 0.92, 0.16],
        ),
        (
            "shoe-4",
            "lightweight shoes",
            [0.82, 0.12, 0.06, 0.96, 0.72, 0.82, 0.10, 0.88],
        ),
        (
            "shoe-5",
            "casual shoes",
            [0.64, 0.82, 0.07, 0.62, 0.34, 0.92, 0.16, 0.62],
        ),
        (
            "bag-1",
            "leather bag",
            [0.45, 0.25, 0.05, 0.35, 0.95, 0.15, 0.05, 0.55],
        ),
    ] {
        index
            .upsert(id, &document(title, values))
            .expect("upsert vector document");
    }

    let query_vector = Tensor::dense(
        tensor_type,
        [0.92, 0.14, 0.05, 0.74, 0.76, 0.42, 0.20, 0.78],
    );
    let vector = index
        .browse()
        .typed_query(Query::nearest_neighbor_with_options(
            "embedding",
            "query_embedding",
            NearestNeighborOptions {
                target_hits: 3,
                approximate: true,
                distance_threshold: None,
                explore_additional_hits: 2,
            },
        ))
        .ranking(Ranking::named("hybrid"))
        .query_input("query_embedding", query_vector.clone())
        .limit(3)
        .execute::<serde_json::Value>()
        .expect("execute 8d nearest-neighbor query");
    assert_eq!(vector.hits[0].id.as_str(), "shoe-1");

    let hybrid = index
        .browse()
        .typed_query(Query::or([
            Query::term("title", "shoes"),
            Query::nearest_neighbor("embedding", "query_embedding"),
        ]))
        .ranking(Ranking::named("hybrid"))
        .query_input(
            "query_embedding",
            Tensor::dense(
                tensor_type,
                [0.45, 0.25, 0.05, 0.35, 0.95, 0.15, 0.05, 0.55],
            ),
        )
        .limit(20)
        .execute::<serde_json::Value>()
        .expect("execute 8d hybrid query");
    let hybrid_ids = hybrid
        .hits
        .iter()
        .map(|hit| hit.id.to_string())
        .collect::<BTreeSet<_>>();
    assert_eq!(hybrid.total_hits.value, 6);
    assert_eq!(hybrid.hits[0].id.as_str(), "bag-1");
    assert!(hybrid_ids.contains("bag-1"));
    assert!(["shoe-1", "shoe-2", "shoe-3", "shoe-4", "shoe-5"]
        .iter()
        .all(|id| hybrid_ids.contains(*id)));

    runtime.close().expect("close vector index");
    std::fs::remove_dir_all(path).expect("remove vector index");
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

#[cfg(all(feature = "internal-api", unix))]
#[test]
fn native_storage_migration_rejects_symlink_source_and_destination() {
    use std::os::unix::fs::symlink;

    let root = std::env::temp_dir().join(format!(
        "searchkernel-native-migrate-symlink-{}-{}",
        std::process::id(),
        unique_suffix()
    ));
    std::fs::create_dir_all(&root).expect("create migration test root");

    let real_source = root.join("real-source");
    let source_link = root.join("source-link");
    let destination = root.join("managed-target");
    let runtime = IndexRuntime::create(
        &real_source,
        IndexDefinition::new().field(Field::text("title")),
    )
    .expect("create source index");
    runtime.close().expect("close source index");
    symlink(&real_source, &source_link).expect("create source symlink");

    let error = searchkernel::Index::migrate_storage(&source_link, &destination)
        .expect_err("symlink source must be rejected");
    assert_eq!(error.code(), ErrorCode::FailedPrecondition);
    assert!(std::fs::symlink_metadata(&source_link)
        .expect("source link metadata")
        .file_type()
        .is_symlink());
    assert!(real_source.is_dir());
    assert!(!destination.exists());

    let source = root.join("source");
    let external = root.join("external");
    let destination_link = root.join("destination-link");
    let runtime = IndexRuntime::create(&source, IndexDefinition::new().field(Field::text("title")))
        .expect("create second source index");
    runtime.close().expect("close second source index");
    std::fs::create_dir(&external).expect("create external directory");
    symlink(&external, &destination_link).expect("create destination symlink");

    let error = searchkernel::Index::migrate_storage(&source, &destination_link)
        .expect_err("symlink destination must be rejected");
    assert_eq!(error.code(), ErrorCode::FailedPrecondition);
    assert!(source.is_dir());
    assert!(std::fs::symlink_metadata(&destination_link)
        .expect("destination link metadata")
        .file_type()
        .is_symlink());
    assert!(external.is_dir());

    std::fs::remove_dir_all(&root).expect("remove migration test root");
}

#[cfg(feature = "internal-api")]
#[test]
fn native_storage_migration_concurrent_equivalent_calls_converge() {
    let root = std::env::temp_dir().join(format!(
        "searchkernel-native-migrate-race-{}-{}",
        std::process::id(),
        unique_suffix()
    ));
    std::fs::create_dir_all(&root).expect("create migration race root");
    let source = root.join("source");
    let destination = root.join("managed-target");
    let runtime = IndexRuntime::create(&source, IndexDefinition::new().field(Field::text("title")))
        .expect("create migration source");
    runtime.close().expect("close migration source");

    let barrier = Arc::new(Barrier::new(3));
    let workers = (0..2)
        .map(|_| {
            let barrier = Arc::clone(&barrier);
            let source = source.clone();
            let destination = destination.clone();
            std::thread::spawn(move || {
                barrier.wait();
                searchkernel::Index::migrate_storage(source, destination)
            })
        })
        .collect::<Vec<_>>();
    barrier.wait();

    for worker in workers {
        worker
            .join()
            .expect("migration worker")
            .expect("equivalent concurrent migration converges");
    }
    assert!(!source.exists());
    assert!(destination.is_dir());

    std::fs::remove_dir_all(&root).expect("remove migration race root");
}
