use searchkernel_json::{request_flatbuffer_to_json, request_json_to_flatbuffer, JsonCodecError};
use serde_json::{json, Value};

fn request_fixtures() -> Vec<(String, Vec<u8>)> {
    let directory =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/requests");
    let mut paths = std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "skrq")
        })
        .collect::<Vec<_>>();
    paths.sort();
    paths
        .into_iter()
        .map(|path| {
            let name = path.file_stem().unwrap().to_string_lossy().into_owned();
            (name, std::fs::read(path).unwrap())
        })
        .collect()
}

fn assert_no_wire_discriminators(value: &Value) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                assert!(
                    !matches!(
                        key.as_str(),
                        "payload_type"
                            | "kind_type"
                            | "expr_type"
                            | "policy_type"
                            | "operation_type"
                            | "target_type"
                    ),
                    "raw FlatBuffers discriminator leaked: {key}"
                );
                assert_no_wire_discriminators(child);
            }
        }
        Value::Array(values) => values.iter().for_each(assert_no_wire_discriminators),
        _ => {}
    }
}

fn assert_request_json_round_trip(name: &str, input: Value, expected: Value) {
    let input = serde_json::to_vec(&input).unwrap();
    let bytes = request_json_to_flatbuffer(&input)
        .unwrap_or_else(|error| panic!("{name}: request JSON encoding failed: {error}"));
    let output = request_flatbuffer_to_json(&bytes)
        .unwrap_or_else(|error| panic!("{name}: request JSON decoding failed: {error}"));
    let output: Value = serde_json::from_slice(&output)
        .unwrap_or_else(|error| panic!("{name}: invalid output JSON: {error}"));
    assert_eq!(output, expected, "{name}");
}

#[test]
fn canonical_symbolic_union_json_is_accepted() {
    let json = br#"{
        "protocol_version": 2,
        "request": {"search": {"query": "foo"}}
    }"#;
    let bytes = request_json_to_flatbuffer(json).expect("canonical request JSON");
    let output = request_flatbuffer_to_json(&bytes).expect("request JSON output");
    assert_eq!(
        serde_json::from_slice::<Value>(&output).unwrap(),
        serde_json::from_slice::<Value>(json).unwrap()
    );
}

#[test]
fn semantic_missing_fields_are_preserved_for_core_validation() {
    let json = br#"{
        "protocol_version": 2,
        "request": {"insert": {}}
    }"#;
    let bytes = request_json_to_flatbuffer(json).expect("codec must preserve absent document");
    let output = request_flatbuffer_to_json(&bytes).expect("request JSON output");
    assert_eq!(
        serde_json::from_slice::<Value>(&output).unwrap(),
        serde_json::from_slice::<Value>(json).unwrap()
    );
}

#[test]
fn lower_only_range_is_preserved() {
    let json = br#"{
        "protocol_version": 2,
        "request": {"search": {"filter": {"filter_range": {
            "field": "price",
            "lower": {"float64": 10.0}
        }}}}
    }"#;
    let bytes = request_json_to_flatbuffer(json).expect("lower-only range is portable");
    let output = request_flatbuffer_to_json(&bytes).expect("request JSON output");
    assert_eq!(
        serde_json::from_slice::<Value>(&output).unwrap(),
        serde_json::from_slice::<Value>(json).unwrap()
    );
}

#[test]
fn conditional_mutations_round_trip_without_field_loss() {
    for json in [
        br#"{
            "protocol_version": 2,
            "request": {"upsert": {
                "document": {"id": "id:ns:type::1", "fields": {}},
                "condition": "user.role == \"editor\""
            }}
        }"#
        .as_slice(),
        br#"{
            "protocol_version": 2,
            "request": {"delete": {
                "id": "id:ns:type::1",
                "condition": "user.role == \"editor\""
            }}
        }"#
        .as_slice(),
    ] {
        let bytes = request_json_to_flatbuffer(json).expect("conditional mutation JSON");
        let output = request_flatbuffer_to_json(&bytes).expect("conditional mutation output");
        assert_eq!(
            serde_json::from_slice::<Value>(&output).unwrap(),
            serde_json::from_slice::<Value>(json).unwrap()
        );
    }
}

#[test]
fn weighted_query_item_i64_round_trips_as_decimal_string() {
    let json = br#"{
        "protocol_version": 2,
        "request": {"search": {"query_expr": {"query_weighted_set": {
            "field": "tags",
            "items": [{"key": {"string": "rust"}, "weight": "9223372036854775807"}]
        }}}}
    }"#;
    let bytes = request_json_to_flatbuffer(json).expect("weighted query JSON");
    let output = request_flatbuffer_to_json(&bytes).expect("weighted query output");
    assert_eq!(
        serde_json::from_slice::<Value>(&output).unwrap(),
        serde_json::from_slice::<Value>(json).unwrap()
    );
}

#[test]
fn filter_in_round_trips_typed_values() {
    let json = br#"{
        "protocol_version": 2,
        "request": {"search": {"filter": {"filter_in": {
            "field": "brand",
            "values": [{"string": "Sony"}, {"int64": "7"}]
        }}}}
    }"#;
    let bytes = request_json_to_flatbuffer(json).expect("filter_in JSON");
    let output = request_flatbuffer_to_json(&bytes).expect("filter_in JSON output");
    assert_eq!(
        serde_json::from_slice::<Value>(&output).unwrap(),
        serde_json::from_slice::<Value>(json).unwrap()
    );
}

#[test]
fn filter_fields_round_trip_element_qualified_refs() {
    let json = br#"{
        "protocol_version": 2,
        "request": {"search": {"filters": [
            {"filter_comparison": {
                "field": {"path": "tags", "element_index": 2},
                "op": "eq",
                "value": {"string": "rust"}
            }},
            {"filter_range": {
                "field": {"path": "prices", "element_index": 1},
                "lower": {"int64": "10"}
            }},
            {"filter_exists": {
                "field": {"path": "labels", "element_index": 0}
            }},
            {"filter_in": {
                "field": {"path": "brands", "element_index": 3},
                "values": [{"string": "Sony"}]
            }}
        ]}}
    }"#;
    let bytes = request_json_to_flatbuffer(json).expect("element-qualified filter JSON");
    let output = request_flatbuffer_to_json(&bytes).expect("element-qualified filter output");
    assert_eq!(
        serde_json::from_slice::<Value>(&output).unwrap(),
        serde_json::from_slice::<Value>(json).unwrap()
    );
}

#[test]
fn query_expression_variants_round_trip_with_explicit_json_shapes() {
    let cases = [
        (
            "term_query",
            json!({"term_query": {"value": {"string": "rust"}}}),
        ),
        (
            "query_contains",
            json!({"query_contains": {
                "field": {"path": "tags", "element_index": 2},
                "item": {"term_query": {"value": {"string": "rust"}}}
            }}),
        ),
        (
            "query_and",
            json!({"query_and": {"clauses": [
                {"term_query": {"value": {"string": "rust"}}},
                {"query_true": {}}
            ]}}),
        ),
        (
            "query_or",
            json!({"query_or": {"clauses": [
                {"term_query": {"value": {"string": "rust"}}},
                {"query_false": {}}
            ]}}),
        ),
        (
            "query_not",
            json!({"query_not": {
                "clause": {"term_query": {"value": {"string": "rust"}}}
            }}),
        ),
        ("query_true", json!({"query_true": {}})),
        ("query_false", json!({"query_false": {}})),
        (
            "query_comparison",
            json!({"query_comparison": {
                "field": "price",
                "op": "gte",
                "value": {"int64": "10"}
            }}),
        ),
        (
            "query_range",
            json!({"query_range": {
                "field": "price",
                "lower": {"int64": "10"},
                "upper": {"int64": "20"},
                "include_lower": true,
                "include_upper": false
            }}),
        ),
        (
            "query_in",
            json!({"query_in": {
                "field": "brand",
                "values": [{"string": "Sony"}, {"string": "Bose"}]
            }}),
        ),
        (
            "query_phrase",
            json!({"query_phrase": {"terms": [
                {"term_query": {"value": {"string": "rust"}}},
                {"term_query": {"value": {"string": "search"}}}
            ]}}),
        ),
        (
            "query_near",
            json!({"query_near": {"terms": [
                {"term_query": {"value": {"string": "rust"}}},
                {"term_query": {"value": {"string": "search"}}}
            ]}}),
        ),
        (
            "query_onear",
            json!({"query_onear": {"terms": [
                {"term_query": {"value": {"string": "rust"}}},
                {"term_query": {"value": {"string": "search"}}}
            ]}}),
        ),
        (
            "query_same_element",
            json!({"query_same_element": {"clauses": [
                {"term_query": {"value": {"string": "rust"}}}
            ]}}),
        ),
        (
            "query_equiv",
            json!({"query_equiv": {"clauses": [
                {"term_query": {"value": {"string": "rust"}}}
            ]}}),
        ),
        (
            "query_alternatives",
            json!({"query_alternatives": {"alternatives": [
                {"query": {"term_query": {"value": {"string": "rust"}}}, "exactness": 0.75},
                {"query": {"term_query": {"value": {"string": "oxidation"}}}}
            ]}}),
        ),
        (
            "query_fuzzy",
            json!({"query_fuzzy": {"value": {"string": "serch"}}}),
        ),
        (
            "query_matches",
            json!({"query_matches": {
                "field": "title",
                "pattern": "rust.*"
            }}),
        ),
        ("query_text", json!({"query_text": {"text": "rust search"}})),
        (
            "query_user_input",
            json!({"query_user_input": {"value": "title:rust"}}),
        ),
        ("query_user_query", json!({"query_user_query": {}})),
        (
            "query_dot_product",
            json!({"query_dot_product": {
                "field": "embedding",
                "items": [{"key": {"string": "rust"}, "weight": "7"}]
            }}),
        ),
        (
            "query_weighted_set",
            json!({"query_weighted_set": {
                "field": "tags",
                "items": [{"key": {"string": "rust"}, "weight": "7"}]
            }}),
        ),
        (
            "query_wand",
            json!({"query_wand": {
                "field": "tags",
                "items": [{"key": {"string": "rust"}, "weight": "7"}]
            }}),
        ),
        (
            "query_weak_and",
            json!({"query_weak_and": {"clauses": [
                {"term_query": {"value": {"string": "rust"}}}
            ]}}),
        ),
        (
            "query_rank",
            json!({"query_rank": {
                "match": {"term_query": {"value": {"string": "rust"}}},
                "rank": [{"term_query": {"value": {"string": "search"}}}]
            }}),
        ),
        (
            "query_labeled",
            json!({"query_labeled": {
                "query": {"term_query": {"value": {"string": "rust"}}},
                "label": "primary",
                "score": 1.5
            }}),
        ),
        (
            "query_non_empty",
            json!({"query_non_empty": {
                "query": {"term_query": {"value": {"string": "rust"}}}
            }}),
        ),
        (
            "query_geo_location",
            json!({"query_geo_location": {
                "field": "location",
                "latitude": 59.91,
                "longitude": 10.75,
                "radius": "10 km"
            }}),
        ),
        (
            "query_geo_bounding_box",
            json!({"query_geo_bounding_box": {
                "field": "location",
                "south": 59.8,
                "west": 10.6,
                "north": 60.0,
                "east": 10.9
            }}),
        ),
        (
            "query_nearest_neighbor",
            json!({"query_nearest_neighbor": {
                "document_field": "embedding",
                "query_tensor": "query_embedding"
            }}),
        ),
        (
            "query_predicate",
            json!({"query_predicate": {
                "field": "access",
                "features": {"user": {"string": "alice"}},
                "range_features": {"age": {"int64": "42"}}
            }}),
        ),
    ];

    for (name, query) in cases {
        let request = json!({
            "protocol_version": 2,
            "request": {"search": {"query_expr": query}}
        });
        let expected_query = match name {
            // Optional scalar fields are emitted as explicit nulls by the
            // canonical JSON encoder when the FlatBuffer field is absent.
            "query_alternatives" => json!({"query_alternatives": {"alternatives": [
                {"query": {"term_query": {"value": {"string": "rust"}}}, "exactness": 0.75},
                {"query": {"term_query": {"value": {"string": "oxidation"}}}, "exactness": null}
            ]}}),
            _ => request["request"]["search"]["query_expr"].clone(),
        };
        let expected = json!({
            "protocol_version": 2,
            "request": {"search": {"query_expr": expected_query}}
        });
        assert_request_json_round_trip(name, request, expected);
    }
}

#[test]
fn filter_variants_round_trip_with_explicit_json_shapes() {
    let cases = [
        (
            "filter_and",
            json!({"filter_and": {"clauses": [
                {"filter_exists": {"field": "title"}}
            ]}}),
        ),
        (
            "filter_or",
            json!({"filter_or": {"clauses": [
                {"filter_exists": {"field": "title"}}
            ]}}),
        ),
        (
            "filter_not",
            json!({"filter_not": {
                "clause": {"filter_exists": {"field": "title"}}
            }}),
        ),
        (
            "filter_comparison",
            json!({"filter_comparison": {
                "field": "price",
                "op": "gte",
                "value": {"int64": "10"}
            }}),
        ),
        (
            "filter_range",
            json!({"filter_range": {
                "field": "price",
                "lower": {"int64": "10"},
                "upper": {"int64": "20"},
                "include_lower": true,
                "include_upper": false
            }}),
        ),
        (
            "filter_exists",
            json!({"filter_exists": {"field": "title"}}),
        ),
        (
            "filter_in",
            json!({"filter_in": {
                "field": "brand",
                "values": [{"string": "Sony"}, {"string": "Bose"}]
            }}),
        ),
    ];

    for (name, filter) in cases {
        let request = json!({
            "protocol_version": 2,
            "request": {"search": {"filter": filter}}
        });
        assert_request_json_round_trip(name, request.clone(), request);
    }
}

#[test]
fn full_query_sources_round_trip_without_loss() {
    for json in [
        br#"{
            "protocol_version": 2,
            "request": {"search": {"query_source": {
                "yql": {
                    "source": "select * from sources * where userInput(@term)",
                    "parameters": {"term": {"string": "rust"}}
                }
            }}}
        }"#
        .as_slice(),
        br#"{
            "protocol_version": 2,
            "request": {"search": {"query_source": {"match_all": {}}}}
        }"#
        .as_slice(),
    ] {
        let bytes = request_json_to_flatbuffer(json).expect("full query source JSON");
        let output = request_flatbuffer_to_json(&bytes).expect("full query source output");
        assert_eq!(
            serde_json::from_slice::<Value>(&output).unwrap(),
            serde_json::from_slice::<Value>(json).unwrap()
        );
    }
}

#[test]
fn v2_typed_query_sort_and_range_semantics_round_trip() {
    let json = br#"{
        "protocol_version": 2,
        "request": {"search": {
            "query_expr": {"query_contains": {
                "field": {"path": "tags", "element_index": 2},
                "item": {"term_query": {"value": {"string": "rust"}}}
            }, "annotations": {"label": {"string": "typed"}}},
            "filter": {"filter_range": {
                "field": "price",
                "lower": {"negative_infinity": {}},
                "upper": {"positive_infinity": {}},
                "include_lower": false,
                "include_upper": false
            }},
            "sort": [{
                "target": {"relevance": {}},
                "direction": "desc",
                "missing": {"as": {"value": {"int64": "7"}}},
                "annotations": {"locale": {"string": "en-US"}}
            }]
        }}
    }"#;
    let bytes = request_json_to_flatbuffer(json).expect("V2 semantic JSON");
    let output = request_flatbuffer_to_json(&bytes).expect("V2 JSON output");
    assert_eq!(
        serde_json::from_slice::<Value>(&output).unwrap(),
        serde_json::from_slice::<Value>(json).unwrap()
    );
}

#[test]
fn v2_search_request_context_round_trips_without_field_loss() {
    let json = br#"{
        "protocol_version": 2,
        "request": {"search": {
            "query": "rust",
            "search_fields": [{"field": "title", "weight": 2.5}],
            "filter_sources": ["user", "tenant"],
            "filter_parameters": {"tenant": {"string": "acme"}},
            "filter": {"filter_exists": {"field": "title"}},
            "ranking": {"profile": "bm25", "properties": {"rankproperty": "value"}},
            "inputs": {
                "query_tensor": {"literal": {"value": {"string": "tensor"}}},
                "embedding": {"expression": {"expression": "embed(@query)"}}
            },
            "properties": {"trace.level": {"int64": "2"}},
            "query_profile": "web",
            "search_chain": "default",
            "query_model": {
                "query_string": "title:rush",
                "default_index": "title",
                "encoding": "utf-8",
                "filter": "default",
                "language": "en",
                "locale": "en-US",
                "type": {
                    "composite": "and",
                    "tokenization": "strict",
                    "syntax": "yql",
                    "profile": "default",
                    "is_yql_default": true
                },
                "sources": ["news", "blogs"],
                "restrict": ["public"],
                "search_path": "/search",
                "search_group": 3
            },
            "projection": ["title", "body"],
            "highlighting": ["title"],
            "summary": "bold",
            "format": "json",
            "bolding": true,
            "timing": true,
            "limit": 20,
            "offset": 5,
            "timeout": "250"
        }}
    }"#;
    let bytes = request_json_to_flatbuffer(json).expect("complete request JSON");
    let output = request_flatbuffer_to_json(&bytes).expect("complete request JSON output");
    assert_eq!(
        serde_json::from_slice::<Value>(&output).unwrap(),
        serde_json::from_slice::<Value>(json).unwrap()
    );
}

#[test]
fn multiple_structured_filters_round_trip_in_full_json_form() {
    let json = br#"{
        "protocol_version": 2,
        "request": {"search": {
            "query": "rust",
            "filters": [
                {"filter_exists": {"field": "title"}},
                {"filter_range": {
                    "field": "price",
                    "lower": {"float64": 10.0},
                    "include_lower": true
                }}
            ],
            "filter_sources": ["user", "tenant"],
            "filter_parameters": {"tenant": {"string": "acme"}}
        }}
    }"#;
    let bytes = request_json_to_flatbuffer(json).expect("multiple structured filters");
    let output = request_flatbuffer_to_json(&bytes).expect("multiple structured filters output");
    assert_eq!(
        serde_json::from_slice::<Value>(&output).unwrap(),
        serde_json::from_slice::<Value>(json).unwrap()
    );
}

#[test]
fn filter_shorthand_remains_canonical_for_one_structured_filter() {
    let json = br#"{
        "protocol_version": 2,
        "request": {"search": {
            "query": "rust",
            "filters": [{"filter_exists": {"field": "title"}}]
        }}
    }"#;
    let bytes = request_json_to_flatbuffer(json).expect("single structured filter");
    let output = request_flatbuffer_to_json(&bytes).expect("single structured filter output");
    assert_eq!(
        serde_json::from_slice::<Value>(&output).unwrap(),
        serde_json::json!({
            "protocol_version": 2,
            "request": {"search": {
                "query": "rust",
                "filter": {"filter_exists": {"field": "title"}}
            }}
        })
    );
}

#[test]
fn generic_value_tensors_round_trip_all_payload_styles() {
    let json = br#"{
        "protocol_version": 2,
        "request": {"insert": {
            "document": {
                "id": "tensor-doc",
                "fields": {
                    "dense": {"tensor": {
                        "type_spec": "tensor<float>(x[3])",
                        "payload": {"dense": {"values": [1.0, 2.5, -3.0]}}
                    }},
                    "sparse": {"tensor": {
                        "type_spec": "tensor<float>(x{})",
                        "payload": {"sparse": {"cells": [
                            {"address": [{"dimension": "x", "label": "a"}], "value": 4.0},
                            {"address": [{"dimension": "x", "label": "b"}], "value": 5.5}
                        ]}}
                    }},
                    "mixed": {"tensor": {
                        "type_spec": "tensor<float>(label{},x[2])",
                        "payload": {"mixed": {"blocks": [
                            {"address": [{"dimension": "label", "label": "first"}], "values": [6.0, 7.0]},
                            {"address": [], "values": [8.0, 9.0]}
                        ]}}
                    }}
                }
            }
        }}
    }"#;
    let bytes = request_json_to_flatbuffer(json).expect("tensor request JSON");
    let output = request_flatbuffer_to_json(&bytes).expect("tensor request JSON output");
    assert_eq!(
        serde_json::from_slice::<Value>(&output).unwrap(),
        serde_json::from_slice::<Value>(json).unwrap()
    );
}

#[test]
fn tensor_values_reject_non_finite_cells() {
    let json = br#"{
        "protocol_version": 2,
        "request": {"insert": {
            "document": {"fields": {
                "embedding": {"tensor": {
                    "type_spec": "tensor<float>(x[1])",
                    "payload": {"dense": {"values": [1e999]}}
                }}
            }}
        }}
    }"#;
    assert!(request_json_to_flatbuffer(json).is_err());
}

#[test]
fn structured_grouping_round_trips_all_decoder_shapes() {
    let json = br#"{
        "protocol_version": 2,
        "request": {"search": {
            "grouping": [{
                "source": {"structured": {
                    "label": "root",
                    "operations": [
                        {"each": {"label": "branch", "operations": [
                            {"group": {"expression": {"attribute": {"field": "brand"}}}},
                            {"output": {"outputs": [
                                {"label": "count", "expression": {"aggregation": {"count": {}}}},
                                {"label": "price", "expression": {"aggregation": {"sum": {"expression": {"attribute": {"field": "price"}}}}}}
                            ]}},
                            {"max": {"finite": {"value": 10}}},
                            {"max": {"infinite": {}}},
                            {"precision": {"value": 100}},
                            {"order": {"expressions": [{"unary": {"op": "negative", "expression": {"aggregation": {"count": {}}}}}]}},
                            {"alias": {"name": "total", "expression": {"binary": {"op": "add", "left": {"literal": {"value": {"int64": "1"}}}, "right": {"literal": {"value": {"int64": "2"}}}}}}},
                            {"filter": {"and": {"clauses": [
                                {"regex": {"expression": {"attribute": {"field": "brand"}}, "pattern": "^A"}},
                                {"range": {"expression": {"attribute": {"field": "price"}}, "lower": {"literal": {"value": {"int64": "1"}}}, "upper": {"literal": {"value": {"int64": "100"}}}, "include_lower": true, "include_upper": false}}
                            ]}}},
                            {"keep": {"not": {"clause": {"is_true": {"expression": {"attribute": {"field": "available"}}}}}}},
                            {"where": {"expression": {"map_lookup": {"base": {"member": {"base": {"attribute": {"field": "metadata"}}, "member": "values"}}, "key": {"literal": {"value": {"string": "color"}}}}}}}
                        ]}},
                        {"output": {"outputs": [{"expression": {"aggregation": {"quantiles": {"quantiles": [0.5, 0.9], "expression": {"attribute": {"field": "price"}}}}}}]}},
                        {"output": {"outputs": [{"expression": {"aggregation": {"summary": {"summary": "default"}}}}]}}
                    ]
                }},
                "continuations": ["after-brand"]
            }]
        }}
    }"#;
    let bytes = request_json_to_flatbuffer(json).expect("structured grouping JSON");
    let output = request_flatbuffer_to_json(&bytes).expect("structured grouping JSON output");
    assert_eq!(
        serde_json::from_slice::<Value>(&output).unwrap(),
        serde_json::from_slice::<Value>(json).unwrap()
    );
}

#[test]
fn engine_timeout_is_preserved_in_canonical_json() {
    let json = br#"{
        "protocol_version": 2,
        "request": {"search": {"query": "foo", "timeout": "200"}}
    }"#;
    let bytes = request_json_to_flatbuffer(json).expect("timeout request JSON");
    let output = request_flatbuffer_to_json(&bytes).expect("request JSON output");
    assert_eq!(
        serde_json::from_slice::<Value>(&output).unwrap(),
        serde_json::from_slice::<Value>(json).unwrap()
    );
}

#[test]
fn official_flatbuffers_union_json_is_rejected() {
    let json = br#"{
        "protocol_version": 2,
        "payload_type": "SearchKernel_V1_SearchRequest",
        "payload": {"query": "foo"}
    }"#;
    assert!(request_json_to_flatbuffer(json).is_err());
}

#[test]
fn malformed_inputs_are_rejected() {
    assert!(request_json_to_flatbuffer(br#"{"#).is_err());
    assert!(request_flatbuffer_to_json(b"not a FlatBuffer").is_err());

    let mut embedded_nul = br#"{\"protocol_version\":2}"#.to_vec();
    embedded_nul.push(0);
    embedded_nul.extend_from_slice(b"garbage");
    assert!(matches!(
        request_json_to_flatbuffer(&embedded_nul),
        Err(JsonCodecError::InvalidJson(message)) if message == "JSON input contains NUL byte"
    ));
}

#[test]
fn stale_v1_request_envelope_is_rejected() {
    let json = br#"{
        "protocol_version": 1,
        "request": {"search": {"query": "foo"}}
    }"#;
    assert!(matches!(
        request_json_to_flatbuffer(json),
        Err(JsonCodecError::Contract(message)) if message == "unsupported protocol_version 1"
    ));
}

#[test]
fn nested_union_payloads_reject_unknown_fields() {
    let invalid: &[&[u8]] = &[
        br#"{"protocol_version":2,"request":{"search":{"query_expr":{"query_contains":{"field":"title","item":{"term_query":{"value":{"string":"rust"}}},"this_should_not_exist":123}}}}}"#,
        br#"{"protocol_version":2,"request":{"search":{"filter":{"filter_and":{"clauses":[],"this_should_not_exist":123}}}}}"#,
        br#"{"protocol_version":2,"request":{"search":{"filter":{"filter_range":{"field":"price","lower":{"positive_infinity":{"this_should_not_exist":123}}}}}}}"#,
        br#"{"protocol_version":2,"request":{"search":{"query_expr":{"query_weighted_set":{"field":"tags","items":[{"key":{"string":"rust"},"weight":"1","this_should_not_exist":123}]}}}}}"#,
        br#"{"protocol_version":2,"request":{"search":{"inputs":{"term":{"literal":{"value":{"string":"rust"},"this_should_not_exist":123}}}}}}"#,
        br#"{"protocol_version":2,"request":{"search":{"sort":[{"field":"price","missing":{"as":{"value":{"int64":"0"},"this_should_not_exist":123}}}]}}}"#,
        br#"{"protocol_version":2,"request":{"search":{"query_expr":{"query_true":{"this_should_not_exist":123}}}}}"#,
        br#"{"protocol_version":2,"request":{"search":{"filter":{"none":{"this_should_not_exist":123}}}}}"#,
        br#"{"protocol_version":2,"request":{"search":{"ranking":{"default":{"this_should_not_exist":123}}}}}"#,
        br#"{"protocol_version":2,"request":{"search":{"patch":{"id":"doc:1","operations":[{"none":{"this_should_not_exist":123}}]}}}}"#,
        br#"{"protocol_version":2,"request":{"search":{"query_expr":{"query_alternatives":{"alternatives":[{"query":{"query_true":{}},"this_should_not_exist":123}]}}}}}"#,
    ];
    for input in invalid {
        assert!(
            request_json_to_flatbuffer(input).is_err(),
            "nested unknown field was accepted: {}",
            String::from_utf8_lossy(input)
        );
    }
}

#[test]
fn canonical_request_corpus_round_trips_without_wire_discriminators() {
    let fixtures = request_fixtures();
    assert_eq!(fixtures.len(), 77);
    for (name, bytes) in fixtures {
        let json =
            request_flatbuffer_to_json(&bytes).unwrap_or_else(|error| panic!("{name}: {error}"));
        let value: Value = serde_json::from_slice(&json).unwrap();
        assert!(
            value.get("request").is_some(),
            "{name}: canonical request envelope missing"
        );
        assert!(
            value.get("payload").is_none(),
            "{name}: raw payload field leaked"
        );
        assert_no_wire_discriminators(&value);
        let round_tripped =
            request_json_to_flatbuffer(&json).unwrap_or_else(|error| panic!("{name}: {error}"));
        let round_tripped_value: Value =
            serde_json::from_slice(&request_flatbuffer_to_json(&round_tripped).unwrap()).unwrap();
        assert_eq!(value, round_tripped_value, "{name}");
    }
}
