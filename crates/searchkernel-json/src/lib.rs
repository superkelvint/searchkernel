//! Strict JSON transport for the canonical Search Kernel FlatBuffers protocol.
//!
//! The public representation uses natural arrays, objects, and base64 strings.
//! For schema vectors whose absence is distinct from an empty value, the
//! generated codec uses the reserved `{"$absent":true}` marker. It is emitted
//! only for that otherwise unrepresentable state and is accepted only as the
//! sole field of a vector payload.

#[path = "generated_codec.rs"]
mod codec;
#[allow(warnings)]
mod generated;

pub use codec::{
    request_flatbuffer_to_json, request_json_to_flatbuffer, response_flatbuffer_to_json,
    response_json_to_flatbuffer, JsonCodecError,
};

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn search_json_builds_verified_skrq() {
        let bytes = request_json_to_flatbuffer(br#"{"protocol_version":1,"operation":"search","request":{"query":"foo","limit":0,"projection":[]}}"#).unwrap();
        assert_eq!(&bytes[4..8], b"SKRQ");
        let envelope =
            flatbuffers::root::<crate::generated::search_kernel::ffi::v1::RequestEnvelope>(&bytes)
                .unwrap();
        assert!(envelope
            .payload_as_search_kernel_v1_search_request()
            .is_some());
    }

    #[test]
    fn requests_round_trip_through_canonical_json() {
        let original =
            br#"{"protocol_version":1,"operation":"search","request":{"query":"foo","limit":0,"projection":[]}}"#;
        let flatbuffer = request_json_to_flatbuffer(original).unwrap();
        let json = request_flatbuffer_to_json(&flatbuffer).unwrap();
        let round_tripped = request_json_to_flatbuffer(&json).unwrap();
        let first = flatbuffers::root::<crate::generated::search_kernel::ffi::v1::RequestEnvelope>(
            &flatbuffer,
        )
        .unwrap();
        let second =
            flatbuffers::root::<crate::generated::search_kernel::ffi::v1::RequestEnvelope>(
                &round_tripped,
            )
            .unwrap();
        let first = first.payload_as_search_kernel_v1_search_request().unwrap();
        let second = second.payload_as_search_kernel_v1_search_request().unwrap();
        assert_eq!(first.query(), second.query());
        assert_eq!(
            first.limit().map(|value| value.value()),
            second.limit().map(|value| value.value())
        );
        assert_eq!(first.projection().unwrap().fields().unwrap().len(), 0);
        assert_eq!(second.projection().unwrap().fields().unwrap().len(), 0);
    }

    #[test]
    fn duplicate_and_unknown_fields_are_rejected() {
        assert!(matches!(
            request_json_to_flatbuffer(
                br#"{"protocol_version":1,"operation":"search","operation":"get","request":{}}"#
            ),
            Err(JsonCodecError::Syntax(_))
        ));
        assert!(matches!(
            request_json_to_flatbuffer(
                br#"{"protocol_version":1,"operation":"search","request":{"limt":1}}"#
            ),
            Err(JsonCodecError::Contract(_))
        ));
        assert!(matches!(
            request_json_to_flatbuffer(
                br#"{"protocol_version":1,"operation":"search","request":{},"response":{}}"#
            ),
            Err(JsonCodecError::Contract(_))
        ));
    }

    #[test]
    fn insert_response_is_encoded_as_natural_json() {
        use crate::generated::search_kernel::ffi::v1 as transport;
        let mut builder = flatbuffers::FlatBufferBuilder::new();
        let id = builder.create_string("doc-1");
        let result = searchkernel_api::v1::InsertResult::create(
            &mut builder,
            &searchkernel_api::v1::InsertResultArgs { id: Some(id) },
        );
        let envelope = transport::ResponseEnvelope::create(
            &mut builder,
            &transport::ResponseEnvelopeArgs {
                protocol_version: 1,
                payload_type: transport::ResponsePayload::SearchKernel_V1_InsertResult,
                payload: Some(result.as_union_value()),
            },
        );
        builder.finish(envelope, Some(transport::RESPONSE_ENVELOPE_IDENTIFIER));
        let json = response_flatbuffer_to_json(builder.finished_data()).unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&json).unwrap(),
            serde_json::json!({"protocol_version":1,"response":{"insert":{"id":"doc-1"}}})
        );
    }

    fn request_payload(input: &str) -> crate::generated::search_kernel::ffi::v1::RequestPayload {
        let bytes = request_json_to_flatbuffer(input.as_bytes()).unwrap();
        let envelope =
            flatbuffers::root::<crate::generated::search_kernel::ffi::v1::RequestEnvelope>(&bytes)
                .unwrap();
        envelope.payload_type()
    }

    #[test]
    fn every_request_union_variant_has_json_decoding() {
        for (operation, payload) in [
            ("insert", r#"{"document":{"id":"d"}}"#),
            ("upsert", r#"{"document":{"id":"d"}}"#),
            ("patch", r#"{"id":"d","operations":[]}"#),
            ("delete", r#"{"id":"d"}"#),
            ("get", r#"{"id":"d"}"#),
            ("search", r#"{}"#),
            ("analyze", r#"{}"#),
            ("describe", r#"{}"#),
            ("inspect_query", r#"{"request":{}}"#),
        ] {
            let input = format!(
                r#"{{"protocol_version":1,"operation":"{operation}","request":{payload}}}"#
            );
            assert_ne!(request_payload(&input).0, 0, "{operation}");
        }
    }

    #[test]
    fn presence_and_large_values_are_preserved() {
        let absent = request_json_to_flatbuffer(
            br#"{"protocol_version":1,"operation":"search","request":{}}"#,
        )
        .unwrap();
        let empty = request_json_to_flatbuffer(
            br#"{"protocol_version":1,"operation":"search","request":{"projection":[]}}"#,
        )
        .unwrap();
        let absent_request =
            flatbuffers::root::<crate::generated::search_kernel::ffi::v1::RequestEnvelope>(&absent)
                .unwrap()
                .payload_as_search_kernel_v1_search_request()
                .unwrap();
        let empty_request =
            flatbuffers::root::<crate::generated::search_kernel::ffi::v1::RequestEnvelope>(&empty)
                .unwrap()
                .payload_as_search_kernel_v1_search_request()
                .unwrap();
        assert!(absent_request.projection().is_none());
        assert_eq!(
            empty_request.projection().unwrap().fields().unwrap().len(),
            0
        );
        assert_eq!(
            request_json_to_flatbuffer(
                br#"{"protocol_version":1,"operation":"search","request":{"limit":null}}"#,
            )
            .unwrap_err()
            .to_string(),
            "JSON contract error: `search.limit` does not accept JSON null"
        );

        let values = request_json_to_flatbuffer(
            br#"{"protocol_version":1,"operation":"insert","request":{"document":{"id":"d","fields":{"signed":{"int64":"-9223372036854775808"},"unsigned":{"uint64":"18446744073709551615"},"bytes":{"bytes":"AQI="},"empty":{"list":[]},"nil":{"null":null}}}}}"#,
        )
        .unwrap();
        let document =
            flatbuffers::root::<crate::generated::search_kernel::ffi::v1::RequestEnvelope>(&values)
                .unwrap()
                .payload_as_search_kernel_v1_insert_request()
                .unwrap()
                .document()
                .unwrap();
        assert_eq!(document.fields().unwrap().len(), 5);

        let absent_inner = request_json_to_flatbuffer(
            br#"{"protocol_version":1,"operation":"insert","request":{"document":{"id":"d","fields":{"list":{"list":{"$absent":true}},"object":{"object":{"$absent":true}},"bytes":{"bytes":{"$absent":true}}}}}}"#,
        )
        .unwrap();
        let insert =
            flatbuffers::root::<crate::generated::search_kernel::ffi::v1::RequestEnvelope>(
                &absent_inner,
            )
            .unwrap()
            .payload_as_search_kernel_v1_insert_request()
            .unwrap();
        let fields = insert.document().unwrap().fields().unwrap();
        for (name, expected_kind) in [
            (
                "list",
                crate::generated::search_kernel::v1::ValueKind::ValueList,
            ),
            (
                "object",
                crate::generated::search_kernel::v1::ValueKind::ValueObject,
            ),
            (
                "bytes",
                crate::generated::search_kernel::v1::ValueKind::BytesValue,
            ),
        ] {
            let value = fields
                .iter()
                .find(|entry| entry.key() == name)
                .unwrap()
                .value()
                .unwrap();
            assert_eq!(value.kind_type(), expected_kind);
            match expected_kind {
                crate::generated::search_kernel::v1::ValueKind::ValueList => {
                    assert!(value.kind_as_value_list().unwrap().values().is_none())
                }
                crate::generated::search_kernel::v1::ValueKind::ValueObject => {
                    assert!(value.kind_as_value_object().unwrap().fields().is_none())
                }
                crate::generated::search_kernel::v1::ValueKind::BytesValue => {
                    assert!(value.kind_as_bytes_value().unwrap().value().is_none())
                }
                _ => unreachable!(),
            }
        }
        let absent_json: Value =
            serde_json::from_slice(&request_flatbuffer_to_json(&absent_inner).unwrap()).unwrap();
        assert_eq!(
            absent_json["request"]["document"]["fields"]["list"]["list"],
            serde_json::json!({"$absent": true})
        );
    }

    #[test]
    fn invalid_enum_and_null_are_rejected_without_normalization() {
        assert!(matches!(
            request_json_to_flatbuffer(
                br#"{"protocol_version":1,"operation":"search","request":{"sort":[{"field":"x","direction":"sideways"}]}}"#,
            ),
            Err(JsonCodecError::Contract(_))
        ));
        assert!(matches!(
            request_json_to_flatbuffer(
                br#"{"protocol_version":1,"operation":"search","request":{"query":null}}"#,
            ),
            Err(JsonCodecError::Contract(_))
        ));
    }

    #[test]
    fn complete_portable_value_domain_round_trips() {
        let input = serde_json::json!({
            "protocol_version": 1,
            "operation": "insert",
            "request": {"document": {"id": "values", "fields": {
                "null": {"null": null},
                "false": {"bool": false},
                "true": {"bool": true},
                "zero": {"int64": "0"},
                "negative": {"int64": "-9223372036854775808"},
                "signed_max": {"int64": "9223372036854775807"},
                "unsigned_max": {"uint64": "18446744073709551615"},
                "float": {"float64": 1.5},
                "empty_string": {"string": ""},
                "unicode": {"string": "耳机 🎧"},
                "empty_bytes": {"bytes": ""},
                "bytes": {"bytes": "AAEC"},
                "empty_list": {"list": []},
                "nested_list": {"list": [{"list": [{"string": "nested"}]}]},
                "empty_object": {"object": {}},
                "nested_object": {"object": {"child": {"object": {"ok": {"bool": true}}}}}
            }}}
        });
        let first = request_json_to_flatbuffer(&serde_json::to_vec(&input).unwrap()).unwrap();
        let json = request_flatbuffer_to_json(&first).unwrap();
        let second = request_json_to_flatbuffer(&json).unwrap();
        let first_json: Value = serde_json::from_slice(&json).unwrap();
        let second_json: Value =
            serde_json::from_slice(&request_flatbuffer_to_json(&second).unwrap()).unwrap();
        assert_eq!(first_json, second_json);
    }

    #[test]
    fn every_response_union_variant_has_json_encoding() {
        use crate::generated::search_kernel::ffi::v1 as transport;
        use searchkernel_api::v1 as api;

        fn encode<F>(kind: transport::ResponsePayload, build: F) -> serde_json::Value
        where
            F: for<'a> FnOnce(
                &mut flatbuffers::FlatBufferBuilder<'a>,
            ) -> flatbuffers::WIPOffset<flatbuffers::UnionWIPOffset>,
        {
            let mut builder = flatbuffers::FlatBufferBuilder::new();
            let payload = build(&mut builder);
            let envelope = transport::ResponseEnvelope::create(
                &mut builder,
                &transport::ResponseEnvelopeArgs {
                    protocol_version: 1,
                    payload_type: kind,
                    payload: Some(payload),
                },
            );
            builder.finish(envelope, Some(transport::RESPONSE_ENVELOPE_IDENTIFIER));
            let json = response_flatbuffer_to_json(builder.finished_data()).unwrap();
            let round_tripped = response_json_to_flatbuffer(&json).unwrap();
            let round_tripped_json = response_flatbuffer_to_json(&round_tripped).unwrap();
            assert_eq!(
                serde_json::from_slice::<serde_json::Value>(&json).unwrap(),
                serde_json::from_slice::<serde_json::Value>(&round_tripped_json).unwrap()
            );
            serde_json::from_slice(&json).unwrap()
        }

        let id_response = |builder: &mut flatbuffers::FlatBufferBuilder<'_>,
                           kind: transport::ResponsePayload| {
            let id = builder.create_string("d");
            let result = match kind.0 {
                x if x == transport::ResponsePayload::SearchKernel_V1_InsertResult.0 => {
                    api::InsertResult::create(builder, &api::InsertResultArgs { id: Some(id) })
                        .as_union_value()
                }
                x if x == transport::ResponsePayload::SearchKernel_V1_UpsertResult.0 => {
                    api::UpsertResult::create(builder, &api::UpsertResultArgs { id: Some(id) })
                        .as_union_value()
                }
                _ => api::PatchResult::create(builder, &api::PatchResultArgs { id: Some(id) })
                    .as_union_value(),
            };
            result
        };

        for (name, kind) in [
            (
                "insert",
                transport::ResponsePayload::SearchKernel_V1_InsertResult,
            ),
            (
                "upsert",
                transport::ResponsePayload::SearchKernel_V1_UpsertResult,
            ),
            (
                "patch",
                transport::ResponsePayload::SearchKernel_V1_PatchResult,
            ),
        ] {
            let value = encode(kind, |builder| id_response(builder, kind));
            assert!(value["response"].get(name).is_some());
        }

        let value = encode(
            transport::ResponsePayload::SearchKernel_V1_DeleteResult,
            |builder| {
                let id = builder.create_string("d");
                api::DeleteResult::create(
                    builder,
                    &api::DeleteResultArgs {
                        id: Some(id),
                        deleted: true,
                    },
                )
                .as_union_value()
            },
        );
        assert!(value["response"]["delete"].is_object());

        let value = encode(
            transport::ResponsePayload::SearchKernel_V1_GetResult,
            |builder| {
                api::GetResult::create(builder, &api::GetResultArgs { document: None })
                    .as_union_value()
            },
        );
        assert!(value["response"]["get"].is_object());

        let value = encode(
            transport::ResponsePayload::SearchKernel_V1_SearchResult,
            |builder| {
                let total = api::TotalHits::create(
                    builder,
                    &api::TotalHitsArgs {
                        value: 0,
                        relation: api::TotalHitsRelation::Exact,
                    },
                );
                api::SearchResult::create(
                    builder,
                    &api::SearchResultArgs {
                        total_hits: Some(total),
                        ..Default::default()
                    },
                )
                .as_union_value()
            },
        );
        assert!(value["response"]["search"].is_object());

        let value = encode(
            transport::ResponsePayload::SearchKernel_V1_AnalyzeResult,
            |builder| {
                api::AnalyzeResult::create(builder, &api::AnalyzeResultArgs { tokens: None })
                    .as_union_value()
            },
        );
        assert!(value["response"]["analyze"].is_object());

        let value = encode(
            transport::ResponsePayload::SearchKernel_V1_Error,
            |builder| {
                let message = builder.create_string("bad request");
                api::Error::create(
                    builder,
                    &api::ErrorArgs {
                        code: api::ErrorCode::InvalidArgument,
                        message: Some(message),
                        ..Default::default()
                    },
                )
                .as_union_value()
            },
        );
        assert!(value["response"]["error"].is_object());

        let value = encode(
            transport::ResponsePayload::SearchKernel_V1_DescribeResult,
            |builder| {
                let index =
                    api::IndexDefinition::create(builder, &api::IndexDefinitionArgs::default());
                api::DescribeResult::create(
                    builder,
                    &api::DescribeResultArgs { index: Some(index) },
                )
                .as_union_value()
            },
        );
        assert!(value["response"]["describe"].is_object());

        let value = encode(
            transport::ResponsePayload::SearchKernel_V1_InspectQueryResult,
            |builder| {
                let request =
                    api::SearchRequest::create(builder, &api::SearchRequestArgs::default());
                api::InspectQueryResult::create(
                    builder,
                    &api::InspectQueryResultArgs {
                        resolved_request: Some(request),
                    },
                )
                .as_union_value()
            },
        );
        assert!(value["response"]["inspect_query"].is_object());
    }

    #[test]
    fn portable_request_corpus_round_trips_through_json_codec() {
        let manifest: Value = serde_json::from_str(include_str!(
            "../../../reference/api-conformance/cases.json"
        ))
        .unwrap();
        let cases = manifest["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 204);

        let mut covered = 0;
        let mut round_tripped = 0;
        let mut rejected = 0;
        for case in cases {
            let operation = case["operation"].as_str().unwrap();
            let path = format!(
                "{}/../../reference/api-conformance/{}",
                env!("CARGO_MANIFEST_DIR"),
                case["case"].as_str().unwrap()
            );
            let case_data: Value =
                serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
            let Some((operation, request)) = transport_request(operation, &case_data["input"])
            else {
                continue;
            };
            covered += 1;
            let envelope = serde_json::json!({
                "protocol_version": 1,
                "operation": operation,
                "request": request,
            });
            let input = serde_json::to_vec(&envelope).unwrap();
            let flatbuffer = match request_json_to_flatbuffer(&input) {
                Ok(flatbuffer) => flatbuffer,
                Err(error) => {
                    assert!(
                        matches!(
                            case["id"].as_str(),
                            Some("analyze.none-target-error")
                                | Some("write.patch-none-operation")
                                | Some("wire.facets-null-equals-empty")
                                | Some("wire.patch-operations-null-equals-empty")
                                | Some("wire.query-null-equals-empty")
                                | Some("wire.sort-null-equals-empty")
                        ),
                        "{} ({operation}) was not representable as canonical JSON: {error}",
                        case["id"]
                    );
                    rejected += 1;
                    continue;
                }
            };
            let json = request_flatbuffer_to_json(&flatbuffer).unwrap();
            let second = request_json_to_flatbuffer(&json).unwrap();
            assert_eq!(request_kind(&flatbuffer), request_kind(&second));
            // The generated JSON form is the semantic decoder used here: if
            // two FlatBuffers values differ in any represented field, their
            // canonical JSON values differ as well. This deliberately avoids
            // depending on incidental FlatBuffers byte layout.
            let second_json = request_flatbuffer_to_json(&second).unwrap();
            assert_eq!(
                serde_json::from_slice::<Value>(&json).unwrap(),
                serde_json::from_slice::<Value>(&second_json).unwrap()
            );
            round_tripped += 1;
        }
        assert_eq!(covered, 77);
        assert_eq!(round_tripped + rejected, covered);
        assert_eq!(rejected, 6);
    }

    fn request_kind(bytes: &[u8]) -> u8 {
        flatbuffers::root::<crate::generated::search_kernel::ffi::v1::RequestEnvelope>(bytes)
            .unwrap()
            .payload_type()
            .0
    }

    fn transport_request(operation: &str, input: &Value) -> Option<(&'static str, Value)> {
        match operation {
            "search_to_ir" => Some(("search", normalize_search(input.clone()))),
            "insert_to_ir" => Some(("insert", input.clone())),
            "upsert_to_ir" => Some(("upsert", input.clone())),
            "delete_to_ir" => Some(("delete", input.clone())),
            "get_to_ir" => Some(("get", normalize_get(input.clone()))),
            "patch_to_ir" => Some(("patch", normalize_patch(input.clone()))),
            "analyze_to_ir" => Some(("analyze", normalize_analyze(input.clone()))),
            "describe" => Some(("describe", input.clone())),
            "inspect_query" => Some(("inspect_query", input.clone())),
            "write_to_ir" => {
                let kind = input["kind"].as_str()?;
                let operation = match kind {
                    "insert" => "insert",
                    "upsert" => "upsert",
                    "patch" => "patch",
                    "delete" => "delete",
                    "get" => "get",
                    _ => return None,
                };
                Some((operation, input["request"].clone()))
            }
            _ => None,
        }
        .map(|(operation, mut request)| {
            normalize_portable_values(&mut request);
            (operation, request)
        })
    }

    fn normalize_portable_values(value: &mut Value) {
        match value {
            Value::Array(values) => {
                for value in values {
                    normalize_portable_values(value);
                }
            }
            Value::Object(object) => {
                for (name, value) in object.iter_mut() {
                    if matches!(name.as_str(), "int64" | "uint64") {
                        if let Some(number) = value.as_i64() {
                            *value = Value::String(number.to_string());
                        } else if let Some(number) = value.as_u64() {
                            *value = Value::String(number.to_string());
                        }
                    }
                    if name == "null" && value == &Value::Bool(true) {
                        *value = Value::Null;
                    }
                    normalize_portable_values(value);
                }
            }
            _ => {}
        }
    }

    fn normalize_search(mut input: Value) -> Value {
        let Some(object) = input.as_object_mut() else {
            return input;
        };
        if let Some(ranking) = object.get_mut("ranking") {
            if let Some(value) = ranking.get("bm25") {
                *ranking = serde_json::json!({"bm25_ranking": value});
            }
            if let Some(value) = ranking.get("default_relevance") {
                *ranking = serde_json::json!({"default_relevance_v1": value});
            }
        }
        for name in ["projection", "search_fields", "highlighting"] {
            if let Some(value) = object.get_mut(name) {
                if let Some(fields) = value.get("fields") {
                    *value = fields.clone();
                } else if value.is_object() {
                    *value = serde_json::json!({"$absent": true});
                }
            }
        }
        input
    }

    fn normalize_get(mut input: Value) -> Value {
        if let Some(projection) = input.get_mut("projection") {
            if let Some(fields) = projection.get("fields") {
                *projection = fields.clone();
            } else if projection.is_object() {
                *projection = serde_json::json!({"$absent": true});
            }
        }
        input
    }

    fn normalize_patch(mut input: Value) -> Value {
        if let Some(operations) = input.get_mut("operations").and_then(Value::as_array_mut) {
            for operation in operations {
                if let Some(object) = operation.as_object_mut() {
                    if let Some(value) = object.remove("set") {
                        object.insert("set_field".to_owned(), value);
                    }
                    if let Some(value) = object.remove("unset") {
                        object.insert("unset_field".to_owned(), value);
                    }
                }
            }
        }
        input
    }

    fn normalize_analyze(input: Value) -> Value {
        let Some(mut object) = input.as_object().cloned() else {
            return input;
        };
        if let Some(analyzer) = object.remove("analyzer") {
            object.insert(
                "target".to_owned(),
                serde_json::json!({"analyzer": analyzer}),
            );
        }
        if let Some(field) = object.remove("field") {
            object.insert(
                "target".to_owned(),
                serde_json::json!({"field": {"field": field}}),
            );
        }
        Value::Object(object)
    }
}
