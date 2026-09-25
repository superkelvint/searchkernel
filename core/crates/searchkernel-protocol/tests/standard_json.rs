use searchkernel_protocol::json::{
    request_flatbuffer_to_json, request_json_to_flatbuffer, response_flatbuffer_to_json,
    response_json_to_flatbuffer,
};

const RICH_REQUEST: &[u8] = br#"
{
  "protocol_version": 2,
  "payload_type": "SearchKernel_V2_InsertRequest",
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
  "protocol_version": 2,
  "payload_type": "SearchKernel_V2_SearchRequest",
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
  "protocol_version": 2,
  "payload_type": "SearchKernel_V2_InsertRequest",
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
  "protocol_version": 2,
  "payload_type": "SearchKernel_V2_SearchRequest",
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
    assert_eq!(value["payload_type"], "SearchKernel_V2_InsertRequest");
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
      "protocol_version": 2,
      "payload_type": "SearchKernel_V2_InsertRequest",
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
        error.to_string().contains("64-bit integer JSON values must be decimal strings"),
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
fn standard_json_handles_response_union_and_errors() {
    let input = br#"{
      "protocol_version": 2,
      "payload_type": "SearchKernel_V2_Error",
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
fn standard_json_carries_ranking_inputs_and_structured_grouping() {
    let bytes =
        request_json_to_flatbuffer(WIDENED_SEARCH_REQUEST).expect("widened V2 request JSON");
    let printed = request_flatbuffer_to_json(&bytes).expect("widened request JSON printer");
    let value: serde_json::Value = serde_json::from_slice(&printed).expect("valid JSON");
    assert_eq!(value["payload_type"], "SearchKernel_V2_SearchRequest");
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
      "protocol_version": 2,
      "payload_type": "SearchKernel_V2_SearchRequest",
      "payload": {
        "filters": {
          "textual": [
            {
              "source": "price <= :max",
              "bindings": [
                {
                  "key": "max",
                  "value": {"kind_type":"Int64Value", "kind":{"value":120}}
                }
              ]
            },
            {
              "source": "rating >= :value",
              "bindings": [
                {
                  "key": "value",
                  "value": {"kind_type":"Int64Value", "kind":{"value":4}}
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
fn custom_union_shape_and_malformed_json_are_rejected() {
    assert!(
        request_json_to_flatbuffer(br#"{"protocol_version":2,"request":{"search":{}}}"#).is_err()
    );
    assert!(request_json_to_flatbuffer(br#"{"#).is_err());
    assert!(request_json_to_flatbuffer(
        br#"{"protocol_version":2,"payload_type":"NotARequest","payload":{}}"#
    )
    .is_err());
}
