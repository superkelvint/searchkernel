use super::*;

#[test]
fn bfbs_json_bridge_preserves_empty_portable_values_into_ir() {
    use searchkernel_protocol::protocol::v1 as wire;

    let json = br#"{
      "protocol_version": 1,
      "payload_type": "SearchKernel_Wire_SearchRequest",
      "payload": {
        "query": {
          "source_type": "MatchAllQuery",
          "source": {}
        },
        "properties": [
          {
            "key": "empty_string",
            "value": {
              "kind_type": "StringValue",
              "kind": { "value": "" }
            }
          },
          {
            "key": "empty_bytes",
            "value": {
              "kind_type": "BytesValue",
              "kind": { "value": [] }
            }
          }
        ]
      }
    }"#;

    let bytes = searchkernel_protocol::json::request_json_to_flatbuffer(json)
        .expect("standard FlatBuffers JSON must parse");
    let envelope = crate::__flatbuffers::root::<wire::RequestEnvelope>(&bytes)
        .expect("request envelope must decode");
    let request = envelope
        .payload_as_search_kernel_wire_search_request()
        .expect("search payload discriminator must match");
    let ir = context()
        .search_to_ir(request)
        .expect("empty portable string/bytes must survive the BFBS JSON bridge");

    assert_eq!(
        ir.properties().get("empty_string"),
        Some(&RequestValueIr::String(String::new()))
    );
    assert_eq!(
        ir.properties().get("empty_bytes"),
        Some(&RequestValueIr::Bytes(Vec::new()))
    );
}
