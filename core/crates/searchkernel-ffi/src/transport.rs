//! FlatBuffers transport framing generated from the versioned FFI schemas.

#[cfg(feature = "ffi-test-support")]
use crate::generated::search_kernel::ffi::v1::RequestEnvelopeArgs;
use crate::generated::search_kernel::ffi::v1::{
    RequestEnvelope, RequestPayload, ResponseEnvelope, ResponseEnvelopeArgs, ResponsePayload,
    REQUEST_ENVELOPE_IDENTIFIER, RESPONSE_ENVELOPE_IDENTIFIER,
};
use flatbuffers::{root, WIPOffset};
#[cfg(feature = "ffi-test-support")]
use searchkernel_ir::api;

pub(crate) const PROTOCOL_VERSION: u32 = 1;

// The discriminants are derived from the generated union bindings. The file
// identifiers are not emitted by flatc's Rust backend, so they remain the
// explicit transport framing constants from the schemas.
pub(crate) const REQUEST_INSERT: u8 = RequestPayload::SearchKernel_V1_InsertRequest.0;
pub(crate) const REQUEST_UPSERT: u8 = RequestPayload::SearchKernel_V1_UpsertRequest.0;
pub(crate) const REQUEST_PATCH: u8 = RequestPayload::SearchKernel_V1_PatchRequest.0;
pub(crate) const REQUEST_DELETE: u8 = RequestPayload::SearchKernel_V1_DeleteRequest.0;
pub(crate) const REQUEST_GET: u8 = RequestPayload::SearchKernel_V1_GetRequest.0;
pub(crate) const REQUEST_SEARCH: u8 = RequestPayload::SearchKernel_V1_SearchRequest.0;
pub(crate) const REQUEST_ANALYZE: u8 = RequestPayload::SearchKernel_V1_AnalyzeRequest.0;
pub(crate) const REQUEST_DESCRIBE: u8 = RequestPayload::SearchKernel_V1_DescribeRequest.0;
pub(crate) const REQUEST_INSPECT_QUERY: u8 = RequestPayload::SearchKernel_V1_InspectQueryRequest.0;

pub(crate) const RESPONSE_INSERT: u8 = ResponsePayload::SearchKernel_V1_InsertResult.0;
pub(crate) const RESPONSE_UPSERT: u8 = ResponsePayload::SearchKernel_V1_UpsertResult.0;
pub(crate) const RESPONSE_PATCH: u8 = ResponsePayload::SearchKernel_V1_PatchResult.0;
pub(crate) const RESPONSE_DELETE: u8 = ResponsePayload::SearchKernel_V1_DeleteResult.0;
pub(crate) const RESPONSE_GET: u8 = ResponsePayload::SearchKernel_V1_GetResult.0;
pub(crate) const RESPONSE_SEARCH: u8 = ResponsePayload::SearchKernel_V1_SearchResult.0;
pub(crate) const RESPONSE_ANALYZE: u8 = ResponsePayload::SearchKernel_V1_AnalyzeResult.0;
pub(crate) const RESPONSE_ERROR: u8 = ResponsePayload::SearchKernel_V1_Error.0;
pub(crate) const RESPONSE_DESCRIBE: u8 = ResponsePayload::SearchKernel_V1_DescribeResult.0;
pub(crate) const RESPONSE_INSPECT_QUERY: u8 = ResponsePayload::SearchKernel_V1_InspectQueryResult.0;

pub(crate) fn parse_request(bytes: &[u8]) -> Result<RequestEnvelope<'_>, ()> {
    if !has_identifier(bytes, REQUEST_ENVELOPE_IDENTIFIER.as_bytes()) {
        return Err(());
    }
    root::<RequestEnvelope<'_>>(bytes).map_err(|_| ())
}

#[cfg(feature = "ffi-test-support")]
pub(crate) fn parse_response(bytes: &[u8]) -> Result<ResponseEnvelope<'_>, ()> {
    if !has_identifier(bytes, RESPONSE_ENVELOPE_IDENTIFIER.as_bytes()) {
        return Err(());
    }
    root::<ResponseEnvelope<'_>>(bytes).map_err(|_| ())
}

fn has_identifier(bytes: &[u8], identifier: &[u8]) -> bool {
    bytes.len() >= 8 && bytes[4..8] == *identifier
}

pub(crate) fn frame_response<F>(kind: u8, encode: F) -> Vec<u8>
where
    F: for<'a> FnOnce(
        &mut flatbuffers::FlatBufferBuilder<'a>,
    ) -> WIPOffset<flatbuffers::UnionWIPOffset>,
{
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let payload = encode(&mut builder);
    let envelope = ResponseEnvelope::create(
        &mut builder,
        &ResponseEnvelopeArgs {
            protocol_version: PROTOCOL_VERSION,
            payload_type: ResponsePayload(kind),
            payload: Some(payload),
        },
    );
    builder.finish(envelope, Some(RESPONSE_ENVELOPE_IDENTIFIER));
    builder.finished_data().to_vec()
}

#[cfg(feature = "ffi-test-support")]
pub(crate) fn frame_request(kind: u8, protocol_version: u32, inner: &[u8]) -> Vec<u8> {
    frame_request_legacy(kind, protocol_version, trim_request_inner(kind, inner))
}

#[cfg(feature = "ffi-test-support")]
fn trim_request_inner(kind: u8, inner: &[u8]) -> &[u8] {
    match kind {
        REQUEST_INSERT => trim_inner(inner, |bytes| root::<api::InsertRequest<'_>>(bytes).is_ok()),
        REQUEST_UPSERT => trim_inner(inner, |bytes| root::<api::UpsertRequest<'_>>(bytes).is_ok()),
        REQUEST_PATCH => trim_inner(inner, |bytes| root::<api::PatchRequest<'_>>(bytes).is_ok()),
        REQUEST_DELETE => trim_inner(inner, |bytes| root::<api::DeleteRequest<'_>>(bytes).is_ok()),
        REQUEST_GET => trim_inner(inner, |bytes| root::<api::GetRequest<'_>>(bytes).is_ok()),
        REQUEST_SEARCH => trim_inner(inner, |bytes| root::<api::SearchRequest<'_>>(bytes).is_ok()),
        REQUEST_ANALYZE => trim_inner(inner, |bytes| {
            root::<api::AnalyzeRequest<'_>>(bytes).is_ok()
        }),
        REQUEST_DESCRIBE => trim_inner(inner, |bytes| {
            root::<api::DescribeRequest<'_>>(bytes).is_ok()
        }),
        REQUEST_INSPECT_QUERY => trim_inner(inner, |bytes| {
            root::<api::InspectQueryRequest<'_>>(bytes).is_ok()
        }),
        _ => inner,
    }
}

#[cfg(feature = "ffi-test-support")]
fn trim_inner(inner: &[u8], verifies: impl Fn(&[u8]) -> bool) -> &[u8] {
    let mut end = inner.len();
    while end > 4 && verifies(&inner[..end - 1]) {
        end -= 1;
    }
    &inner[..end]
}

/// The frozen verifier support API accepts an already-built inner table. Its
/// legacy wrapper must therefore embed that table in the envelope. Production
/// responses use the generated same-builder path above.
#[cfg(feature = "ffi-test-support")]
fn frame_request_legacy(kind: u8, protocol_version: u32, inner: &[u8]) -> Vec<u8> {
    assert!(
        inner.len() >= 4,
        "inner FlatBuffer is missing its root offset"
    );
    let root_offset = usize::try_from(u32::from_le_bytes(inner[..4].try_into().unwrap())).unwrap();
    assert!(
        root_offset < inner.len(),
        "inner FlatBuffer root is out of bounds"
    );

    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let mut copied_inner = Vec::with_capacity(inner.len() + 4);
    copied_inner.resize(4, 0);
    copied_inner.extend_from_slice(inner);
    let inner_vector = builder.create_byte_string(&copied_inner);
    let payload_value = inner_vector.value() - 8 - root_offset as u32;
    let payload = WIPOffset::<flatbuffers::UnionWIPOffset>::new(payload_value);
    let envelope = RequestEnvelope::create(
        &mut builder,
        &RequestEnvelopeArgs {
            protocol_version,
            payload_type: RequestPayload(kind),
            payload: Some(payload),
        },
    );
    builder.finish(envelope, Some(REQUEST_ENVELOPE_IDENTIFIER));
    let finished = builder.finished_data();
    // The helper embeds a copied table in a byte string. Remove its terminator
    // so a truncated complete envelope cannot be mistaken for a valid prefix.
    finished[..finished.len() - 1].to_vec()
}

#[cfg(all(test, feature = "ffi-test-support"))]
mod tests {
    use super::*;

    #[test]
    fn generated_envelopes_verify_and_preserve_payloads() {
        let mut builder = flatbuffers::FlatBufferBuilder::new();
        let inner =
            api::InsertRequest::create(&mut builder, &api::InsertRequestArgs { document: None });
        builder.finish(inner, None);
        let inner_bytes = builder.finished_data().to_vec();

        let request_bytes = frame_request(REQUEST_INSERT, PROTOCOL_VERSION, &inner_bytes);
        let request = parse_request(&request_bytes).expect("request envelope");
        assert_eq!(request.payload_type().0, REQUEST_INSERT);
        assert!(request
            .payload_as_search_kernel_v1_insert_request()
            .is_some());

        let response_bytes = frame_response(RESPONSE_INSERT, |builder| {
            searchkernel_ir::insert_result_offset(
                builder,
                &searchkernel_ir::InsertResult {
                    id: "doc-1".to_owned(),
                },
            )
            .as_union_value()
        });
        let response = parse_response(&response_bytes).expect("response envelope");
        assert_eq!(response.payload_type().0, RESPONSE_INSERT);
        assert_eq!(
            response
                .payload_as_search_kernel_v1_insert_result()
                .and_then(|result| result.id()),
            Some("doc-1")
        );

        let mut fields = std::collections::BTreeMap::new();
        fields.insert(
            "title".to_owned(),
            searchkernel_ir::Value::String("Wireless Headphones".to_owned()),
        );
        let result = searchkernel_ir::SearchResult {
            hits: vec![searchkernel_ir::Hit {
                id: "doc-1".to_owned(),
                score: Some(1.25),
                fields,
                highlights: vec![searchkernel_ir::HighlightResult {
                    field: "title".to_owned(),
                    fragments: vec![searchkernel_ir::HighlightFragment {
                        text: "Wireless Headphones".to_owned(),
                        matches: vec![searchkernel_ir::HighlightSpan {
                            start_byte: 9,
                            end_byte: 19,
                        }],
                    }],
                }],
            }],
            total_hits: searchkernel_ir::TotalHits {
                value: 1,
                relation: searchkernel_ir::TotalHitsRelation::Exact,
            },
            facets: vec![searchkernel_ir::FacetResult {
                field: "brand".to_owned(),
                values: vec![searchkernel_ir::FacetValue {
                    value: searchkernel_ir::Value::String("Sony".to_owned()),
                    count: 1,
                }],
            }],
        };
        let response_bytes = frame_response(RESPONSE_SEARCH, |builder| {
            searchkernel_ir::search_result_offset(builder, &result).as_union_value()
        });
        let response = parse_response(&response_bytes).expect("search response envelope");
        assert!(response
            .payload_as_search_kernel_v1_search_result()
            .is_some());

        let mut request_builder = flatbuffers::FlatBufferBuilder::new();
        let query = request_builder.create_string("headphones");
        let request = api::SearchRequest::create(
            &mut request_builder,
            &api::SearchRequestArgs {
                query: Some(query),
                ..Default::default()
            },
        );
        request_builder.finish(request, None);
        let inner = request_builder.finished_data().to_vec();
        let framed = frame_request(REQUEST_SEARCH, PROTOCOL_VERSION, &inner);
        assert!(parse_request(&framed).is_ok());
        for end in 0..framed.len() {
            assert!(
                parse_request(&framed[..end]).is_err(),
                "valid prefix length {end}"
            );
        }
    }
}
