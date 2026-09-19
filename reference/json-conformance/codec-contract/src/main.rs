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
        .join("../../../core/crates/searchkernel-json/tests/fixtures")
        .join(kind);
    let mut paths = std::fs::read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == extension))
        .collect::<Vec<_>>();
    paths.sort_by_key(|path| {
        path.file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_default()
    });
    paths
}

fn fixture_manifest() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../expected/fixture-manifest.json");
    json(&std::fs::read(path).expect("fixture manifest must be readable"))
}

fn manifest_names(manifest: &Value, kind: &str) -> Vec<String> {
    manifest
        .get(kind)
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("fixture manifest missing {kind} list"))
        .iter()
        .map(|value| value.as_str().unwrap_or_else(|| panic!("{kind} fixture name must be a string")))
        .map(ToOwned::to_owned)
        .collect()
}

fn assert_fixture_set(paths: &[PathBuf], expected: &[String], kind: &str) {
    let actual = paths
        .iter()
        .map(|path| path.file_stem().unwrap().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    assert_eq!(actual, expected, "{kind} fixture set changed; update fixture-manifest.json");
}

fn assert_request_roundtrip_corpus() {
    let paths = fixtures("requests", "skrq");
    let manifest = fixture_manifest();
    assert_fixture_set(&paths, &manifest_names(&manifest, "requests"), "request");
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
    let manifest = fixture_manifest();
    assert_fixture_set(&paths, &manifest_names(&manifest, "responses"), "response");
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
      "protocol_version":2,
      "request": {"search": {"query": "foo"}}
    }"#;
    let fb = request_json_to_flatbuffer(input).expect("canonical request JSON must decode");
    let output = json(&request_flatbuffer_to_json(&fb).unwrap());
    let expected = json(input);
    assert_eq!(output, expected);
}

fn assert_symbolic_nested_unions() {
    let input = br#"{
      "protocol_version":2,
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
          "ranking": "bm25"
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
      "protocol_version":2,
      "response": {
        "search": {
          "root": {"result_group": {"children": []}},
          "total_hits": {"value": "9007199254740993", "relation": "exact"}
        }
      }
    }"#;
    let fb = response_json_to_flatbuffer(input).expect("canonical scalar policy must decode");
    let output = json(&response_flatbuffer_to_json(&fb).unwrap());
    assert_eq!(output, json(input));

    for invalid in [
        br#"{"protocol_version":2,"response":{"search":{"root":{"result_group":{"children":[]}},"total_hits":{"value":42,"relation":"exact"}}}}"#.as_slice(),
        br#"{"protocol_version":2,"response":{"search":{"root":{"result_group":{"children":[]}},"total_hits":{"value":"+1","relation":"exact"}}}}"#.as_slice(),
        br#"{"protocol_version":2,"response":{"search":{"root":{"result_group":{"children":[]}},"total_hits":{"value":"01","relation":"exact"}}}}"#.as_slice(),
        br#"{"protocol_version":2,"response":{"search":{"root":{"result_group":{"children":[]}},"total_hits":{"value":"-1","relation":"exact"}}}}"#.as_slice(),
    ] {
        assert!(response_json_to_flatbuffer(invalid).is_err(), "invalid canonical scalar was accepted: {}", String::from_utf8_lossy(invalid));
    }
}

fn assert_presence_policy() {
    let absent = br#"{"protocol_version":2,"request":{"search":{"query":"x"}}}"#;
    let empty = br#"{"protocol_version":2,"request":{"search":{"query":"x","search_fields":[]}}}"#;
    let zero = br#"{"protocol_version":2,"request":{"search":{"query":"x","limit":0}}}"#;

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
        br#"{"protocol_version":2,"request":{"search":{"query":"x","limt":20}}}"#,
        br#"{"protocol_version":2,"request":{"search":{"query":"x","query":"y"}}}"#,
        br#"{"protocol_version":2,"request":{"search":{"query":"x","limit":null}}}"#,
        br#"{"protocol_version":2,"request":{"search":{"query":"x","filter":{}}}}"#,
        br#"{"protocol_version":2,"request":{"search":{"query":"x","filter":{"filter_exists":{"field":"a"},"filter_not":{"clause":{"filter_exists":{"field":"b"}}}}}}}"#,
        br#"{"protocol_version":2,"request":{"search":{"query":"x","filter":{"made_up":{"field":"a"}}}}}"#,
        br#"{"protocol_version":2,"request":{}}"#,
        br#"{"protocol_version":2,"request":{"search":{"query":"x"},"get":{"id":"1"}}}"#,
        br#"{"protocol_version":2,"request":{"search":{"query":"x","ranking":{"made_up":{}}}}}"#,
        br#"{"protocol_version":2,"request":{"search":{"query":"x","sort":[{"field":"x","direction":"made_up"}]}}}"#,
        br#"{"protocol_version":2,"payload_type":"SearchKernel_V1_SearchRequest","payload":{"query":"x"}}"#,
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
