use crate::{buffer, handle, transport};
use searchkernel_ir::api;
use searchkernel_ir::{Backend, IndexContext};
use std::collections::BTreeMap;
use std::sync::Arc;

#[cfg(feature = "c7-test-support")]
use searchkernel_testkit::{ExpectedCall, RecordingBackend as C7RecordingBackend, ScriptedBackend};
#[cfg(feature = "c7-test-support")]
use std::ffi::CStr;

#[cfg(test)]
pub(crate) use searchkernel_testkit::{
    build_component_transport_case, RecordingBackend, TransportCase,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RequestKind {
    Insert,
    Upsert,
    Patch,
    Delete,
    Get,
    Search,
    Analyze,
    Describe,
    InspectQuery,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DecodedResponse {
    Insert(searchkernel_ir::InsertResult),
    Upsert(searchkernel_ir::UpsertResult),
    Patch(searchkernel_ir::PatchResult),
    Delete(searchkernel_ir::DeleteResult),
    Get(searchkernel_ir::GetResult),
    Search(searchkernel_ir::SearchResult),
    Analyze(searchkernel_ir::AnalyzeResult),
    Error(searchkernel_ir::PortableError),
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
        RequestKind::Describe => transport::REQUEST_DESCRIBE,
        RequestKind::InspectQuery => transport::REQUEST_INSPECT_QUERY,
    }
}

pub fn decode_response(bytes: &[u8]) -> Result<DecodedResponse, String> {
    let envelope = transport::parse_response(bytes)
        .map_err(|_| "response is not a verified SKRS envelope".to_owned())?;
    if envelope.protocol_version() != transport::PROTOCOL_VERSION {
        return Err("unsupported response protocol version".to_owned());
    }

    match envelope.payload_type().0 {
        transport::RESPONSE_INSERT => Ok(DecodedResponse::Insert(searchkernel_ir::InsertResult {
            id: required(
                envelope
                    .payload_as_search_kernel_v1_insert_result()
                    .and_then(|result| result.id()),
                "id",
            )?,
        })),
        transport::RESPONSE_UPSERT => Ok(DecodedResponse::Upsert(searchkernel_ir::UpsertResult {
            id: required(
                envelope
                    .payload_as_search_kernel_v1_upsert_result()
                    .and_then(|result| result.id()),
                "id",
            )?,
        })),
        transport::RESPONSE_PATCH => Ok(DecodedResponse::Patch(searchkernel_ir::PatchResult {
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
            Ok(DecodedResponse::Delete(searchkernel_ir::DeleteResult {
                id: required(result.id(), "id")?,
                deleted: result.deleted(),
            }))
        }
        transport::RESPONSE_GET => {
            let result = envelope
                .payload_as_search_kernel_v1_get_result()
                .ok_or_else(|| "get payload is missing".to_owned())?;
            Ok(DecodedResponse::Get(searchkernel_ir::GetResult {
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
            Ok(DecodedResponse::Search(searchkernel_ir::SearchResult {
                hits,
                total_hits: searchkernel_ir::TotalHits {
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
                            Ok(searchkernel_ir::AnalyzedToken {
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
            Ok(DecodedResponse::Analyze(searchkernel_ir::AnalyzeResult {
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

/// Open the deterministic C7 composition fixture through the normal C5
/// dispatcher.  This entry point is compiled only into the non-production
/// verifier library; the public production ABI remains unchanged.
#[cfg(feature = "c7-test-support")]
#[allow(dead_code)]
pub(crate) fn open_c7_fixture(path: *const std::os::raw::c_char) -> Result<u64, (u32, String)> {
    let name = if path.is_null() {
        "recording"
    } else {
        // SAFETY: the C5 open contract supplies a NUL-terminated path.
        unsafe { CStr::from_ptr(path) }
            .to_str()
            .unwrap_or("recording")
    };
    let context = c7_context().map_err(|error| (crate::SK_STATUS_INVALID_ARGUMENT, error))?;
    if name == "scripted" {
        let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
        let request = api::SearchRequest::create(&mut builder, &api::SearchRequestArgs::default());
        builder.finish(request, None);
        let request =
            searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(builder.finished_data())
                .map_err(|error| (crate::SK_STATUS_INTERNAL, error.to_string()))?;
        let expected = context
            .search_to_ir(request)
            .map_err(|error| (crate::SK_STATUS_INTERNAL, error.to_string()))?;
        let scripted = ScriptedBackend::new([ExpectedCall::error(
            searchkernel_ir::OperationIr::Search(expected),
            searchkernel_ir::PortableError::unsupported(
                "c7_scripted",
                None::<String>,
                "scripted C7 portable error",
            ),
        )]);
        Ok(register_index(context, Arc::new(scripted)))
    } else {
        Ok(register_index(context, Arc::new(C7RecordingBackend::new())))
    }
}

#[cfg(feature = "c7-test-support")]
#[allow(dead_code)]
fn c7_context() -> Result<Arc<IndexContext>, String> {
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let name = builder.create_string("title");
    let stored = api::OptionalBool::new(true);
    let searchable = api::OptionalBool::new(true);
    let field = api::FieldDefinition::create(
        &mut builder,
        &api::FieldDefinitionArgs {
            name: Some(name),
            kind: api::FieldKind::Text,
            cardinality: api::FieldCardinality::Single,
            stored: Some(&stored),
            searchable: Some(&searchable),
            ..Default::default()
        },
    );
    let fields = builder.create_vector(&[field]);
    let definition = api::IndexDefinition::create(
        &mut builder,
        &api::IndexDefinitionArgs {
            fields: Some(fields),
            ..Default::default()
        },
    );
    builder.finish(definition, None);
    let definition =
        searchkernel_ir::__flatbuffers::root::<api::IndexDefinition>(builder.finished_data())
            .map_err(|error| error.to_string())?;
    IndexContext::from_api(definition)
        .map(Arc::new)
        .map_err(|error| error.to_string())
}

fn required(value: Option<&str>, name: &str) -> Result<String, String> {
    value
        .map(ToOwned::to_owned)
        .ok_or_else(|| format!("{name} is missing"))
}

fn decode_document(document: api::Document<'_>) -> Result<searchkernel_ir::DocumentResult, String> {
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
    Ok(searchkernel_ir::DocumentResult {
        id: required(document.id(), "document id")?,
        fields,
    })
}

fn decode_value(value: api::Value<'_>) -> Result<searchkernel_ir::Value, String> {
    match value.kind_type() {
        api::ValueKind::NullValueValue => Ok(searchkernel_ir::Value::Null),
        api::ValueKind::BoolValue => Ok(searchkernel_ir::Value::Bool(
            value
                .kind_as_bool_value()
                .ok_or_else(|| "bool value is missing".to_owned())?
                .value(),
        )),
        api::ValueKind::Int64Value => Ok(searchkernel_ir::Value::Int64(
            value
                .kind_as_int_64_value()
                .ok_or_else(|| "int64 value is missing".to_owned())?
                .value(),
        )),
        api::ValueKind::UInt64Value => Ok(searchkernel_ir::Value::UInt64(
            value
                .kind_as_uint_64_value()
                .ok_or_else(|| "uint64 value is missing".to_owned())?
                .value(),
        )),
        api::ValueKind::Float64Value => Ok(searchkernel_ir::Value::Float64(
            value
                .kind_as_float_64_value()
                .ok_or_else(|| "float64 value is missing".to_owned())?
                .value(),
        )),
        api::ValueKind::StringValue => Ok(searchkernel_ir::Value::String(required(
            value
                .kind_as_string_value()
                .ok_or_else(|| "string value is missing".to_owned())?
                .value(),
            "string value",
        )?)),
        api::ValueKind::BytesValue => Ok(searchkernel_ir::Value::Bytes(
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
            Ok(searchkernel_ir::Value::List(values))
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
            Ok(searchkernel_ir::Value::Object(fields))
        }
        _ => Err("unknown value kind".to_owned()),
    }
}

fn decode_hit(hit: api::Hit<'_>) -> Result<searchkernel_ir::Hit, String> {
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
    Ok(searchkernel_ir::Hit {
        id: required(hit.id(), "hit id")?,
        score: hit.score().map(|score| score.value()),
        fields,
        highlights,
    })
}

fn decode_highlight(
    highlight: api::HighlightResult<'_>,
) -> Result<searchkernel_ir::HighlightResult, String> {
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
                                .map(|span| searchkernel_ir::HighlightSpan {
                                    start_byte: span.start_byte(),
                                    end_byte: span.end_byte(),
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    Ok(searchkernel_ir::HighlightFragment {
                        text: required(fragment.text(), "highlight text")?,
                        matches,
                    })
                })
                .collect::<Result<Vec<_>, String>>()
        })
        .transpose()?
        .unwrap_or_default();
    Ok(searchkernel_ir::HighlightResult {
        field: required(highlight.field(), "highlight field")?,
        fragments,
    })
}

fn decode_facet(facet: api::FacetResult<'_>) -> Result<searchkernel_ir::FacetResult, String> {
    let values = facet
        .values()
        .map(|values| {
            values
                .iter()
                .map(|value| {
                    Ok(searchkernel_ir::FacetValue {
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
    Ok(searchkernel_ir::FacetResult {
        field: required(facet.field(), "facet field")?,
        values,
    })
}

fn decode_total_hits_relation(
    relation: api::TotalHitsRelation,
) -> Result<searchkernel_ir::TotalHitsRelation, String> {
    match relation {
        api::TotalHitsRelation::Exact => Ok(searchkernel_ir::TotalHitsRelation::Exact),
        api::TotalHitsRelation::LowerBound => Ok(searchkernel_ir::TotalHitsRelation::LowerBound),
        _ => Err("unknown total hits relation".to_owned()),
    }
}

fn decode_error(error: api::Error<'_>) -> Result<searchkernel_ir::PortableError, String> {
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
                    Ok(searchkernel_ir::ErrorDetail {
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
    Ok(searchkernel_ir::PortableError {
        code: decode_error_code(error.code())?,
        message: required(error.message(), "error message")?,
        details,
        retryable: error.retryable(),
    })
}

fn decode_error_code(code: api::ErrorCode) -> Result<searchkernel_ir::ErrorCode, String> {
    match code {
        api::ErrorCode::InvalidArgument => Ok(searchkernel_ir::ErrorCode::InvalidArgument),
        api::ErrorCode::NotFound => Ok(searchkernel_ir::ErrorCode::NotFound),
        api::ErrorCode::AlreadyExists => Ok(searchkernel_ir::ErrorCode::AlreadyExists),
        api::ErrorCode::FailedPrecondition => Ok(searchkernel_ir::ErrorCode::FailedPrecondition),
        api::ErrorCode::Conflict => Ok(searchkernel_ir::ErrorCode::Conflict),
        api::ErrorCode::Unsupported => Ok(searchkernel_ir::ErrorCode::Unsupported),
        api::ErrorCode::ResourceExhausted => Ok(searchkernel_ir::ErrorCode::ResourceExhausted),
        api::ErrorCode::Internal => Ok(searchkernel_ir::ErrorCode::Internal),
        api::ErrorCode::Unavailable => Ok(searchkernel_ir::ErrorCode::Unavailable),
        api::ErrorCode::DeadlineExceeded => Ok(searchkernel_ir::ErrorCode::DeadlineExceeded),
        _ => Err("unknown error code".to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_framed_portable_errors() {
        let error = searchkernel_ir::PortableError::new(
            searchkernel_ir::ErrorCode::Unavailable,
            "temporarily unavailable",
        )
        .with_detail(
            "backend_unavailable",
            Some("search"),
            "temporarily unavailable",
        );
        let envelope = transport::frame_response(transport::RESPONSE_ERROR, |builder| {
            searchkernel_ir::error_offset(builder, &error).as_union_value()
        });
        assert_eq!(
            decode_response(&envelope),
            Ok(DecodedResponse::Error(error))
        );
    }
}
