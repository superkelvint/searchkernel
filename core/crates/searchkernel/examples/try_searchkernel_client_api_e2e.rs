//! End-to-end showcase for the public Rust Search Kernel API.
//!
//! Run from the repository root with:
//!
//!   cargo run -p searchkernel --features native --example try_searchkernel_client_api_e2e
//!
//! Suggested repository location:
//!
//!   core/crates/searchkernel/examples/try_searchkernel_client_api_e2e.rs
//!
//! This deliberately uses only the public `searchkernel` API. It does not
//! reach into searchkernel-ir, the adapter, generated FlatBuffers types, or
//! native bindings to paper over missing product-API capabilities.

#[cfg(not(feature = "native"))]
fn main() {
    eprintln!("this example requires `--features native`");
}

#[cfg(feature = "native")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    showcase::run()
}

#[cfg(feature = "native")]
mod showcase {
    use searchkernel::{
        field, ErrorCode, Facet, FacetSort, Field, Filter, Index, IndexDefinition, Patch,
        SearchResult, Sort,
    };
    use serde_json::{json, Value};
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    pub fn run() -> Result<(), Box<dyn std::error::Error>> {
        // Keep all showcase state, including any sibling lock files created by
        // Search Kernel, underneath one unique temporary directory. The guard
        // removes the whole directory when the example exits.
        let temp = TempWorkspace::new()?;
        let path = temp.index_path();

        println!("temporary workspace: {}", temp.root().display());

        heading("1. CREATE INDEX");

        let definition = IndexDefinition::new()
            .field(Field::text("title").stored(true).searchable(true))
            .field(
                Field::keyword("brand")
                    .stored(true)
                    .filterable(true)
                    .sortable(true)
                    .facetable(true),
            )
            .field(
                Field::keyword("category")
                    .stored(true)
                    .filterable(true)
                    .facetable(true),
            )
            .field(
                Field::float64("price")
                    .stored(true)
                    .filterable(true)
                    .sortable(true),
            )
            .field(
                Field::float64("sale_price")
                    .stored(true)
                    .filterable(true)
                    .sortable(true),
            )
            .field(
                Field::int64("rating")
                    .stored(true)
                    .filterable(true)
                    .sortable(true),
            )
            .field(
                Field::int64("inventory")
                    .stored(true)
                    .filterable(true)
                    .sortable(true),
            )
            .field(
                Field::bool("in_stock")
                    .stored(true)
                    .filterable(true),
            );

        let index = Index::create(&path, definition)?;
        println!("created: {}", path.display());

        heading("2. INSERT DOCUMENTS");

        let documents = [
            (
                "shoe-1",
                json!({
                    "title": "Jumped Running Shoes for Daily Training",
                    "brand": "Acme",
                    "category": "shoes",
                    "price": 89.0,
                    "sale_price": 79.0,
                    "rating": 5,
                    "inventory": 10,
                    "in_stock": true,
                }),
            ),
            (
                "shoe-2",
                json!({
                    "title": "Trail Running Shoes",
                    "brand": "TrailCo",
                    "category": "shoes",
                    "price": 129.0,
                    "rating": 5,
                    "inventory": 4,
                    "in_stock": true,
                }),
            ),
            (
                "shoe-3",
                json!({
                    "title": "Budget Walking Shoes",
                    "brand": "Budget",
                    "category": "shoes",
                    "price": 49.0,
                    "rating": 3,
                    "inventory": 25,
                    "in_stock": true,
                }),
            ),
            (
                "shoe-4",
                json!({
                    "title": "Acme Racing Shoes",
                    "brand": "Acme",
                    "category": "shoes",
                    "price": 159.0,
                    "rating": 4,
                    "inventory": 0,
                    "in_stock": false,
                }),
            ),
            (
                "shoe-5",
                json!({
                    "title": "TrailCo Hiking Shoes",
                    "brand": "TrailCo",
                    "category": "shoes",
                    "price": 109.0,
                    "rating": 4,
                    "inventory": 8,
                    "in_stock": true,
                }),
            ),
            (
                "bag-1",
                json!({
                    "title": "Lightweight Running Backpack",
                    "brand": "Acme",
                    "category": "bags",
                    "price": 69.0,
                    "rating": 4,
                    "inventory": 12,
                    "in_stock": true,
                }),
            ),
        ];

        for (id, document) in &documents {
            index.insert(*id, document)?;
            println!("insert: {id}");
        }

        heading("3. GET BY ID");

        let doc = index
            .get::<Value>("shoe-1")?
            .expect("shoe-1 should exist");
        println!("{doc:#}");
        assert_eq!(doc["price"], json!(89.0));
        assert_eq!(doc["brand"], json!("Acme"));

        heading("4. FULL-TEXT SEARCH");

        let results = index.search("shoes").limit(20).execute::<Value>()?;
        print_results(&results);
        assert_id_set(
            &results,
            ["shoe-1", "shoe-2", "shoe-3", "shoe-4", "shoe-5"],
        );
        assert_eq!(results.total_hits.value, 5);

        heading("5. STEMMING: 'jump' -> 'Jumped'");

        let results = index.search("jump").limit(20).execute::<Value>()?;
        print_results(&results);
        assert!(results.hits.iter().any(|hit| hit.id.as_str() == "shoe-1"));
        println!("STEMMING PASS");

        heading("6. TYPED FILTER");

        let results = index
            .search("shoes")
            .filter(field("price").lte(100.0_f64))
            .sort(Sort::asc("price"))
            .limit(20)
            .execute::<Value>()?;
        print_results(&results);
        assert_ids(&results, ["shoe-3", "shoe-1"]);

        heading("7. REPEATED FILTERS ARE AND");

        let results = index
            .search("shoes")
            .filter(field("price").lte(120.0_f64))
            .filter(field("rating").gte(4_i64))
            .filter(field("in_stock").eq(true))
            .sort(Sort::asc("price"))
            .limit(20)
            .execute::<Value>()?;
        print_results(&results);
        assert_ids(&results, ["shoe-1", "shoe-5"]);

        heading("8. BOOLEAN FILTER TREE: OR / NOT / PARENTHESES EQUIVALENT");

        // Equivalent to:
        //   (brand IN ("Acme", "TrailCo") OR rating = 3)
        //   AND NOT (in_stock = false)
        let filter = Filter::all([
            Filter::any([
                field("brand").eq("Acme"),
                field("brand").eq("TrailCo"),
                field("rating").eq(3_i64),
            ]),
            Filter::not(field("in_stock").eq(false)),
        ]);
        let results = index
            .search("shoes")
            .filter(filter)
            .sort(Sort::asc("price"))
            .limit(20)
            .execute::<Value>()?;
        print_results(&results);
        assert_ids(&results, ["shoe-3", "shoe-1", "shoe-5", "shoe-2"]);

        heading("9. SORT + FIELD PROJECTION + HIT ACCESS");

        let results = index
            .search("shoes")
            .sort(Sort::desc("price"))
            .projection(["title", "brand", "price"])
            .limit(2)
            .execute::<Value>()?;
        print_results(&results);
        assert_ids(&results, ["shoe-4", "shoe-2"]);
        assert_eq!(results.hits[0].document["title"], json!("Acme Racing Shoes"));
        assert_eq!(results.hits[0].document["brand"], json!("Acme"));
        assert_eq!(results.hits[0].document["price"], json!(159.0));
        assert!(results.hits[0].document.get("rating").is_none());

        heading("10. OFFSET / LIMIT PAGINATION");

        let all_sorted = index
            .search("shoes")
            .sort(Sort::asc("price"))
            .limit(20)
            .execute::<Value>()?;
        let window = index
            .search("shoes")
            .sort(Sort::asc("price"))
            .offset(1)
            .limit(2)
            .execute::<Value>()?;

        println!("all:");
        print_results(&all_sorted);
        println!("window:");
        print_results(&window);

        let expected_window: Vec<String> = all_sorted.hits[1..3]
            .iter()
            .map(|hit| hit.id.to_string())
            .collect();
        assert_eq!(ids(&window), expected_window);
        assert_eq!(window.total_hits, all_sorted.total_hits);

        // Rust's builder takes u32 for limit/offset, so negative values are
        // rejected by the type system rather than by a runtime check.

        heading("11. FACETS");

        let results = index
            .search("shoes")
            .facet(Facet::new("brand").limit(10).sort(FacetSort::CountDesc))
            .facet(
                Facet::new("category")
                    .limit(10)
                    .sort(FacetSort::CountDesc),
            )
            .limit(20)
            .execute::<Value>()?;
        print_results(&results);

        assert_eq!(
            facet_counts(&results, "brand"),
            BTreeMap::from([
                ("Acme".to_owned(), 2),
                ("Budget".to_owned(), 1),
                ("TrailCo".to_owned(), 2),
            ])
        );
        assert_eq!(
            facet_counts(&results, "category"),
            BTreeMap::from([("shoes".to_owned(), 5)])
        );

        heading("12. FILTERED FACET");

        let results = index
            .search("shoes")
            .filter(field("in_stock").eq(true))
            .facet(Facet::new("brand").limit(10).sort(FacetSort::CountDesc))
            .limit(20)
            .execute::<Value>()?;
        print_results(&results);
        assert_eq!(
            facet_counts(&results, "brand"),
            BTreeMap::from([
                ("Acme".to_owned(), 1),
                ("Budget".to_owned(), 1),
                ("TrailCo".to_owned(), 2),
            ])
        );

        heading("13. ANALYZE / LINGUISTICS");

        let analyzed = index.analyze("Jumped shoes").field("title").execute()?;
        for token in &analyzed.tokens {
            println!(
                "  position={} surface={:?} term={:?} bytes={}..{}",
                token.position, token.surface, token.term, token.start_byte, token.end_byte
            );
        }
        assert!(analyzed.tokens.iter().any(|token| token.term == "jump"));

        heading("14. STRUCTURED FILTER ERROR");

        let error = index
            .search("shoes")
            .filter(field("prcie").lte(100.0_f64))
            .execute::<Value>()
            .expect_err("unknown field filter should fail");
        println!("code: {:?}", error.code());
        println!("message: {error}");
        println!("details: {:#?}", error.details());
        assert_eq!(error.code(), ErrorCode::InvalidArgument);
        assert!(!error.details().is_empty());
        assert_eq!(error.details()[0].code, "unknown_field");
        assert_eq!(error.details()[0].path.as_deref(), Some("filter.field"));

        heading("15. PATCH SET / UNSET");

        println!("before: {:#}", index.get::<Value>("shoe-1")?.unwrap());
        index.patch(
            "shoe-1",
            Patch::new()
                .set("price", 79.0_f64)
                .set("rating", 4_i64)
                .unset("sale_price"),
        )?;

        let updated = index
            .get::<Value>("shoe-1")?
            .expect("shoe-1 should still exist");
        println!("after: {updated:#}");
        assert_eq!(updated["price"], json!(79.0));
        assert_eq!(updated["rating"], json!(4));
        assert!(updated.get("sale_price").is_none());

        let results = index
            .search("shoes")
            .filter(field("price").lt(80.0_f64))
            .sort(Sort::asc("price"))
            .limit(20)
            .execute::<Value>()?;
        print_results(&results);
        assert_ids(&results, ["shoe-3", "shoe-1"]);

        heading("16. DELETE");

        assert!(index.delete("shoe-3")?);
        assert!(index.get::<Value>("shoe-3")?.is_none());

        let results = index.search("shoes").limit(20).execute::<Value>()?;
        print_results(&results);
        assert!(!results.hits.iter().any(|hit| hit.id.as_str() == "shoe-3"));
        assert_eq!(results.total_hits.value, 4);

        let results = index
            .search("shoes")
            .facet("brand".into())
            .limit(20)
            .execute::<Value>()?;
        assert!(!facet_counts(&results, "brand").contains_key("Budget"));

        assert!(!index.delete("shoe-3")?);
        println!("DELETE PASS");

        heading("17. UPSERT REPLACEMENT");

        index.upsert(
            "shoe-2",
            &json!({
                "title": "Trail Running Shoes - Updated Edition",
                "brand": "TrailCo",
                "category": "shoes",
                "price": 119.0,
                "rating": 5,
                "inventory": 4,
                "in_stock": true,
            }),
        )?;

        let doc = index
            .get::<Value>("shoe-2")?
            .expect("shoe-2 should exist");
        println!("{doc:#}");
        assert_eq!(doc["price"], json!(119.0));
        assert!(doc["title"]
            .as_str()
            .expect("title should be a string")
            .contains("Updated Edition"));

        let results = index.search("updated").limit(20).execute::<Value>()?;
        print_results(&results);
        assert_id_set(&results, ["shoe-2"]);

        heading("18. INDEX CLOSED");
        index.close()?;
        println!("The first native Index instance has shut down.");

        heading("19. REOPEN PERSISTED INDEX");

        let index = Index::open(&path)?;

        assert!(index.get::<Value>("shoe-3")?.is_none());

        let shoe1 = index
            .get::<Value>("shoe-1")?
            .expect("shoe-1 should survive reopen");
        println!("shoe-1 after reopen: {shoe1:#}");
        assert_eq!(shoe1["price"], json!(79.0));
        assert_eq!(shoe1["rating"], json!(4));
        assert!(shoe1.get("sale_price").is_none());

        let shoe2 = index
            .get::<Value>("shoe-2")?
            .expect("shoe-2 should survive reopen");
        println!("shoe-2 after reopen: {shoe2:#}");
        assert_eq!(shoe2["price"], json!(119.0));
        assert!(shoe2["title"]
            .as_str()
            .expect("title should be a string")
            .contains("Updated Edition"));

        let results = index
            .search("shoes")
            .filter(field("price").lte(120.0_f64))
            .sort(Sort::asc("price"))
            .limit(20)
            .execute::<Value>()?;
        print_results(&results);
        assert_ids(&results, ["shoe-1", "shoe-5", "shoe-2"]);

        heading("20. BROWSE AFTER REOPEN");

        let results = index
            .browse()
            .filter(field("in_stock").eq(true))
            .sort(Sort::asc("price"))
            .facet("brand".into())
            .limit(20)
            .execute::<Value>()?;
        print_results(&results);

        let browse_ids: BTreeSet<String> = ids(&results).into_iter().collect();
        assert!(!browse_ids.contains("shoe-4"));
        assert!(!browse_ids.contains("shoe-3"));
        for expected in ["shoe-1", "shoe-2", "shoe-5", "bag-1"] {
            assert!(browse_ids.contains(expected), "missing browse hit: {expected}");
        }

        index.close()?;

        heading("21. ALL CURRENT RUST CLIENT API E2E CHECKS PASSED");

        println!(
            r#"
Successfully demonstrated through the public Rust facade:

  DOCUMENTS
  ✓ create/open persistent index
  ✓ insert
  ✓ upsert replacement
  ✓ get
  ✓ delete + duplicate delete
  ✓ Patch set/unset
  ✓ persistence across reopen

  SEARCH / FILTERING
  ✓ full-text search
  ✓ stemming representative
  ✓ typed filters
  ✓ repeated filters -> AND
  ✓ explicit OR / NOT filter trees
  ✓ sorting
  ✓ projection
  ✓ offset/limit
  ✓ total hits
  ✓ structured unknown-field error

  FACETS
  ✓ facet convenience
  ✓ facet ordering request
  ✓ filtered facets

  LINGUISTICS
  ✓ field-aware analyze request

Rust API parity still missing versus the Python showcase:

  - public string-filter parser/binding API
  - one-based page() convenience + page/offset/limit misuse checks
  - root aggregations
  - structured grouping / nested groups / aliases / keep filters
  - grouping summary-hit children and continuation tokens
  - typed-query DSL
  - query-input namespace
  - rank-property namespace
  - named rank-profile selection
  - YQL + property substitution
  - advanced update/increment builder

Those gaps are intentionally NOT bypassed here through IR, FlatBuffers, or
adapter internals. This file is meant to exercise the actual public Rust API.
"#
        );

        Ok(())
    }

    fn heading(title: &str) {
        println!();
        println!("{}", "=".repeat(80));
        println!("{title}");
        println!("{}", "=".repeat(80));
    }

    fn print_results(results: &SearchResult<Value>) {
        println!(
            "total_hits = {} relation = {:?}",
            results.total_hits.value, results.total_hits.relation
        );

        for hit in &results.hits {
            println!(
                "  id={:?} score={:?} document={}",
                hit.id.as_str(),
                hit.score,
                hit.document
            );
        }

        if !results.facets.is_empty() {
            println!("facets:");
            for facet in &results.facets {
                println!("  {}:", facet.field);
                for value in &facet.values {
                    println!("    {:?}: {}", value.value, value.count);
                }
            }
        }
    }

    fn ids(results: &SearchResult<Value>) -> Vec<String> {
        results
            .hits
            .iter()
            .map(|hit| hit.id.to_string())
            .collect()
    }

    fn assert_ids<const N: usize>(results: &SearchResult<Value>, expected: [&str; N]) {
        let expected: Vec<String> = expected.into_iter().map(str::to_owned).collect();
        assert_eq!(ids(results), expected);
    }

    fn assert_id_set<const N: usize>(results: &SearchResult<Value>, expected: [&str; N]) {
        let actual: BTreeSet<String> = ids(results).into_iter().collect();
        let expected: BTreeSet<String> = expected.into_iter().map(str::to_owned).collect();
        assert_eq!(actual, expected);
    }

    fn facet_counts(results: &SearchResult<Value>, field: &str) -> BTreeMap<String, u64> {
        let facet = results
            .facets
            .iter()
            .find(|facet| facet.field == field)
            .unwrap_or_else(|| panic!("missing facet {field:?}"));

        facet
            .values
            .iter()
            .map(|value| {
                let key = value
                    .value
                    .as_str()
                    .unwrap_or_else(|| panic!("facet {field:?} returned non-string value"))
                    .to_owned();
                (key, value.count)
            })
            .collect()
    }

    struct TempWorkspace {
        root: PathBuf,
    }

    impl TempWorkspace {
        fn new() -> std::io::Result<Self> {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();

            let root = std::env::temp_dir().join(format!(
                "searchkernel-client-api-e2e-{}-{nonce}",
                std::process::id()
            ));

            std::fs::create_dir_all(&root)?;
            Ok(Self { root })
        }

        fn root(&self) -> &Path {
            &self.root
        }

        fn index_path(&self) -> PathBuf {
            self.root.join("showcase-index-rust")
        }
    }

    impl Drop for TempWorkspace {
        fn drop(&mut self) {
            if let Err(error) = std::fs::remove_dir_all(&self.root) {
                if error.kind() != std::io::ErrorKind::NotFound {
                    eprintln!(
                        "warning: failed to remove temporary showcase workspace {}: {error}",
                        self.root.display()
                    );
                }
            }
        }
    }
}
