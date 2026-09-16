use crate::{buffer, handle, transport};
use searchkernel_ir::api;
use searchkernel_ir::{Backend, IndexContext};
use std::collections::BTreeMap;
use std::sync::Arc;

#[cfg(feature = "c7-test-support")]
use searchkernel_testkit::{ExpectedCall, RecordingBackend as C7RecordingBackend, ScriptedBackend};
#[cfg(feature = "c7-test-support")]
use std::ffi::CStr;
#[cfg(feature = "c7-test-support")]
use std::fs;

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
    if let Some(request_path) = name.strip_prefix("c7-auto|") {
        return open_c7_auto(request_path, context);
    }
    if let Some(spec) = name.strip_prefix("c7-script|") {
        return open_c7_script(name, spec, context);
    }
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
fn open_c7_script(
    name: &str,
    spec: &str,
    context: Arc<IndexContext>,
) -> Result<u64, (u32, String)> {
    let (request_path, response_path) = spec.split_once('|').ok_or_else(|| {
        (
            crate::SK_STATUS_INVALID_ARGUMENT,
            format!("invalid C7 fixture spec: {name}"),
        )
    })?;
    let request = fs::read(request_path).map_err(|error| {
        (
            crate::SK_STATUS_INVALID_ARGUMENT,
            format!("read C7 request fixture: {error}"),
        )
    })?;
    let response = fs::read(response_path).map_err(|error| {
        (
            crate::SK_STATUS_INVALID_ARGUMENT,
            format!("read C7 response fixture: {error}"),
        )
    })?;
    let envelope = transport::parse_request(&request).map_err(|_| {
        (
            crate::SK_STATUS_INVALID_ARGUMENT,
            "C7 request is not a valid SKRQ envelope".to_owned(),
        )
    })?;
    let expected = match envelope.payload_type().0 {
        transport::REQUEST_INSERT => envelope
            .payload_as_search_kernel_v1_insert_request()
            .ok_or_else(|| {
                (
                    crate::SK_STATUS_INVALID_ARGUMENT,
                    "insert request payload is missing".to_owned(),
                )
            })
            .and_then(|request| {
                context
                    .insert_to_ir(request)
                    .map(searchkernel_ir::OperationIr::Insert)
                    .map_err(|error| (crate::SK_STATUS_INVALID_ARGUMENT, error.to_string()))
            })?,
        transport::REQUEST_UPSERT => envelope
            .payload_as_search_kernel_v1_upsert_request()
            .ok_or_else(|| {
                (
                    crate::SK_STATUS_INVALID_ARGUMENT,
                    "upsert request payload is missing".to_owned(),
                )
            })
            .and_then(|request| {
                context
                    .upsert_to_ir(request)
                    .map(searchkernel_ir::OperationIr::Upsert)
                    .map_err(|error| (crate::SK_STATUS_INVALID_ARGUMENT, error.to_string()))
            })?,
        transport::REQUEST_PATCH => envelope
            .payload_as_search_kernel_v1_patch_request()
            .ok_or_else(|| {
                (
                    crate::SK_STATUS_INVALID_ARGUMENT,
                    "patch request payload is missing".to_owned(),
                )
            })
            .and_then(|request| {
                context
                    .patch_to_ir(request)
                    .map(searchkernel_ir::OperationIr::Patch)
                    .map_err(|error| (crate::SK_STATUS_INVALID_ARGUMENT, error.to_string()))
            })?,
        transport::REQUEST_DELETE => envelope
            .payload_as_search_kernel_v1_delete_request()
            .ok_or_else(|| {
                (
                    crate::SK_STATUS_INVALID_ARGUMENT,
                    "delete request payload is missing".to_owned(),
                )
            })
            .and_then(|request| {
                context
                    .delete_to_ir(request)
                    .map(searchkernel_ir::OperationIr::Delete)
                    .map_err(|error| (crate::SK_STATUS_INVALID_ARGUMENT, error.to_string()))
            })?,
        transport::REQUEST_GET => envelope
            .payload_as_search_kernel_v1_get_request()
            .ok_or_else(|| {
                (
                    crate::SK_STATUS_INVALID_ARGUMENT,
                    "get request payload is missing".to_owned(),
                )
            })
            .and_then(|request| {
                context
                    .get_to_ir(request)
                    .map(searchkernel_ir::OperationIr::Get)
                    .map_err(|error| (crate::SK_STATUS_INVALID_ARGUMENT, error.to_string()))
            })?,
        transport::REQUEST_SEARCH => envelope
            .payload_as_search_kernel_v1_search_request()
            .ok_or_else(|| {
                (
                    crate::SK_STATUS_INVALID_ARGUMENT,
                    "search request payload is missing".to_owned(),
                )
            })
            .and_then(|request| {
                context
                    .search_to_ir(request)
                    .map(searchkernel_ir::OperationIr::Search)
                    .map_err(|error| (crate::SK_STATUS_INVALID_ARGUMENT, error.to_string()))
            })?,
        transport::REQUEST_ANALYZE => envelope
            .payload_as_search_kernel_v1_analyze_request()
            .ok_or_else(|| {
                (
                    crate::SK_STATUS_INVALID_ARGUMENT,
                    "analyze request payload is missing".to_owned(),
                )
            })
            .and_then(|request| {
                context
                    .analyze_to_ir(request)
                    .map(searchkernel_ir::OperationIr::Analyze)
                    .map_err(|error| (crate::SK_STATUS_INVALID_ARGUMENT, error.to_string()))
            })?,
        _ => return Ok(register_index(context, Arc::new(C7RecordingBackend::new()))),
    };
    let decoded = decode_response(&response).map_err(|error| {
        (
            crate::SK_STATUS_INVALID_ARGUMENT,
            format!("decode C7 response fixture: {error}"),
        )
    })?;
    let backend: Arc<dyn Backend> = match (expected, decoded) {
        (searchkernel_ir::OperationIr::Insert(request), DecodedResponse::Insert(result)) => {
            Arc::new(ScriptedBackend::new([ExpectedCall::success(
                searchkernel_ir::OperationIr::Insert(request),
                searchkernel_testkit::ScriptedResult::Insert(result),
            )]))
        }
        (searchkernel_ir::OperationIr::Upsert(request), DecodedResponse::Upsert(result)) => {
            Arc::new(ScriptedBackend::new([ExpectedCall::success(
                searchkernel_ir::OperationIr::Upsert(request),
                searchkernel_testkit::ScriptedResult::Upsert(result),
            )]))
        }
        (searchkernel_ir::OperationIr::Patch(request), DecodedResponse::Patch(result)) => {
            Arc::new(ScriptedBackend::new([ExpectedCall::success(
                searchkernel_ir::OperationIr::Patch(request),
                searchkernel_testkit::ScriptedResult::Patch(result),
            )]))
        }
        (searchkernel_ir::OperationIr::Delete(request), DecodedResponse::Delete(result)) => {
            Arc::new(ScriptedBackend::new([ExpectedCall::success(
                searchkernel_ir::OperationIr::Delete(request),
                searchkernel_testkit::ScriptedResult::Delete(result),
            )]))
        }
        (searchkernel_ir::OperationIr::Get(request), DecodedResponse::Get(result)) => {
            Arc::new(ScriptedBackend::new([ExpectedCall::success(
                searchkernel_ir::OperationIr::Get(request),
                searchkernel_testkit::ScriptedResult::Get(result),
            )]))
        }
        (searchkernel_ir::OperationIr::Search(request), DecodedResponse::Search(result)) => {
            Arc::new(ScriptedBackend::new([ExpectedCall::success(
                searchkernel_ir::OperationIr::Search(request),
                searchkernel_testkit::ScriptedResult::Search(result),
            )]))
        }
        (searchkernel_ir::OperationIr::Analyze(request), DecodedResponse::Analyze(result)) => {
            Arc::new(ScriptedBackend::new([ExpectedCall::success(
                searchkernel_ir::OperationIr::Analyze(request),
                searchkernel_testkit::ScriptedResult::Analyze(result),
            )]))
        }
        (expected, DecodedResponse::Error(error)) => {
            Arc::new(ScriptedBackend::new([ExpectedCall::error(expected, error)]))
        }
        _ => {
            return Err((
                crate::SK_STATUS_INVALID_ARGUMENT,
                "C7 request/response operation mismatch".to_owned(),
            ))
        }
    };
    Ok(register_index(context, backend))
}

#[cfg(feature = "c7-test-support")]
fn open_c7_auto(request_path: &str, context: Arc<IndexContext>) -> Result<u64, (u32, String)> {
    let request = fs::read(request_path).map_err(|error| {
        (
            crate::SK_STATUS_INVALID_ARGUMENT,
            format!("read C7 request fixture: {error}"),
        )
    })?;
    let envelope = transport::parse_request(&request).map_err(|_| {
        (
            crate::SK_STATUS_INVALID_ARGUMENT,
            "C7 request is not a valid SKRQ envelope".to_owned(),
        )
    })?;
    let Some(expected) = c7_expected_operation(&context, &envelope) else {
        return Ok(register_index(context, Arc::new(C7RecordingBackend::new())));
    };
    Ok(register_index(context, rich_backend(expected)))
}

#[cfg(feature = "c7-test-support")]
fn c7_expected_operation(
    context: &IndexContext,
    envelope: &crate::generated::search_kernel::ffi::v1::RequestEnvelope<'_>,
) -> Option<searchkernel_ir::OperationIr> {
    match envelope.payload_type().0 {
        transport::REQUEST_INSERT => envelope
            .payload_as_search_kernel_v1_insert_request()
            .and_then(|request| context.insert_to_ir(request).ok())
            .map(searchkernel_ir::OperationIr::Insert),
        transport::REQUEST_UPSERT => envelope
            .payload_as_search_kernel_v1_upsert_request()
            .and_then(|request| context.upsert_to_ir(request).ok())
            .map(searchkernel_ir::OperationIr::Upsert),
        transport::REQUEST_PATCH => envelope
            .payload_as_search_kernel_v1_patch_request()
            .and_then(|request| context.patch_to_ir(request).ok())
            .map(searchkernel_ir::OperationIr::Patch),
        transport::REQUEST_DELETE => envelope
            .payload_as_search_kernel_v1_delete_request()
            .and_then(|request| context.delete_to_ir(request).ok())
            .map(searchkernel_ir::OperationIr::Delete),
        transport::REQUEST_GET => envelope
            .payload_as_search_kernel_v1_get_request()
            .and_then(|request| context.get_to_ir(request).ok())
            .map(searchkernel_ir::OperationIr::Get),
        transport::REQUEST_SEARCH => envelope
            .payload_as_search_kernel_v1_search_request()
            .and_then(|request| context.search_to_ir(request).ok())
            .map(searchkernel_ir::OperationIr::Search),
        transport::REQUEST_ANALYZE => envelope
            .payload_as_search_kernel_v1_analyze_request()
            .and_then(|request| context.analyze_to_ir(request).ok())
            .map(searchkernel_ir::OperationIr::Analyze),
        _ => None,
    }
}

#[cfg(feature = "c7-test-support")]
fn rich_backend(expected: searchkernel_ir::OperationIr) -> Arc<dyn Backend> {
    use searchkernel_testkit::ScriptedResult;
    let call = match expected {
        searchkernel_ir::OperationIr::Insert(request) => ExpectedCall::success(
            searchkernel_ir::OperationIr::Insert(request.clone()),
            ScriptedResult::Insert(searchkernel_ir::InsertResult {
                id: request.document().id().as_str().to_owned(),
            }),
        ),
        searchkernel_ir::OperationIr::Upsert(request) => ExpectedCall::success(
            searchkernel_ir::OperationIr::Upsert(request.clone()),
            ScriptedResult::Upsert(searchkernel_ir::UpsertResult {
                id: request.document().id().as_str().to_owned(),
            }),
        ),
        searchkernel_ir::OperationIr::Patch(request) => ExpectedCall::success(
            searchkernel_ir::OperationIr::Patch(request.clone()),
            ScriptedResult::Patch(searchkernel_ir::PatchResult {
                id: request.id().as_str().to_owned(),
            }),
        ),
        searchkernel_ir::OperationIr::Delete(request) => ExpectedCall::success(
            searchkernel_ir::OperationIr::Delete(request.clone()),
            ScriptedResult::Delete(searchkernel_ir::DeleteResult {
                id: request.id().as_str().to_owned(),
                deleted: true,
            }),
        ),
        searchkernel_ir::OperationIr::Get(request) => ExpectedCall::success(
            searchkernel_ir::OperationIr::Get(request),
            ScriptedResult::Get(searchkernel_ir::GetResult {
                document: Some(searchkernel_ir::DocumentResult {
                    id: "c7-rich-document".to_owned(),
                    fields: BTreeMap::from([
                        (
                            "title".to_owned(),
                            searchkernel_ir::Value::String("C7 portable result".to_owned()),
                        ),
                        ("price".to_owned(), searchkernel_ir::Value::Float64(42.5)),
                    ]),
                }),
            }),
        ),
        searchkernel_ir::OperationIr::Search(request) => ExpectedCall::success(
            searchkernel_ir::OperationIr::Search(request),
            ScriptedResult::Search(searchkernel_ir::SearchResult {
                hits: vec![searchkernel_ir::Hit {
                    id: "c7-rich-document".to_owned(),
                    score: Some(3.5),
                    fields: BTreeMap::from([
                        (
                            "title".to_owned(),
                            searchkernel_ir::Value::String("C7 portable result".to_owned()),
                        ),
                        (
                            "brand".to_owned(),
                            searchkernel_ir::Value::String("Acme".to_owned()),
                        ),
                    ]),
                    highlights: vec![searchkernel_ir::HighlightResult {
                        field: "title".to_owned(),
                        fragments: vec![searchkernel_ir::HighlightFragment {
                            text: "C7 portable result".to_owned(),
                            matches: vec![searchkernel_ir::HighlightSpan {
                                start_byte: 3,
                                end_byte: 5,
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
                        value: searchkernel_ir::Value::String("Acme".to_owned()),
                        count: 1,
                    }],
                }],
            }),
        ),
        searchkernel_ir::OperationIr::Analyze(request) => ExpectedCall::success(
            searchkernel_ir::OperationIr::Analyze(request),
            ScriptedResult::Analyze(searchkernel_ir::AnalyzeResult {
                tokens: vec![searchkernel_ir::AnalyzedToken {
                    surface: "Portable".to_owned(),
                    term: "portable".to_owned(),
                    position: 0,
                    start_byte: 0,
                    end_byte: 8,
                }],
            }),
        ),
    };
    Arc::new(ScriptedBackend::new([call]))
}

#[cfg(feature = "c7-test-support")]
#[allow(dead_code)]
fn c7_context() -> Result<Arc<IndexContext>, String> {
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let analyzer_name = builder.create_string("standard");
    let analyzer = api::Analyzer::create(
        &mut builder,
        &api::AnalyzerArgs {
            name: Some(analyzer_name),
            version: 1,
            options: None,
        },
    );
    let definitions = [
        (
            "title",
            api::FieldKind::Text,
            api::FieldCardinality::Single,
            true,
            true,
            false,
            false,
            false,
            true,
        ),
        (
            "description",
            api::FieldKind::Text,
            api::FieldCardinality::Single,
            true,
            true,
            false,
            false,
            false,
            true,
        ),
        (
            "brand",
            api::FieldKind::Keyword,
            api::FieldCardinality::Single,
            true,
            false,
            true,
            true,
            true,
            false,
        ),
        (
            "category",
            api::FieldKind::Keyword,
            api::FieldCardinality::Single,
            true,
            false,
            true,
            true,
            true,
            false,
        ),
        (
            "price",
            api::FieldKind::Float64,
            api::FieldCardinality::Single,
            true,
            false,
            true,
            true,
            true,
            false,
        ),
        (
            "in_stock",
            api::FieldKind::Bool,
            api::FieldCardinality::Single,
            true,
            false,
            true,
            false,
            false,
            false,
        ),
        (
            "popularity",
            api::FieldKind::Int64,
            api::FieldCardinality::Single,
            true,
            false,
            true,
            true,
            false,
            false,
        ),
        (
            "tags",
            api::FieldKind::Keyword,
            api::FieldCardinality::Multi,
            true,
            false,
            true,
            false,
            true,
            false,
        ),
        (
            "sku",
            api::FieldKind::Keyword,
            api::FieldCardinality::Single,
            true,
            false,
            true,
            true,
            false,
            false,
        ),
        (
            "image_url",
            api::FieldKind::Keyword,
            api::FieldCardinality::Single,
            true,
            false,
            false,
            false,
            false,
            false,
        ),
    ];
    let fields = definitions
        .iter()
        .map(
            |(
                name,
                kind,
                cardinality,
                stored_value,
                searchable_value,
                filterable_value,
                sortable_value,
                facetable_value,
                text,
            )| {
                let name = builder.create_string(name);
                let stored = api::OptionalBool::new(*stored_value);
                let searchable = api::OptionalBool::new(*searchable_value);
                let filterable = api::OptionalBool::new(*filterable_value);
                let sortable = api::OptionalBool::new(*sortable_value);
                let facetable = api::OptionalBool::new(*facetable_value);
                api::FieldDefinition::create(
                    &mut builder,
                    &api::FieldDefinitionArgs {
                        name: Some(name),
                        kind: *kind,
                        cardinality: *cardinality,
                        stored: Some(&stored),
                        searchable: Some(&searchable),
                        filterable: Some(&filterable),
                        sortable: Some(&sortable),
                        facetable: Some(&facetable),
                        analyzer: if *text { Some(analyzer) } else { None },
                    },
                )
            },
        )
        .collect::<Vec<_>>();
    let fields = builder.create_vector(&fields);
    let title = builder.create_string("title");
    let description = builder.create_string("description");
    let title_weight = api::OptionalDouble::new(4.0);
    let description_weight = api::OptionalDouble::new(1.0);
    let title = api::SearchField::create(
        &mut builder,
        &api::SearchFieldArgs {
            field: Some(title),
            weight: Some(&title_weight),
        },
    );
    let description = api::SearchField::create(
        &mut builder,
        &api::SearchFieldArgs {
            field: Some(description),
            weight: Some(&description_weight),
        },
    );
    let default_search_fields = builder.create_vector(&[title, description]);
    let default_search_fields = api::SearchFieldSelection::create(
        &mut builder,
        &api::SearchFieldSelectionArgs {
            fields: Some(default_search_fields),
        },
    );
    let index_name = builder.create_string("products");
    let definition = api::IndexDefinition::create(
        &mut builder,
        &api::IndexDefinitionArgs {
            name: Some(index_name),
            fields: Some(fields),
            default_search_fields: Some(default_search_fields),
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
