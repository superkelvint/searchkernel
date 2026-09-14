use searchkernel_json::{request_flatbuffer_to_json, request_json_to_flatbuffer, JsonCodecError};
use serde_json::Value;

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

#[test]
fn canonical_symbolic_union_json_is_accepted() {
    let json = br#"{
        "protocol_version": 1,
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
fn official_flatbuffers_union_json_is_rejected() {
    let json = br#"{
        "protocol_version": 1,
        "payload_type": "SearchKernel_V1_SearchRequest",
        "payload": {"query": "foo"}
    }"#;
    assert!(request_json_to_flatbuffer(json).is_err());
}

#[test]
fn malformed_inputs_are_rejected() {
    assert!(request_json_to_flatbuffer(br#"{"#).is_err());
    assert!(request_flatbuffer_to_json(b"not a FlatBuffer").is_err());

    let mut embedded_nul = br#"{\"protocol_version\":1}"#.to_vec();
    embedded_nul.push(0);
    embedded_nul.extend_from_slice(b"garbage");
    assert!(matches!(
        request_json_to_flatbuffer(&embedded_nul),
        Err(JsonCodecError::InvalidJson(message)) if message == "JSON input contains NUL byte"
    ));
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
