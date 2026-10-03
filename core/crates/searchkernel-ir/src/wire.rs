//! Versioned wire adapters around the wire-agnostic Searchkernel IR.
//!
//! Generated FlatBuffers types are intentionally reachable only below a wire
//! major. Core model, backend, lifecycle registry, and resolver APIs remain
//! version-free.

use crate::ControlPlaneRequest;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WireVersion {
    V1,
}

#[derive(Debug)]
pub struct DecodedControlPlaneRequest {
    pub version: WireVersion,
    pub request: ControlPlaneRequest,
}

#[derive(Debug)]
pub enum ControlPlaneDecodeError {
    InvalidRequest,
    UnsupportedProtocol(u32),
    Semantic {
        version: WireVersion,
        error: crate::PortableError,
    },
}

pub fn decode_control_plane_request(
    bytes: &[u8],
) -> Result<DecodedControlPlaneRequest, ControlPlaneDecodeError> {
    match v1::protocol_version(bytes)? {
        searchkernel_protocol::protocol::v1::PROTOCOL_VERSION => {
            let request = v1::decode_control_plane_request(bytes)?;
            Ok(DecodedControlPlaneRequest {
                version: WireVersion::V1,
                request,
            })
        }
        version => Err(ControlPlaneDecodeError::UnsupportedProtocol(version)),
    }
}

/// Encode the canonical persisted index-definition representation.
pub fn encode_persisted_index_definition(definition: &crate::IndexDefinitionIr) -> Vec<u8> {
    // Storage format is deliberately independent of the request wire major.
    // Wire V1 is the current canonical persisted representation.
    v1::index_definition_ir_to_api(definition)
}

pub fn encode_create_index_result(
    version: WireVersion,
    definition: &crate::IndexDefinitionIr,
    generation: &str,
) -> Vec<u8> {
    match version {
        WireVersion::V1 => v1::encode_create_index_result(definition, generation),
    }
}

pub fn encode_open_index_result(
    version: WireVersion,
    definition: &crate::IndexDefinitionIr,
    generation: &str,
) -> Vec<u8> {
    match version {
        WireVersion::V1 => v1::encode_open_index_result(definition, generation),
    }
}

pub fn encode_delete_index_result(version: WireVersion) -> Vec<u8> {
    match version {
        WireVersion::V1 => v1::encode_delete_index_result(),
    }
}

pub fn encode_list_indexes_result(version: WireVersion, targets: &[String]) -> Vec<u8> {
    match version {
        WireVersion::V1 => v1::encode_list_indexes_result(targets),
    }
}

pub fn encode_error(version: WireVersion, error: &crate::PortableError) -> Vec<u8> {
    match version {
        WireVersion::V1 => v1::encode_error(error),
    }
}

pub mod v1 {
    use super::{ControlPlaneDecodeError, ControlPlaneRequest, WireVersion};
    use crate::api_to_ir::control_plane::{
        invalid_target_error, missing_definition_error, missing_target_error,
        unexpected_target_error,
    };
    use flatbuffers::root;
    use searchkernel_protocol::protocol::v1::{self as wire, PROTOCOL_VERSION};

    pub use searchkernel_protocol::wire::v1 as schema;

    pub use crate::api_to_ir::{
        analyze_result_from_api, analyzer_to_ir, delete_result_from_api, field_to_ir,
        get_result_from_api, index_definition_to_ir, insert_result_from_api, patch_result_from_api,
        portable_error_from_api, search_result_from_api, update_result_from_api,
        upsert_result_from_api, WriteRequestRef,
    };
    pub use crate::ir_to_api::*;

    const REQUEST_IDENTIFIER: &[u8; 4] = b"SKRQ";
    const RESPONSE_IDENTIFIER: &str = "SKRS";

    fn parse_request(bytes: &[u8]) -> Result<wire::RequestEnvelope<'_>, ControlPlaneDecodeError> {
        if bytes.len() < 8 || bytes[4..8] != *REQUEST_IDENTIFIER {
            return Err(ControlPlaneDecodeError::InvalidRequest);
        }
        root::<wire::RequestEnvelope<'_>>(bytes)
            .map_err(|_| ControlPlaneDecodeError::InvalidRequest)
    }

    pub(super) fn protocol_version(bytes: &[u8]) -> Result<u32, ControlPlaneDecodeError> {
        Ok(parse_request(bytes)?.protocol_version())
    }

    pub(super) fn decode_control_plane_request(
        bytes: &[u8],
    ) -> Result<ControlPlaneRequest, ControlPlaneDecodeError> {
        let envelope = parse_request(bytes)?;
        if envelope.protocol_version() != PROTOCOL_VERSION {
            return Err(ControlPlaneDecodeError::UnsupportedProtocol(
                envelope.protocol_version(),
            ));
        }

        // Discovery is server-scoped. A target on this operation is not merely
        // unused: accepting one would hide malformed envelopes and weaken the
        // protocol invariant that server operations are explicitly targetless.
        if envelope.payload_type() == wire::RequestPayload::SearchKernel_Wire_ListIndexesRequest {
            envelope
                .payload_as_search_kernel_wire_list_indexes_request()
                .ok_or(ControlPlaneDecodeError::InvalidRequest)?;
            if envelope.target().is_some() {
                return Err(ControlPlaneDecodeError::Semantic {
                    version: WireVersion::V1,
                    error: unexpected_target_error(),
                });
            }
            return Ok(ControlPlaneRequest::ListIndexes);
        }

        let target_view = envelope
            .target()
            .ok_or_else(|| ControlPlaneDecodeError::Semantic {
                version: WireVersion::V1,
                error: missing_target_error(),
            })?;
        let target_name = target_view
            .name()
            .ok_or_else(|| ControlPlaneDecodeError::Semantic {
                version: WireVersion::V1,
                error: invalid_target_error(crate::TargetValidationError::Empty),
            })?;
        let target = crate::validate_index_target(target_name)
            .map_err(|error| ControlPlaneDecodeError::Semantic {
                version: WireVersion::V1,
                error: invalid_target_error(error),
            })?
            .to_owned();

        match envelope.payload_type() {
            wire::RequestPayload::SearchKernel_Wire_CreateIndexRequest => {
                let payload = envelope
                    .payload_as_search_kernel_wire_create_index_request()
                    .ok_or(ControlPlaneDecodeError::InvalidRequest)?;
                let definition =
                    payload
                        .definition()
                        .ok_or_else(|| ControlPlaneDecodeError::Semantic {
                            version: WireVersion::V1,
                            error: missing_definition_error(),
                        })?;
                let definition = index_definition_to_ir(definition).map_err(|error| {
                    ControlPlaneDecodeError::Semantic {
                        version: WireVersion::V1,
                        error,
                    }
                })?;
                Ok(ControlPlaneRequest::Create {
                    target,
                    definition: Box::new(definition),
                })
            }
            wire::RequestPayload::SearchKernel_Wire_OpenIndexRequest => {
                envelope
                    .payload_as_search_kernel_wire_open_index_request()
                    .ok_or(ControlPlaneDecodeError::InvalidRequest)?;
                Ok(ControlPlaneRequest::Open { target })
            }
            wire::RequestPayload::SearchKernel_Wire_DeleteIndexRequest => {
                envelope
                    .payload_as_search_kernel_wire_delete_index_request()
                    .ok_or(ControlPlaneDecodeError::InvalidRequest)?;
                Ok(ControlPlaneRequest::Delete {
                    target,
                    expected_generation: target_view.generation().map(ToOwned::to_owned),
                })
            }
            wire::RequestPayload::NONE => Err(ControlPlaneDecodeError::InvalidRequest),
            _ => Ok(ControlPlaneRequest::Route {
                target,
                expected_generation: target_view.generation().map(ToOwned::to_owned),
            }),
        }
    }

    fn encode_lifecycle_result(
        definition: &crate::IndexDefinitionIr,
        payload_type: wire::ResponsePayload,
        generation: &str,
    ) -> Vec<u8> {
        let mut builder = flatbuffers::FlatBufferBuilder::new();
        let index = index_definition_offset(&mut builder, definition);
        let generation = builder.create_string(generation);
        let payload = match payload_type {
            wire::ResponsePayload::SearchKernel_Wire_CreateIndexResult => {
                schema::CreateIndexResult::create(
                    &mut builder,
                    &schema::CreateIndexResultArgs {
                        index: Some(index),
                        generation: Some(generation),
                    },
                )
                .as_union_value()
            }
            wire::ResponsePayload::SearchKernel_Wire_OpenIndexResult => {
                schema::OpenIndexResult::create(
                    &mut builder,
                    &schema::OpenIndexResultArgs {
                        index: Some(index),
                        generation: Some(generation),
                    },
                )
                .as_union_value()
            }
            _ => unreachable!("lifecycle result kind is fixed by caller"),
        };
        encode_response(&mut builder, payload_type, payload)
    }

    pub(super) fn encode_create_index_result(
        definition: &crate::IndexDefinitionIr,
        generation: &str,
    ) -> Vec<u8> {
        encode_lifecycle_result(
            definition,
            wire::ResponsePayload::SearchKernel_Wire_CreateIndexResult,
            generation,
        )
    }

    pub(super) fn encode_open_index_result(
        definition: &crate::IndexDefinitionIr,
        generation: &str,
    ) -> Vec<u8> {
        encode_lifecycle_result(
            definition,
            wire::ResponsePayload::SearchKernel_Wire_OpenIndexResult,
            generation,
        )
    }

    pub(super) fn encode_delete_index_result() -> Vec<u8> {
        let mut builder = flatbuffers::FlatBufferBuilder::new();
        let payload = schema::DeleteIndexResult::create(
            &mut builder,
            &schema::DeleteIndexResultArgs::default(),
        )
        .as_union_value();
        encode_response(
            &mut builder,
            wire::ResponsePayload::SearchKernel_Wire_DeleteIndexResult,
            payload,
        )
    }

    pub(super) fn encode_list_indexes_result(targets: &[String]) -> Vec<u8> {
        let mut builder = flatbuffers::FlatBufferBuilder::new();
        let names: Vec<_> = targets
            .iter()
            .map(|name| builder.create_string(name))
            .collect();
        let targets = builder.create_vector(&names);
        let payload = schema::ListIndexesResult::create(
            &mut builder,
            &schema::ListIndexesResultArgs {
                targets: Some(targets),
            },
        )
        .as_union_value();
        encode_response(
            &mut builder,
            wire::ResponsePayload::SearchKernel_Wire_ListIndexesResult,
            payload,
        )
    }

    pub(super) fn encode_error(error: &crate::PortableError) -> Vec<u8> {
        let mut builder = flatbuffers::FlatBufferBuilder::new();
        let payload = error_offset(&mut builder, error).as_union_value();
        encode_response(
            &mut builder,
            wire::ResponsePayload::SearchKernel_Wire_Error,
            payload,
        )
    }

    fn encode_response(
        builder: &mut flatbuffers::FlatBufferBuilder<'_>,
        payload_type: wire::ResponsePayload,
        payload: flatbuffers::WIPOffset<flatbuffers::UnionWIPOffset>,
    ) -> Vec<u8> {
        let envelope = wire::ResponseEnvelope::create(
            builder,
            &wire::ResponseEnvelopeArgs {
                protocol_version: PROTOCOL_VERSION,
                payload_type,
                payload: Some(payload),
            },
        );
        builder.finish(envelope, Some(RESPONSE_IDENTIFIER));
        builder.finished_data().to_vec()
    }

    pub fn index_context_from_api(
        definition: schema::IndexDefinition<'_>,
    ) -> crate::Result<crate::IndexContext> {
        crate::IndexContext::from_api(definition)
    }

    pub fn insert_to_ir(
        context: &crate::IndexContext,
        request: schema::InsertRequest<'_>,
    ) -> crate::Result<crate::InsertRequestIr> {
        context.insert_to_ir(request)
    }

    pub fn upsert_to_ir(
        context: &crate::IndexContext,
        request: schema::UpsertRequest<'_>,
    ) -> crate::Result<crate::UpsertRequestIr> {
        context.upsert_to_ir(request)
    }

    pub fn patch_to_ir(
        context: &crate::IndexContext,
        request: schema::PatchRequest<'_>,
    ) -> crate::Result<crate::PatchRequestIr> {
        context.patch_to_ir(request)
    }

    pub fn update_to_ir(
        context: &crate::IndexContext,
        request: schema::UpdateRequest<'_>,
    ) -> crate::Result<crate::UpdateRequestIr> {
        context.update_to_ir(request)
    }

    pub fn delete_to_ir(
        context: &crate::IndexContext,
        request: schema::DeleteRequest<'_>,
    ) -> crate::Result<crate::DeleteRequestIr> {
        context.delete_to_ir(request)
    }

    pub fn get_to_ir(
        context: &crate::IndexContext,
        request: schema::GetRequest<'_>,
    ) -> crate::Result<crate::GetRequestIr> {
        context.get_to_ir(request)
    }

    pub fn value_to_ir(
        context: &crate::IndexContext,
        field: &str,
        value: schema::Value<'_>,
    ) -> crate::Result<crate::FieldValueIr> {
        context.value_to_ir(field, value)
    }

    pub fn document_to_ir(
        context: &crate::IndexContext,
        document: schema::Document<'_>,
    ) -> crate::Result<crate::DocumentIr> {
        context.document_to_ir(document)
    }

    pub fn write_to_ir(
        context: &crate::IndexContext,
        request: WriteRequestRef<'_>,
    ) -> crate::Result<crate::WriteRequestIr> {
        context.write_to_ir(request)
    }

    pub fn bulk_write_to_ir<'a>(
        context: &crate::IndexContext,
        requests: impl IntoIterator<Item = WriteRequestRef<'a>>,
    ) -> crate::Result<Vec<crate::WriteRequestIr>> {
        context.bulk_write_to_ir(requests)
    }

    pub fn filter_to_ir(
        context: &crate::IndexContext,
        filter: schema::FilterExpr<'_>,
    ) -> crate::Result<crate::FilterIr> {
        context.filter_to_ir(filter)
    }

    pub fn comparison_to_ir(
        context: &crate::IndexContext,
        comparison: schema::FilterComparison<'_>,
    ) -> crate::Result<crate::ComparisonIr> {
        context.comparison_to_ir(comparison)
    }

    pub fn range_to_ir(
        context: &crate::IndexContext,
        range: schema::FilterRange<'_>,
    ) -> crate::Result<crate::RangeIr> {
        context.range_to_ir(range)
    }

    pub fn inspect_query(
        context: &crate::IndexContext,
        request: schema::SearchRequest<'_>,
    ) -> crate::Result<crate::SearchRequestIr> {
        context.inspect_query(request)
    }

    pub fn search_to_ir(
        context: &crate::IndexContext,
        request: schema::SearchRequest<'_>,
    ) -> crate::Result<crate::SearchRequestIr> {
        context.search_to_ir(request)
    }

    pub fn analyze_to_ir(
        context: &crate::IndexContext,
        request: schema::AnalyzeRequest<'_>,
    ) -> crate::Result<crate::AnalyzeRequestIr> {
        context.analyze_to_ir(request)
    }

    pub fn search_fields_to_ir(
        context: &crate::IndexContext,
        selection: schema::SearchFieldSelection<'_>,
    ) -> crate::Result<Vec<crate::SearchFieldIr>> {
        context.search_fields_to_ir(selection)
    }

    pub fn query_to_ir(
        context: &crate::IndexContext,
        text: &str,
        expression: Option<schema::QueryExpr<'_>>,
    ) -> crate::Result<crate::QueryIr> {
        context.query_to_ir(text, expression)
    }

    pub fn sort_to_ir(
        context: &crate::IndexContext,
        sort: schema::SortExpr<'_>,
    ) -> crate::Result<crate::SortIr> {
        context.sort_to_ir(sort)
    }

    pub fn projection_to_ir(
        context: &crate::IndexContext,
        projection: Option<schema::FieldProjection<'_>>,
    ) -> crate::Result<crate::ProjectionIr> {
        context.projection_to_ir(projection)
    }

    pub fn highlighting_to_ir(
        context: &crate::IndexContext,
        highlighting: Option<schema::HighlightRequest<'_>>,
    ) -> crate::Result<crate::HighlightingIr> {
        context.highlighting_to_ir(highlighting)
    }

    pub fn ranking_request_to_ir(
        context: &crate::IndexContext,
        ranking: Option<schema::RankingRequest<'_>>,
    ) -> crate::Result<crate::RankingRequestIr> {
        context.ranking_request_to_ir(ranking)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use searchkernel_protocol::protocol::v1 as protocol;

    fn target<'a>(
        builder: &mut flatbuffers::FlatBufferBuilder<'a>,
        name: &'a str,
    ) -> flatbuffers::WIPOffset<v1::schema::IndexTarget<'a>> {
        let name = builder.create_string(name);
        v1::schema::IndexTarget::create(
            builder,
            &v1::schema::IndexTargetArgs {
                name: Some(name),
                generation: None,
            },
        )
    }

    #[test]
    fn missing_target_is_a_versioned_semantic_error() {
        let mut builder = flatbuffers::FlatBufferBuilder::new();
        let payload = v1::schema::OpenIndexRequest::create(
            &mut builder,
            &v1::schema::OpenIndexRequestArgs::default(),
        );
        let envelope = protocol::RequestEnvelope::create(
            &mut builder,
            &protocol::RequestEnvelopeArgs {
                protocol_version: protocol::PROTOCOL_VERSION,
                payload_type: protocol::RequestPayload::SearchKernel_Wire_OpenIndexRequest,
                payload: Some(payload.as_union_value()),
                ..Default::default()
            },
        );
        protocol::finish_request_envelope_buffer(&mut builder, envelope);

        match decode_control_plane_request(builder.finished_data()) {
            Err(ControlPlaneDecodeError::Semantic {
                version: WireVersion::V1,
                error,
            }) => assert_eq!(
                error.details[0].code,
                crate::MISSING_INDEX_TARGET_DETAIL_CODE
            ),
            other => panic!("expected semantic missing-target error, got {other:?}"),
        }
    }

    #[test]
    fn discovery_is_server_scoped_and_preserves_wire_discriminators() {
        assert_eq!(
            protocol::RequestPayload::SearchKernel_Wire_DeleteIndexRequest.0,
            13
        );
        assert_eq!(
            protocol::RequestPayload::SearchKernel_Wire_ListIndexesRequest.0,
            14
        );
        assert_eq!(
            protocol::ResponsePayload::SearchKernel_Wire_DeleteIndexResult.0,
            14
        );
        assert_eq!(
            protocol::ResponsePayload::SearchKernel_Wire_ListIndexesResult.0,
            15
        );
        let mut builder = flatbuffers::FlatBufferBuilder::new();
        let payload = v1::schema::ListIndexesRequest::create(&mut builder, &Default::default());
        let envelope = protocol::RequestEnvelope::create(
            &mut builder,
            &protocol::RequestEnvelopeArgs {
                protocol_version: protocol::PROTOCOL_VERSION,
                payload_type: protocol::RequestPayload::SearchKernel_Wire_ListIndexesRequest,
                payload: Some(payload.as_union_value()),
                target: None,
            },
        );
        protocol::finish_request_envelope_buffer(&mut builder, envelope);
        assert!(matches!(
            decode_control_plane_request(builder.finished_data())
                .unwrap()
                .request,
            ControlPlaneRequest::ListIndexes
        ));

        let mut builder = flatbuffers::FlatBufferBuilder::new();
        let target = target(&mut builder, "products");
        let payload = v1::schema::ListIndexesRequest::create(&mut builder, &Default::default());
        let envelope = protocol::RequestEnvelope::create(
            &mut builder,
            &protocol::RequestEnvelopeArgs {
                protocol_version: protocol::PROTOCOL_VERSION,
                payload_type: protocol::RequestPayload::SearchKernel_Wire_ListIndexesRequest,
                payload: Some(payload.as_union_value()),
                target: Some(target),
            },
        );
        protocol::finish_request_envelope_buffer(&mut builder, envelope);
        match decode_control_plane_request(builder.finished_data()) {
            Err(ControlPlaneDecodeError::Semantic { error, .. }) => {
                assert_eq!(
                    error.details[0].code,
                    crate::UNEXPECTED_INDEX_TARGET_DETAIL_CODE
                );
                assert_eq!(error.details[0].path.as_deref(), Some(crate::TARGET_PATH));
            }
            other => panic!("target-bearing ListIndexes must be rejected, got {other:?}"),
        }
        let targets = vec!["alpha".to_owned(), "zeta".to_owned()];
        let bytes = encode_list_indexes_result(WireVersion::V1, &targets);
        let envelope = flatbuffers::root::<protocol::ResponseEnvelope>(&bytes).unwrap();
        let result = envelope
            .payload_as_search_kernel_wire_list_indexes_result()
            .unwrap();
        assert_eq!(
            result.targets().unwrap().iter().collect::<Vec<_>>(),
            ["alpha", "zeta"]
        );
    }

    #[test]
    fn create_decodes_to_owned_definition() {
        let definition = crate::resolve_rust_index_definition(crate::RustIndexDefinitionInput {
            name: Some("products".to_owned()),
            fields: vec![crate::RustFieldInput {
                name: "title".to_owned(),
                kind: crate::RustFieldKind::Text,
                multi: false,
                stored: Some(true),
                searchable: Some(true),
                filterable: None,
                sortable: None,
                facetable: None,
                analyzer: None,
                distance_metric: None,
                hnsw: None,
            }],
            default_search_fields: None,
        })
        .expect("definition resolves");

        let mut builder = flatbuffers::FlatBufferBuilder::new();
        let target = target(&mut builder, "products");
        let definition = v1::index_definition_offset(&mut builder, &definition);
        let payload = v1::schema::CreateIndexRequest::create(
            &mut builder,
            &v1::schema::CreateIndexRequestArgs {
                definition: Some(definition),
            },
        );
        let envelope = protocol::RequestEnvelope::create(
            &mut builder,
            &protocol::RequestEnvelopeArgs {
                protocol_version: protocol::PROTOCOL_VERSION,
                target: Some(target),
                payload_type: protocol::RequestPayload::SearchKernel_Wire_CreateIndexRequest,
                payload: Some(payload.as_union_value()),
            },
        );
        protocol::finish_request_envelope_buffer(&mut builder, envelope);

        let decoded =
            decode_control_plane_request(builder.finished_data()).expect("create request decodes");
        assert_eq!(decoded.version, WireVersion::V1);
        match decoded.request {
            ControlPlaneRequest::Create { target, definition } => {
                assert_eq!(target, "products");
                assert_eq!(
                    definition.name().map(|name| name.as_str()),
                    Some("products")
                );
            }
            other => panic!("expected owned create request, got {other:?}"),
        }
    }

    #[test]
    fn unsupported_protocol_is_rejected_before_registry_semantics() {
        let mut builder = flatbuffers::FlatBufferBuilder::new();
        let target = target(&mut builder, "products");
        let payload = v1::schema::OpenIndexRequest::create(
            &mut builder,
            &v1::schema::OpenIndexRequestArgs::default(),
        );
        let envelope = protocol::RequestEnvelope::create(
            &mut builder,
            &protocol::RequestEnvelopeArgs {
                protocol_version: 2,
                target: Some(target),
                payload_type: protocol::RequestPayload::SearchKernel_Wire_OpenIndexRequest,
                payload: Some(payload.as_union_value()),
            },
        );
        protocol::finish_request_envelope_buffer(&mut builder, envelope);

        assert!(matches!(
            decode_control_plane_request(builder.finished_data()),
            Err(ControlPlaneDecodeError::UnsupportedProtocol(2))
        ));
    }
}
