use searchkernel_json::{request_flatbuffer_to_json, request_json_to_flatbuffer};

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

#[test]
fn official_union_json_is_accepted() {
    let json = br#"{
        "protocol_version": 1,
        "payload_type": "SearchKernel_V1_SearchRequest",
        "payload": {"query": "foo"}
    }"#;
    let bytes = request_json_to_flatbuffer(json).expect("official request JSON");
    let output = request_flatbuffer_to_json(&bytes).expect("request JSON output");
    let value: serde_json::Value = serde_json::from_slice(&output).expect("valid JSON");
    assert_eq!(value["payload_type"], "SearchKernel_V1_SearchRequest");
    assert_eq!(value["payload"]["query"], "foo");
}

#[test]
fn malformed_inputs_are_rejected() {
    assert!(request_json_to_flatbuffer(br#"{"#).is_err());
    assert!(request_flatbuffer_to_json(b"not a FlatBuffer").is_err());
}

#[test]
fn canonical_request_corpus_round_trips_through_official_json() {
    let fixtures = request_fixtures();
    assert_eq!(fixtures.len(), 77);
    for (name, bytes) in fixtures {
        let json =
            request_flatbuffer_to_json(&bytes).unwrap_or_else(|error| panic!("{name}: {error}"));
        let value: serde_json::Value = serde_json::from_slice(&json).unwrap();
        assert!(
            value.get("operation").is_none(),
            "{name}: old operation envelope"
        );
        assert!(
            value.get("request").is_none(),
            "{name}: old request envelope"
        );
        let round_tripped =
            request_json_to_flatbuffer(&json).unwrap_or_else(|error| panic!("{name}: {error}"));
        let round_tripped_json = request_flatbuffer_to_json(&round_tripped).unwrap();
        let round_tripped_value: serde_json::Value =
            serde_json::from_slice(&round_tripped_json).unwrap();
        assert_eq!(value, round_tripped_value, "{name}");
    }
}
