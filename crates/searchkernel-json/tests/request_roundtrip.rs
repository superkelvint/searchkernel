use searchkernel_json::{request_flatbuffer_to_json, request_json_to_flatbuffer, JsonCodecError};

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

    let mut embedded_nul = br#"{\"protocol_version\":1}"#.to_vec();
    embedded_nul.push(0);
    embedded_nul.extend_from_slice(b"garbage");
    assert!(matches!(
        request_json_to_flatbuffer(&embedded_nul),
        Err(JsonCodecError::InvalidJson(message)) if message == "JSON input contains NUL byte"
    ));
}

#[test]
fn rich_search_request_round_trips_all_nested_request_components() {
    let json = br#"{
        "protocol_version": 1,
        "payload_type": "SearchKernel_V1_SearchRequest",
        "payload": {
            "query": "wireless",
            "search_fields": {"fields": [
                {"field": "title", "weight": {"value": 1.25}}
            ]},
            "filter": {
                "expr_type": "FilterAnd",
                "expr": {"clauses": [
                    {
                        "expr_type": "FilterComparison",
                        "expr": {"field": "brand", "op": "Eq", "value": {
                            "kind_type": "StringValue", "kind": {"value": "Sony"}
                        }}
                    },
                    {
                        "expr_type": "FilterRange",
                        "expr": {"field": "price", "lower": {
                            "kind_type": "Float64Value", "kind": {"value": 10.0}
                        }, "upper": {
                            "kind_type": "Float64Value", "kind": {"value": 100.0}
                        }, "include_lower": {"value": true}, "include_upper": {"value": false}}
                    }
                ]}
            },
            "sort": [{"field": "price", "direction": "Asc", "missing": {"value": "First"}}],
            "facets": [{"field": "brand", "limit": {"value": 25}, "sort": {"value": "CountDesc"}}],
            "ranking": {"policy_type": "Bm25Ranking", "policy": {}},
            "limit": {"value": 0},
            "offset": {"value": 17},
            "projection": {"fields": ["title", "brand"]},
            "highlighting": {"fields": ["title"]},
            "language": "en",
            "query_expr": {
                "expr_type": "TermQuery",
                "expr": {"field": "title", "value": {
                    "kind_type": "StringValue", "kind": {"value": "wireless"}
                }}
            }
        }
    }"#;
    let bytes = request_json_to_flatbuffer(json).expect("rich request JSON");
    let output = request_flatbuffer_to_json(&bytes).expect("rich request output");
    let value: serde_json::Value = serde_json::from_slice(&output).expect("valid JSON");
    let round_tripped = request_json_to_flatbuffer(&output).expect("rich request roundtrip");
    let round_tripped_json = request_flatbuffer_to_json(&round_tripped).unwrap();
    let round_tripped_value: serde_json::Value =
        serde_json::from_slice(&round_tripped_json).unwrap();
    assert_eq!(value, round_tripped_value);
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
