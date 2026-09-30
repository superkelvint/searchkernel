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

#![recursion_limit = "256"]

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
        QueryValue, RankProfile, Ranking, ResultValue, SchemaStructField, SearchResult, Sort,
        Tensor, TotalHitsRelation, Update, WandOptions, WeightedQueryItem,
    };
    use serde_json::{json, Value};
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::{Path, PathBuf};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    const E2E_CORPUS: &str =
        include_str!("../../../../reference/client-real-e2e-parity/corpus.json");
    const VECTOR_TYPE: &str = "tensor<float>(x[8])";

    fn corpus_documents() -> Result<Vec<(String, Value)>, Box<dyn std::error::Error>> {
        let corpus: Value = serde_json::from_str(E2E_CORPUS)?;
        let mut documents = Vec::new();
        for source in corpus["documents"]
            .as_array()
            .ok_or("corpus documents missing")?
        {
            let id = source["_id"].as_str().ok_or("corpus document id missing")?;
            let mut document = source.clone();
            document
                .as_object_mut()
                .ok_or("corpus document must be an object")?
                .remove("_id");
            if let Some(cells) = document.get("embedding").and_then(Value::as_array) {
                let values = cells
                    .iter()
                    .map(|cell| cell.as_f64().ok_or("embedding cell must be numeric"))
                    .collect::<Result<Vec<_>, _>>()?;
                document["embedding"] = serde_json::to_value(Tensor::dense(VECTOR_TYPE, values))?;
            }
            documents.push((id.to_owned(), document));
        }
        Ok(documents)
    }

    fn corpus_vector(document_id: &str) -> Result<Vec<f64>, Box<dyn std::error::Error>> {
        let corpus: Value = serde_json::from_str(E2E_CORPUS)?;
        let document = corpus["documents"]
            .as_array()
            .and_then(|documents| documents.iter().find(|doc| doc["_id"] == document_id))
            .ok_or("requested vector document missing from E2E corpus")?;
        let cells = document["embedding"]
            .as_array()
            .ok_or("corpus embedding must be a dense vector")?;
        Ok(cells
            .iter()
            .map(|cell| {
                cell.as_f64()
                    .map(|value| f64::from(value as f32))
                    .ok_or("embedding cell must be numeric")
            })
            .collect::<Result<Vec<_>, _>>()?)
    }

    fn corpus_typed_query(scenario: &str) -> Result<Query, Box<dyn std::error::Error>> {
        let corpus: Value = serde_json::from_str(E2E_CORPUS)?;
        let query = &corpus["scenario_inputs"][scenario]["query"];
        query_from_spec(query).map_err(Into::into)
    }

    fn query_from_spec(spec: &Value) -> Result<Query, String> {
        let kind = spec["kind"].as_str().ok_or("typed query kind missing")?;
        let field = || spec["field"].as_str().ok_or("typed query field missing");
        let text = |name: &str| spec[name].as_str().ok_or("typed query value missing");
        let children = || {
            spec["clauses"]
                .as_array()
                .ok_or("typed query clauses missing")?
                .iter()
                .map(query_from_spec)
                .collect::<Result<Vec<_>, _>>()
        };
        Ok(match kind {
            "contains" => Query::contains(field()?, Query::term_value(text("value")?)),
            "phrase" => Query::phrase(
                field()?,
                spec["terms"]
                    .as_array()
                    .ok_or("phrase terms missing")?
                    .iter()
                    .map(|term| term.as_str().ok_or("phrase term must be a string"))
                    .collect::<Result<Vec<_>, _>>()?
                    .join(" "),
            ),
            "near" | "ordered_near" => {
                let field = field()?;
                let terms = spec["terms"]
                    .as_array()
                    .ok_or("proximity terms missing")?
                    .iter()
                    .map(|term| {
                        term.as_str()
                            .ok_or("proximity term must be text")
                            .map(|term| Query::term(field, term))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                if kind == "near" {
                    Query::near(field, terms)
                } else {
                    Query::onear(field, terms)
                }
            }
            "range" => {
                let lower = spec["lower"].as_f64().ok_or("range lower bound missing")?;
                let upper = spec["upper"].as_f64().ok_or("range upper bound missing")?;
                Query::range(
                    field()?,
                    QueryBound::Included(lower.into()),
                    QueryBound::Included(upper.into()),
                )
            }
            "and" => Query::and(children()?),
            "or" => Query::or(children()?),
            "not" => Query::not(query_from_spec(&spec["clause"])?),
            "in" => Query::in_values(
                field()?,
                spec["values"]
                    .as_array()
                    .ok_or("IN values missing")?
                    .iter()
                    .map(|value| {
                        value
                            .as_str()
                            .map(QueryValue::from)
                            .ok_or("IN value must be a string")
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            "equals" => Query::equals(field()?, text("value")?),
            "fuzzy" => Query::fuzzy(field()?, text("value")?),
            "weak_and" => Query::weak_and(children()?, 100),
            "wand" | "weighted_set" | "dot_product" => {
                let items = spec["items"]
                    .as_object()
                    .ok_or("weighted query items missing")?
                    .iter()
                    .map(|(key, weight)| {
                        Ok(WeightedQueryItem::new(
                            key.as_str(),
                            weight
                                .as_i64()
                                .ok_or("weighted query weight must be an integer")?,
                        ))
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                match kind {
                    "wand" => Query::wand(field()?, items),
                    "weighted_set" => Query::weighted_set(field()?, items),
                    _ => Query::dot_product(field()?, items),
                }
            }
            "same_element" => Query::same_element(field()?, children()?),
            "geo_location" => Query::geo_location(
                field()?,
                spec["latitude"].as_f64().ok_or("geo latitude missing")?,
                spec["longitude"].as_f64().ok_or("geo longitude missing")?,
                spec["radius"].as_str().ok_or("geo radius missing")?,
            ),
            "geo_box" => Query::geo_bounding_box(
                field()?,
                spec["south"].as_f64().ok_or("geo south missing")?,
                spec["west"].as_f64().ok_or("geo west missing")?,
                spec["north"].as_f64().ok_or("geo north missing")?,
                spec["east"].as_f64().ok_or("geo east missing")?,
            ),
            _ => return Err(format!("unsupported typed query intent: {kind}")),
        })
    }

    fn corpus_definition() -> Result<IndexDefinition, Box<dyn std::error::Error>> {
        let corpus: Value = serde_json::from_str(E2E_CORPUS)?;
        let schema = &corpus["schema"];
        let mut definition = IndexDefinition::new();
        for (name, spec) in schema["fields"]
            .as_object()
            .ok_or("corpus fields missing")?
        {
            let type_name = spec["type"].as_str().ok_or("field type missing")?;
            let mut field = match type_name {
                "text" => Field::text(name),
                "keyword" => Field::keyword(name),
                "float64" => Field::float64(name),
                "int64" => Field::int64(name),
                "bool" => Field::bool(name),
                "position" => Field::position(name),
                "array" => {
                    let element = &spec["element"];
                    if element["type"] != "struct" {
                        return Err(format!("unsupported corpus array element for {name}").into());
                    }
                    let members = element["fields"]
                        .as_object()
                        .ok_or("corpus struct fields missing")?
                        .iter()
                        .map(|(member, member_spec)| {
                            let data_type = match member_spec["type"].as_str() {
                                Some("keyword" | "text") => searchkernel::DataType::Primitive(
                                    searchkernel::PrimitiveDataType::String,
                                ),
                                Some("int64") => searchkernel::DataType::Primitive(
                                    searchkernel::PrimitiveDataType::Int64,
                                ),
                                _ => {
                                    return Err(
                                        format!("unsupported corpus struct field {member}").into()
                                    )
                                }
                            };
                            Ok(SchemaStructField {
                                name: member.clone(),
                                data_type,
                            })
                        })
                        .collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()?;
                    Field::array_of_struct(name, members)
                }
                tensor if tensor.starts_with("tensor<") => Field::tensor(name, tensor),
                _ => return Err(format!("unknown corpus field type: {type_name}").into()),
            };
            if let Some(value) = spec.get("stored").and_then(Value::as_bool) {
                field = field.stored(value);
            }
            if let Some(value) = spec.get("searchable").and_then(Value::as_bool) {
                field = field.searchable(value);
            }
            if let Some(value) = spec.get("filterable").and_then(Value::as_bool) {
                field = field.filterable(value);
            }
            if let Some(value) = spec.get("sortable").and_then(Value::as_bool) {
                field = field.sortable(value);
            }
            if let Some(value) = spec.get("facetable").and_then(Value::as_bool) {
                field = field.facetable(value);
            }
            definition = definition.field(field);
        }
        let default_fields = schema["default_search_fields"]
            .as_array()
            .ok_or("default search fields missing")?
            .iter()
            .map(|value| value.as_str().ok_or("search field must be text"))
            .collect::<Result<Vec<_>, _>>()?;
        definition = definition.default_search_fields(default_fields);
        for spec in schema["rank_profiles"]
            .as_array()
            .ok_or("rank profiles missing")?
        {
            let name = spec["name"].as_str().ok_or("rank profile name missing")?;
            let first_phase = spec["first_phase"]
                .as_str()
                .ok_or("rank profile first phase missing")?;
            let mut profile = RankProfile::new(name).first_phase(first_phase);
            if let Some(second_phase) = spec.get("second_phase").and_then(Value::as_str) {
                profile = profile.second_phase(second_phase);
            }
            if let Some(count) = spec.get("rerank_count").and_then(Value::as_u64) {
                profile = profile.rerank_count(count as u32);
            }
            if let Some(inputs) = spec.get("query_inputs").and_then(Value::as_object) {
                for (input_name, type_spec) in inputs {
                    profile = profile.query_input(
                        input_name,
                        type_spec.as_str().ok_or("query input type must be text")?,
                    );
                }
            }
            if let Some(features) = spec.get("match_features").and_then(Value::as_array) {
                for feature in features {
                    profile = profile
                        .match_feature(feature.as_str().ok_or("match feature must be text")?);
                }
            }
            definition = definition.rank_profile(profile);
        }
        Ok(definition)
    }

    pub fn run() -> Result<(), Box<dyn std::error::Error>> {
        // Keep all showcase state, including any sibling lock files created by
        // Search Kernel, underneath one unique temporary directory. The guard
        // removes the whole directory when the example exits.
        let temp = TempWorkspace::new()?;
        let path = temp.index_path();

        println!("temporary workspace: {}", temp.root().display());

        heading("1. CREATE INDEX");

        let definition = corpus_definition()?;

        let index = Index::create(&path, definition)?;
        println!("created: {}", path.display());

        heading("2. INSERT DOCUMENTS");

        let corpus = corpus_documents()?;
        for (id, document) in &corpus {
            index.insert(id, document)?;
            println!("insert: {id}");
        }
        let corpus: Value = serde_json::from_str(E2E_CORPUS)?;
        let probe_count = corpus["coverage_probes"]["count"]
            .as_u64()
            .ok_or("coverage probe count missing")?;
        let probe_template = &corpus["coverage_probes"]["document"];
        let probe_vector = probe_template["embedding"]
            .as_array()
            .ok_or("coverage probe embedding missing")?
            .iter()
            .map(|cell| cell.as_f64().ok_or("embedding cell must be numeric"))
            .collect::<Result<Vec<_>, _>>()?;
        for probe in 0..probe_count {
            let mut document = probe_template.clone();
            document["embedding"] =
                serde_json::to_value(Tensor::dense(VECTOR_TYPE, probe_vector.clone()))?;
            index.insert(format!("degraded-{probe}"), &document)?;
        }
        println!("inserted 200 deterministic coverage probe documents");

        let parity_sorted = index
            .search("shoes")
            .sort(Sort::asc("price"))
            .limit(20)
            .execute::<Value>()?;
        let parity_filtered = index
            .search("shoes")
            .filter(field("in_stock").eq(true))
            .sort(Sort::asc("price"))
            .limit(20)
            .execute::<Value>()?;
        let parity_faceted = index
            .search("shoes")
            .facet("brand".into())
            .limit(20)
            .execute::<Value>()?;
        let scenario_inputs: Value = serde_json::from_str(E2E_CORPUS)?;
        let nearest_intent = &scenario_inputs["scenario_inputs"]["nearest_neighbor"];
        let parity_nearest = index
            .search("")
            .ranking(Ranking::named(
                nearest_intent["ranking_profile"]
                    .as_str()
                    .ok_or("nearest ranking profile missing")?,
            ))
            .typed_query(Query::nearest_neighbor(
                nearest_intent["query"]["field"]
                    .as_str()
                    .ok_or("nearest field missing")?,
                nearest_intent["query"]["query_input"]
                    .as_str()
                    .ok_or("nearest query input missing")?,
            ))
            .query_input(
                nearest_intent["query"]["query_input"]
                    .as_str()
                    .ok_or("nearest query input missing")?,
                Tensor::dense(
                    VECTOR_TYPE,
                    corpus_vector(
                        nearest_intent["query_input_document"]
                            .as_str()
                            .ok_or("nearest vector document missing")?,
                    )?,
                ),
            )
            .limit(
                nearest_intent["limit"]
                    .as_u64()
                    .ok_or("nearest limit missing")? as u32,
            )
            .execute::<Value>()?;
        let parity_reranked = index
            .search("candidate")
            .ranking(Ranking::named("two_phase"))
            .limit(4)
            .execute::<Value>()?;
        let parity_error = index
            .search("shoes")
            .filter("prcie = 1")
            .execute::<Value>()
            .expect_err("unknown field must fail");
        let parity_document = index.get::<Value>("shoe-1")?.expect("shoe-1 exists");
        let parity_full_text = index.search("shoes").limit(20).execute::<Value>()?;
        let parity_stemming = index.search("jump").limit(20).execute::<Value>()?;
        let parity_text_binding = index
            .search("shoes")
            .filter(("price <= :max", params! { "max" => 100.0_f64 }))
            .sort(Sort::asc("price"))
            .limit(20)
            .execute::<Value>()?;
        let parity_repeated = index
            .search("shoes")
            .filter(("price <= :value", params! { "value" => 120.0_f64 }))
            .filter(("rating >= :value", params! { "value" => 4_i64 }))
            .filter("in_stock = true")
            .sort(Sort::asc("price"))
            .limit(20)
            .execute::<Value>()?;
        let parity_boolean = index
            .search("shoes")
            .filter((
                "(brand IN (:brands) OR rating = :rating) AND NOT (in_stock = false)",
                params! { "brands" => vec!["Acme", "TrailCo"], "rating" => 3_i64 },
            ))
            .sort(Sort::asc("price"))
            .limit(20)
            .execute::<Value>()?;
        let parity_projection = index
            .search("shoes")
            .sort(Sort::desc("price"))
            .projection(["title", "brand", "price"])
            .limit(2)
            .execute::<Value>()?;
        let parity_all_sorted = index
            .search("shoes")
            .sort(Sort::asc("price"))
            .limit(20)
            .execute::<Value>()?;
        let parity_page = index
            .search("shoes")
            .sort(Sort::asc("price"))
            .offset(1)
            .limit(2)
            .execute::<Value>()?;
        let parity_filtered_facet = index
            .search("shoes")
            .filter(field("in_stock").eq(true))
            .facet(Facet::new("brand").limit(10).sort(FacetSort::CountDesc))
            .limit(20)
            .execute::<Value>()?;
        let parity_grouped = index
            .search("shoes")
            .grouping(
                GroupingPlan::new().level(
                    GroupingPlan::by("brand")
                        .output(GroupingAggregation::count())
                        .output(GroupingAggregation::average("price")),
                ),
            )
            .execute::<Value>()?;
        let parity_nested = index
            .browse()
            .filter((
                "category IN (:categories)",
                params! { "categories" => vec!["shoes", "bags"] },
            ))
            .grouping(
                GroupingPlan::new().level(
                    GroupingPlan::by("category")
                        .output(GroupingAggregation::count())
                        .nested(
                            GroupingPlan::by("rating")
                                .output(GroupingAggregation::count())
                                .output(GroupingAggregation::average("price")),
                        ),
                ),
            )
            .execute::<Value>()?;
        let parity_root_aggregations = index
            .browse()
            .filter("category = 'shoes'")
            .grouping(
                GroupingPlan::new()
                    .root_output(GroupingAggregation::named(
                        "products",
                        GroupingAggregation::count(),
                    ))
                    .root_output(GroupingAggregation::named(
                        "sum_price",
                        GroupingAggregation::sum("price"),
                    ))
                    .root_output(GroupingAggregation::named(
                        "avg_price",
                        GroupingAggregation::average("price"),
                    )),
            )
            .limit(0)
            .execute::<Value>()?;
        let parity_alias = index
            .search("shoes")
            .grouping(
                GroupingPlan::new().level(
                    GroupingPlan::by("brand")
                        .alias("revenue", GroupingAggregation::sum("price"))
                        .order_by_alias("revenue", true)
                        .output(GroupingAggregation::named(
                            "products",
                            GroupingAggregation::count(),
                        ))
                        .output(GroupingAggregation::named(
                            "revenue",
                            GroupingAggregation::sum("price"),
                        )),
                ),
            )
            .execute::<Value>()?;
        let parity_keep = index
            .search("shoes")
            .grouping(
                GroupingPlan::new().level(
                    GroupingPlan::by("brand")
                        .keep_regex("brand", "^A.*")
                        .output(GroupingAggregation::named(
                            "products",
                            GroupingAggregation::count(),
                        )),
                ),
            )
            .execute::<Value>()?;
        let parity_yql = index
            .yql("select * from sources * where title contains text(@q) order by price asc")
            .yql_parameter("q", "shoes")
            .limit(3)
            .execute::<Value>()?;
        let page_limit_rejected = index
            .search("shoes")
            .page(2, 20)
            .limit(20)
            .execute::<Value>()
            .is_err();
        let page_offset_rejected = index
            .search("shoes")
            .page(2, 20)
            .offset(20)
            .execute::<Value>()
            .is_err();
        let typed_scenarios = [
            "typed_query_contains",
            "typed_query_phrase",
            "typed_query_near",
            "typed_query_ordered_near",
            "typed_query_range",
            "typed_query_boolean",
            "typed_query_in",
            "typed_query_equals",
            "typed_query_fuzzy",
            "typed_query_weak_and",
            "typed_query_wand",
            "typed_query_weighted_set",
            "typed_query_dot_product",
            "typed_query_same_element",
            "typed_query_geo_location",
            "typed_query_geo_box",
        ];
        let mut typed_observations = serde_json::Map::new();
        for scenario in typed_scenarios {
            let mut results = index
                .search("")
                .typed_query(corpus_typed_query(scenario)?)
                .limit(100)
                .execute::<Value>()?;
            let corpus: Value = serde_json::from_str(E2E_CORPUS)?;
            let input = corpus["scenario_inputs"][scenario].clone();
            if input["result_order"] == "sorted_ids" {
                results.hits.sort_by(|left, right| left.id.cmp(&right.id));
            }
            typed_observations.insert(
                scenario.to_owned(),
                json!({
                    "observation": {"intent": input["query"], "query_result": ids(&results)},
                    "capabilities": corpus["client_scenario_contract"][scenario]["rust"],
                }),
            );
        }
        let parity_rank_input = index
            .search("shoes")
            .ranking(Ranking::named("boosted"))
            .query_input("boost", 3.0_f64)
            .limit(20)
            .execute::<Value>()?;
        let parity_rank_property = index
            .search("shoes")
            .ranking(Ranking::named("boosted"))
            .rank_property("boost", "4.0")
            .limit(20)
            .execute::<Value>()?;
        let hybrid_intent = &scenario_inputs["scenario_inputs"]["hybrid_text_vector"];
        let hybrid_query = &hybrid_intent["query"];
        let nearest_clause = hybrid_query["clauses"]
            .as_array()
            .ok_or("hybrid clauses missing")?
            .iter()
            .find(|clause| clause["kind"] == "nearest_neighbor")
            .ok_or("hybrid nearest clause missing")?;
        let text_clause = hybrid_query["clauses"]
            .as_array()
            .ok_or("hybrid clauses missing")?
            .iter()
            .find(|clause| clause["kind"] == "contains")
            .ok_or("hybrid text clause missing")?;
        let parity_hybrid = index
            .search("")
            .ranking(Ranking::named(
                hybrid_intent["ranking_profile"]
                    .as_str()
                    .ok_or("hybrid ranking profile missing")?,
            ))
            .typed_query(Query::or([
                Query::contains(
                    text_clause["field"]
                        .as_str()
                        .ok_or("hybrid text field missing")?,
                    Query::term_value(
                        text_clause["value"]
                            .as_str()
                            .ok_or("hybrid text value missing")?,
                    ),
                ),
                Query::nearest_neighbor(
                    nearest_clause["field"]
                        .as_str()
                        .ok_or("hybrid vector field missing")?,
                    nearest_clause["query_input"]
                        .as_str()
                        .ok_or("hybrid query input missing")?,
                ),
            ]))
            .query_input(
                nearest_clause["query_input"]
                    .as_str()
                    .ok_or("hybrid query input missing")?,
                Tensor::dense(
                    VECTOR_TYPE,
                    corpus_vector(
                        hybrid_intent["query_input_document"]
                            .as_str()
                            .ok_or("hybrid vector document missing")?,
                    )?,
                ),
            )
            .limit(
                hybrid_intent["limit"]
                    .as_u64()
                    .ok_or("hybrid limit missing")? as u32,
            )
            .execute::<Value>()?;
        let parity_group_buckets = direct_group_buckets(&parity_grouped.groups);
        let mut parity_grouping_aggregations = parity_group_buckets
            .into_iter()
            .map(parity_grouping_aggregation)
            .collect::<Vec<_>>();
        parity_grouping_aggregations.sort_by(|left, right| {
            right["products"]
                .as_u64()
                .cmp(&left["products"].as_u64())
                .then_with(|| left["brand"].as_str().cmp(&right["brand"].as_str()))
        });
        let mut parity_observations = json!({
            "document_crud": {"id": "shoe-1", "title": parity_document["title"], "brand": parity_document["brand"], "price": parity_document["price"]},
            "full_text_search": {"ids": ids(&parity_full_text), "total": parity_full_text.total_hits.value},
            "stemming": {"ids": ids(&parity_stemming)},
            "text_filter_binding": {"ids": ids(&parity_text_binding)},
            "repeated_filters": {"ids": ids(&parity_repeated)},
            "boolean_filter_grammar": {"ids": ids(&parity_boolean)},
            "sorting_projection": {"hits": parity_projection.hits.iter().map(|hit| json!({"id": hit.id.as_str(), "fields": hit.document})).collect::<Vec<_>>()},
            "pagination": {"all_ids": ids(&parity_all_sorted), "offset_ids": ids(&parity_page), "total": parity_page.total_hits.value},
            "builder_misuse": {
                "observation": {"page_and_limit_rejected": page_limit_rejected, "page_and_offset_rejected": page_offset_rejected},
                "capabilities": {
                    "negative_limit_rejected": {"kind": "not_expressible", "reason": "SearchBuilder::limit accepts u32, so a negative limit cannot be represented."},
                    "negative_offset_rejected": {"kind": "not_expressible", "reason": "SearchBuilder::offset accepts u32, so a negative offset cannot be represented."}
                }
            },
            "sorted_search": {"ids": ids(&parity_sorted), "total": parity_sorted.total_hits.value},
            "filtered_search": {"ids": ids(&parity_filtered), "total": parity_filtered.total_hits.value},
            "facets": facet_counts(&parity_faceted, "brand"),
            "root_aggregations": parity_root_aggregations.aggregations.iter().map(|(key, value)| (key.clone(), parity_value(value))).collect::<serde_json::Map<_, _>>(),
            "filtered_facets": facet_counts(&parity_filtered_facet, "brand"),
            "grouping_aggregations": parity_grouping_aggregations,
            "nested_grouping": direct_group_buckets(&parity_nested.groups).into_iter()
                .map(parity_nested_group)
                .collect::<Vec<_>>(),
            "group_alias": direct_group_buckets(&parity_alias.groups).into_iter()
                .map(|group| json!({"value": group.key.as_ref().map(parity_value), "fields": group.aggregations.iter().map(|(key, value)| (key.clone(), parity_value(value))).collect::<serde_json::Map<_, _>>() })).collect::<Vec<_>>(),
            "group_keep_filter": direct_group_buckets(&parity_keep.groups).into_iter()
                .map(|group| json!({"value": group.key.as_ref().map(parity_value), "fields": group.aggregations.iter().map(|(key, value)| (key.clone(), parity_value(value))).collect::<serde_json::Map<_, _>>() })).collect::<Vec<_>>(),
            "typed_query_contains": typed_observations["typed_query_contains"],
            "typed_query_phrase": typed_observations["typed_query_phrase"],
            "typed_query_range": typed_observations["typed_query_range"],
            "typed_query_boolean": typed_observations["typed_query_boolean"],
            "typed_query_in": typed_observations["typed_query_in"],
            "typed_query_equals": typed_observations["typed_query_equals"],
            "typed_query_fuzzy": typed_observations["typed_query_fuzzy"],
            "typed_query_weak_and": typed_observations["typed_query_weak_and"],
            "typed_query_wand": typed_observations["typed_query_wand"],
            "typed_query_weighted_set": typed_observations["typed_query_weighted_set"],
            "typed_query_dot_product": typed_observations["typed_query_dot_product"],
            "typed_query_same_element": typed_observations["typed_query_same_element"],
            "typed_query_geo_location": typed_observations["typed_query_geo_location"],
            "typed_query_geo_box": typed_observations["typed_query_geo_box"],
            "yql": {"ids": ids(&parity_yql), "total": parity_yql.total_hits.value},
            "rank_inputs": parity_rank_input.hits.iter().map(|hit| json!({"id": hit.id.as_str(), "score": hit.score, "feature": parity_feature(hit, "query(boost)")})).collect::<Vec<_>>(),
            "rank_properties": parity_rank_property.hits.iter().map(|hit| json!({"id": hit.id.as_str(), "score": hit.score, "feature": parity_feature(hit, "query(boost)")})).collect::<Vec<_>>(),
            "tensor_readback": parity_document["embedding"]["$searchkernel_tensor"]["dense"],
            "nearest_neighbor": {"observation": {"intent": nearest_intent, "ids": ids(&parity_nearest), "scores": parity_nearest.hits.iter().map(|hit| hit.score).collect::<Vec<_>>()}, "capabilities": {}},
            "two_phase_reranking": {"ids": ids(&parity_reranked)},
            "hybrid_text_vector": {"observation": {"intent": hybrid_intent, "ids": ids(&parity_hybrid), "top_score": parity_hybrid.hits.first().and_then(|hit| hit.score)}, "capabilities": {}},
            "two_phase": {"ids": ids(&parity_reranked)},
            "unknown_field_error": {
                "code": "invalid_argument",
                "details": parity_error.details().iter().map(|detail| detail.code.as_str()).collect::<Vec<_>>()
            }
        });

        heading("3. GET BY ID");

        let doc = index.get::<Value>("shoe-1")?.expect("shoe-1 should exist");
        println!("{doc:#}");
        assert_eq!(doc["price"], json!(89.0));
        assert_eq!(doc["brand"], json!("Acme"));
        assert_eq!(
            doc["embedding"],
            json!({
                "$searchkernel_tensor": { "type_spec": VECTOR_TYPE, "dense": corpus_vector("shoe-1")? }
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
            results.hits[0].document.as_ref().unwrap()["title"],
            json!("Acme Racing Shoes")
        );
        assert_eq!(
            results.hits[0].document.as_ref().unwrap()["brand"],
            json!("Acme")
        );
        assert_eq!(
            results.hits[0].document.as_ref().unwrap()["price"],
            json!(159.0)
        );
        assert!(results.hits[0]
            .document
            .as_ref()
            .unwrap()
            .get("rating")
            .is_none());
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
                    match &value.value {
                        ResultValue::String(value) => value.as_str(),
                        _ => panic!("brand facet value should be a string"),
                    },
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
            .find(|group| {
                group
                    .key
                    .as_ref()
                    .is_some_and(|key| parity_value(key) == json!("shoes"))
            })
            .expect("shoes category group");
        assert_eq!(parity_value(&shoes_group.aggregations["count"]), json!(5));
        assert_eq!(
            parity_value(&shoes_group.aggregations["average(price)"]),
            json!(107.0)
        );
        assert_eq!(shoes_group.groups.len(), 3);
        for (rating, count, average) in [(3, 1, 49.0), (4, 2, 134.0), (5, 2, 109.0)] {
            let rating_group = shoes_group
                .groups
                .iter()
                .find(|group| {
                    group
                        .key
                        .as_ref()
                        .is_some_and(|key| parity_value(key) == json!(rating))
                })
                .unwrap_or_else(|| panic!("missing rating group {rating}"));
            assert_eq!(
                parity_value(&rating_group.aggregations["count"]),
                json!(count)
            );
            assert_eq!(
                parity_value(&rating_group.aggregations["average(price)"]),
                json!(average)
            );
        }
        println!("nested grouping keys, count, and average values PASS");

        heading("17. TENSOR READBACK + NEAREST NEIGHBOR");

        let nearest = index
            .search("")
            .ranking(Ranking::named("semantic"))
            .typed_query(Query::nearest_neighbor("embedding", "query_embedding"))
            .query_input(
                "query_embedding",
                Tensor::dense(VECTOR_TYPE, corpus_vector("shoe-1")?),
            )
            .limit(3)
            .execute::<Value>()?;
        assert_eq!(
            nearest.hits.first().map(|hit| hit.id.as_str()),
            Some("shoe-1")
        );
        assert!(nearest.coverage.is_some());
        println!("tensor query input and native nearest-neighbor search PASS");

        heading("18. PUBLIC OPTIONS AND VESPA HIGHLIGHTING");

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
        let highlighted = index
            .search("shoes")
            .summary("default")
            .projection(["category"])
            .highlight(["title"])
            .execute::<Value>()?;
        assert!(!highlighted.hits.is_empty());
        assert_eq!(
            highlighted.hits[0].document,
            Some(json!({"category": "shoes"}))
        );
        assert_eq!(highlighted.hits[0].highlights.len(), 1);
        assert_eq!(highlighted.hits[0].highlights[0].field, "title");
        assert!(!highlighted.hits[0].highlights[0].fragments[0]
            .matches
            .is_empty());
        for query in [
            Query::fuzzy("title", "runnng"),
            Query::prefix("title", "Run"),
            Query::substring("title", "unn"),
            Query::suffix("title", "ing"),
        ] {
            let error = index
                .search("")
                .typed_query(query)
                .highlight(["title"])
                .execute::<Value>()
                .expect_err("same-field unsupported query shapes must fail before native search");
            assert_eq!(error.code(), ErrorCode::Unsupported);
            assert!(error
                .details()
                .iter()
                .any(|detail| detail.code == "highlighting_query_not_supported"));
        }
        let mixed_highlighting = index
            .search("")
            .typed_query(Query::and([
                Query::term("title", "running"),
                Query::in_values("category", [QueryValue::from("shoes")]),
            ]))
            .highlight(["title"])
            .execute::<Value>()?;
        assert_id_set(&mixed_highlighting, ["shoe-1", "shoe-2"]);
        assert_eq!(mixed_highlighting.hits[0].highlights.len(), 1);
        assert!(!mixed_highlighting.hits[0].highlights[0].fragments[0]
            .matches
            .is_empty());
        println!(
            "search_fields/language/timeout/inspect/missing-first, field-aware highlighting, and Vespa highlighting PASS"
        );

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
        let coverage = degraded
            .coverage
            .as_ref()
            .expect("degraded result coverage");
        assert_eq!(coverage.full, Some(false));
        assert_eq!(degraded.hits.len(), 10);
        parity_observations["coverage_degradation"] = json!({
            "ids": ids(&degraded),
            "full": coverage.full,
            "degraded": coverage.degraded.is_some()
        });
        let degradation = coverage.degraded.as_ref().expect("degradation reason");
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
            Some("filters.structured[0].comparison.field")
        );
        parity_observations["structured_filter_error"] = json!({
            "code": "invalid_argument",
            "details": error.details().iter().map(|detail| detail.code.as_str()).collect::<Vec<_>>()
        });

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
        parity_observations["text_filter_syntax_error"] = json!({
            "code": "invalid_argument",
            "details": error.details().iter().map(|detail| detail.code.as_str()).collect::<Vec<_>>()
        });
        println!("textual filter parser, clause-local bindings, grammar, and syntax errors PASS");

        heading("22. PATCH SET / UNSET");

        index.insert("parity-mutation", &json!({"title": "Parity Mutation", "category": "parity", "price": 100.0, "sale_price": 110.0, "rating": 1, "inventory": 1, "in_stock": true}))?;
        index.patch(
            "parity-mutation",
            Patch::new().set("price", 75.0_f64).unset("sale_price"),
        )?;
        let parity_mutated = index
            .get::<Value>("parity-mutation")?
            .expect("parity mutation should exist");
        parity_observations["patch"] = json!({
            "price": parity_mutated["price"],
            "sale_price_present": parity_mutated.get("sale_price").is_some()
        });
        let parity_deleted = index.delete("parity-mutation")?;
        parity_observations["delete"] = json!({
            "deleted": parity_deleted,
            "absent_after_delete": index.get::<Value>("parity-mutation")?.is_none()
        });
        index.insert(
            "parity-advanced-update",
            &json!({"title": "before", "inventory": 10, "sale_price": 1.0}),
        )?;
        index.update(
            "parity-advanced-update",
            Update::new()
                .set("title", "after")
                .unset("sale_price")
                .increment("inventory", 3.0),
        )?;
        index.update(
            "parity-advanced-update",
            Update::new().decrement("inventory", 1.0),
        )?;
        index.update(
            "parity-advanced-update",
            Update::new().multiply("inventory", 2.0),
        )?;
        index.update(
            "parity-advanced-update",
            Update::new().divide("inventory", 4.0),
        )?;
        let advanced_update = index
            .get::<Value>("parity-advanced-update")?
            .expect("updated document remains");
        parity_observations["advanced_update"] = json!({
            "title": advanced_update["title"],
            "inventory": advanced_update["inventory"],
            "sale_price_present": advanced_update.get("sale_price").is_some()
        });
        index.insert("parity-reopen", &json!({"title": "Parity Reopen", "category": "parity", "price": 42.0, "inventory": 7, "in_stock": true}))?;

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
                "embedding": Tensor::dense(VECTOR_TYPE, corpus_vector("shoe-3")?),
            }),
        )?;

        let doc = index.get::<Value>("shoe-2")?.expect("shoe-2 should exist");
        let upsert_title_matches = doc["title"]
            .as_str()
            .is_some_and(|title| title.contains("Updated Edition"));
        let upsert_search = index.search("updated").limit(20).execute::<Value>()?;
        parity_observations["upsert_replacement"] = json!({
            "price": doc["price"],
            "updated_title_matches": upsert_title_matches,
            "search_ids": ids(&upsert_search)
        });
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
                    "type_spec": VECTOR_TYPE,
                    "dense": corpus_vector("shoe-3")?
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
                    "type_spec": VECTOR_TYPE,
                    "dense": corpus_vector("shoe-1")?
                }
            })
        );
        let parity_persisted = index
            .get::<Value>("parity-reopen")?
            .expect("parity document should survive reopen");
        let persisted_shoe2 = index
            .get::<Value>("shoe-2")?
            .expect("shoe-2 should survive reopen");
        parity_observations["reopen_persistence"] = json!({
            "marker": {
                "title": parity_persisted["title"],
                "price": parity_persisted["price"],
                "inventory": parity_persisted["inventory"]
            },
            "upserted": {
                "updated_title_matches": persisted_shoe2["title"].as_str().is_some_and(|title| title.contains("Updated Edition")),
                "price": persisted_shoe2["price"]
            },
            "deleted_absent": index.get::<Value>("parity-mutation")?.is_none()
        });
        if let Some(output) = std::env::var_os("SEARCHKERNEL_REAL_E2E_PARITY_OUTPUT") {
            let snapshot = json!({"version": 1, "scenarios": parity_observations});
            std::fs::write(output, serde_json::to_vec_pretty(&snapshot)?)?;
        }

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
                    "type_spec": VECTOR_TYPE,
                    "dense": corpus_vector("shoe-3")?
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
            .find(|group| {
                group
                    .key
                    .as_ref()
                    .is_some_and(|key| parity_value(key) == json!("shoes"))
            })
            .expect("reopened shoes category group");
        assert_eq!(
            parity_value(&reopened_shoes_group.aggregations["count"]),
            json!(4)
        );
        assert_eq!(
            parity_value(&reopened_shoes_group.aggregations["average(price)"]),
            json!(116.5)
        );
        for (rating, count, average) in [(4, 3, 115.66666666666667), (5, 1, 119.0)] {
            let rating_group = reopened_shoes_group
                .groups
                .iter()
                .find(|group| {
                    group
                        .key
                        .as_ref()
                        .is_some_and(|key| parity_value(key) == json!(rating))
                })
                .unwrap_or_else(|| panic!("missing reopened rating group {rating}"));
            assert_eq!(
                parity_value(&rating_group.aggregations["count"]),
                json!(count)
            );
            let actual_average = match &rating_group.aggregations["average(price)"] {
                ResultValue::Float64(value) => *value,
                _ => panic!("reopened average should be numeric"),
            };
            assert!((actual_average - average).abs() < 1e-9);
        }

        let reopened_nearest = index
            .search("")
            .ranking(Ranking::named("semantic"))
            .typed_query(Query::nearest_neighbor("embedding", "query_embedding"))
            .query_input(
                "query_embedding",
                Tensor::dense(VECTOR_TYPE, corpus_vector("shoe-1")?),
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
  ✓ field-aware highlighting compatibility for fuzzy and regexp-family queries
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

ADDITIONAL SHARED CLIENT-PARITY SCENARIOS:

  ✓ raw YQL through the public facade
  ✓ root aggregations and grouping output aliases / keep filters
  ✓ page-builder conflict validation
  ✓ set, unset, increment, decrement, multiply, and divide updates

REMAINING LIMITATIONS:

  - sort-value readback (`Hit.sort_values`) is not exposed by the current native ABI
  - typed exists query (public Filter::exists is covered)
  - opaque tensor-backed feature output
  - summary-hit grouping children and continuation tokens

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

    fn parity_value(value: &ResultValue) -> Value {
        match value {
            ResultValue::Null => Value::Null,
            ResultValue::Bool(value) => json!(value),
            ResultValue::Int64(value) => json!(value),
            ResultValue::UInt64(value) => json!(value),
            ResultValue::Float64(value) => json!(value),
            ResultValue::String(value) => json!(value),
            ResultValue::Bytes(value) => json!(value),
            ResultValue::List(values) => Value::Array(values.iter().map(parity_value).collect()),
            ResultValue::Object(values) => Value::Object(
                values
                    .iter()
                    .map(|(key, value)| (key.clone(), parity_value(value)))
                    .collect(),
            ),
            ResultValue::NonFiniteFloat(value) => json!(format!("{value:?}")),
            ResultValue::Tensor(value) => json!({"type_spec": value.type_spec}),
            _ => panic!("unsupported result value variant in parity snapshot"),
        }
    }

    fn parity_aggregation(
        group: &searchkernel::GroupingResult<Value>,
        name: &str,
    ) -> Option<Value> {
        group.aggregations.get(name).map(parity_value)
    }

    fn direct_group_buckets<T>(
        groups: &[searchkernel::GroupingResult<T>],
    ) -> Vec<&searchkernel::GroupingResult<T>> {
        if let [root] = groups {
            if root.key.is_none() && root.aggregations.is_empty() {
                return root.groups.iter().collect();
            }
        }
        groups.iter().collect()
    }

    fn parity_grouping_aggregation(group: &searchkernel::GroupingResult<Value>) -> Value {
        json!({
            "brand": group.key.as_ref().map(parity_value),
            "products": parity_aggregation(group, "count"),
            "avg_price": parity_aggregation(group, "average(price)"),
        })
    }

    fn parity_nested_group(group: &searchkernel::GroupingResult<Value>) -> Value {
        json!({
            "category": group.key.as_ref().map(parity_value),
            "products": parity_aggregation(group, "count"),
            "ratings": group.groups.iter().map(|rating| json!({
                "rating": rating.key.as_ref().map(parity_value),
                "products": parity_aggregation(rating, "count"),
                "avg_price": parity_aggregation(rating, "average(price)"),
            })).collect::<Vec<_>>(),
        })
    }

    fn parity_feature(hit: &Hit<Value>, name: &str) -> Option<f64> {
        match hit.features.get(name) {
            Some(FeatureSet::Value(FeatureValue::Double(value))) => Some(*value),
            _ => None,
        }
    }

    fn print_results(results: &SearchResult<Value>) {
        println!(
            "total_hits = {} relation = {:?}",
            results.total_hits.value, results.total_hits.relation
        );

        for hit in &results.hits {
            println!(
                "  id={:?} score={:?} features={:?} sort_values={:?} document={:?}",
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
                let key = &value.value;
                let key = match key {
                    ResultValue::String(value) => value.clone(),
                    _ => panic!("facet {field:?} returned non-string value"),
                };
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
