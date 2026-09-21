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
//! reach into implementation internals to paper over missing product-API
//! capabilities.

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
    use searchkernel::params;
    use searchkernel::{
        field, ErrorCode, Facet, FacetSort, FeatureSet, FeatureValue, Field, Filter,
        GroupingAggregation, GroupingPlan, Hit, Index, IndexDefinition, Patch, Query, QueryBound,
        QueryValue, RankProfile, Ranking, SearchResult, Sort, Tensor, TotalHitsRelation,
        WandOptions, WeightedQueryItem,
    };
    use serde_json::{json, Value};
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::{Path, PathBuf};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

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
            .field(Field::text("description").stored(true).searchable(true))
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
                    .sortable(true)
                    .facetable(true),
            )
            .field(
                Field::int64("inventory")
                    .stored(true)
                    .filterable(true)
                    .sortable(true),
            )
            .field(Field::bool("in_stock").stored(true).filterable(true))
            .field(Field::tensor("embedding", "tensor<float>(x[2])").stored(true))
            .rank_profile(
                RankProfile::new("boosted")
                    .first_phase("query(boost)")
                    .match_feature("query(boost)"),
            )
            .rank_profile(
                RankProfile::new("semantic")
                    .first_phase("closeness(embedding)")
                    .query_input("query_embedding", "tensor<float>(x[2])"),
            )
            .default_search_fields(["title", "description"]);
        let definition = definition.rank_profile(
            RankProfile::new("two_phase")
                .first_phase("attribute(rating)")
                .second_phase("attribute(inventory)")
                .rerank_count(3),
        );
        let definition = definition
            .rank_profile(RankProfile::new("first_phase_only").first_phase("attribute(rating)"));
        let definition = definition.rank_profile(
            RankProfile::new("operator")
                .first_phase("nativeRank")
                .match_feature("nativeRank"),
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
                    "embedding": Tensor::dense("tensor<float>(x[2])", vec![1.0, 0.0]),
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
                    "embedding": Tensor::dense("tensor<float>(x[2])", vec![0.8, 0.2]),
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
                    "embedding": Tensor::dense("tensor<float>(x[2])", vec![0.0, 1.0]),
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
                    "embedding": Tensor::dense("tensor<float>(x[2])", vec![0.7, 0.3]),
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
                    "embedding": Tensor::dense("tensor<float>(x[2])", vec![0.9, 0.1]),
                }),
            ),
            (
                "bag-1",
                json!({
                    "title": "Lightweight Running Backpack",
                    "description": "exclusive catalog marker",
                    "brand": "Acme",
                    "category": "bags",
                    "price": 69.0,
                    "rating": 4,
                    "inventory": 12,
                    "in_stock": true,
                    "embedding": Tensor::dense("tensor<float>(x[2])", vec![0.6, 0.4]),
                }),
            ),
            (
                "rerank-a",
                json!({
                    "title": "Rerank Candidate A",
                    "category": "two-phase",
                    "rating": 100,
                    "inventory": 1,
                }),
            ),
            (
                "rerank-b",
                json!({
                    "title": "Rerank Candidate B",
                    "category": "two-phase",
                    "rating": 90,
                    "inventory": 100,
                }),
            ),
            (
                "rerank-c",
                json!({
                    "title": "Rerank Candidate C",
                    "category": "two-phase",
                    "rating": 80,
                    "inventory": 90,
                }),
            ),
            (
                "rerank-d",
                json!({
                    "title": "Rerank Candidate D",
                    "category": "two-phase",
                    "rating": 70,
                    "inventory": 1000,
                }),
            ),
        ];

        for (id, document) in &documents {
            index.insert(*id, document)?;
            println!("insert: {id}");
        }
        for probe in 0..200 {
            index.insert(
                format!("degraded-{probe}"),
                &json!({
                    "title": "Coverage Degradation Probe",
                    "category": "degradation",
                    "price": 18.0,
                    "rating": 1,
                    "inventory": 18,
                    "in_stock": true,
                    "embedding": Tensor::dense("tensor<float>(x[2])", vec![0.0, 1.0]),
                }),
            )?;
        }
        println!("inserted 200 deterministic coverage probe documents");

        heading("3. GET BY ID");

        let doc = index.get::<Value>("shoe-1")?.expect("shoe-1 should exist");
        println!("{doc:#}");
        assert_eq!(doc["price"], json!(89.0));
        assert_eq!(doc["brand"], json!("Acme"));
        assert_eq!(
            doc["embedding"],
            json!({
                "$searchkernel_tensor": {
                    "type_spec": "tensor<float>(x[2])",
                    "dense": [1.0, 0.0]
                }
            })
        );

        heading("4. DESCRIBE + GET PROJECTION");

        let described = index.describe()?;
        assert!(described
            .fields()
            .iter()
            .any(|field| field.name() == "embedding"));
        let projected = index
            .get_with_projection::<Value>("shoe-1", ["title", "brand"])?
            .expect("projected shoe-1 should exist");
        assert!(projected.get("title").is_some());
        assert!(projected.get("brand").is_some());
        assert!(projected.get("price").is_none());

        heading("5. FULL-TEXT SEARCH");

        let results = index.search("shoes").limit(20).execute::<Value>()?;
        print_results(&results);
        assert_id_set(&results, ["shoe-1", "shoe-2", "shoe-3", "shoe-4", "shoe-5"]);
        assert_eq!(results.total_hits.value, 5);
        assert_eq!(results.total_hits.relation, TotalHitsRelation::Unknown);

        heading("6. STEMMING: 'jump' -> 'Jumped'");

        let results = index.search("jump").limit(20).execute::<Value>()?;
        print_results(&results);
        assert!(results.hits.iter().any(|hit| hit.id.as_str() == "shoe-1"));
        println!("STEMMING PASS");

        heading("7. TYPED QUERIES THROUGH THE PUBLIC DSL");

        let term = index
            .search("")
            .typed_query(Query::term("title", "running"))
            .limit(20)
            .execute::<Value>()?;
        assert_id_set(&term, ["shoe-1", "shoe-2", "bag-1"]);

        let phrase = index
            .search("")
            .typed_query(Query::phrase("title", "running shoes"))
            .limit(20)
            .execute::<Value>()?;
        assert_id_set(&phrase, ["shoe-1", "shoe-2"]);

        let boolean = index
            .search("")
            .typed_query(Query::and([
                Query::or([
                    Query::equals("rating", 5_i64),
                    Query::equals("rating", 4_i64),
                ]),
                Query::not(Query::equals("in_stock", false)),
            ]))
            .limit(20)
            .execute::<Value>()?;
        print_results(&boolean);
        assert_id_set(&boolean, ["shoe-1", "shoe-2", "shoe-5", "bag-1"]);

        let range = index
            .search("")
            .typed_query(Query::range(
                "price",
                QueryBound::Excluded(49.0_f64.into()),
                QueryBound::Included(100.0_f64.into()),
            ))
            .limit(20)
            .execute::<Value>()?;
        assert_id_set(&range, ["shoe-1", "bag-1"]);

        let upper_excluded = index
            .search("")
            .typed_query(Query::range(
                "price",
                QueryBound::Included(89.0_f64.into()),
                QueryBound::Excluded(129.0_f64.into()),
            ))
            .sort(Sort::asc("price"))
            .limit(20)
            .execute::<Value>()?;
        assert_ids(&upper_excluded, ["shoe-1", "shoe-5"]);

        let in_query = index
            .search("")
            .typed_query(Query::in_values(
                "brand",
                [QueryValue::from("Acme"), QueryValue::from("Budget")],
            ))
            .limit(20)
            .execute::<Value>()?;
        assert_id_set(&in_query, ["shoe-1", "shoe-3", "shoe-4", "bag-1"]);

        let weak_and = index
            .search("")
            .typed_query(Query::weak_and(
                [
                    Query::term("title", "running"),
                    Query::term("title", "shoes"),
                ],
                10,
            ))
            .limit(20)
            .execute::<Value>()?;
        assert_id_set(
            &weak_and,
            ["shoe-1", "shoe-2", "shoe-3", "shoe-4", "shoe-5", "bag-1"],
        );

        let fuzzy = index
            .search("")
            .typed_query(Query::fuzzy("brand", "Acmx"))
            .limit(20)
            .execute::<Value>()?;
        let exact_misspelling = index
            .search("")
            .typed_query(Query::in_values("brand", [QueryValue::from("Acmx")]))
            .limit(20)
            .execute::<Value>()?;
        assert!(exact_misspelling.hits.is_empty());
        assert_id_set(&fuzzy, ["shoe-1", "shoe-4", "bag-1"]);

        let weighted_items = [
            WeightedQueryItem::new("Acme", 10),
            WeightedQueryItem::new("TrailCo", 5),
        ];
        let wand = index
            .search("")
            .ranking(Ranking::named("operator"))
            .typed_query(Query::Wand {
                field: "brand".to_owned(),
                items: weighted_items.to_vec(),
                options: WandOptions {
                    target_hits: 100,
                    score_threshold: 9,
                    threshold_boost_factor: 1.0,
                },
            })
            .limit(20)
            .execute::<Value>()?;
        let weighted_set = index
            .search("")
            .ranking(Ranking::named("operator"))
            .typed_query(Query::weighted_set("brand", weighted_items.iter().cloned()))
            .limit(20)
            .execute::<Value>()?;
        let dot_product = index
            .search("")
            .ranking(Ranking::named("operator"))
            .typed_query(Query::dot_product("brand", weighted_items.iter().cloned()))
            .limit(20)
            .execute::<Value>()?;
        assert_id_set(&wand, ["shoe-1", "shoe-4", "bag-1"]);
        assert_ids(
            &weighted_set,
            ["shoe-1", "shoe-4", "bag-1", "shoe-2", "shoe-5"],
        );
        assert_ids(
            &dot_product,
            ["shoe-1", "shoe-2", "shoe-4", "shoe-5", "bag-1"],
        );
        for results in [&wand, &weighted_set, &dot_product] {
            assert!(
                results.hits.iter().all(|hit| hit.score.is_some()),
                "weighted query must expose Vespa scores"
            );
        }
        let native_rank = |hit: &Hit<Value>| match hit.features.get("nativeRank") {
            Some(FeatureSet::Value(FeatureValue::Double(value))) => *value,
            other => panic!("operator query did not expose nativeRank: {other:?}"),
        };
        assert!(native_rank(&weighted_set.hits[0]) > native_rank(&weighted_set.hits[3]));
        assert!(
            native_rank(&dot_product.hits[0]) == native_rank(&dot_product.hits[1])
                && native_rank(&dot_product.hits[1]) == native_rank(&dot_product.hits[4])
        );
        assert_ne!(
            native_rank(&weighted_set.hits[0]),
            native_rank(&dot_product.hits[0])
        );

        let prefix = index
            .search("")
            .typed_query(Query::prefix("brand", "Ac"))
            .limit(20)
            .execute::<Value>()?;
        assert_id_set(&prefix, ["shoe-1", "shoe-4", "bag-1"]);
        let substring = index
            .search("")
            .typed_query(Query::substring("brand", "cme"))
            .limit(20)
            .execute::<Value>()?;
        assert_id_set(&substring, ["shoe-1", "shoe-4", "bag-1"]);
        let suffix = index
            .search("")
            .typed_query(Query::suffix("brand", "me"))
            .limit(20)
            .execute::<Value>()?;
        assert_id_set(&suffix, ["shoe-1", "shoe-4", "bag-1"]);
        let literal_pattern = index
            .search("")
            .typed_query(Query::prefix("brand", "Ac."))
            .limit(20)
            .execute::<Value>()?;
        assert!(literal_pattern.hits.is_empty());
        for query in [
            Query::substring("brand", "c.me"),
            Query::suffix("brand", ".me"),
        ] {
            let literal_pattern = index
                .search("")
                .typed_query(query)
                .limit(20)
                .execute::<Value>()?;
            assert!(literal_pattern.hits.is_empty());
        }

        println!("typed term/phrase/AND/OR/NOT/equality/inclusive-exclusive range/IN/weakAnd/WAND/weighted-set/dot-product/fuzzy/literal prefix/substring/suffix PASS");

        heading("8. TYPED FILTER");

        let results = index
            .search("shoes")
            .filter(field("price").lte(100.0_f64))
            .sort(Sort::asc("price"))
            .limit(20)
            .execute::<Value>()?;
        print_results(&results);
        assert_ids(&results, ["shoe-3", "shoe-1"]);

        let exists = index
            .search("shoes")
            .filter(field("sale_price").exists())
            .limit(20)
            .execute::<Value>()?;
        assert_id_set(&exists, ["shoe-1"]);

        heading("9. REPEATED FILTERS ARE AND");

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

        heading("10. BOOLEAN FILTER TREE: OR / NOT / PARENTHESES EQUIVALENT");

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

        heading("10A. TEXTUAL FILTER PARSER AND CLAUSE-LOCAL BINDINGS");

        let scalar = index
            .search("shoes")
            .filter(("price <= :max", params! { "max" => 100.0_f64 }))
            .sort(Sort::asc("price"))
            .limit(20)
            .execute::<Value>()?;
        assert_ids(&scalar, ["shoe-3", "shoe-1"]);

        let repeated = index
            .search("shoes")
            .filter(("price <= :value", params! { "value" => 120.0_f64 }))
            .filter(("rating >= :value", params! { "value" => 4_i64 }))
            .filter("in_stock = true")
            .sort(Sort::asc("price"))
            .limit(20)
            .execute::<Value>()?;
        assert_ids(&repeated, ["shoe-1", "shoe-5"]);

        let textual = index
            .search("shoes")
            .filter((
                "(brand IN (:brands) OR rating = :rating) AND NOT (in_stock = false)",
                params! {
                    "brands" => vec!["Acme", "TrailCo"],
                    "rating" => 3_i64,
                },
            ))
            .sort(Sort::asc("price"))
            .limit(20)
            .execute::<Value>()?;
        assert_ids(&textual, ["shoe-3", "shoe-1", "shoe-5", "shoe-2"]);

        let structured_equivalent = index
            .search("shoes")
            .filter(Filter::all([
                Filter::any([
                    field("brand").eq("Acme"),
                    field("brand").eq("TrailCo"),
                    field("rating").eq(3_i64),
                ]),
                Filter::not(field("in_stock").eq(false)),
            ]))
            .sort(Sort::asc("price"))
            .limit(20)
            .execute::<Value>()?;
        assert_eq!(ids(&textual), ids(&structured_equivalent));
        println!("textual filter parser, clause-local bindings, grammar, and textual/structured equivalence PASS");

        heading("11. SORT + FIELD PROJECTION + HIT ACCESS");

        let results = index
            .search("shoes")
            .summary("default")
            .sort(Sort::desc("price"))
            .projection(["title", "brand", "price"])
            .limit(2)
            .execute::<Value>()?;
        print_results(&results);
        assert_ids(&results, ["shoe-4", "shoe-2"]);
        assert_eq!(
            results.hits[0].document["title"],
            json!("Acme Racing Shoes")
        );
        assert_eq!(results.hits[0].document["brand"], json!("Acme"));
        assert_eq!(results.hits[0].document["price"], json!(159.0));
        assert!(results.hits[0].document.get("rating").is_none());
        println!("default summary selection + projection docsums PASS");

        heading("12. OFFSET / LIMIT PAGINATION");

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
        assert_eq!(all_sorted.total_hits.relation, TotalHitsRelation::Unknown);
        assert_eq!(window.total_hits.relation, TotalHitsRelation::Unknown);

        // Rust's builder takes u32 for limit/offset, so negative values are
        // rejected by the type system rather than by a runtime check.

        heading("13. FACETS");

        let results = index
            .search("shoes")
            .facet(Facet::new("brand").limit(10).sort(FacetSort::CountDesc))
            .facet(Facet::new("category").limit(10).sort(FacetSort::CountDesc))
            .limit(20)
            .execute::<Value>()?;
        print_results(&results);

        let brand_facet = results
            .facets
            .iter()
            .find(|facet| facet.field == "brand")
            .expect("brand facet should be present");
        let ordered_brand_values: Vec<_> = brand_facet
            .values
            .iter()
            .map(|value| {
                (
                    value
                        .value
                        .as_str()
                        .expect("brand facet value should be a string"),
                    value.count,
                )
            })
            .collect();
        assert_eq!(
            ordered_brand_values,
            [("Acme", 2), ("TrailCo", 2), ("Budget", 1)]
        );

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

        heading("14. FILTERED FACET");

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

        heading("15. RANKING, QUERY INPUTS, RANK PROPERTIES, FEATURES");

        let query_feature_results = index
            .search("shoes")
            .ranking(Ranking::named("boosted"))
            .query_input("boost", 3.0_f64)
            .limit(20)
            .execute::<Value>()?;
        assert!(!query_feature_results.hits.is_empty());
        assert!(query_feature_results.hits.iter().all(|hit| {
            hit.score.is_some()
                && matches!(
                    hit.features.get("query(boost)"),
                    Some(FeatureSet::Value(FeatureValue::Double(value)))
                        if (*value - 3.0).abs() < f64::EPSILON
                )
        }));

        let rank_property_results = index
            .search("shoes")
            .ranking(Ranking::profile("boosted"))
            .rank_property("boost", "4.0")
            .limit(20)
            .execute::<Value>()?;
        assert!(!rank_property_results.hits.is_empty());
        assert!(rank_property_results.hits.iter().all(|hit| {
            hit.score.is_some()
                && matches!(
                    hit.features.get("query(boost)"),
                    Some(FeatureSet::Value(FeatureValue::Double(value)))
                        if (*value - 4.0).abs() < f64::EPSILON
                )
        }));
        println!("named/profile ranking, scalar query input, rank property, score, and scalar feature PASS");

        heading("TWO-PHASE RANKING / SECOND-PHASE RERANK");

        let first_phase = index
            .search("candidate")
            .ranking(Ranking::named("first_phase_only"))
            .limit(4)
            .execute::<Value>()?;
        assert_ids(
            &first_phase,
            ["rerank-a", "rerank-b", "rerank-c", "rerank-d"],
        );
        let first_phase_scores: Vec<f64> = first_phase
            .hits
            .iter()
            .map(|hit| hit.score.expect("first-phase results expose Vespa scores"))
            .collect();
        assert_eq!(first_phase_scores, [100.0, 90.0, 80.0, 70.0]);

        let two_phase = index
            .search("candidate")
            .ranking(Ranking::named("two_phase"))
            .limit(4)
            .execute::<Value>()?;
        assert_ids(&two_phase, ["rerank-b", "rerank-c", "rerank-a", "rerank-d"]);
        assert_eq!(two_phase.total_hits.value, 4);
        let two_phase_scores: Vec<f64> = two_phase
            .hits
            .iter()
            .map(|hit| hit.score.expect("two-phase results expose Vespa scores"))
            .collect();
        assert_eq!(&two_phase_scores[..3], &[100.0, 90.0, 1.0]);
        assert!(two_phase_scores[3].is_finite());
        assert_ne!(two_phase_scores[3], 1000.0);
        println!(
            "first phase A>B>C>D; rerank window A,B,C; second phase B>C>A; final B,C,A,D PASS"
        );

        heading("16. GENERAL NESTED GROUPING");

        let grouping = GroupingPlan::new().level(
            GroupingPlan::by("category")
                .output(GroupingAggregation::count())
                .output(GroupingAggregation::average("price"))
                .nested(
                    GroupingPlan::by("rating")
                        .output(GroupingAggregation::count())
                        .output(GroupingAggregation::average("price")),
                ),
        );
        let grouped = index
            .search("shoes")
            .grouping(grouping)
            .limit(0)
            .execute::<Value>()?;
        let category_root = grouped.groups.first().expect("grouping root");
        let shoes_group = category_root
            .groups
            .iter()
            .find(|group| group.key == Some(json!("shoes")))
            .expect("shoes category group");
        assert_eq!(shoes_group.aggregations["count"], json!(5));
        assert_eq!(shoes_group.aggregations["average(price)"], json!(107.0));
        assert_eq!(shoes_group.groups.len(), 3);
        for (rating, count, average) in [(3, 1, 49.0), (4, 2, 134.0), (5, 2, 109.0)] {
            let rating_group = shoes_group
                .groups
                .iter()
                .find(|group| group.key == Some(json!(rating)))
                .unwrap_or_else(|| panic!("missing rating group {rating}"));
            assert_eq!(rating_group.aggregations["count"], json!(count));
            assert_eq!(rating_group.aggregations["average(price)"], json!(average));
        }
        println!("nested grouping keys, count, and average values PASS");

        heading("17. TENSOR READBACK + NEAREST NEIGHBOR");

        let nearest = index
            .search("")
            .ranking(Ranking::named("semantic"))
            .typed_query(Query::nearest_neighbor("embedding", "query_embedding"))
            .query_input(
                "query_embedding",
                Tensor::dense("tensor<float>(x[2])", vec![1.0, 0.0]),
            )
            .limit(3)
            .execute::<Value>()?;
        assert_eq!(
            nearest.hits.first().map(|hit| hit.id.as_str()),
            Some("shoe-1")
        );
        assert!(nearest.coverage.is_some());
        println!("tensor query input and native nearest-neighbor search PASS");

        heading("18. PUBLIC OPTIONS AND EXPLICITLY UNSUPPORTED HIGHLIGHTING");

        let inspected = index
            .search("shoes")
            .search_fields(["title"])
            .language("en")
            .timeout(std::time::Duration::from_secs(1))
            .sort(Sort::asc("sale_price").missing_first())
            .inspect()?;
        assert_eq!(inspected.query, "shoes");
        assert_eq!(inspected.search_fields[0].field(), "title");
        assert_eq!(inspected.language.as_deref(), Some("en"));
        assert_eq!(inspected.timeout, Some(Duration::from_secs(1)));
        let title_only = index
            .search("exclusive")
            .search_fields(["title"])
            .limit(20)
            .execute::<Value>()?;
        assert!(title_only.hits.is_empty());
        let default_fields = index.search("exclusive").limit(20).execute::<Value>()?;
        assert_ids(&default_fields, ["bag-1"]);
        let missing_first = index
            .search("shoes")
            .sort(Sort::asc("sale_price").missing_first())
            .limit(20)
            .execute::<Value>()?;
        assert_eq!(
            ids(&missing_first).last().map(String::as_str),
            Some("shoe-1")
        );
        let missing_last = index
            .search("shoes")
            .sort(Sort::asc("sale_price").missing_last())
            .limit(20)
            .execute::<Value>()?;
        assert_eq!(
            ids(&missing_last).first().map(String::as_str),
            Some("shoe-1")
        );
        let highlight_error = index
            .search("shoes")
            .highlight(["title"])
            .execute::<Value>()
            .expect_err("native highlight result path should report unsupported");
        assert_eq!(highlight_error.code(), ErrorCode::Unsupported);
        println!("search_fields execution, language, timeout, inspect, and missing-first PASS; highlighting is explicit UNSUPPORTED");

        heading("19. COVERAGE AND DEGRADATION");

        let normal = index.search("shoes").limit(20).execute::<Value>()?;
        assert!(normal
            .coverage
            .as_ref()
            .is_some_and(|coverage| coverage.full == Some(true)));
        let degraded = index
            .search("")
            .typed_query(Query::equals("inventory", 18_i64))
            .sort(Sort::asc("inventory"))
            .rank_property("vespa.matchphase.degradation.maxfiltercoverage", "0.9")
            .rank_property("vespa.matchphase.degradation.attribute", "inventory")
            .rank_property("vespa.matchphase.degradation.maxhits", "10")
            .limit(10)
            .execute::<Value>()?;
        assert!(!degraded.hits.is_empty());
        let coverage = degraded.coverage.expect("degraded result coverage");
        assert_eq!(coverage.full, Some(false));
        assert_eq!(degraded.hits.len(), 10);
        let degradation = coverage.degraded.expect("degradation reason");
        assert_eq!(degradation.match_phase, Some(true));
        assert_eq!(degradation.timeout, Some(false));
        println!("valid degraded result retains hits, coverage, and degradation reason PASS");

        let hard_failure = index
            .search("shoes")
            .sort(Sort::asc("title"))
            .execute::<Value>()
            .expect_err("sorting on a non-attribute field should be a hard semantic failure");
        assert_eq!(hard_failure.code(), ErrorCode::InvalidArgument);
        println!("invalid semantic request remains a hard operation error PASS");

        heading("20. ANALYZE / LINGUISTICS");

        let analyzed = index.analyze("Jumped shoes").field("title").execute()?;
        for token in &analyzed.tokens {
            println!(
                "  position={} surface={:?} term={:?} bytes={}..{}",
                token.position, token.surface, token.term, token.start_byte, token.end_byte
            );
        }
        assert!(analyzed.tokens.iter().any(|token| token.term == "jump"));

        heading("21. STRUCTURED FILTER ERROR");

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
        assert_eq!(
            error.details()[0].path.as_deref(),
            Some("filters.structured[0].field")
        );

        heading("21A. TEXTUAL FILTER SYNTAX ERROR");

        let error = index
            .search("shoes")
            .filter("price <= :max AND (")
            .execute::<Value>()
            .expect_err("malformed textual filter should fail in the parser");
        assert_eq!(error.code(), ErrorCode::InvalidArgument);
        let syntax_error = error
            .details()
            .iter()
            .find(|detail| detail.code == "syntax_error")
            .expect("malformed textual filter should report syntax_error");
        assert_eq!(
            syntax_error.path.as_deref(),
            Some("filters.textual[0].source")
        );
        println!("textual filter parser, clause-local bindings, grammar, and syntax errors PASS");

        heading("22. PATCH SET / UNSET");

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

        heading("23. DELETE");

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

        heading("24. UPSERT REPLACEMENT");

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
                "embedding": Tensor::dense("tensor<float>(x[2])", vec![0.0, 1.0]),
            }),
        )?;

        let doc = index.get::<Value>("shoe-2")?.expect("shoe-2 should exist");
        println!("{doc:#}");
        assert_eq!(doc["price"], json!(119.0));
        assert!(doc["title"]
            .as_str()
            .expect("title should be a string")
            .contains("Updated Edition"));
        assert_eq!(
            doc["embedding"],
            json!({
                "$searchkernel_tensor": {
                    "type_spec": "tensor<float>(x[2])",
                    "dense": [0.0, 1.0]
                }
            })
        );
        let tensor_projection = index
            .get_with_projection::<Value>("shoe-2", ["embedding"])?
            .expect("tensor projection should exist");
        assert_eq!(
            tensor_projection.as_object().map(|fields| fields.len()),
            Some(1)
        );
        assert_eq!(tensor_projection["embedding"], doc["embedding"]);

        let results = index.search("updated").limit(20).execute::<Value>()?;
        print_results(&results);
        assert_id_set(&results, ["shoe-2"]);

        heading("25. INDEX CLOSED");
        index.close()?;
        println!("The first native Index instance has shut down.");

        heading("26. REOPEN PERSISTED INDEX");

        let index = Index::open(&path)?;

        assert!(index.get::<Value>("shoe-3")?.is_none());

        let shoe1 = index
            .get::<Value>("shoe-1")?
            .expect("shoe-1 should survive reopen");
        println!("shoe-1 after reopen: {shoe1:#}");
        assert_eq!(shoe1["price"], json!(79.0));
        assert_eq!(shoe1["rating"], json!(4));
        assert!(shoe1.get("sale_price").is_none());
        assert_eq!(
            shoe1["embedding"],
            json!({
                "$searchkernel_tensor": {
                    "type_spec": "tensor<float>(x[2])",
                    "dense": [1.0, 0.0]
                }
            })
        );

        let shoe2 = index
            .get::<Value>("shoe-2")?
            .expect("shoe-2 should survive reopen");
        println!("shoe-2 after reopen: {shoe2:#}");
        assert_eq!(shoe2["price"], json!(119.0));
        assert!(shoe2["title"]
            .as_str()
            .expect("title should be a string")
            .contains("Updated Edition"));
        assert_eq!(
            shoe2["embedding"],
            json!({
                "$searchkernel_tensor": {
                    "type_spec": "tensor<float>(x[2])",
                    "dense": [0.0, 1.0]
                }
            })
        );

        let results = index
            .search("shoes")
            .filter(field("price").lte(120.0_f64))
            .sort(Sort::asc("price"))
            .limit(20)
            .execute::<Value>()?;
        print_results(&results);
        assert_ids(&results, ["shoe-1", "shoe-5", "shoe-2"]);

        let reopened_ranked = index
            .search("shoes")
            .ranking(Ranking::named("boosted"))
            .query_input("boost", 3.0_f64)
            .limit(20)
            .execute::<Value>()?;
        assert!(reopened_ranked.hits.iter().all(|hit| {
            hit.score.is_some()
                && matches!(
                    hit.features.get("query(boost)"),
                    Some(FeatureSet::Value(FeatureValue::Double(value)))
                        if (*value - 3.0).abs() < f64::EPSILON
                )
        }));

        let reopened_grouping = GroupingPlan::new().level(
            GroupingPlan::by("category")
                .output(GroupingAggregation::count())
                .output(GroupingAggregation::average("price"))
                .nested(
                    GroupingPlan::by("rating")
                        .output(GroupingAggregation::count())
                        .output(GroupingAggregation::average("price")),
                ),
        );
        let reopened_grouped = index
            .search("shoes")
            .grouping(reopened_grouping)
            .limit(0)
            .execute::<Value>()?;
        let reopened_category_root = reopened_grouped.groups.first().expect("grouping root");
        let reopened_shoes_group = reopened_category_root
            .groups
            .iter()
            .find(|group| group.key == Some(json!("shoes")))
            .expect("reopened shoes category group");
        assert_eq!(reopened_shoes_group.aggregations["count"], json!(4));
        assert_eq!(
            reopened_shoes_group.aggregations["average(price)"],
            json!(116.5)
        );
        for (rating, count, average) in [(4, 3, 115.66666666666667), (5, 1, 119.0)] {
            let rating_group = reopened_shoes_group
                .groups
                .iter()
                .find(|group| group.key == Some(json!(rating)))
                .unwrap_or_else(|| panic!("missing reopened rating group {rating}"));
            assert_eq!(rating_group.aggregations["count"], json!(count));
            let actual_average = rating_group.aggregations["average(price)"]
                .as_f64()
                .expect("reopened average should be numeric");
            assert!((actual_average - average).abs() < 1e-9);
        }

        let reopened_nearest = index
            .search("")
            .ranking(Ranking::named("semantic"))
            .typed_query(Query::nearest_neighbor("embedding", "query_embedding"))
            .query_input(
                "query_embedding",
                Tensor::dense("tensor<float>(x[2])", vec![1.0, 0.0]),
            )
            .limit(3)
            .execute::<Value>()?;
        assert_eq!(
            reopened_nearest.hits.first().map(|hit| hit.id.as_str()),
            Some("shoe-1")
        );
        println!("ranking configuration, grouping source data, tensor persistence, and ANN after reopen PASS");

        heading("27. BROWSE AFTER REOPEN");

        let results = index
            .browse()
            .filter(field("in_stock").eq(true))
            .sort(Sort::asc("price"))
            .facet("brand".into())
            // The 200 degradation probes intentionally have the lowest price,
            // so a 20-hit page cannot also contain the four expected catalog
            // documents. Request the complete filtered set for this browse
            // assertion while preserving the real native sort and facet path.
            .limit(256)
            .execute::<Value>()?;
        print_results(&results);

        let browse_ids: BTreeSet<String> = ids(&results).into_iter().collect();
        assert!(!browse_ids.contains("shoe-4"));
        assert!(!browse_ids.contains("shoe-3"));
        for expected in ["shoe-1", "shoe-2", "shoe-5", "bag-1"] {
            assert!(
                browse_ids.contains(expected),
                "missing browse hit: {expected}"
            );
        }

        index.close()?;

        heading("28. ALL CURRENT RUST CLIENT API E2E CHECKS PASSED");

        println!(
            r#"
Successfully demonstrated through the public Rust facade:

  CREATE / OPEN / PERSISTENCE
  ✓ create/open persistent index
  ✓ insert
  ✓ upsert replacement
  ✓ get
  ✓ delete + duplicate delete
  ✓ Patch set/unset
  ✓ scalar mutations, deletions, upserts, tensors, ranking, grouping, and ANN after reopen

  SEARCH / QUERY DSL
  ✓ full-text search
  ✓ stemming representative
  ✓ typed term, phrase, AND, OR, NOT, equality, range, IN, and weakAnd queries
  ✓ typed WAND, weighted-set, dot-product, fuzzy, prefix, substring, and suffix queries
  ✓ typed filters, exists, repeated filters -> AND, and explicit OR / NOT trees
  ✓ textual filter bindings, clause-local namespaces, full grammar, and syntax errors
  ✓ textual and structured equivalent filters return identical hit IDs
  ✓ sorting
  ✓ missing-first / missing-last sorting
  ✓ projection
  ✓ offset/limit
  ✓ total hits and relation
  ✓ structured unknown-field error

  RESULTS
  ✓ stored fields and tensor readback
  ✓ scores, scalar query/rank features
  ✓ facet convenience
  ✓ facet ordering request
  ✓ filtered facets
  ✓ nested grouping keys, counts, averages, and nested children
  ✓ coverage and valid match-phase degradation metadata
  ✓ invalid semantic request remains a hard operation error

  RANKING / TENSORS
  ✓ named rank profile and profile selection
  ✓ real two-phase reranking with a finite candidate window and propagated scores
  ✓ scalar query input and rank property
  ✓ tensor put/upsert/get/persistence
  ✓ tensor query input and nearest-neighbor search before and after reopen

  OTHER PUBLIC OPERATIONS
  ✓ field-aware analyze request
  ✓ search-field execution, language, timeout, and inspect

EXPLICITLY UNSUPPORTED / NOT YET PUBLIC:

  - highlight result generation (the public native path reports Unsupported)
  - sort-value readback (`Hit.sort_values`) is not exposed by the current native ABI
  - typed sameElement requires a public array-of-struct field definition, which
    is not currently public
  - typed geo and predicate require public geo/predicate field definitions,
    which are not currently public
  - typed exists query (public Filter::exists is covered)
  - YQL/property-substitution
  - opaque tensor-backed feature output
  - page() convenience, aliases/keep-filters, summary-hit grouping children,
    continuation tokens, root aggregations, and advanced update/increment APIs

Every check above uses only the public `searchkernel` facade and the real
native Vespa backend.
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
                "  id={:?} score={:?} features={:?} sort_values={:?} document={}",
                hit.id.as_str(),
                hit.score,
                hit.features,
                hit.sort_values,
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

        if !results.groups.is_empty() {
            println!("groups: {} root result group(s)", results.groups.len());
            for group in &results.groups {
                println!(
                    "  key={:?} aggregations={:?}",
                    group.key, group.aggregations
                );
            }
        }

        if let Some(coverage) = &results.coverage {
            println!(
                "coverage: percent={:?} documents={:?} full={:?} degraded={:?}",
                coverage.coverage_percent, coverage.documents, coverage.full, coverage.degraded
            );
        }

        if !results.issues.is_empty() {
            println!("issues: {:?}", results.issues);
        }
    }

    fn ids(results: &SearchResult<Value>) -> Vec<String> {
        results.hits.iter().map(|hit| hit.id.to_string()).collect()
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
