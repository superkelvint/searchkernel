use searchkernel_json::{response_flatbuffer_to_json, response_json_to_flatbuffer};

#[test]
fn canonical_response_corpus_round_trips_through_official_json() {
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
        let value: serde_json::Value = serde_json::from_slice(&json).unwrap();
        assert!(
            value.get("operation").is_none(),
            "{name}: old operation envelope"
        );
        assert!(
            value.get("response").is_none(),
            "{name}: old response envelope"
        );
        let round_tripped =
            response_json_to_flatbuffer(&json).unwrap_or_else(|error| panic!("{name}: {error}"));
        let round_tripped_json = response_flatbuffer_to_json(&round_tripped).unwrap();
        let round_tripped_value: serde_json::Value =
            serde_json::from_slice(&round_tripped_json).unwrap();
        assert_eq!(value, round_tripped_value, "{name}");
    }
}

#[test]
fn malformed_response_is_rejected() {
    assert!(response_json_to_flatbuffer(br#"{"#).is_err());
    assert!(response_flatbuffer_to_json(b"not a FlatBuffer").is_err());
}
