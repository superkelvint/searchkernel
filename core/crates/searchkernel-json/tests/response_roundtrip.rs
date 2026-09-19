use searchkernel_json::{response_flatbuffer_to_json, response_json_to_flatbuffer};
use serde_json::Value;

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

#[test]
fn canonical_response_corpus_round_trips_without_wire_discriminators() {
    let directory =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/responses");
    let mut paths = std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "skrs")
        })
        .collect::<Vec<_>>();
    paths.sort();
    assert_eq!(paths.len(), 67);
    for path in paths {
        let name = path.file_stem().unwrap().to_string_lossy().into_owned();
        let bytes = std::fs::read(&path).unwrap();
        let json =
            response_flatbuffer_to_json(&bytes).unwrap_or_else(|error| panic!("{name}: {error}"));
        let value: Value = serde_json::from_slice(&json).unwrap();
        assert!(
            value.get("response").is_some(),
            "{name}: canonical response envelope missing"
        );
        assert!(
            value.get("payload").is_none(),
            "{name}: raw payload field leaked"
        );
        assert_no_wire_discriminators(&value);
        let round_tripped =
            response_json_to_flatbuffer(&json).unwrap_or_else(|error| panic!("{name}: {error}"));
        let round_tripped_value: Value =
            serde_json::from_slice(&response_flatbuffer_to_json(&round_tripped).unwrap()).unwrap();
        assert_eq!(value, round_tripped_value, "{name}");
    }
}

#[test]
fn malformed_response_is_rejected() {
    assert!(response_json_to_flatbuffer(br#"{"#).is_err());
    assert!(response_flatbuffer_to_json(b"not a FlatBuffer").is_err());
}

#[test]
fn deadline_exceeded_is_a_supported_error_code() {
    let input = br#"{
      "protocol_version": 2,
      "response": {
        "error": {
          "code": "deadline_exceeded",
          "message": "deadline",
          "retryable": true
        }
      }
    }"#;
    let bytes = response_json_to_flatbuffer(input).expect("deadline error JSON must decode");
    let output = response_flatbuffer_to_json(&bytes).expect("deadline error must encode");
    assert_eq!(
        serde_json::from_slice::<Value>(&output).unwrap(),
        serde_json::from_slice::<Value>(input).unwrap()
    );
}

#[test]
fn response_payload_required_fields_are_enforced_by_the_codec() {
    for input in [
        br#"{"protocol_version":2,"response":{"insert":{}}}"#.as_slice(),
        br#"{"protocol_version":2,"response":{"search":{}}}"#.as_slice(),
        br#"{"protocol_version":2,"response":{"error":{}}}"#.as_slice(),
    ] {
        assert!(
            response_json_to_flatbuffer(input).is_err(),
            "response missing required fields was accepted: {}",
            String::from_utf8_lossy(input)
        );
    }
}

#[test]
fn inspect_query_resolved_request_round_trips_all_fields() {
    let input = serde_json::json!({
        "protocol_version": 2,
        "response": {"inspect_query": {"resolved_request": {
            "query": "shoes",
            "search_fields": [{"field": "title", "weight": 4.0}],
            "filters": [{"filter_exists": {"field": "brand"}}],
            "sort": [{
                "target": {"field": {"field": "price"}},
                "direction": "asc"
            }],
            "grouping": [{
                "root": {"operations": [{
                    "group": {"expression": {"attribute": {"field": "brand"}}}
                }]},
                "continuations": ["cursor"]
            }],
            "ranking": {"profile": "bm25", "properties": {"k1": "1.2"}},
            "inputs": {
                "term": {"literal": {"value": {"string": "shoes"}}},
                "embedding": {"expression": {"expression": "embed(@term)"}}
            },
            "limit": 20,
            "offset": 2,
            "timeout_ms": "1500",
            "query_model": {"language": "en", "locale": "en_US"},
            "presentation": {
                "summary": "short",
                "fields": ["title", "brand"],
                "highlighting": {"fields": ["title"]},
                "format": "json",
                "bolding": true,
                "timing": false
            },
            "properties": {"trace.level": {"string": "2"}},
            "query_profile": "default",
            "search_chain": "query"
        }}}
    });
    let input = serde_json::to_vec(&input).unwrap();
    let bytes = response_json_to_flatbuffer(&input).expect("inspect response JSON");
    let output = response_flatbuffer_to_json(&bytes).expect("inspect response output");
    assert_eq!(
        serde_json::from_slice::<Value>(&output).unwrap(),
        serde_json::from_slice::<Value>(&input).unwrap()
    );
}

#[test]
fn canonical_scalar_policy_round_trips() {
    let json = br#"{
      "protocol_version": 2,
      "response": {
        "search": {
          "root": {"result_group": {"children": []}},
          "total_hits": {"value": "9007199254740993", "relation": "exact"}
        }
      }
    }"#;
    let bytes = response_json_to_flatbuffer(json).expect("canonical scalar response JSON");
    let output = response_flatbuffer_to_json(&bytes).expect("canonical scalar output");
    assert_eq!(
        serde_json::from_slice::<Value>(&output).unwrap(),
        serde_json::from_slice::<Value>(json).unwrap()
    );
}

#[test]
fn optional_uint64_fields_round_trip_as_decimal_strings() {
    let json = br#"{
      "protocol_version": 2,
      "response": {
        "search": {
          "root": {"result_group": {"children": []}},
          "total_hits": {"value": "18446744073709551615", "relation": "exact"},
          "coverage": {
            "documents": "18446744073709551615",
            "results": "9007199254740993",
            "results_full": "18446744073709551615"
          }
        }
      }
    }"#;
    let bytes = response_json_to_flatbuffer(json).expect("canonical uint64 response JSON");
    let output = response_flatbuffer_to_json(&bytes).expect("canonical uint64 response output");
    assert_eq!(
        serde_json::from_slice::<Value>(&output).unwrap(),
        serde_json::from_slice::<Value>(json).unwrap()
    );
}

#[test]
fn optional_uint64_fields_reject_json_numbers() {
    let input = br#"{
      "protocol_version": 2,
      "response": {
        "search": {
          "root": {"result_group": {"children": []}},
          "total_hits": {"value": "1", "relation": "exact"},
          "coverage": {"documents": 1}
        }
      }
    }"#;
    assert!(response_json_to_flatbuffer(input).is_err());
}

#[test]
fn tensor_values_round_trip_in_search_results() {
    let input = br#"{
      "protocol_version": 2,
      "response": {
        "search": {
          "root": {"result_group": {
            "fields": {
              "embedding": {"tensor": {
                "type_spec": "tensor<float>(x[2])",
                "payload": {"dense": {"values": [0.25, 0.5]}}
              }}
            },
            "children": []
          }},
          "total_hits": {"value": "1", "relation": "exact"}
        }
      }
    }"#;
    let bytes = response_json_to_flatbuffer(input).expect("tensor response JSON");
    let output = response_flatbuffer_to_json(&bytes).expect("tensor response JSON output");
    assert_eq!(
        serde_json::from_slice::<Value>(&output).unwrap(),
        serde_json::from_slice::<Value>(input).unwrap()
    );
}

#[test]
fn rich_index_definition_round_trips_through_json() {
    let input = serde_json::json!({
        "protocol_version": 2,
        "response": {"describe": {"index": {
            "name": "schema",
            "document_type": "doc",
            "schema_inherits": ["base"],
            "fields": [
                {"name": "text", "data_type": {"primitive": "string"}, "stored": true,
                 "matching": {"mode": "exact", "options": {"case": {"bool": true}}},
                 "attribute": {"enabled": true, "fast_search": true}},
                {"name": "tensor", "data_type": {"tensor": {"type_spec": "tensor(x[2])"}}},
                {"name": "array", "data_type": {"array": {"element_type": {"primitive": "int64"}}}},
                {"name": "map", "data_type": {"map": {"key_type": {"primitive": "string"}, "value_type": {"primitive": "float32"}}}},
                {"name": "weighted", "data_type": {"weighted_set": {"element_type": {"primitive": "string"}}}},
                {"name": "inline", "data_type": {"struct": {"fields": [{"name": "part", "data_type": {"primitive": "bool"}}]}}},
                {"name": "named", "data_type": {"struct_reference": {"name": "address"}}},
                {"name": "predicate", "data_type": {"predicate": {}}},
                {"name": "position", "data_type": {"position": {}}},
                {"name": "parent", "data_type": {"reference": {"document_type": "parent_doc"}}}
            ],
            "default_search_fields": [{"field": "text", "weight": 1.0}],
            "properties": {"schema.version": {"int64": "2"}},
            "structs": [{"name": "address", "fields": [{"name": "city", "data_type": {"primitive": "string"}}]}],
            "fieldsets": [{"name": "all_text", "fields": ["text"], "query_commands": ["stem"]}],
            "schema_fields": [{"field": {"name": "derived", "data_type": {"primitive": "string"}}}],
            "imported_fields": [{"reference_field": "parent", "target_field": "name", "alias": "parent_name"}],
            "document_summaries": [{"name": "short", "fields": [{"name": "text", "source": "text", "data_type": {"primitive": "string"}}], "from_disk": false, "omit_summary_features": true}],
            "constants": [{"name": "bias", "data_type": {"primitive": "float64"}, "source": {"literal": {"value": {"float64": 1.0}}}}],
            "onnx_models": [{"name": "model", "file": "model.onnx", "inputs": {"input": "query(q)"}, "gpu_device": 0, "intraop_threads": 1}],
            "rank_profiles": [{"name": "rank", "match_features": ["score"], "rank_properties": {"foo": "bar"}, "strict": true}]
        }}}
    });
    let input = serde_json::to_vec(&input).unwrap();
    let bytes = response_json_to_flatbuffer(&input).expect("rich index JSON");
    let output = response_flatbuffer_to_json(&bytes).expect("rich index output");
    assert_eq!(
        serde_json::from_slice::<Value>(&output).unwrap(),
        serde_json::from_slice::<Value>(&input).unwrap()
    );
}
