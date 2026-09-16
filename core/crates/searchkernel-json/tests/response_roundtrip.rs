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
      "protocol_version": 1,
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
        br#"{"protocol_version":1,"response":{"insert":{}}}"#.as_slice(),
        br#"{"protocol_version":1,"response":{"search":{}}}"#.as_slice(),
        br#"{"protocol_version":1,"response":{"error":{}}}"#.as_slice(),
    ] {
        assert!(
            response_json_to_flatbuffer(input).is_err(),
            "response missing required fields was accepted: {}",
            String::from_utf8_lossy(input)
        );
    }
}

#[test]
fn canonical_scalar_policy_round_trips() {
    let json = br#"{
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
    let bytes = response_json_to_flatbuffer(json).expect("canonical scalar response JSON");
    let output = response_flatbuffer_to_json(&bytes).expect("canonical scalar output");
    assert_eq!(
        serde_json::from_slice::<Value>(&output).unwrap(),
        serde_json::from_slice::<Value>(json).unwrap()
    );
}
