use crate::{buffer, handle, transport};
use searchkernel_core::api;
use searchkernel_core::{Backend, IndexContext};
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RequestKind {
    Insert,
    Upsert,
    Patch,
    Delete,
    Get,
    Search,
    Analyze,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DecodedResponse {
    Insert(searchkernel_core::InsertResult),
    Upsert(searchkernel_core::UpsertResult),
    Patch(searchkernel_core::PatchResult),
    Delete(searchkernel_core::DeleteResult),
    Get(searchkernel_core::GetResult),
    Search(searchkernel_core::SearchResult),
    Analyze(searchkernel_core::AnalyzeResult),
    Error(searchkernel_core::PortableError),
}

pub fn register_index(context: Arc<IndexContext>, backend: Arc<dyn Backend>) -> u64 {
    handle::register(context, backend)
}

pub fn live_handle_count() -> usize {
    handle::live_count()
}

pub fn live_buffer_count() -> usize {
    buffer::live_count()
}

pub fn wrap_request(kind: RequestKind, inner_flatbuffer: &[u8]) -> Vec<u8> {
    wrap_request_with_protocol(kind, inner_flatbuffer, transport::PROTOCOL_VERSION)
}

pub fn wrap_request_with_protocol(
    kind: RequestKind,
    inner_flatbuffer: &[u8],
    protocol_version: u32,
) -> Vec<u8> {
    transport::frame_request(request_kind(kind), protocol_version, inner_flatbuffer)
}

pub fn wrap_unknown_operation(operation_discriminant: u8, inner_flatbuffer: &[u8]) -> Vec<u8> {
    transport::frame_request(
        operation_discriminant,
        transport::PROTOCOL_VERSION,
        inner_flatbuffer,
    )
}

fn request_kind(kind: RequestKind) -> u8 {
    match kind {
        RequestKind::Insert => transport::REQUEST_INSERT,
        RequestKind::Upsert => transport::REQUEST_UPSERT,
        RequestKind::Patch => transport::REQUEST_PATCH,
        RequestKind::Delete => transport::REQUEST_DELETE,
        RequestKind::Get => transport::REQUEST_GET,
        RequestKind::Search => transport::REQUEST_SEARCH,
        RequestKind::Analyze => transport::REQUEST_ANALYZE,
    }
}

pub fn decode_response(bytes: &[u8]) -> Result<DecodedResponse, String> {
    let envelope = transport::parse_response(bytes)
        .map_err(|_| "response is not a verified SKRS envelope".to_owned())?;
    if envelope.protocol_version() != transport::PROTOCOL_VERSION {
        return Err("unsupported response protocol version".to_owned());
    }

    match envelope.payload_type().0 {
        transport::RESPONSE_INSERT => {
            Ok(DecodedResponse::Insert(searchkernel_core::InsertResult {
                id: required(
                    envelope
                        .payload_as_search_kernel_v1_insert_result()
                        .and_then(|result| result.id()),
                    "id",
                )?,
            }))
        }
        transport::RESPONSE_UPSERT => {
            Ok(DecodedResponse::Upsert(searchkernel_core::UpsertResult {
                id: required(
                    envelope
                        .payload_as_search_kernel_v1_upsert_result()
                        .and_then(|result| result.id()),
                    "id",
                )?,
            }))
        }
        transport::RESPONSE_PATCH => Ok(DecodedResponse::Patch(searchkernel_core::PatchResult {
            id: required(
                envelope
                    .payload_as_search_kernel_v1_patch_result()
                    .and_then(|result| result.id()),
                "id",
            )?,
        })),
        transport::RESPONSE_DELETE => {
            let result = envelope
                .payload_as_search_kernel_v1_delete_result()
                .ok_or_else(|| "delete payload is missing".to_owned())?;
            Ok(DecodedResponse::Delete(searchkernel_core::DeleteResult {
                id: required(result.id(), "id")?,
                deleted: result.deleted(),
            }))
        }
        transport::RESPONSE_GET => {
            let result = envelope
                .payload_as_search_kernel_v1_get_result()
                .ok_or_else(|| "get payload is missing".to_owned())?;
            Ok(DecodedResponse::Get(searchkernel_core::GetResult {
                document: result.document().map(decode_document).transpose()?,
            }))
        }
        transport::RESPONSE_SEARCH => {
            let result = envelope
                .payload_as_search_kernel_v1_search_result()
                .ok_or_else(|| "search payload is missing".to_owned())?;
            let total_hits = result
                .total_hits()
                .ok_or_else(|| "search total_hits is missing".to_owned())?;
            let hits = result
                .hits()
                .map(|hits| hits.iter().map(decode_hit).collect::<Result<Vec<_>, _>>())
                .transpose()?
                .unwrap_or_default();
            let facets = result
                .facets()
                .map(|facets| {
                    facets
                        .iter()
                        .map(decode_facet)
                        .collect::<Result<Vec<_>, _>>()
                })
                .transpose()?
                .unwrap_or_default();
            Ok(DecodedResponse::Search(searchkernel_core::SearchResult {
                hits,
                total_hits: searchkernel_core::TotalHits {
                    value: total_hits.value(),
                    relation: decode_total_hits_relation(total_hits.relation())?,
                },
                facets,
            }))
        }
        transport::RESPONSE_ANALYZE => {
            let result = envelope
                .payload_as_search_kernel_v1_analyze_result()
                .ok_or_else(|| "analyze payload is missing".to_owned())?;
            let tokens = result
                .tokens()
                .map(|tokens| {
                    tokens
                        .iter()
                        .map(|token| {
                            Ok(searchkernel_core::AnalyzedToken {
                                surface: required(token.surface(), "surface")?,
                                term: required(token.term(), "term")?,
                                position: token.position(),
                                start_byte: token.start_byte(),
                                end_byte: token.end_byte(),
                            })
                        })
                        .collect::<Result<Vec<_>, String>>()
                })
                .transpose()?
                .unwrap_or_default();
            Ok(DecodedResponse::Analyze(searchkernel_core::AnalyzeResult {
                tokens,
            }))
        }
        transport::RESPONSE_ERROR => Ok(DecodedResponse::Error(decode_error(
            envelope
                .payload_as_search_kernel_v1_error()
                .ok_or_else(|| "error payload is missing".to_owned())?,
        )?)),
        0 => Err("response payload is missing".to_owned()),
        _ => Err("unsupported response payload".to_owned()),
    }
}

fn required(value: Option<&str>, name: &str) -> Result<String, String> {
    value
        .map(ToOwned::to_owned)
        .ok_or_else(|| format!("{name} is missing"))
}

fn decode_document(
    document: api::Document<'_>,
) -> Result<searchkernel_core::DocumentResult, String> {
    let fields = document
        .fields()
        .map(|fields| {
            fields
                .iter()
                .map(|entry| {
                    Ok((
                        entry.key().to_owned(),
                        decode_value(
                            entry
                                .value()
                                .ok_or_else(|| "document value is missing".to_owned())?,
                        )?,
                    ))
                })
                .collect::<Result<BTreeMap<_, _>, String>>()
        })
        .transpose()?
        .unwrap_or_default();
    Ok(searchkernel_core::DocumentResult {
        id: required(document.id(), "document id")?,
        fields,
    })
}

fn decode_value(value: api::Value<'_>) -> Result<searchkernel_core::Value, String> {
    match value.kind_type() {
        api::ValueKind::NullValueValue => Ok(searchkernel_core::Value::Null),
        api::ValueKind::BoolValue => Ok(searchkernel_core::Value::Bool(
            value
                .kind_as_bool_value()
                .ok_or_else(|| "bool value is missing".to_owned())?
                .value(),
        )),
        api::ValueKind::Int64Value => Ok(searchkernel_core::Value::Int64(
            value
                .kind_as_int_64_value()
                .ok_or_else(|| "int64 value is missing".to_owned())?
                .value(),
        )),
        api::ValueKind::UInt64Value => Ok(searchkernel_core::Value::UInt64(
            value
                .kind_as_uint_64_value()
                .ok_or_else(|| "uint64 value is missing".to_owned())?
                .value(),
        )),
        api::ValueKind::Float64Value => Ok(searchkernel_core::Value::Float64(
            value
                .kind_as_float_64_value()
                .ok_or_else(|| "float64 value is missing".to_owned())?
                .value(),
        )),
        api::ValueKind::StringValue => Ok(searchkernel_core::Value::String(required(
            value
                .kind_as_string_value()
                .ok_or_else(|| "string value is missing".to_owned())?
                .value(),
            "string value",
        )?)),
        api::ValueKind::BytesValue => Ok(searchkernel_core::Value::Bytes(
            value
                .kind_as_bytes_value()
                .ok_or_else(|| "bytes value is missing".to_owned())?
                .value()
                .map(|bytes| bytes.iter().collect())
                .unwrap_or_default(),
        )),
        api::ValueKind::ValueList => {
            let list = value
                .kind_as_value_list()
                .ok_or_else(|| "value list is missing".to_owned())?;
            let values = list
                .values()
                .map(|values| values.iter().map(decode_value).collect())
                .transpose()?
                .unwrap_or_default();
            Ok(searchkernel_core::Value::List(values))
        }
        api::ValueKind::ValueObject => {
            let object = value
                .kind_as_value_object()
                .ok_or_else(|| "value object is missing".to_owned())?;
            let fields = object
                .fields()
                .map(|fields| {
                    fields
                        .iter()
                        .map(|entry| {
                            Ok((
                                entry.key().to_owned(),
                                decode_value(
                                    entry
                                        .value()
                                        .ok_or_else(|| "object value is missing".to_owned())?,
                                )?,
                            ))
                        })
                        .collect::<Result<BTreeMap<_, _>, String>>()
                })
                .transpose()?
                .unwrap_or_default();
            Ok(searchkernel_core::Value::Object(fields))
        }
        _ => Err("unknown value kind".to_owned()),
    }
}

fn decode_hit(hit: api::Hit<'_>) -> Result<searchkernel_core::Hit, String> {
    let fields = hit
        .fields()
        .map(|fields| {
            fields
                .iter()
                .map(|entry| {
                    Ok((
                        entry.key().to_owned(),
                        decode_value(
                            entry
                                .value()
                                .ok_or_else(|| "hit value is missing".to_owned())?,
                        )?,
                    ))
                })
                .collect::<Result<BTreeMap<_, _>, String>>()
        })
        .transpose()?
        .unwrap_or_default();
    let highlights = hit
        .highlights()
        .map(|highlights| {
            highlights
                .iter()
                .map(decode_highlight)
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?
        .unwrap_or_default();
    Ok(searchkernel_core::Hit {
        id: required(hit.id(), "hit id")?,
        score: hit.score().map(|score| score.value()),
        fields,
        highlights,
    })
}

fn decode_highlight(
    highlight: api::HighlightResult<'_>,
) -> Result<searchkernel_core::HighlightResult, String> {
    let fragments = highlight
        .fragments()
        .map(|fragments| {
            fragments
                .iter()
                .map(|fragment| {
                    let matches = fragment
                        .matches()
                        .map(|matches| {
                            matches
                                .iter()
                                .map(|span| searchkernel_core::HighlightSpan {
                                    start_byte: span.start_byte(),
                                    end_byte: span.end_byte(),
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    Ok(searchkernel_core::HighlightFragment {
                        text: required(fragment.text(), "highlight text")?,
                        matches,
                    })
                })
                .collect::<Result<Vec<_>, String>>()
        })
        .transpose()?
        .unwrap_or_default();
    Ok(searchkernel_core::HighlightResult {
        field: required(highlight.field(), "highlight field")?,
        fragments,
    })
}

fn decode_facet(facet: api::FacetResult<'_>) -> Result<searchkernel_core::FacetResult, String> {
    let values = facet
        .values()
        .map(|values| {
            values
                .iter()
                .map(|value| {
                    Ok(searchkernel_core::FacetValue {
                        value: decode_value(
                            value
                                .value()
                                .ok_or_else(|| "facet value is missing".to_owned())?,
                        )?,
                        count: value.count(),
                    })
                })
                .collect::<Result<Vec<_>, String>>()
        })
        .transpose()?
        .unwrap_or_default();
    Ok(searchkernel_core::FacetResult {
        field: required(facet.field(), "facet field")?,
        values,
    })
}

fn decode_total_hits_relation(
    relation: api::TotalHitsRelation,
) -> Result<searchkernel_core::TotalHitsRelation, String> {
    match relation {
        api::TotalHitsRelation::Exact => Ok(searchkernel_core::TotalHitsRelation::Exact),
        api::TotalHitsRelation::LowerBound => Ok(searchkernel_core::TotalHitsRelation::LowerBound),
        _ => Err("unknown total hits relation".to_owned()),
    }
}

fn decode_error(error: api::Error<'_>) -> Result<searchkernel_core::PortableError, String> {
    let details = error
        .details()
        .map(|details| {
            details
                .iter()
                .map(|detail| {
                    let metadata = detail
                        .metadata()
                        .map(|entries| {
                            entries
                                .iter()
                                .map(|entry| {
                                    Ok((
                                        entry.key().to_owned(),
                                        required(entry.value(), "error metadata value")?,
                                    ))
                                })
                                .collect::<Result<BTreeMap<_, _>, String>>()
                        })
                        .transpose()?
                        .unwrap_or_default();
                    Ok(searchkernel_core::ErrorDetail {
                        code: required(detail.code(), "error detail code")?,
                        path: detail.path().map(ToOwned::to_owned),
                        message: required(detail.message(), "error detail message")?,
                        metadata,
                    })
                })
                .collect::<Result<Vec<_>, String>>()
        })
        .transpose()?
        .unwrap_or_default();
    Ok(searchkernel_core::PortableError {
        code: decode_error_code(error.code())?,
        message: required(error.message(), "error message")?,
        details,
        retryable: error.retryable(),
    })
}

fn decode_error_code(code: api::ErrorCode) -> Result<searchkernel_core::ErrorCode, String> {
    match code {
        api::ErrorCode::InvalidArgument => Ok(searchkernel_core::ErrorCode::InvalidArgument),
        api::ErrorCode::NotFound => Ok(searchkernel_core::ErrorCode::NotFound),
        api::ErrorCode::AlreadyExists => Ok(searchkernel_core::ErrorCode::AlreadyExists),
        api::ErrorCode::FailedPrecondition => Ok(searchkernel_core::ErrorCode::FailedPrecondition),
        api::ErrorCode::Conflict => Ok(searchkernel_core::ErrorCode::Conflict),
        api::ErrorCode::Unsupported => Ok(searchkernel_core::ErrorCode::Unsupported),
        api::ErrorCode::ResourceExhausted => Ok(searchkernel_core::ErrorCode::ResourceExhausted),
        api::ErrorCode::Internal => Ok(searchkernel_core::ErrorCode::Internal),
        api::ErrorCode::Unavailable => Ok(searchkernel_core::ErrorCode::Unavailable),
        _ => Err("unknown error code".to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_framed_portable_errors() {
        let error = searchkernel_core::PortableError::new(
            searchkernel_core::ErrorCode::Unavailable,
            "temporarily unavailable",
        )
        .with_detail(
            "backend_unavailable",
            Some("search"),
            "temporarily unavailable",
        );
        let envelope = transport::frame_response(transport::RESPONSE_ERROR, |builder| {
            searchkernel_core::error_offset(builder, &error).as_union_value()
        });
        assert_eq!(
            decode_response(&envelope),
            Ok(DecodedResponse::Error(error))
        );
    }
}
