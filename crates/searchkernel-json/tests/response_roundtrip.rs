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

#[test]
fn rich_search_response_round_trips_nested_values_and_highlights() {
    let json = br#"{
        "protocol_version": 1,
        "payload_type": "SearchKernel_V1_SearchResult",
        "payload": {
            "hits": [{
                "id": "doc-1",
                "score": {"value": 0.0},
                "fields": [{
                    "key": "tags",
                    "value": {
                        "kind_type": "ValueList",
                        "kind": {"values": [
                            {"kind_type": "StringValue", "kind": {"value": "sale"}},
                            {"kind_type": "BoolValue", "kind": {"value": true}},
                            {"kind_type": "Float64Value", "kind": {"value": 1.5}},
                            {"kind_type": "Int64Value", "kind": {"value": -9223372036854775808}},
                            {"kind_type": "UInt64Value", "kind": {"value": 18446744073709551615}},
                            {"kind_type": "BytesValue", "kind": {"value": [0, 1, 255]}},
                            {"kind_type": "ValueObject", "kind": {"fields": [
                                {"key": "nested", "value": {"kind_type": "StringValue", "kind": {"value": "yes"}}}
                            ]}}
                        ]}
                    }
                }],
                "highlights": [{
                    "field": "title",
                    "fragments": [{
                        "text": "wireless audio",
                        "matches": [{"start_byte": 0, "end_byte": 8}]
                    }]
                }]
            }],
            "total_hits": {"value": 9007199254740993, "relation": "Exact"},
            "facets": [{
                "field": "brand",
                "values": [{
                    "value": {"kind_type": "StringValue", "kind": {"value": "Sony"}},
                    "count": 42
                }]
            }]
        }
    }"#;
    let bytes = response_json_to_flatbuffer(json).expect("rich response JSON");
    let output = response_flatbuffer_to_json(&bytes).expect("rich response output");
    let value: serde_json::Value = serde_json::from_slice(&output).expect("valid JSON");
    let round_tripped = response_json_to_flatbuffer(&output).expect("rich response roundtrip");
    let round_tripped_json = response_flatbuffer_to_json(&round_tripped).unwrap();
    let round_tripped_value: serde_json::Value =
        serde_json::from_slice(&round_tripped_json).unwrap();
    assert_eq!(value, round_tripped_value);
}

#[test]
fn rich_describe_response_round_trips_schema_presence_and_defaults() {
    let json = br#"{
        "protocol_version": 1,
        "payload_type": "SearchKernel_V1_DescribeResult",
        "payload": {
            "index": {
                "name": "products",
                "fields": [{
                    "name": "title",
                    "kind": "Text",
                    "cardinality": "Single",
                    "stored": {"value": true},
                    "searchable": {"value": true},
                    "filterable": {"value": false},
                    "sortable": {"value": false},
                    "facetable": {"value": false},
                    "analyzer": {"name": "standard", "version": 1, "options": []}
                }, {
                    "name": "tags",
                    "kind": "Keyword",
                    "cardinality": "Multi",
                    "stored": {"value": true},
                    "searchable": {"value": true},
                    "filterable": {"value": true},
                    "sortable": {"value": false},
                    "facetable": {"value": true}
                }],
                "default_search_fields": {"fields": [
                    {"field": "title", "weight": {"value": 2.0}}
                ]}
            }
        }
    }"#;
    let bytes = response_json_to_flatbuffer(json).expect("rich describe JSON");
    let output = response_flatbuffer_to_json(&bytes).expect("rich describe output");
    let value: serde_json::Value = serde_json::from_slice(&output).expect("valid JSON");
    let round_tripped = response_json_to_flatbuffer(&output).expect("rich describe roundtrip");
    let round_tripped_json = response_flatbuffer_to_json(&round_tripped).unwrap();
    let round_tripped_value: serde_json::Value =
        serde_json::from_slice(&round_tripped_json).unwrap();
    assert_eq!(value, round_tripped_value);
}
