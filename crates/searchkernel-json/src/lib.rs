//! Strict JSON transport for the canonical Search Kernel FlatBuffers protocol.

#[path = "generated_codec.rs"]
mod codec;
#[allow(warnings)]
mod generated;

pub use codec::{
    request_flatbuffer_to_json, request_json_to_flatbuffer, response_flatbuffer_to_json,
    JsonCodecError,
};

#[cfg(test)]
mod tests {
    use super::*;

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
}
