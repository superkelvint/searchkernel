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
        GroupingAggregation, GroupingLevel, GroupingOrder, GroupingPlan, Hit, Index,
        IndexDefinition, Patch, Query, QueryBound, QueryValue, RankProfile, Ranking, ResultValue,
        SchemaStructField, SearchResult, Sort, Tensor, TotalHitsRelation, Update, WandOptions,
        WeightedQueryItem,
    };
    use serde_json::{json, Value};
    use std::cell::RefCell;
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::{Path, PathBuf};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    const E2E_CORPUS: &str =
        include_str!("../../../../reference/client-real-e2e-parity/corpus.json");
    fn vector_type() -> &'static str {
        static VECTOR_TYPE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
        VECTOR_TYPE
            .get_or_init(|| {
                let corpus: Value = serde_json::from_str(E2E_CORPUS)
                    .expect("shared parity corpus JSON should be valid");
                corpus["schema"]["fields"]["embedding"]["type"]
                    .as_str()
                    .expect("shared corpus embedding type should be present")
                    .to_owned()
            })
            .as_str()
    }

    fn sort_from_intent(field: &str, direction: &str) -> Result<Sort, Box<dyn std::error::Error>> {
        match direction {
            "asc" => Ok(Sort::asc(field)),
            "desc" => Ok(Sort::desc(field)),
            other => Err(format!("unsupported sort direction: {other}").into()),
        }
    }

    fn facet_sort_from_intent(value: &str) -> Result<FacetSort, Box<dyn std::error::Error>> {
        match value {
            "count_asc" => Ok(FacetSort::CountAsc),
            "count_desc" => Ok(FacetSort::CountDesc),
            "value_asc" => Ok(FacetSort::ValueAsc),
            "value_desc" => Ok(FacetSort::ValueDesc),
            other => Err(format!("unsupported facet sort: {other}").into()),
        }
    }

    fn aggregation_expression_from_intent(
        spec: &Value,
    ) -> Result<GroupingAggregation, Box<dyn std::error::Error>> {
        Ok(
            match spec["kind"].as_str().ok_or("aggregation kind missing")? {
                "count" => GroupingAggregation::count(),
                "sum" => GroupingAggregation::sum(
                    spec["field"]
                        .as_str()
                        .ok_or("sum aggregation field missing")?,
                ),
                "avg" => GroupingAggregation::average(
                    spec["field"]
                        .as_str()
                        .ok_or("average aggregation field missing")?,
                ),
                kind => return Err(format!("unsupported parity aggregation kind: {kind}").into()),
            },
        )
    }

    fn aggregation_output_from_intent(
        spec: &Value,
    ) -> Result<GroupingAggregation, Box<dyn std::error::Error>> {
        if spec["kind"] == "alias" {
            return Ok(GroupingAggregation::named(
                spec["output_name"]
                    .as_str()
                    .ok_or("alias output name missing")?,
                GroupingAggregation::alias_ref(
                    spec["name"]
                        .as_str()
                        .ok_or("alias output reference missing")?,
                ),
            ));
        }
        Ok(GroupingAggregation::named(
            spec["name"]
                .as_str()
                .ok_or("aggregation output name missing")?,
            aggregation_expression_from_intent(spec)?,
        ))
    }

    fn add_group_outputs(
        mut level: GroupingLevel,
        outputs: &[Value],
    ) -> Result<GroupingLevel, Box<dyn std::error::Error>> {
        for output in outputs {
            level = level.output(aggregation_output_from_intent(output)?);
        }
        Ok(level)
    }

    fn add_group_aliases(
        mut level: GroupingLevel,
        aliases: &[Value],
    ) -> Result<GroupingLevel, Box<dyn std::error::Error>> {
        for alias in aliases {
            level = level.alias(
                alias["name"].as_str().ok_or("group alias name missing")?,
                aggregation_expression_from_intent(alias)?,
            );
        }
        Ok(level)
    }

    #[cfg(test)]
    mod parity_intent_tests {
        use super::*;

        #[test]
        fn nested_aggregation_kind_changes_constructed_public_aggregation() {
            let average = json!({"kind": "avg", "field": "price", "name": "metric"});
            let sum = json!({"kind": "sum", "field": "price", "name": "metric"});
            let average_output = aggregation_output_from_intent(&average).unwrap();
            let sum_output = aggregation_output_from_intent(&sum).unwrap();
            assert_eq!(
                average_output,
                GroupingAggregation::named("metric", GroupingAggregation::average("price"))
            );
            assert_eq!(
                sum_output,
                GroupingAggregation::named("metric", GroupingAggregation::sum("price"))
            );
            assert_ne!(average_output, sum_output);
        }
    }

    struct TrackedIntent<'a> {
        scenario: String,
        source: &'a Value,
        consumed: &'a RefCell<BTreeMap<String, serde_json::Map<String, Value>>>,
    }

    impl TrackedIntent<'_> {
        fn get(&self, key: &str) -> Option<&Value> {
            let value = self.source.get(key)?;
            self.consumed
                .borrow_mut()
                .entry(self.scenario.clone())
                .or_default()
                .insert(key.to_owned(), value.clone());
            Some(value)
        }
    }

    impl std::ops::Index<&str> for TrackedIntent<'_> {
        type Output = Value;

        fn index(&self, key: &str) -> &Self::Output {
            let value = &self.source[key];
            self.consumed
                .borrow_mut()
                .entry(self.scenario.clone())
                .or_default()
                .insert(key.to_owned(), value.clone());
            value
        }
    }

    fn tracked_intent<'a>(
        scenario: &str,
        shared_inputs: &'a Value,
        consumed: &'a RefCell<BTreeMap<String, serde_json::Map<String, Value>>>,
    ) -> TrackedIntent<'a> {
        TrackedIntent {
            scenario: scenario.to_owned(),
            source: &shared_inputs[scenario],
            consumed,
        }
    }

    fn corpus_documents() -> Result<Vec<(String, Value)>, Box<dyn std::error::Error>> {
        let corpus: Value = serde_json::from_str(E2E_CORPUS)?;
        let schema_fields = corpus["schema"]["fields"]
            .as_object()
            .ok_or("corpus schema fields missing")?;
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
                document["embedding"] = serde_json::to_value(Tensor::dense(vector_type(), values))?;
            }
            normalize_document_values(&mut document, schema_fields)?;
            documents.push((id.to_owned(), document));
        }
        Ok(documents)
    }

    fn normalize_document_values(
        document: &mut Value,
        schema_fields: &serde_json::Map<String, Value>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let fields = document
            .as_object_mut()
            .ok_or("corpus document must be an object")?;
        for (name, definition) in schema_fields {
            if definition["type"] != "float64" {
                continue;
            }
            let Some(value) = fields.get_mut(name) else {
                continue;
            };
            let number = value
                .as_f64()
                .ok_or_else(|| format!("corpus field {name} must be numeric"))?;
            *value = Value::Number(
                serde_json::Number::from_f64(number)
                    .ok_or_else(|| format!("corpus field {name} must be finite"))?,
            );
        }
        Ok(())
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

    fn typed_query_for_scenario(
        scenario: &str,
        input: &TrackedIntent<'_>,
    ) -> Result<Query, String> {
        let field = || input["field"].as_str().ok_or("typed query field missing");
        let text = |name: &str| input[name].as_str().ok_or("typed query value missing");
        let text_terms = || {
            input["terms"]
                .as_array()
                .ok_or("typed query terms missing")?
                .iter()
                .map(|term| term.as_str().ok_or("typed query term must be text"))
                .collect::<Result<Vec<_>, _>>()
        };
        let weighted = || {
            input["items"]
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
                .collect::<Result<Vec<_>, String>>()
        };

        Ok(match scenario {
            "typed_query_term" => Query::term(field()?, text("value")?),
            "typed_query_contains" => Query::contains(field()?, Query::term_value(text("value")?)),
            "typed_query_phrase" => Query::phrase(field()?, text_terms()?.join(" ")),
            "typed_query_near" => {
                Query::near(field()?, text_terms()?.into_iter().map(Query::term_value))
            }
            "typed_query_ordered_near" => {
                Query::onear(field()?, text_terms()?.into_iter().map(Query::term_value))
            }
            "typed_query_range" | "typed_query_range_exclusive" => {
                let lower = input["lower"].as_f64().ok_or("range lower bound missing")?;
                let upper = input["upper"].as_f64().ok_or("range upper bound missing")?;
                let lower = if input["include_lower"]
                    .as_bool()
                    .ok_or("range include_lower missing")?
                {
                    QueryBound::Included(lower.into())
                } else {
                    QueryBound::Excluded(lower.into())
                };
                let upper = if input["include_upper"]
                    .as_bool()
                    .ok_or("range include_upper missing")?
                {
                    QueryBound::Included(upper.into())
                } else {
                    QueryBound::Excluded(upper.into())
                };
                Query::range(field()?, lower, upper)
            }
            "typed_query_boolean" => {
                let terms = input["or_terms"]
                    .as_array()
                    .ok_or("boolean or_terms missing")?;
                if terms.len() != 2 {
                    return Err("boolean parity scenario requires exactly two OR terms".to_owned());
                }
                let left = &terms[0];
                let right = &terms[1];
                let negative = &input["not_term"];
                Query::and([
                    Query::or([
                        Query::term(
                            left["field"].as_str().ok_or("boolean left field missing")?,
                            left["value"].as_str().ok_or("boolean left value missing")?,
                        ),
                        Query::term(
                            right["field"]
                                .as_str()
                                .ok_or("boolean right field missing")?,
                            right["value"]
                                .as_str()
                                .ok_or("boolean right value missing")?,
                        ),
                    ]),
                    Query::not(Query::term(
                        negative["field"]
                            .as_str()
                            .ok_or("boolean negative field missing")?,
                        negative["value"]
                            .as_str()
                            .ok_or("boolean negative value missing")?,
                    )),
                ])
            }
            "typed_query_in" => Query::in_values(
                field()?,
                input["values"]
                    .as_array()
                    .ok_or("IN values missing")?
                    .iter()
                    .map(|value| {
                        value
                            .as_str()
                            .map(QueryValue::from)
                            .ok_or("IN value must be text")
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            "typed_query_equals" => Query::equals(field()?, text("value")?),
            "typed_query_fuzzy" => Query::fuzzy(field()?, text("value")?),
            "typed_query_weak_and" => Query::weak_and(
                input["terms"]
                    .as_array()
                    .ok_or("weak-and terms missing")?
                    .iter()
                    .map(|term| {
                        Ok(Query::term(
                            term["field"].as_str().ok_or("weak-and field missing")?,
                            term["value"].as_str().ok_or("weak-and value missing")?,
                        ))
                    })
                    .collect::<Result<Vec<_>, String>>()?,
                input["limit"]
                    .as_u64()
                    .ok_or("weak-and target hits missing")? as u32,
            ),
            "typed_query_wand" => Query::wand(field()?, weighted()?),
            "typed_query_weighted_set" => Query::weighted_set(field()?, weighted()?),
            "typed_query_dot_product" => Query::dot_product(field()?, weighted()?),
            // SKC: query.prefix_suffix_substring/basic kinds=ergonomic runtime=native
            "typed_query_prefix" => Query::prefix(field()?, text("value")?),
            "typed_query_substring" => Query::substring(field()?, text("value")?),
            "typed_query_suffix" => Query::suffix(field()?, text("value")?),
            "typed_query_same_element" => Query::same_element(
                field()?,
                [
                    Query::term(
                        input["first_field"]
                            .as_str()
                            .ok_or("same-element first field missing")?,
                        input["first_value"]
                            .as_str()
                            .ok_or("same-element first value missing")?,
                    ),
                    Query::term(
                        input["last_field"]
                            .as_str()
                            .ok_or("same-element last field missing")?,
                        input["last_value"]
                            .as_str()
                            .ok_or("same-element last value missing")?,
                    ),
                ],
            ),
            "typed_query_geo_location" => Query::geo_location(
                field()?,
                input["latitude"].as_f64().ok_or("geo latitude missing")?,
                input["longitude"].as_f64().ok_or("geo longitude missing")?,
                input["radius"].as_str().ok_or("geo radius missing")?,
            ),
            "typed_query_geo_box" => Query::geo_bounding_box(
                field()?,
                input["south"].as_f64().ok_or("geo south missing")?,
                input["west"].as_f64().ok_or("geo west missing")?,
                input["north"].as_f64().ok_or("geo north missing")?,
                input["east"].as_f64().ok_or("geo east missing")?,
            ),
            _ => return Err(format!("unsupported typed parity scenario: {scenario}")),
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

        let corpus_schema: Value = serde_json::from_str(E2E_CORPUS)?;
        let schema_fields = corpus_schema["schema"]["fields"]
            .as_object()
            .ok_or("corpus schema fields missing")?;
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
                serde_json::to_value(Tensor::dense(vector_type(), probe_vector.clone()))?;
            normalize_document_values(
                &mut document,
                corpus["schema"]["fields"]
                    .as_object()
                    .ok_or("corpus schema fields missing")?,
            )?;
            index.insert(format!("degraded-{probe}"), &document)?;
        }
        println!("inserted 200 deterministic coverage probe documents");

        let scenario_inputs: Value = serde_json::from_str(E2E_CORPUS)?;
        let shared_inputs = &scenario_inputs["scenario_inputs"];
        let consumed_intents: RefCell<BTreeMap<String, serde_json::Map<String, Value>>> =
            RefCell::new(BTreeMap::new());
        macro_rules! intent {
            ($name:literal) => {{
                tracked_intent($name, shared_inputs, &consumed_intents)
            }};
        }
        let pagination_intent = intent!("pagination");
        let pagination_query = pagination_intent["query"]
            .as_str()
            .ok_or("pagination query missing")?;
        let pagination_sort = pagination_intent["sort"][0]["field"]
            .as_str()
            .ok_or("pagination sort field missing")?;
        let pagination_direction = pagination_intent["sort"][0]["direction"]
            .as_str()
            .ok_or("pagination sort direction missing")?;
        let pagination_all_limit = pagination_intent["all_limit"]
            .as_u64()
            .ok_or("pagination all limit missing")? as u32;
        let filtered_facets_intent = intent!("filtered_facets");
        let parity_sorted = index
            .search(pagination_query)
            .sort(sort_from_intent(pagination_sort, pagination_direction)?)
            .limit(pagination_all_limit)
            .execute::<Value>()?;
        let parity_filtered = index
            .search(
                filtered_facets_intent["query"]
                    .as_str()
                    .ok_or("filtered facet query missing")?,
            )
            .filter(
                filtered_facets_intent["filter"]
                    .as_str()
                    .ok_or("filtered facet filter missing")?,
            )
            .sort(sort_from_intent(pagination_sort, pagination_direction)?)
            .limit(
                filtered_facets_intent["page_limit"]
                    .as_u64()
                    .ok_or("filtered search page limit missing")? as u32,
            )
            .execute::<Value>()?;
        let facet_intent = intent!("facets");
        let parity_faceted = index
            .search(
                facet_intent["query"]
                    .as_str()
                    .ok_or("facet query missing")?,
            )
            .facet(
                Facet::new(
                    facet_intent["field"]
                        .as_str()
                        .ok_or("facet field missing")?,
                )
                .name(
                    facet_intent["name"]
                        .as_str()
                        .ok_or("facet result name missing")?,
                )
                .limit(
                    facet_intent["limit"]
                        .as_u64()
                        .ok_or("facet limit missing")? as u32,
                )
                .sort(facet_sort_from_intent(
                    facet_intent["sort"].as_str().ok_or("facet sort missing")?,
                )?),
            )
            .limit(
                facet_intent["page_limit"]
                    .as_u64()
                    .ok_or("facet page limit missing")? as u32,
            )
            .execute::<Value>()?;
        let nearest_intent = intent!("nearest_neighbor");
        let parity_nearest = index
            .search("")
            .ranking(Ranking::named(
                nearest_intent["ranking_profile"]
                    .as_str()
                    .ok_or("nearest ranking profile missing")?,
            ))
            .typed_query(Query::nearest_neighbor(
                nearest_intent["field"]
                    .as_str()
                    .ok_or("nearest field missing")?,
                nearest_intent["query_input"]
                    .as_str()
                    .ok_or("nearest query input missing")?,
            ))
            .query_input(
                nearest_intent["query_input"]
                    .as_str()
                    .ok_or("nearest query input missing")?,
                Tensor::dense(
                    vector_type(),
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
        let rerank_intent = intent!("two_phase_reranking");
        let parity_reranked = index
            .search(
                rerank_intent["query"]
                    .as_str()
                    .ok_or("rerank query missing")?,
            )
            .ranking(Ranking::named(
                rerank_intent["ranking_profile"]
                    .as_str()
                    .ok_or("rerank profile missing")?,
            ))
            .limit(
                rerank_intent["limit"]
                    .as_u64()
                    .ok_or("rerank limit missing")? as u32,
            )
            .execute::<Value>()?;
        let structured_error_intent = intent!("structured_filter_error");
        let parity_error = index
            .search(
                structured_error_intent["query"]
                    .as_str()
                    .ok_or("structured error query missing")?,
            )
            .filter(
                structured_error_intent["filter"]
                    .as_str()
                    .ok_or("structured error filter missing")?,
            )
            .execute::<Value>()
            .expect_err("unknown field must fail");
        let document_intent = intent!("document_crud");
        let parity_document = index
            .get::<Value>(
                document_intent["document_id"]
                    .as_str()
                    .ok_or("document id missing")?,
            )?
            .expect("shoe-1 exists");
        let tensor_intent = intent!("tensor_readback");
        let tensor_document = index
            .get::<Value>(
                tensor_intent["document_id"]
                    .as_str()
                    .ok_or("tensor readback document id missing")?,
            )?
            .ok_or("tensor readback document missing")?;
        let tensor_field = tensor_intent["field"]
            .as_str()
            .ok_or("tensor readback field missing")?;
        let full_text_intent = intent!("full_text_search");
        let parity_full_text = index
            .search(
                full_text_intent["query"]
                    .as_str()
                    .ok_or("full-text query missing")?,
            )
            .limit(
                full_text_intent["limit"]
                    .as_u64()
                    .ok_or("full-text limit missing")? as u32,
            )
            .execute::<Value>()?;
        let stemming_intent = intent!("stemming");
        let parity_stemming = index
            .search(
                stemming_intent["query"]
                    .as_str()
                    .ok_or("stemming query missing")?,
            )
            .limit(
                stemming_intent["limit"]
                    .as_u64()
                    .ok_or("stemming limit missing")? as u32,
            )
            .execute::<Value>()?;
        let binding_intent = intent!("text_filter_binding");
        let parity_text_binding = index
            .search(binding_intent["query"].as_str().ok_or("binding query missing")?)
            .filter((binding_intent["filter"].as_str().ok_or("binding filter missing")?, params! { "max" => binding_intent["bindings"]["max"].as_f64().ok_or("max binding missing")? }))
            .sort(sort_from_intent(binding_intent["sort"][0]["field"].as_str().ok_or("binding sort missing")?, binding_intent["sort"][0]["direction"].as_str().ok_or("binding sort direction missing")?)?)
            .limit(binding_intent["limit"].as_u64().ok_or("binding limit missing")? as u32)
            .execute::<Value>()?;
        let repeated_intent = intent!("repeated_filters");
        let parity_repeated = index
            .search(repeated_intent["query"].as_str().ok_or("repeated filter query missing")?)
            .filter((repeated_intent["filters"][0]["source"].as_str().ok_or("first repeated filter missing")?, params! { "value" => repeated_intent["filters"][0]["bindings"]["value"].as_f64().ok_or("repeated float binding missing")? }))
            .filter((repeated_intent["filters"][1]["source"].as_str().ok_or("second repeated filter missing")?, params! { "value" => repeated_intent["filters"][1]["bindings"]["value"].as_i64().ok_or("repeated integer binding missing")? }))
            .filter(repeated_intent["filters"][2]["source"].as_str().ok_or("third repeated filter missing")?)
            .sort(sort_from_intent(repeated_intent["sort"][0]["field"].as_str().ok_or("repeated sort missing")?, repeated_intent["sort"][0]["direction"].as_str().ok_or("repeated sort direction missing")?)?)
            .limit(repeated_intent["limit"].as_u64().ok_or("repeated limit missing")? as u32)
            .execute::<Value>()?;
        let boolean_intent = intent!("boolean_filter_grammar");
        let parity_boolean = index
            .search(boolean_intent["query"].as_str().ok_or("boolean filter query missing")?)
            .filter((
                boolean_intent["filter"].as_str().ok_or("boolean filter missing")?,
                params! {
                    "brands" => boolean_intent["bindings"]["brands"].as_array().ok_or("brands binding missing")?.iter().map(|brand| brand.as_str().ok_or("brand must be text")).collect::<Result<Vec<_>, _>>()?,
                    "rating" => boolean_intent["bindings"]["rating"].as_i64().ok_or("rating binding missing")?
                },
            ))
            .sort(sort_from_intent(boolean_intent["sort"][0]["field"].as_str().ok_or("boolean sort missing")?, boolean_intent["sort"][0]["direction"].as_str().ok_or("boolean sort direction missing")?)?)
            .limit(boolean_intent["limit"].as_u64().ok_or("boolean limit missing")? as u32)
            .execute::<Value>()?;
        let projection_intent = intent!("sorting_projection");
        let parity_projection = index
            .search(
                projection_intent["query"]
                    .as_str()
                    .ok_or("projection query missing")?,
            )
            .sort(sort_from_intent(
                projection_intent["sort"][0]["field"]
                    .as_str()
                    .ok_or("projection sort missing")?,
                projection_intent["sort"][0]["direction"]
                    .as_str()
                    .ok_or("projection sort direction missing")?,
            )?)
            .projection(
                projection_intent["projection"]
                    .as_array()
                    .ok_or("projection fields missing")?
                    .iter()
                    .map(|field| field.as_str().ok_or("projection field must be text"))
                    .collect::<Result<Vec<_>, _>>()?,
            )
            .limit(
                projection_intent["limit"]
                    .as_u64()
                    .ok_or("projection limit missing")? as u32,
            )
            .execute::<Value>()?;
        let parity_all_sorted = index
            .search(pagination_query)
            .sort(sort_from_intent(pagination_sort, pagination_direction)?)
            .limit(pagination_all_limit)
            .execute::<Value>()?;
        let parity_page = index
            .search(pagination_query)
            .sort(sort_from_intent(pagination_sort, pagination_direction)?)
            .offset(
                pagination_intent["offset"]
                    .as_u64()
                    .ok_or("pagination offset missing")? as u32,
            )
            .limit(
                pagination_intent["limit"]
                    .as_u64()
                    .ok_or("pagination limit missing")? as u32,
            )
            .execute::<Value>()?;
        let parity_filtered_facet = index
            .search(
                filtered_facets_intent["query"]
                    .as_str()
                    .ok_or("filtered facet query missing")?,
            )
            .filter(
                filtered_facets_intent["filter"]
                    .as_str()
                    .ok_or("filtered facet filter missing")?,
            )
            .facet(
                Facet::new(
                    filtered_facets_intent["field"]
                        .as_str()
                        .ok_or("filtered facet field missing")?,
                )
                .limit(
                    filtered_facets_intent["limit"]
                        .as_u64()
                        .ok_or("filtered facet limit missing")? as u32,
                )
                .sort(facet_sort_from_intent(
                    filtered_facets_intent["sort"]
                        .as_str()
                        .ok_or("filtered facet sort missing")?,
                )?),
            )
            .limit(
                filtered_facets_intent["page_limit"]
                    .as_u64()
                    .ok_or("filtered facet page limit missing")? as u32,
            )
            .execute::<Value>()?;
        let grouping_intent = intent!("grouping_aggregations");
        let grouping_order = grouping_intent["order"]
            .as_array()
            .ok_or("grouping order missing")?
            .iter()
            .map(|order| {
                let descending = order["direction"] == "desc";
                match order["kind"].as_str().ok_or("group order kind missing")? {
                    "count" => Ok(GroupingOrder::aggregation(
                        GroupingAggregation::count(),
                        descending,
                    )),
                    "field" => Ok(GroupingOrder::field(
                        order["field"].as_str().ok_or("group order field missing")?,
                        descending,
                    )),
                    kind => Err(format!("unsupported grouping order kind: {kind}")),
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        let grouping_outputs = grouping_intent["outputs"]
            .as_array()
            .ok_or("grouping outputs missing")?;
        let grouping_level = add_group_outputs(
            GroupingPlan::by(
                grouping_intent["group_by"]
                    .as_str()
                    .ok_or("grouping field missing")?,
            )
            .max_groups(
                grouping_intent["limit"]
                    .as_u64()
                    .ok_or("group limit missing")? as u32,
            )
            .precision(
                grouping_intent["precision"]
                    .as_u64()
                    .ok_or("group precision missing")? as u32,
            )
            .order_by(grouping_order),
            grouping_outputs,
        )?;
        let parity_grouped = index
            .search(
                grouping_intent["query"]
                    .as_str()
                    .ok_or("grouping query missing")?,
            )
            .grouping(GroupingPlan::new().level(grouping_level))
            .execute::<Value>()?;
        let nested_intent = intent!("nested_grouping");
        if nested_intent["kind"].as_str() != Some("browse") {
            return Err("nested_grouping requires browse intent".into());
        }
        let nested_child = add_group_outputs(
            GroupingPlan::by(
                nested_intent["nested"]["group_by"]
                    .as_str()
                    .ok_or("nested child field missing")?,
            )
            .max_groups(
                nested_intent["nested"]["limit"]
                    .as_u64()
                    .ok_or("nested child limit missing")? as u32,
            ),
            nested_intent["nested"]["outputs"]
                .as_array()
                .ok_or("nested child outputs missing")?,
        )?;
        let nested_level = add_group_outputs(
            GroupingPlan::by(
                nested_intent["group_by"]
                    .as_str()
                    .ok_or("nested group field missing")?,
            )
            .max_groups(
                nested_intent["limit"]
                    .as_u64()
                    .ok_or("nested group limit missing")? as u32,
            ),
            nested_intent["outputs"]
                .as_array()
                .ok_or("nested outputs missing")?,
        )?
        .nested(nested_child);
        let parity_nested = index
            .browse()
            .filter((
                nested_intent["filter"]["source"].as_str().ok_or("nested filter missing")?,
                params! { "categories" => nested_intent["filter"]["bindings"]["categories"].as_array().ok_or("nested categories missing")?.iter().map(|value| value.as_str().ok_or("nested category must be text")).collect::<Result<Vec<_>, _>>()? },
            ))
            .grouping(GroupingPlan::new().level(nested_level))
            .execute::<Value>()?;
        let root_intent = intent!("root_aggregations");
        if root_intent["kind"].as_str() != Some("browse") {
            return Err("root_aggregations requires browse intent".into());
        }
        let mut root_plan = GroupingPlan::new();
        for aggregation in root_intent["aggregations"]
            .as_array()
            .ok_or("root aggregations missing")?
        {
            root_plan = root_plan.root_output(aggregation_output_from_intent(aggregation)?);
        }
        let parity_root_aggregations = index
            .browse()
            .filter(
                root_intent["filter"]
                    .as_str()
                    .ok_or("root aggregation filter missing")?,
            )
            .grouping(root_plan)
            .limit(
                root_intent["limit"]
                    .as_u64()
                    .ok_or("root aggregation limit missing")? as u32,
            )
            .execute::<Value>()?;
        let alias_intent = intent!("group_alias");
        let alias_level = add_group_outputs(
            add_group_aliases(
                GroupingPlan::by(
                    alias_intent["group_by"]
                        .as_str()
                        .ok_or("alias group field missing")?,
                )
                .max_groups(
                    alias_intent["limit"]
                        .as_u64()
                        .ok_or("alias group limit missing")? as u32,
                ),
                alias_intent["aliases"]
                    .as_array()
                    .ok_or("group aliases missing")?,
            )?
            .order_by_alias(
                alias_intent["order_by_alias"]["name"]
                    .as_str()
                    .ok_or("alias order field missing")?,
                alias_intent["order_by_alias"]["direction"] == "desc",
            ),
            alias_intent["outputs"]
                .as_array()
                .ok_or("alias outputs missing")?,
        )?;
        let parity_alias = index
            .search(
                alias_intent["query"]
                    .as_str()
                    .ok_or("alias query missing")?,
            )
            .grouping(GroupingPlan::new().level(alias_level))
            .execute::<Value>()?;
        let keep_intent = intent!("group_keep_filter");
        let keep_level = add_group_outputs(
            GroupingPlan::by(
                keep_intent["group_by"]
                    .as_str()
                    .ok_or("keep group field missing")?,
            )
            .max_groups(
                keep_intent["limit"]
                    .as_u64()
                    .ok_or("keep group limit missing")? as u32,
            )
            .keep_regex(
                keep_intent["keep_regex"]["field"]
                    .as_str()
                    .ok_or("keep regex field missing")?,
                keep_intent["keep_regex"]["pattern"]
                    .as_str()
                    .ok_or("keep regex pattern missing")?,
            ),
            keep_intent["outputs"]
                .as_array()
                .ok_or("keep outputs missing")?,
        )?;
        let parity_keep = index
            .search(keep_intent["query"].as_str().ok_or("keep query missing")?)
            .grouping(GroupingPlan::new().level(keep_level))
            .execute::<Value>()?;
        let yql_intent = intent!("yql");
        let (yql_parameter_name, yql_parameter_value) = yql_intent["bindings"]
            .as_object()
            .and_then(|bindings| bindings.iter().next())
            .ok_or("YQL binding missing")?;
        let yql_page = i64::try_from(yql_intent["page"].as_u64().ok_or("YQL page missing")?)?;
        let yql_page_size = i64::try_from(
            yql_intent["page_size"]
                .as_u64()
                .ok_or("YQL page size missing")?,
        )?;
        let parity_yql = index
            .yql(yql_intent["source"].as_str().ok_or("YQL source missing")?)
            .yql_parameter(
                yql_parameter_name,
                yql_parameter_value
                    .as_str()
                    .ok_or("YQL binding value must be text")?,
            )
            .ranking(Ranking::named(
                yql_intent["ranking_profile"]
                    .as_str()
                    .ok_or("YQL ranking profile missing")?,
            ))
            .page(yql_page, yql_page_size)
            .execute::<Value>()?;
        let misuse_intent = intent!("builder_misuse");
        let misuse_query = misuse_intent["query"]
            .as_str()
            .ok_or("builder misuse query missing")?;
        let page_number = i64::try_from(
            misuse_intent["page"]
                .as_u64()
                .ok_or("builder misuse page missing")?,
        )?;
        let page_size = u32::try_from(
            misuse_intent["page_size"]
                .as_u64()
                .ok_or("builder misuse page size missing")?,
        )?;
        let misuse_limit = misuse_intent["limit"]
            .as_u64()
            .ok_or("builder misuse limit missing")? as u32;
        let negative_limit = misuse_intent["negative_limit"]
            .as_i64()
            .ok_or("builder misuse negative limit missing")?;
        let negative_offset = misuse_intent["negative_offset"]
            .as_i64()
            .ok_or("builder misuse negative offset missing")?;
        if negative_limit >= 0 || negative_offset >= 0 {
            return Err("builder misuse negative cases must remain negative".into());
        }
        let page_limit_rejected = index
            .search(misuse_query)
            .page(page_number, i64::from(page_size))
            .limit(misuse_limit)
            .execute::<Value>()
            .is_err();
        let page_offset_rejected = index
            .search(misuse_query)
            .page(page_number, i64::from(page_size))
            .offset(page_size)
            .execute::<Value>()
            .is_err();
        let typed_scenarios = [
            "typed_query_term",
            "typed_query_contains",
            "typed_query_phrase",
            "typed_query_near",
            "typed_query_ordered_near",
            "typed_query_range",
            "typed_query_range_exclusive",
            "typed_query_boolean",
            "typed_query_in",
            "typed_query_equals",
            "typed_query_fuzzy",
            "typed_query_weak_and",
            "typed_query_wand",
            "typed_query_weighted_set",
            "typed_query_dot_product",
            "typed_query_prefix",
            "typed_query_substring",
            "typed_query_suffix",
            "typed_query_same_element",
            "typed_query_geo_location",
            "typed_query_geo_box",
        ];
        let mut typed_observations = serde_json::Map::new();
        for scenario in typed_scenarios {
            let input = tracked_intent(scenario, shared_inputs, &consumed_intents);
            let mut builder = index.search("").typed_query(
                typed_query_for_scenario(scenario, &input)
                    .map_err(|error| format!("{scenario}: {error}"))?,
            );
            if let Some(profile) = input.get("ranking_profile").and_then(Value::as_str) {
                builder = builder.ranking(Ranking::named(profile));
            }
            let mut results = builder
                .limit(input["limit"].as_u64().ok_or("typed query limit missing")? as u32)
                .execute::<Value>()?;
            let result_order = input["result_order"]
                .as_str()
                .ok_or("typed query result_order missing")?;
            let query_result = if let Some(feature) = input.get("feature").and_then(Value::as_str) {
                json!(results
                    .hits
                    .iter()
                    .map(|hit| json!({
                        "id": hit.id.as_str(),
                        "feature": parity_feature(hit, feature),
                    }))
                    .collect::<Vec<_>>())
            } else {
                if result_order == "sorted_ids" {
                    results.hits.sort_by(|left, right| left.id.cmp(&right.id));
                }
                json!(ids(&results))
            };
            let consumed = consumed_intents
                .borrow()
                .get(scenario)
                .cloned()
                .ok_or_else(|| {
                    format!("typed parity scenario {scenario} consumed no intent fields")
                })?;
            typed_observations.insert(
                scenario.to_owned(),
                json!({
                    "observation": {"intent": consumed, "query_result": query_result},
                    "capabilities": scenario_inputs["client_scenario_contract"][scenario]["rust"],
                }),
            );
        }
        let rank_input_intent = intent!("rank_inputs");
        let (rank_input_name, rank_input_value) = rank_input_intent["inputs"]
            .as_object()
            .and_then(|inputs| inputs.iter().next())
            .ok_or("rank input missing")?;
        let rank_input_type = rank_input_intent["input_types"][rank_input_name]
            .as_str()
            .ok_or("rank input type missing")?;
        if rank_input_type != "float64" {
            return Err(format!("unsupported parity rank input type: {rank_input_type}").into());
        }
        let parity_rank_input = index
            .search(
                rank_input_intent["query"]
                    .as_str()
                    .ok_or("rank input query missing")?,
            )
            .ranking(Ranking::named(
                rank_input_intent["ranking_profile"]
                    .as_str()
                    .ok_or("rank input profile missing")?,
            ))
            .query_input(
                rank_input_name,
                rank_input_value
                    .as_f64()
                    .ok_or("rank input value missing")?,
            )
            .limit(
                rank_input_intent["limit"]
                    .as_u64()
                    .ok_or("rank input limit missing")? as u32,
            )
            .execute::<Value>()?;
        let rank_property_intent = intent!("rank_properties");
        let (rank_property_name, rank_property_value) = rank_property_intent["rank_properties"]
            .as_object()
            .and_then(|properties| properties.iter().next())
            .ok_or("rank property missing")?;
        let parity_rank_property = index
            .search(
                rank_property_intent["query"]
                    .as_str()
                    .ok_or("rank property query missing")?,
            )
            .ranking(Ranking::named(
                rank_property_intent["ranking_profile"]
                    .as_str()
                    .ok_or("rank property profile missing")?,
            ))
            .rank_property(
                rank_property_name,
                rank_property_value
                    .as_str()
                    .ok_or("rank property value missing")?,
            )
            .limit(
                rank_property_intent["limit"]
                    .as_u64()
                    .ok_or("rank property limit missing")? as u32,
            )
            .execute::<Value>()?;
        let hybrid_intent = intent!("hybrid_text_vector");
        let parity_hybrid = index
            .search("")
            .ranking(Ranking::named(
                hybrid_intent["ranking_profile"]
                    .as_str()
                    .ok_or("hybrid ranking profile missing")?,
            ))
            .typed_query(Query::or([
                Query::term(
                    hybrid_intent["text_field"]
                        .as_str()
                        .ok_or("hybrid text field missing")?,
                    hybrid_intent["text_value"]
                        .as_str()
                        .ok_or("hybrid text value missing")?,
                ),
                Query::nearest_neighbor(
                    hybrid_intent["vector_field"]
                        .as_str()
                        .ok_or("hybrid vector field missing")?,
                    hybrid_intent["query_input"]
                        .as_str()
                        .ok_or("hybrid query input missing")?,
                ),
            ]))
            .query_input(
                hybrid_intent["query_input"]
                    .as_str()
                    .ok_or("hybrid query input missing")?,
                Tensor::dense(
                    vector_type(),
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
        let grouping_count_name = grouping_intent["outputs"][0]["name"]
            .as_str()
            .ok_or("group count output name missing")?;
        let grouping_average_name = grouping_intent["outputs"][1]["name"]
            .as_str()
            .ok_or("group average output name missing")?;
        let parity_grouping_aggregations = parity_group_buckets
            .into_iter()
            .map(|group| {
                parity_grouping_aggregation(group, grouping_count_name, grouping_average_name)
            })
            .collect::<Vec<_>>();
        let nested_count_name = nested_intent["outputs"][0]["name"]
            .as_str()
            .ok_or("nested group count output name missing")?;
        let nested_child_count_name = nested_intent["nested"]["outputs"][0]["name"]
            .as_str()
            .ok_or("nested child count output name missing")?;
        let nested_child_average_name = nested_intent["nested"]["outputs"][1]["name"]
            .as_str()
            .ok_or("nested child average output name missing")?;
        let mut parity_observations = json!({
            "document_crud": {"id": document_intent["document_id"], "title": parity_document["title"], "brand": parity_document["brand"], "price": parity_document["price"]},
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
            "facets": facet_entries(&parity_faceted, "brand"),
            "root_aggregations": parity_root_aggregations.aggregations.iter().map(|(key, value)| (key.clone(), parity_value(value))).collect::<serde_json::Map<_, _>>(),
            "filtered_facets": facet_entries(&parity_filtered_facet, "brand"),
            "grouping_aggregations": parity_grouping_aggregations,
            "nested_grouping": direct_group_buckets(&parity_nested.groups).into_iter()
                .map(|group| parity_nested_group(group, nested_count_name, nested_child_count_name, nested_child_average_name))
                .collect::<Vec<_>>(),
            "group_alias": direct_group_buckets(&parity_alias.groups).into_iter()
                .map(|group| json!({"value": group.key.as_ref().map(parity_value), "fields": group.aggregations.iter().map(|(key, value)| (key.clone(), parity_value(value))).collect::<serde_json::Map<_, _>>() })).collect::<Vec<_>>(),
            "group_keep_filter": direct_group_buckets(&parity_keep.groups).into_iter()
                .map(|group| json!({"value": group.key.as_ref().map(parity_value), "fields": group.aggregations.iter().map(|(key, value)| (key.clone(), parity_value(value))).collect::<serde_json::Map<_, _>>() })).collect::<Vec<_>>(),
            "typed_query_term": typed_observations["typed_query_term"],
            "typed_query_contains": typed_observations["typed_query_contains"],
            "typed_query_phrase": typed_observations["typed_query_phrase"],
            "typed_query_near": typed_observations["typed_query_near"],
            "typed_query_ordered_near": typed_observations["typed_query_ordered_near"],
            "typed_query_range": typed_observations["typed_query_range"],
            "typed_query_range_exclusive": typed_observations["typed_query_range_exclusive"],
            "typed_query_boolean": typed_observations["typed_query_boolean"],
            "typed_query_in": typed_observations["typed_query_in"],
            "typed_query_equals": typed_observations["typed_query_equals"],
            "typed_query_fuzzy": typed_observations["typed_query_fuzzy"],
            "typed_query_weak_and": typed_observations["typed_query_weak_and"],
            "typed_query_wand": typed_observations["typed_query_wand"],
            "typed_query_weighted_set": typed_observations["typed_query_weighted_set"],
            "typed_query_dot_product": typed_observations["typed_query_dot_product"],
            "typed_query_prefix": typed_observations["typed_query_prefix"],
            "typed_query_substring": typed_observations["typed_query_substring"],
            "typed_query_suffix": typed_observations["typed_query_suffix"],
            "typed_query_same_element": typed_observations["typed_query_same_element"],
            "typed_query_geo_location": typed_observations["typed_query_geo_location"],
            "typed_query_geo_box": typed_observations["typed_query_geo_box"],
            "yql": {"ids": ids(&parity_yql), "total": parity_yql.total_hits.value},
            "rank_inputs": parity_rank_input.hits.iter().map(|hit| json!({"id": hit.id.as_str(), "score": hit.score, "feature": parity_feature(hit, "query(boost)")})).collect::<Vec<_>>(),
            "rank_properties": parity_rank_property.hits.iter().map(|hit| json!({"id": hit.id.as_str(), "score": hit.score, "feature": parity_feature(hit, "query(boost)")})).collect::<Vec<_>>(),
            "tensor_readback": tensor_document[tensor_field]["$searchkernel_tensor"]["dense"].clone(),
            "nearest_neighbor": {"observation": {"ids": ids(&parity_nearest), "scores": parity_nearest.hits.iter().map(|hit| hit.score).collect::<Vec<_>>()}, "capabilities": {}},
            "two_phase_reranking": {"ids": ids(&parity_reranked)},
            "hybrid_text_vector": {"observation": {"ids": ids(&parity_hybrid), "top_score": parity_hybrid.hits.first().and_then(|hit| hit.score)}, "capabilities": {}},
            "two_phase": {"ids": ids(&parity_reranked)},
            "unknown_field_error": {
                "code": "invalid_argument",
                "details": parity_error.details().iter().map(|detail| detail.code.as_str()).collect::<Vec<_>>()
            },
            "structured_filter_error": {
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
                "$searchkernel_tensor": { "type_spec": vector_type(), "dense": corpus_vector("shoe-1")? }
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
                Tensor::dense(vector_type(), corpus_vector("shoe-1")?),
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
        let degradation_intent = intent!("coverage_degradation");
        if degradation_intent["kind"].as_str() != Some("browse") {
            return Err("coverage_degradation requires browse intent".into());
        }
        let degraded = index
            .browse()
            .filter((
                degradation_intent["filter"]["source"].as_str().ok_or("degradation filter missing")?,
                params! { "value" => degradation_intent["filter"]["bindings"]["value"].as_i64().ok_or("degradation filter binding missing")? },
            ))
            .sort(sort_from_intent(
                degradation_intent["sort"][0]["field"].as_str().ok_or("degradation sort missing")?,
                degradation_intent["sort"][0]["direction"].as_str().ok_or("degradation sort direction missing")?,
            )?)
            .rank_property("vespa.matchphase.degradation.maxfiltercoverage", degradation_intent["rank_properties"]["vespa.matchphase.degradation.maxfiltercoverage"].as_str().ok_or("degradation coverage property missing")?)
            .rank_property("vespa.matchphase.degradation.attribute", degradation_intent["rank_properties"]["vespa.matchphase.degradation.attribute"].as_str().ok_or("degradation attribute property missing")?)
            .rank_property("vespa.matchphase.degradation.maxhits", degradation_intent["rank_properties"]["vespa.matchphase.degradation.maxhits"].as_str().ok_or("degradation hits property missing")?)
            .limit(degradation_intent["limit"].as_u64().ok_or("degradation limit missing")? as u32)
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
        heading("21A. TEXTUAL FILTER SYNTAX ERROR");

        let syntax_intent = intent!("text_filter_syntax_error");
        let error = index
            .search(
                syntax_intent["query"]
                    .as_str()
                    .ok_or("syntax error query missing")?,
            )
            .filter(
                syntax_intent["filter"]
                    .as_str()
                    .ok_or("syntax error filter missing")?,
            )
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

        let patch_intent = intent!("patch");
        let mut patch_initial = patch_intent["initial"].clone();
        normalize_document_values(&mut patch_initial, schema_fields)?;
        index.insert(
            patch_intent["document_id"]
                .as_str()
                .ok_or("patch document id missing")?,
            &patch_initial,
        )?;
        let patch_field = patch_intent["set"]
            .as_object()
            .and_then(|values| values.keys().next())
            .ok_or("patch set field missing")?;
        index.patch(
            patch_intent["document_id"]
                .as_str()
                .ok_or("patch document id missing")?,
            Patch::new()
                .set(
                    patch_field,
                    patch_intent["set"][patch_field]
                        .as_f64()
                        .ok_or("patch set value missing")?,
                )
                .unset(
                    patch_intent["unset"][0]
                        .as_str()
                        .ok_or("patch unset field missing")?,
                ),
        )?;
        let parity_mutated = index
            .get::<Value>(
                patch_intent["document_id"]
                    .as_str()
                    .ok_or("patch document id missing")?,
            )?
            .expect("parity mutation should exist");
        parity_observations["patch"] = json!({
            "price": parity_mutated["price"],
            "sale_price_present": parity_mutated.get("sale_price").is_some()
        });
        let delete_intent = intent!("delete");
        let parity_deleted = index.delete(
            delete_intent["document_id"]
                .as_str()
                .ok_or("delete document id missing")?,
        )?;
        parity_observations["delete"] = json!({
            "deleted": parity_deleted,
            "absent_after_delete": index.get::<Value>(delete_intent["document_id"].as_str().ok_or("delete document id missing")?)?.is_none()
        });
        let update_intent = intent!("advanced_update");
        let mut update_initial = update_intent["initial"].clone();
        normalize_document_values(&mut update_initial, schema_fields)?;
        index.insert(
            update_intent["document_id"]
                .as_str()
                .ok_or("update document id missing")?,
            &update_initial,
        )?;
        let set_field = update_intent["set"]
            .as_object()
            .and_then(|values| values.keys().next())
            .ok_or("update set field missing")?;
        let set_value = update_intent["set"][set_field]
            .as_str()
            .ok_or("update set value missing")?;
        let unset_field = update_intent["unset"][0]
            .as_str()
            .ok_or("update unset field missing")?;
        let increment_field = update_intent["increments"]
            .as_object()
            .and_then(|values| values.keys().next())
            .ok_or("increment field missing")?;
        let increment_value = update_intent["increments"][increment_field]
            .as_f64()
            .ok_or("increment amount missing")?;
        index.update(
            update_intent["document_id"]
                .as_str()
                .ok_or("update document id missing")?,
            Update::new()
                .set(set_field, set_value)
                .unset(unset_field)
                .increment(increment_field, increment_value),
        )?;
        let decrement_field = update_intent["decrements"]
            .as_object()
            .and_then(|values| values.keys().next())
            .ok_or("decrement field missing")?;
        let decrement_value = update_intent["decrements"][decrement_field]
            .as_f64()
            .ok_or("decrement amount missing")?;
        index.update(
            update_intent["document_id"]
                .as_str()
                .ok_or("update document id missing")?,
            Update::new().decrement(decrement_field, decrement_value),
        )?;
        let multiplier_field = update_intent["multipliers"]
            .as_object()
            .and_then(|values| values.keys().next())
            .ok_or("multiplier field missing")?;
        let multiplier_value = update_intent["multipliers"][multiplier_field]
            .as_f64()
            .ok_or("multiplier amount missing")?;
        index.update(
            update_intent["document_id"]
                .as_str()
                .ok_or("update document id missing")?,
            Update::new().multiply(multiplier_field, multiplier_value),
        )?;
        let divisor_field = update_intent["divisors"]
            .as_object()
            .and_then(|values| values.keys().next())
            .ok_or("divisor field missing")?;
        let divisor_value = update_intent["divisors"][divisor_field]
            .as_f64()
            .ok_or("divisor amount missing")?;
        index.update(
            update_intent["document_id"]
                .as_str()
                .ok_or("update document id missing")?,
            Update::new().divide(divisor_field, divisor_value),
        )?;
        let advanced_update = index
            .get::<Value>(
                update_intent["document_id"]
                    .as_str()
                    .ok_or("update document id missing")?,
            )?
            .expect("updated document remains");
        parity_observations["advanced_update"] = json!({
            "title": advanced_update["title"],
            "inventory": advanced_update["inventory"],
            "sale_price_present": advanced_update.get("sale_price").is_some()
        });
        let reopen_intent = intent!("reopen_persistence");
        let mut reopen_marker = reopen_intent["marker"].clone();
        normalize_document_values(&mut reopen_marker, schema_fields)?;
        index.insert(
            reopen_intent["marker_id"]
                .as_str()
                .ok_or("reopen marker id missing")?,
            &reopen_marker,
        )?;

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

        let upsert_intent = intent!("upsert_replacement");
        let vector_document_id = upsert_intent["replacement"]["vector_document_id"]
            .as_str()
            .ok_or("upsert vector id missing")?;
        let mut replacement = upsert_intent["replacement"].clone();
        let replacement_object = replacement
            .as_object_mut()
            .ok_or("upsert replacement must be an object")?;
        replacement_object.remove("vector_document_id");
        replacement_object.insert(
            "embedding".to_owned(),
            serde_json::to_value(Tensor::dense(
                vector_type(),
                corpus_vector(vector_document_id)?,
            ))?,
        );
        normalize_document_values(&mut replacement, schema_fields)?;
        let upsert_id = upsert_intent["document_id"]
            .as_str()
            .ok_or("upsert document id missing")?;
        index.upsert(upsert_id, &replacement)?;

        let doc = index
            .get::<Value>(upsert_id)?
            .expect("upsert document should exist");
        let upsert_title_matches = doc["title"]
            .as_str()
            .is_some_and(|title| title.contains("Updated Edition"));
        let upsert_search = index
            .search(
                upsert_intent["search_query"]
                    .as_str()
                    .ok_or("upsert search query missing")?,
            )
            .limit(20)
            .execute::<Value>()?;
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
                    "type_spec": vector_type(),
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
                    "type_spec": vector_type(),
                    "dense": corpus_vector("shoe-1")?
                }
            })
        );
        let reopen_intent = intent!("reopen_persistence");
        let upsert_intent = intent!("upsert_replacement");
        let delete_intent = intent!("delete");
        let parity_persisted = index
            .get::<Value>(
                reopen_intent["marker_id"]
                    .as_str()
                    .ok_or("reopen marker id missing")?,
            )?
            .expect("parity document should survive reopen");
        let persisted_shoe2 = index
            .get::<Value>(
                upsert_intent["document_id"]
                    .as_str()
                    .ok_or("upsert id missing")?,
            )?
            .expect("upserted document should survive reopen");
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
            "deleted_absent": index.get::<Value>(delete_intent["document_id"].as_str().ok_or("delete id missing")?)?.is_none()
        });
        for scenario in shared_inputs
            .as_object()
            .ok_or("scenario inputs must be an object")?
            .keys()
        {
            if scenario.starts_with("typed_query_") {
                continue;
            }
            let intent = consumed_intents.borrow().get(scenario).cloned().ok_or_else(|| {
                format!("parity scenario {scenario} emitted an observation without consuming its shared intent fields")
            })?;
            let value = parity_observations
                .get_mut(scenario)
                .ok_or_else(|| format!("missing parity observation for {scenario}"))?;
            if value.get("observation").is_some() && value.get("capabilities").is_some() {
                value["observation"]["intent"] = Value::Object(intent.clone());
            } else if let Some(observation) = value.as_object_mut() {
                observation.insert("intent".to_owned(), Value::Object(intent.clone()));
            } else {
                *value = json!({"intent": intent, "result": value.clone()});
            }
        }

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
                    "type_spec": vector_type(),
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
                Tensor::dense(vector_type(), corpus_vector("shoe-1")?),
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

    fn parity_grouping_aggregation(
        group: &searchkernel::GroupingResult<Value>,
        count_name: &str,
        average_name: &str,
    ) -> Value {
        json!({
            "brand": group.key.as_ref().map(parity_value),
            "products": parity_aggregation(group, count_name),
            "avg_price": parity_aggregation(group, average_name),
        })
    }

    fn parity_nested_group(
        group: &searchkernel::GroupingResult<Value>,
        count_name: &str,
        nested_count_name: &str,
        nested_average_name: &str,
    ) -> Value {
        json!({
            "category": group.key.as_ref().map(parity_value),
            "products": parity_aggregation(group, count_name),
            "ratings": group.groups.iter().map(|rating| json!({
                "rating": rating.key.as_ref().map(parity_value),
                "products": parity_aggregation(rating, nested_count_name),
                "avg_price": parity_aggregation(rating, nested_average_name),
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

    fn facet_entries(results: &SearchResult<Value>, field: &str) -> Vec<Value> {
        let facet = results
            .facets
            .iter()
            .find(|facet| facet.field == field)
            .unwrap_or_else(|| {
                let available = results
                    .facets
                    .iter()
                    .map(|facet| (facet.field.as_str(), facet.name.as_str()))
                    .collect::<Vec<_>>();
                panic!("missing facet {field:?}; available facets: {available:?}")
            });

        facet
            .values
            .iter()
            .map(|value| {
                let key = match &value.value {
                    ResultValue::String(value) => value.clone(),
                    _ => panic!("facet {field:?} returned non-string value"),
                };
                json!({"value": key, "count": value.count})
            })
            .collect()
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
