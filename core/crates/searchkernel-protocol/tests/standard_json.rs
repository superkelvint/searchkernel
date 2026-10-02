use searchkernel_protocol::json::{
    request_flatbuffer_to_json, request_json_to_flatbuffer, response_flatbuffer_to_json,
    response_json_to_flatbuffer,
};
use searchkernel_protocol::protocol::v1 as wire;

const RICH_REQUEST: &[u8] = br#"
{
  "protocol_version": 1,
  "payload_type": "SearchKernel_Wire_InsertRequest",
  "payload": {
    "document": {
      "id": "id:ns:type::1",
      "fields": [
        {"key":"text","value":{"kind_type":"StringValue","kind":{"value":"hello"}}},
        {"key":"count","value":{"kind_type":"Int64Value","kind":{"value":"9223372036854775807"}}},
        {"key":"raw","value":{"kind_type":"BytesValue","kind":{"value":[1,2,3]}}},
        {"key":"list","value":{"kind_type":"ValueList","kind":{"values":[{"kind_type":"BoolValue","kind":{"value":true}}]}}},
        {"key":"tensor","value":{"kind_type":"TensorValue","kind":{"type_spec":"tensor<float>(x[2])","payload_type":"DenseTensorValues","payload":{"values":[1.0,2.0]}}}}
      ]
    }
  }
}
"#;

const WIDENED_SEARCH_REQUEST: &[u8] = br#"
{
  "protocol_version": 1,
  "payload_type": "SearchKernel_Wire_SearchRequest",
  "payload": {
    "query": {"source_type":"SearchKernelTextQuery","source":{"text":"fox"}},
    "ranking": {"profile":"bm25","properties":[{"key":"rankproperty","value":"value"}]},
    "inputs": [{"name":"query_embedding","input_type":"LiteralQueryInput","input":{"value":{"kind_type":"Float64Value","kind":{"value":0.5}}}}],
    "grouping": [{"source_type":"StructuredGroupingSource","source":{"root":{"operations":[
      {"operation_type":"GroupingGroupOperation","operation":{"expression":{"expr_type":"GroupingAttributeExpression","expr":{"field":"brand"}}}},
      {"operation_type":"GroupingOutputOperation","operation":{"outputs":[{"label":"count","expression":{"expr_type":"GroupingAggregationExpression","expr":{"aggregation_type":"CountAggregation","aggregation":{}}}}]}}
    ]}}}]
  }
}
"#;

const INTEGER_BOUNDARY_REQUEST: &[u8] = br#"
{
  "protocol_version": 1,
  "payload_type": "SearchKernel_Wire_InsertRequest",
  "payload": {
    "document": {
      "id": "id:ns:type::integer-boundaries",
      "fields": [
        {"key":"i64_min","value":{"kind_type":"Int64Value","kind":{"value":"-9223372036854775808"}}},
        {"key":"i64_max","value":{"kind_type":"Int64Value","kind":{"value":"9223372036854775807"}}},
        {"key":"u64_max","value":{"kind_type":"UInt64Value","kind":{"value":"18446744073709551615"}}}
      ]
    }
  }
}
"#;

const STRUCTURED_QUERY_REQUEST: &[u8] = br#"
{
  "protocol_version": 1,
  "payload_type": "SearchKernel_Wire_SearchRequest",
  "payload": {
    "query": {"source_type":"TypedQuery","source":{"query":{"expr_type":"TermQuery","expr":{"value":{"kind_type":"StringValue","kind":{"value":"fox"}}}}}}
  }
}
"#;

#[test]
fn standard_json_round_trips_envelopes_unions_scalars_bytes_lists_and_tensors() {
    let bytes = request_json_to_flatbuffer(RICH_REQUEST).expect("standard request JSON");
    assert_eq!(&bytes[4..8], b"SKRQ");

    let printed = request_flatbuffer_to_json(&bytes).expect("request JSON printer");
    let value: serde_json::Value = serde_json::from_slice(&printed).expect("valid JSON");
    assert_eq!(value["payload_type"], "SearchKernel_Wire_InsertRequest");
    let fields = value["payload"]["document"]["fields"]
        .as_array()
        .expect("document fields array");
    let field = |name: &str| {
        fields
            .iter()
            .find(|field| field["key"] == name)
            .unwrap_or_else(|| panic!("missing field {name}"))
    };
    assert_eq!(
        field("count")["value"]["kind"]["value"],
        "9223372036854775807"
    );
    assert_eq!(
        field("raw")["value"]["kind"]["value"],
        serde_json::json!([1, 2, 3])
    );
    assert_eq!(
        field("tensor")["value"]["kind"]["payload_type"],
        "DenseTensorValues"
    );

    let again =
        request_json_to_flatbuffer(&printed).expect("printed standard JSON remains readable");
    assert!(!again.is_empty());
}

#[test]
fn standard_json_rejects_bare_64_bit_integer_scalars() {
    let input = br#"{
      "protocol_version": 1,
      "payload_type": "SearchKernel_Wire_InsertRequest",
      "payload": {
        "document": {
          "id": "id:ns:type::bare-int64",
          "fields": [
            {"key":"count","value":{"kind_type":"Int64Value","kind":{"value":9223372036854775807}}}
          ]
        }
      }
    }"#;

    let error = request_json_to_flatbuffer(input).expect_err("bare int64 JSON must fail closed");
    assert!(
        error
            .to_string()
            .contains("64-bit integer JSON values must be decimal strings"),
        "unexpected error: {error}"
    );
}

#[test]
fn standard_json_prints_64_bit_integer_boundaries_as_decimal_strings() {
    let bytes =
        request_json_to_flatbuffer(INTEGER_BOUNDARY_REQUEST).expect("quoted integer boundary JSON");
    let printed = request_flatbuffer_to_json(&bytes).expect("integer boundary JSON printer");
    let value: serde_json::Value = serde_json::from_slice(&printed).expect("valid JSON");
    let fields = value["payload"]["document"]["fields"]
        .as_array()
        .expect("document fields array");
    let field = |name: &str| {
        fields
            .iter()
            .find(|field| field["key"] == name)
            .unwrap_or_else(|| panic!("missing field {name}"))
    };

    for (name, expected) in [
        ("i64_min", "-9223372036854775808"),
        ("i64_max", "9223372036854775807"),
        ("u64_max", "18446744073709551615"),
    ] {
        assert_eq!(
            field(name)["value"]["kind"]["value"].as_str(),
            Some(expected),
            "{name} must use the canonical decimal-string JSON representation"
        );
    }

    let again = request_json_to_flatbuffer(&printed)
        .expect("printed integer boundary JSON remains readable");
    assert!(!again.is_empty());
}

#[test]
fn standard_json_canonicalizes_64_bit_scalars_outside_value_unions() {
    let request = br#"{
      "protocol_version": 1,
      "payload_type": "SearchKernel_Wire_SearchRequest",
      "payload": {
        "timeout_ms": {"value": "18446744073709551615"}
      }
    }"#;
    let request_bytes = request_json_to_flatbuffer(request).expect("request uint64 decimal string");
    let request_printed =
        request_flatbuffer_to_json(&request_bytes).expect("request uint64 JSON printer");
    let request_value: serde_json::Value =
        serde_json::from_slice(&request_printed).expect("valid request JSON");
    assert_eq!(
        request_value["payload"]["timeout_ms"]["value"].as_str(),
        Some("18446744073709551615")
    );

    let response = br#"{
      "protocol_version": 1,
      "payload_type": "SearchKernel_Wire_SearchResult",
      "payload": {
        "total_hits": {"value": "18446744073709551615", "relation": "Exact"},
        "errors": [{"code": "-9223372036854775808", "message": "boundary"}]
      }
    }"#;
    let response_bytes =
        response_json_to_flatbuffer(response).expect("response 64-bit decimal strings");
    let response_printed =
        response_flatbuffer_to_json(&response_bytes).expect("response 64-bit JSON printer");
    let response_value: serde_json::Value =
        serde_json::from_slice(&response_printed).expect("valid response JSON");
    assert_eq!(
        response_value["payload"]["total_hits"]["value"].as_str(),
        Some("18446744073709551615")
    );
    assert_eq!(
        response_value["payload"]["errors"][0]["code"].as_str(),
        Some("-9223372036854775808")
    );
}

#[test]
fn standard_json_handles_response_union_and_errors() {
    let input = br#"{
      "protocol_version": 1,
      "payload_type": "SearchKernel_Wire_Error",
      "payload": {
        "code": "InvalidArgument",
        "message": "bad request",
        "retryable": true,
        "details": [{"code":"unknown_field","path":"query","message":"not supported"}]
      }
    }"#;
    let bytes = response_json_to_flatbuffer(input).expect("standard response JSON");
    assert_eq!(&bytes[4..8], b"SKRS");
    let printed = response_flatbuffer_to_json(&bytes).expect("response JSON printer");
    let value: serde_json::Value = serde_json::from_slice(&printed).expect("valid JSON");
    assert_eq!(value["payload"]["code"], "InvalidArgument");
    assert_eq!(value["payload"]["details"][0]["code"], "unknown_field");
}

#[test]
fn standard_json_accepts_flatbuffers_positional_empty_union_tables() {
    let input = br#"{
      "protocol_version": 1,
      "payload_type": "SearchKernel_Wire_SearchRequest",
      "payload": {
        "query": {"source_type":"MatchAllQuery","source":[]},
        "sort": [{
          "target": {"target_type":"FieldSortTarget","target":{"field":"price"}},
          "direction": "Desc",
          "missing": {"policy_type":"MissingLast","policy":[]}
        }]
      }
    }"#;

    let bytes = request_json_to_flatbuffer(input)
        .expect("FlatBuffers positional empty tables must remain accepted");
    let printed = request_flatbuffer_to_json(&bytes).expect("sort request JSON printer");
    request_json_to_flatbuffer(&printed)
        .expect("printed sort request with empty marker table remains readable");
}

#[test]
fn standard_json_carries_ranking_inputs_and_structured_grouping() {
    let bytes = request_json_to_flatbuffer(WIDENED_SEARCH_REQUEST).expect("widened request JSON");
    let printed = request_flatbuffer_to_json(&bytes).expect("widened request JSON printer");
    let value: serde_json::Value = serde_json::from_slice(&printed).expect("valid JSON");
    assert_eq!(value["payload_type"], "SearchKernel_Wire_SearchRequest");
    assert_eq!(value["payload"]["ranking"]["profile"], "bm25");
    assert_eq!(
        value["payload"]["inputs"][0]["input_type"],
        "LiteralQueryInput"
    );
    assert_eq!(
        value["payload"]["grouping"][0]["source_type"],
        "StructuredGroupingSource"
    );
    assert_eq!(
        value["payload"]["grouping"][0]["source"]["root"]["operations"][0]["operation_type"],
        "GroupingGroupOperation"
    );
}

#[test]
fn standard_json_carries_structured_query_unions() {
    let bytes =
        request_json_to_flatbuffer(STRUCTURED_QUERY_REQUEST).expect("structured query JSON");
    let printed = request_flatbuffer_to_json(&bytes).expect("structured query JSON printer");
    let value: serde_json::Value = serde_json::from_slice(&printed).expect("valid JSON");
    assert_eq!(value["payload"]["query"]["source_type"], "TypedQuery");
    assert_eq!(
        value["payload"]["query"]["source"]["query"]["expr_type"],
        "TermQuery"
    );
}

#[test]
fn standard_json_preserves_textual_filter_clause_bindings() {
    let input = br#"{
      "protocol_version": 1,
      "payload_type": "SearchKernel_Wire_SearchRequest",
      "payload": {
        "filters": {
          "textual": [
            {
              "source": "price <= :max",
              "bindings": [
                {
                  "key": "max",
                  "value": {"kind_type":"Int64Value", "kind":{"value":"120"}}
                }
              ]
            },
            {
              "source": "rating >= :value",
              "bindings": [
                {
                  "key": "value",
                  "value": {"kind_type":"Int64Value", "kind":{"value":"4"}}
                }
              ]
            }
          ]
        }
      }
    }"#;
    let bytes = request_json_to_flatbuffer(input).expect("textual filter JSON");
    let printed = request_flatbuffer_to_json(&bytes).expect("textual filter JSON output");
    let value: serde_json::Value = serde_json::from_slice(&printed).expect("valid JSON");
    let clauses = &value["payload"]["filters"]["textual"];
    assert_eq!(clauses[0]["source"], "price <= :max");
    assert_eq!(clauses[0]["bindings"][0]["key"], "max");
    assert_eq!(clauses[1]["source"], "rating >= :value");
    assert_eq!(clauses[1]["bindings"][0]["key"], "value");
    let again = request_json_to_flatbuffer(&printed).expect("printed textual filter JSON");
    let printed_again = request_flatbuffer_to_json(&again).expect("textual filter JSON again");
    assert_eq!(printed_again, printed);
}

#[test]
fn multi_index_protocol_baseline_pins_wire_layout() {
    assert_eq!(wire::RequestPayload::SearchKernel_Wire_InsertRequest.0, 1);
    assert_eq!(wire::RequestPayload::SearchKernel_Wire_UpsertRequest.0, 2);
    assert_eq!(wire::RequestPayload::SearchKernel_Wire_PatchRequest.0, 3);
    assert_eq!(wire::RequestPayload::SearchKernel_Wire_DeleteRequest.0, 4);
    assert_eq!(wire::RequestPayload::SearchKernel_Wire_GetRequest.0, 5);
    assert_eq!(wire::RequestPayload::SearchKernel_Wire_SearchRequest.0, 6);
    assert_eq!(wire::RequestPayload::SearchKernel_Wire_AnalyzeRequest.0, 7);
    assert_eq!(wire::RequestPayload::SearchKernel_Wire_DescribeRequest.0, 8);
    assert_eq!(
        wire::RequestPayload::SearchKernel_Wire_InspectQueryRequest.0,
        9
    );
    assert_eq!(wire::RequestPayload::SearchKernel_Wire_UpdateRequest.0, 10);
    assert_eq!(
        wire::RequestPayload::SearchKernel_Wire_CreateIndexRequest.0,
        11
    );
    assert_eq!(
        wire::RequestPayload::SearchKernel_Wire_OpenIndexRequest.0,
        12
    );

    assert_eq!(wire::ResponsePayload::SearchKernel_Wire_InsertResult.0, 1);
    assert_eq!(wire::ResponsePayload::SearchKernel_Wire_UpsertResult.0, 2);
    assert_eq!(wire::ResponsePayload::SearchKernel_Wire_PatchResult.0, 3);
    assert_eq!(wire::ResponsePayload::SearchKernel_Wire_DeleteResult.0, 4);
    assert_eq!(wire::ResponsePayload::SearchKernel_Wire_GetResult.0, 5);
    assert_eq!(wire::ResponsePayload::SearchKernel_Wire_SearchResult.0, 6);
    assert_eq!(wire::ResponsePayload::SearchKernel_Wire_AnalyzeResult.0, 7);
    assert_eq!(wire::ResponsePayload::SearchKernel_Wire_Error.0, 8);
    assert_eq!(wire::ResponsePayload::SearchKernel_Wire_DescribeResult.0, 9);
    assert_eq!(
        wire::ResponsePayload::SearchKernel_Wire_InspectQueryResult.0,
        10
    );
    assert_eq!(wire::ResponsePayload::SearchKernel_Wire_UpdateResult.0, 11);
    assert_eq!(
        wire::ResponsePayload::SearchKernel_Wire_CreateIndexResult.0,
        12
    );
    assert_eq!(
        wire::ResponsePayload::SearchKernel_Wire_OpenIndexResult.0,
        13
    );

    // #848 establishes this layout as the compatibility baseline. Future
    // protocol evolution appends fields/variants rather than reusing slots.
    assert_eq!(wire::RequestEnvelope::VT_PROTOCOL_VERSION, 4);
    assert_eq!(wire::RequestEnvelope::VT_PAYLOAD_TYPE, 6);
    assert_eq!(wire::RequestEnvelope::VT_PAYLOAD, 8);
    assert_eq!(wire::RequestEnvelope::VT_TARGET, 10);
    assert_eq!(wire::RequestEnvelope::VT_EXPECTED_GENERATION, 12);
    assert_eq!(wire::ResponseEnvelope::VT_PROTOCOL_VERSION, 4);
    assert_eq!(wire::ResponseEnvelope::VT_PAYLOAD_TYPE, 6);
    assert_eq!(wire::ResponseEnvelope::VT_PAYLOAD, 8);

    // Lifecycle tables introduced by #848 are part of the same durable
    // compatibility baseline. Their field slots must remain append-only too.
    assert_eq!(wire::IndexTarget::VT_NAME, 4);
    assert_eq!(wire::CreateIndexRequest::VT_DEFINITION, 4);
    assert_eq!(wire::CreateIndexResult::VT_INDEX, 4);
    assert_eq!(wire::CreateIndexResult::VT_GENERATION, 6);
    assert_eq!(wire::OpenIndexResult::VT_INDEX, 4);
    assert_eq!(wire::OpenIndexResult::VT_GENERATION, 6);

    // ErrorCode values are portable wire ABI and must never be renumbered.
    assert_eq!(wire::ErrorCode::Unspecified.0, 0);
    assert_eq!(wire::ErrorCode::InvalidArgument.0, 1);
    assert_eq!(wire::ErrorCode::NotFound.0, 2);
    assert_eq!(wire::ErrorCode::AlreadyExists.0, 3);
    assert_eq!(wire::ErrorCode::FailedPrecondition.0, 4);
    assert_eq!(wire::ErrorCode::Conflict.0, 5);
    assert_eq!(wire::ErrorCode::Unsupported.0, 6);
    assert_eq!(wire::ErrorCode::ResourceExhausted.0, 7);
    assert_eq!(wire::ErrorCode::Internal.0, 8);
    assert_eq!(wire::ErrorCode::Unavailable.0, 9);
    assert_eq!(wire::ErrorCode::DeadlineExceeded.0, 10);
}

#[test]
fn standard_json_round_trips_targeted_open_lifecycle() {
    let input = br#"{
      "protocol_version": 1,
      "target": {"name": "products"},
      "payload_type": "SearchKernel_Wire_OpenIndexRequest",
      "payload": {}
    }"#;

    let bytes = request_json_to_flatbuffer(input).expect("targeted open-index request JSON");
    let envelope =
        flatbuffers::root::<wire::RequestEnvelope<'_>>(&bytes).expect("targeted request envelope");
    assert_eq!(
        envelope.target().and_then(|target| target.name()),
        Some("products")
    );
    assert_eq!(
        envelope.payload_type(),
        wire::RequestPayload::SearchKernel_Wire_OpenIndexRequest
    );

    let printed = request_flatbuffer_to_json(&bytes).expect("targeted open-index JSON printer");
    let value: serde_json::Value = serde_json::from_slice(&printed).expect("valid JSON");
    assert_eq!(value["target"]["name"], "products");
    assert_eq!(value["payload_type"], "SearchKernel_Wire_OpenIndexRequest");

    request_json_to_flatbuffer(&printed).expect("printed lifecycle JSON remains readable");
}

#[test]
fn create_index_keeps_logical_target_and_schema_identity_distinct() {
    let request = br#"{
      "protocol_version": 1,
      "target": {"name": "catalog-a"},
      "payload_type": "SearchKernel_Wire_CreateIndexRequest",
      "payload": {
        "definition": {
          "name": "product_schema",
          "fields": []
        }
      }
    }"#;
    let request_bytes = request_json_to_flatbuffer(request).expect("create-index request JSON");
    let envelope =
        flatbuffers::root::<wire::RequestEnvelope<'_>>(&request_bytes).expect("request envelope");
    assert_eq!(
        envelope.target().and_then(|target| target.name()),
        Some("catalog-a")
    );
    assert_eq!(
        envelope.payload_type(),
        wire::RequestPayload::SearchKernel_Wire_CreateIndexRequest
    );

    let request_printed =
        request_flatbuffer_to_json(&request_bytes).expect("create-index JSON printer");
    let request_value: serde_json::Value =
        serde_json::from_slice(&request_printed).expect("valid request JSON");
    assert_eq!(request_value["target"]["name"], "catalog-a");
    assert_eq!(
        request_value["payload_type"],
        "SearchKernel_Wire_CreateIndexRequest"
    );
    assert_eq!(
        request_value["payload"]["definition"]["name"],
        "product_schema"
    );

    for (payload_type, schema_name) in [
        ("SearchKernel_Wire_CreateIndexResult", "product_schema"),
        ("SearchKernel_Wire_OpenIndexResult", "product_schema"),
    ] {
        let response = format!(
            r#"{{
              "protocol_version": 1,
              "payload_type": "{payload_type}",
              "payload": {{"index": {{"name": "{schema_name}", "fields": []}}}}
            }}"#
        );
        let bytes =
            response_json_to_flatbuffer(response.as_bytes()).expect("lifecycle response JSON");
        let printed = response_flatbuffer_to_json(&bytes).expect("lifecycle response JSON printer");
        let value: serde_json::Value = serde_json::from_slice(&printed).expect("valid JSON");
        assert_eq!(value["payload_type"], payload_type);
        assert_eq!(value["payload"]["index"]["name"], schema_name);
    }
}

#[test]
fn targetless_frames_remain_valid_for_embedded_callers_but_not_daemon_routing() {
    let input = br#"{
      "protocol_version": 1,
      "payload_type": "SearchKernel_Wire_DescribeRequest",
      "payload": {}
    }"#;

    let bytes = request_json_to_flatbuffer(input).expect("embedded targetless request JSON");
    let envelope = flatbuffers::root::<wire::RequestEnvelope<'_>>(&bytes)
        .expect("targetless request envelope");
    assert!(envelope.target().is_none());

    let printed = request_flatbuffer_to_json(&bytes).expect("targetless JSON printer");
    let value: serde_json::Value = serde_json::from_slice(&printed).expect("valid JSON");
    assert!(value.get("target").is_none());
    assert_eq!(value["payload_type"], "SearchKernel_Wire_DescribeRequest");
}

#[test]
fn target_domain_strings_remain_representation_values_after_structural_decode() {
    let input = br#"{
      "protocol_version": 1,
      "target": {"name": "../products"},
      "payload_type": "SearchKernel_Wire_OpenIndexRequest",
      "payload": {}
    }"#;
    let bytes = request_json_to_flatbuffer(input)
        .expect("FlatBuffers schema permits a string before semantic target validation");
    let envelope =
        flatbuffers::root::<wire::RequestEnvelope<'_>>(&bytes).expect("request envelope");
    assert_eq!(
        envelope.target().and_then(|target| target.name()),
        Some("../products")
    );
}

#[test]
fn custom_union_shape_and_malformed_json_are_rejected() {
    assert!(
        request_json_to_flatbuffer(br#"{"protocol_version":1,"request":{"search":{}}}"#).is_err()
    );
    assert!(request_json_to_flatbuffer(br#"{"#).is_err());
    assert!(request_json_to_flatbuffer(
        br#"{"protocol_version":1,"payload_type":"NotARequest","payload":{}}"#
    )
    .is_err());
}
