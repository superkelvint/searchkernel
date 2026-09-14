use searchkernel_json::{
    request_flatbuffer_to_json, request_json_to_flatbuffer, response_flatbuffer_to_json,
    response_json_to_flatbuffer,
};
use serde_json::Value;
use std::path::{Path, PathBuf};

fn json(bytes: &[u8]) -> Value {
    serde_json::from_slice(bytes).expect("codec must emit valid JSON")
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
                    "raw FlatBuffers discriminator leaked into canonical JSON: {key}"
                );
                assert_no_wire_discriminators(child);
            }
        }
        Value::Array(values) => {
            for child in values {
                assert_no_wire_discriminators(child);
            }
        }
        _ => {}
    }
}

fn fixtures(kind: &str, extension: &str) -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../crates/searchkernel-json/tests/fixtures")
        .join(kind);
    let mut paths = std::fs::read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == extension))
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

fn assert_request_roundtrip_corpus() {
    let paths = fixtures("requests", "skrq");
    assert_eq!(paths.len(), 77, "request corpus size changed; update frozen C6 verifier first");
    for path in paths {
        let name = path.file_name().unwrap().to_string_lossy();
        let native = std::fs::read(&path).unwrap();
        let encoded = request_flatbuffer_to_json(&native)
            .unwrap_or_else(|e| panic!("{name}: FB→JSON failed: {e}"));
        let value = json(&encoded);
        assert!(value.get("request").is_some(), "{name}: missing canonical request envelope");
        assert!(value.get("payload").is_none(), "{name}: raw payload field leaked");
        assert_no_wire_discriminators(&value);
        let roundtrip = request_json_to_flatbuffer(&encoded)
            .unwrap_or_else(|e| panic!("{name}: JSON→FB failed: {e}"));
        let again = request_flatbuffer_to_json(&roundtrip).unwrap();
        assert_eq!(value, json(&again), "{name}: canonical JSON round-trip changed meaning");
    }
}

fn assert_response_roundtrip_corpus() {
    let paths = fixtures("responses", "skrs");
    assert_eq!(paths.len(), 67, "response corpus size changed; update frozen C6 verifier first");
    for path in paths {
        let name = path.file_name().unwrap().to_string_lossy();
        let native = std::fs::read(&path).unwrap();
        let encoded = response_flatbuffer_to_json(&native)
            .unwrap_or_else(|e| panic!("{name}: FB→JSON failed: {e}"));
        let value = json(&encoded);
        assert!(value.get("response").is_some(), "{name}: missing canonical response envelope");
        assert!(value.get("payload").is_none(), "{name}: raw payload field leaked");
        assert_no_wire_discriminators(&value);
        let roundtrip = response_json_to_flatbuffer(&encoded)
            .unwrap_or_else(|e| panic!("{name}: JSON→FB failed: {e}"));
        let again = response_flatbuffer_to_json(&roundtrip).unwrap();
        assert_eq!(value, json(&again), "{name}: canonical JSON round-trip changed meaning");
    }
}

fn assert_minimal_request_shape() {
    let input = br#"{
      "protocol_version": 1,
      "request": {"search": {"query": "foo"}}
    }"#;
    let fb = request_json_to_flatbuffer(input).expect("canonical request JSON must decode");
    let output = json(&request_flatbuffer_to_json(&fb).unwrap());
    let expected = json(input);
    assert_eq!(output, expected);
}

fn assert_symbolic_nested_unions() {
    let input = br#"{
      "protocol_version": 1,
      "request": {
        "search": {
          "query": "wireless",
          "filter": {
            "filter_and": {
              "clauses": [
                {"filter_comparison": {
                  "field": "brand",
                  "op": "eq",
                  "value": {"string": "Sony"}
                }},
                {"filter_range": {
                  "field": "price",
                  "lower": {"float64": 10.0},
                  "upper": {"float64": 100.0},
                  "include_lower": true,
                  "include_upper": false
                }}
              ]
            }
          },
          "ranking": {"bm25_ranking": {}},
          "query_expr": {
            "term_query": {
              "field": "title",
              "value": {"string": "wireless"}
            }
          }
        }
      }
    }"#;
    let fb = request_json_to_flatbuffer(input).expect("symbolic unions must decode");
    let output = json(&request_flatbuffer_to_json(&fb).unwrap());
    assert_eq!(output, json(input));
    assert_no_wire_discriminators(&output);
}

fn assert_scalar_policy() {
    let input = br#"{
      "protocol_version": 1,
      "response": {
        "search": {
          "total_hits": {"value": "9007199254740993", "relation": "exact"},
          "facets": [{
            "field": "values",
            "values": [
              {"value": {"int64": "-9223372036854775808"}, "count": "42"},
              {"value": {"uint64": "18446744073709551615"}, "count": "18446744073709551615"},
              {"value": {"bytes": "AAH/"}, "count": "1"},
              {"value": {"float64": 1.5}, "count": "1"}
            ]
          }]
        }
      }
    }"#;
    let fb = response_json_to_flatbuffer(input).expect("canonical scalar policy must decode");
    let output = json(&response_flatbuffer_to_json(&fb).unwrap());
    assert_eq!(output, json(input));

    for invalid in [
        br#"{"protocol_version":1,"response":{"search":{"facets":[{"field":"x","values":[{"value":{"int64":42},"count":"1"}]}]}}}"#.as_slice(),
        br#"{"protocol_version":1,"response":{"search":{"facets":[{"field":"x","values":[{"value":{"uint64":18446744073709551615},"count":"1"}]}]}}}"#.as_slice(),
        br#"{"protocol_version":1,"response":{"search":{"facets":[{"field":"x","values":[{"value":{"int64":"+1"},"count":"1"}]}]}}}"#.as_slice(),
        br#"{"protocol_version":1,"response":{"search":{"facets":[{"field":"x","values":[{"value":{"int64":"01"},"count":"1"}]}]}}}"#.as_slice(),
        br#"{"protocol_version":1,"response":{"search":{"facets":[{"field":"x","values":[{"value":{"int64":"-0"},"count":"1"}]}]}}}"#.as_slice(),
        br#"{"protocol_version":1,"response":{"search":{"facets":[{"field":"x","values":[{"value":{"bytes":"***"},"count":"1"}]}]}}}"#.as_slice(),
        br#"{"protocol_version":1,"response":{"search":{"facets":[{"field":"x","values":[{"value":{"int64":"9223372036854775808"},"count":"1"}]}]}}}"#.as_slice(),
        br#"{"protocol_version":1,"response":{"search":{"facets":[{"field":"x","values":[{"value":{"uint64":"18446744073709551616"},"count":"1"}]}]}}}"#.as_slice(),
        br#"{"protocol_version":1,"response":{"search":{"facets":[{"field":"x","values":[{"value":{"uint64":"-1"},"count":"1"}]}]}}}"#.as_slice(),
    ] {
        assert!(response_json_to_flatbuffer(invalid).is_err(), "invalid canonical scalar was accepted: {}", String::from_utf8_lossy(invalid));
    }
}

fn assert_presence_policy() {
    let absent = br#"{"protocol_version":1,"request":{"search":{"query":"x"}}}"#;
    let empty = br#"{"protocol_version":1,"request":{"search":{"query":"x","search_fields":[]}}}"#;
    let zero = br#"{"protocol_version":1,"request":{"search":{"query":"x","limit":0}}}"#;

    let absent_fb = request_json_to_flatbuffer(absent).unwrap();
    let empty_fb = request_json_to_flatbuffer(empty).unwrap();
    let zero_fb = request_json_to_flatbuffer(zero).unwrap();

    let absent_out = json(&request_flatbuffer_to_json(&absent_fb).unwrap());
    let empty_out = json(&request_flatbuffer_to_json(&empty_fb).unwrap());
    let zero_out = json(&request_flatbuffer_to_json(&zero_fb).unwrap());

    assert_eq!(absent_out, json(absent));
    assert_eq!(empty_out, json(empty));
    assert_eq!(zero_out, json(zero));
    assert_ne!(absent_out, empty_out);
    assert_ne!(absent_out, zero_out);
}

fn assert_strict_rejections() {
    let invalid: &[&[u8]] = &[
        br#"{"protocol_version":1,"request":{"search":{"query":"x","limt":20}}}"#,
        br#"{"protocol_version":1,"request":{"search":{"query":"x","query":"y"}}}"#,
        br#"{"protocol_version":1,"request":{"search":{"query":"x","limit":null}}}"#,
        br#"{"protocol_version":1,"request":{"search":{"query":"x","filter":{}}}}"#,
        br#"{"protocol_version":1,"request":{"search":{"query":"x","filter":{"filter_exists":{"field":"a"},"filter_not":{"clause":{"filter_exists":{"field":"b"}}}}}}}"#,
        br#"{"protocol_version":1,"request":{"search":{"query":"x","filter":{"made_up":{"field":"a"}}}}}"#,
        br#"{"protocol_version":1,"request":{}}"#,
        br#"{"protocol_version":1,"request":{"search":{"query":"x"},"get":{"id":"1"}}}"#,
        br#"{"protocol_version":1,"request":{"search":{"query":"x","ranking":{"made_up":{}}}}}"#,
        br#"{"protocol_version":1,"request":{"search":{"query":"x","sort":[{"field":"x","direction":"made_up"}]}}}"#,
        br#"{"protocol_version":1,"payload_type":"SearchKernel_V1_SearchRequest","payload":{"query":"x"}}"#,
    ];
    for bytes in invalid {
        assert!(
            request_json_to_flatbuffer(bytes).is_err(),
            "invalid canonical request was accepted: {}",
            String::from_utf8_lossy(bytes)
        );
    }
}

fn main() {
    assert_minimal_request_shape();
    assert_symbolic_nested_unions();
    assert_scalar_policy();
    assert_presence_policy();
    assert_strict_rejections();
    assert_request_roundtrip_corpus();
    assert_response_roundtrip_corpus();
    println!("C6 canonical JSON contract PASS");
}
