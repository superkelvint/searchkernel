//! The two small FlatBuffers tables that frame the portable Search Kernel API.

use flatbuffers::{root, ForwardsUOffset, Table, Verifiable, Verifier, WIPOffset};
use searchkernel_core::api;

pub(crate) const PROTOCOL_VERSION: u32 = 1;

pub(crate) const REQUEST_INSERT: u8 = 1;
pub(crate) const REQUEST_UPSERT: u8 = 2;
pub(crate) const REQUEST_PATCH: u8 = 3;
pub(crate) const REQUEST_DELETE: u8 = 4;
pub(crate) const REQUEST_GET: u8 = 5;
pub(crate) const REQUEST_SEARCH: u8 = 6;
pub(crate) const REQUEST_ANALYZE: u8 = 7;

pub(crate) const RESPONSE_INSERT: u8 = 1;
pub(crate) const RESPONSE_UPSERT: u8 = 2;
pub(crate) const RESPONSE_PATCH: u8 = 3;
pub(crate) const RESPONSE_DELETE: u8 = 4;
pub(crate) const RESPONSE_GET: u8 = 5;
pub(crate) const RESPONSE_SEARCH: u8 = 6;
pub(crate) const RESPONSE_ANALYZE: u8 = 7;
pub(crate) const RESPONSE_ERROR: u8 = 8;

#[derive(Clone, Copy)]
pub(crate) struct RequestEnvelope<'a> {
    tab: Table<'a>,
}

impl<'a> flatbuffers::Follow<'a> for RequestEnvelope<'a> {
    type Inner = Self;

    unsafe fn follow(buf: &'a [u8], loc: usize) -> Self::Inner {
        Self {
            // SAFETY: called only by FlatBuffers after its verifier has
            // validated the table location.
            tab: unsafe { Table::new(buf, loc) },
        }
    }
}

impl<'a> RequestEnvelope<'a> {
    const VT_PROTOCOL_VERSION: flatbuffers::VOffsetT = 4;
    const VT_PAYLOAD_TYPE: flatbuffers::VOffsetT = 6;
    const VT_PAYLOAD: flatbuffers::VOffsetT = 8;

    pub(crate) fn protocol_version(&self) -> u32 {
        // SAFETY: access follows successful envelope verification.
        unsafe {
            self.tab
                .get::<u32>(Self::VT_PROTOCOL_VERSION, Some(PROTOCOL_VERSION))
                .unwrap()
        }
    }

    pub(crate) fn payload_type(&self) -> u8 {
        // SAFETY: access follows successful envelope verification.
        unsafe { self.tab.get::<u8>(Self::VT_PAYLOAD_TYPE, Some(0)).unwrap() }
    }

    fn payload(&self) -> Option<Table<'a>> {
        // SAFETY: access follows successful envelope verification.
        unsafe {
            self.tab
                .get::<ForwardsUOffset<Table<'a>>>(Self::VT_PAYLOAD, None)
        }
    }

    pub(crate) fn insert(&self) -> Option<api::InsertRequest<'a>> {
        self.payload().map(|table| {
            // SAFETY: the envelope verifier validates this table as an
            // InsertRequest when its discriminator is InsertRequest.
            unsafe { api::InsertRequest::init_from_table(table) }
        })
    }

    pub(crate) fn upsert(&self) -> Option<api::UpsertRequest<'a>> {
        self.payload().map(|table| {
            // SAFETY: see `insert`.
            unsafe { api::UpsertRequest::init_from_table(table) }
        })
    }

    pub(crate) fn patch(&self) -> Option<api::PatchRequest<'a>> {
        self.payload().map(|table| {
            // SAFETY: see `insert`.
            unsafe { api::PatchRequest::init_from_table(table) }
        })
    }

    pub(crate) fn delete(&self) -> Option<api::DeleteRequest<'a>> {
        self.payload().map(|table| {
            // SAFETY: see `insert`.
            unsafe { api::DeleteRequest::init_from_table(table) }
        })
    }

    pub(crate) fn get(&self) -> Option<api::GetRequest<'a>> {
        self.payload().map(|table| {
            // SAFETY: see `insert`.
            unsafe { api::GetRequest::init_from_table(table) }
        })
    }

    pub(crate) fn search(&self) -> Option<api::SearchRequest<'a>> {
        self.payload().map(|table| {
            // SAFETY: see `insert`.
            unsafe { api::SearchRequest::init_from_table(table) }
        })
    }

    pub(crate) fn analyze(&self) -> Option<api::AnalyzeRequest<'a>> {
        self.payload().map(|table| {
            // SAFETY: see `insert`.
            unsafe { api::AnalyzeRequest::init_from_table(table) }
        })
    }
}

impl Verifiable for RequestEnvelope<'_> {
    fn run_verifier(
        verifier: &mut Verifier,
        pos: usize,
    ) -> Result<(), flatbuffers::InvalidFlatbuffer> {
        verifier
            .visit_table(pos)?
            .visit_field::<u32>("protocol_version", Self::VT_PROTOCOL_VERSION, false)?
            .visit_union::<u8, _>(
                "payload_type",
                Self::VT_PAYLOAD_TYPE,
                "payload",
                Self::VT_PAYLOAD,
                false,
                |kind, verifier, payload_pos| match kind {
                    REQUEST_INSERT => verifier
                        .verify_union_variant::<ForwardsUOffset<api::InsertRequest>>(
                            "InsertRequest",
                            payload_pos,
                        ),
                    REQUEST_UPSERT => verifier
                        .verify_union_variant::<ForwardsUOffset<api::UpsertRequest>>(
                            "UpsertRequest",
                            payload_pos,
                        ),
                    REQUEST_PATCH => verifier
                        .verify_union_variant::<ForwardsUOffset<api::PatchRequest>>(
                            "PatchRequest",
                            payload_pos,
                        ),
                    REQUEST_DELETE => verifier
                        .verify_union_variant::<ForwardsUOffset<api::DeleteRequest>>(
                            "DeleteRequest",
                            payload_pos,
                        ),
                    REQUEST_GET => verifier
                        .verify_union_variant::<ForwardsUOffset<api::GetRequest>>(
                            "GetRequest",
                            payload_pos,
                        ),
                    REQUEST_SEARCH => verifier
                        .verify_union_variant::<ForwardsUOffset<api::SearchRequest>>(
                            "SearchRequest",
                            payload_pos,
                        ),
                    REQUEST_ANALYZE => verifier
                        .verify_union_variant::<ForwardsUOffset<api::AnalyzeRequest>>(
                            "AnalyzeRequest",
                            payload_pos,
                        ),
                    _ => Ok(()),
                },
            )?
            .finish();
        Ok(())
    }
}

#[derive(Clone, Copy)]
#[cfg(feature = "ffi-test-support")]
pub(crate) struct ResponseEnvelope<'a> {
    tab: Table<'a>,
}

#[cfg(feature = "ffi-test-support")]
impl<'a> flatbuffers::Follow<'a> for ResponseEnvelope<'a> {
    type Inner = Self;

    unsafe fn follow(buf: &'a [u8], loc: usize) -> Self::Inner {
        Self {
            // SAFETY: called only by FlatBuffers after its verifier has
            // validated the table location.
            tab: unsafe { Table::new(buf, loc) },
        }
    }
}

#[cfg(feature = "ffi-test-support")]
impl<'a> ResponseEnvelope<'a> {
    const VT_PROTOCOL_VERSION: flatbuffers::VOffsetT = 4;
    const VT_PAYLOAD_TYPE: flatbuffers::VOffsetT = 6;
    const VT_PAYLOAD: flatbuffers::VOffsetT = 8;

    pub(crate) fn protocol_version(&self) -> u32 {
        // SAFETY: access follows successful envelope verification.
        unsafe {
            self.tab
                .get::<u32>(Self::VT_PROTOCOL_VERSION, Some(PROTOCOL_VERSION))
                .unwrap()
        }
    }

    pub(crate) fn payload_type(&self) -> u8 {
        // SAFETY: access follows successful envelope verification.
        unsafe { self.tab.get::<u8>(Self::VT_PAYLOAD_TYPE, Some(0)).unwrap() }
    }

    fn payload(&self) -> Option<Table<'a>> {
        // SAFETY: access follows successful envelope verification.
        unsafe {
            self.tab
                .get::<ForwardsUOffset<Table<'a>>>(Self::VT_PAYLOAD, None)
        }
    }

    fn payload_as<T>(payload: Option<Table<'a>>, init: unsafe fn(Table<'a>) -> T) -> Option<T> {
        payload.map(|table| {
            // SAFETY: the response envelope verifier selected and validated
            // the concrete table type before this accessor is called.
            unsafe { init(table) }
        })
    }

    pub(crate) fn insert(&self) -> Option<api::InsertResult<'a>> {
        Self::payload_as(self.payload(), api::InsertResult::init_from_table)
    }

    pub(crate) fn upsert(&self) -> Option<api::UpsertResult<'a>> {
        Self::payload_as(self.payload(), api::UpsertResult::init_from_table)
    }

    pub(crate) fn patch(&self) -> Option<api::PatchResult<'a>> {
        Self::payload_as(self.payload(), api::PatchResult::init_from_table)
    }

    pub(crate) fn delete(&self) -> Option<api::DeleteResult<'a>> {
        Self::payload_as(self.payload(), api::DeleteResult::init_from_table)
    }

    pub(crate) fn get(&self) -> Option<api::GetResult<'a>> {
        Self::payload_as(self.payload(), api::GetResult::init_from_table)
    }

    pub(crate) fn search(&self) -> Option<api::SearchResult<'a>> {
        Self::payload_as(self.payload(), api::SearchResult::init_from_table)
    }

    pub(crate) fn analyze(&self) -> Option<api::AnalyzeResult<'a>> {
        Self::payload_as(self.payload(), api::AnalyzeResult::init_from_table)
    }

    pub(crate) fn error(&self) -> Option<api::Error<'a>> {
        Self::payload_as(self.payload(), api::Error::init_from_table)
    }
}

#[cfg(feature = "ffi-test-support")]
impl Verifiable for ResponseEnvelope<'_> {
    fn run_verifier(
        verifier: &mut Verifier,
        pos: usize,
    ) -> Result<(), flatbuffers::InvalidFlatbuffer> {
        verifier
            .visit_table(pos)?
            .visit_field::<u32>("protocol_version", Self::VT_PROTOCOL_VERSION, false)?
            .visit_union::<u8, _>(
                "payload_type",
                Self::VT_PAYLOAD_TYPE,
                "payload",
                Self::VT_PAYLOAD,
                false,
                |kind, verifier, payload_pos| match kind {
                    RESPONSE_INSERT => verifier
                        .verify_union_variant::<ForwardsUOffset<api::InsertResult>>(
                            "InsertResult",
                            payload_pos,
                        ),
                    RESPONSE_UPSERT => verifier
                        .verify_union_variant::<ForwardsUOffset<api::UpsertResult>>(
                            "UpsertResult",
                            payload_pos,
                        ),
                    RESPONSE_PATCH => verifier
                        .verify_union_variant::<ForwardsUOffset<api::PatchResult>>(
                            "PatchResult",
                            payload_pos,
                        ),
                    RESPONSE_DELETE => verifier
                        .verify_union_variant::<ForwardsUOffset<api::DeleteResult>>(
                            "DeleteResult",
                            payload_pos,
                        ),
                    RESPONSE_GET => verifier
                        .verify_union_variant::<ForwardsUOffset<api::GetResult>>(
                            "GetResult",
                            payload_pos,
                        ),
                    RESPONSE_SEARCH => verifier
                        .verify_union_variant::<ForwardsUOffset<api::SearchResult>>(
                            "SearchResult",
                            payload_pos,
                        ),
                    RESPONSE_ANALYZE => verifier
                        .verify_union_variant::<ForwardsUOffset<api::AnalyzeResult>>(
                            "AnalyzeResult",
                            payload_pos,
                        ),
                    RESPONSE_ERROR => verifier
                        .verify_union_variant::<ForwardsUOffset<api::Error>>("Error", payload_pos),
                    _ => Ok(()),
                },
            )?
            .finish();
        Ok(())
    }
}

pub(crate) fn parse_request(bytes: &[u8]) -> Result<RequestEnvelope<'_>, ()> {
    if !has_identifier(bytes, b"SKRQ") {
        return Err(());
    }
    root::<RequestEnvelope<'_>>(bytes).map_err(|_| ())
}

#[cfg(feature = "ffi-test-support")]
pub(crate) fn parse_response(bytes: &[u8]) -> Result<ResponseEnvelope<'_>, ()> {
    if !has_identifier(bytes, b"SKRS") {
        return Err(());
    }
    root::<ResponseEnvelope<'_>>(bytes).map_err(|_| ())
}

fn has_identifier(bytes: &[u8], identifier: &[u8; 4]) -> bool {
    bytes.len() >= 8 && bytes[4..8] == *identifier
}

/// Put a complete standalone FlatBuffer table inside an envelope.
///
/// `create_byte_string` copies the table bytes into the builder. The payload
/// offset is then aimed at the inner root table rather than at the byte-vector
/// prefix, so all inner relative offsets remain valid after the copy. The
/// leading four-byte pad gives the copied table the eight-byte alignment that
/// some portable tables require (for example, tables containing `u64` values).
#[cfg(feature = "ffi-test-support")]
pub(crate) fn frame_request(kind: u8, protocol_version: u32, inner: &[u8]) -> Vec<u8> {
    frame(
        kind,
        protocol_version,
        trim_request_inner(kind, inner),
        b"SKRQ",
    )
}

pub(crate) fn frame_response(kind: u8, inner: &[u8]) -> Vec<u8> {
    frame(
        kind,
        PROTOCOL_VERSION,
        trim_response_inner(kind, inner),
        b"SKRS",
    )
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
        _ => inner,
    }
}

fn trim_response_inner(kind: u8, inner: &[u8]) -> &[u8] {
    match kind {
        RESPONSE_INSERT => trim_inner(inner, |bytes| root::<api::InsertResult<'_>>(bytes).is_ok()),
        RESPONSE_UPSERT => trim_inner(inner, |bytes| root::<api::UpsertResult<'_>>(bytes).is_ok()),
        RESPONSE_PATCH => trim_inner(inner, |bytes| root::<api::PatchResult<'_>>(bytes).is_ok()),
        RESPONSE_DELETE => trim_inner(inner, |bytes| root::<api::DeleteResult<'_>>(bytes).is_ok()),
        RESPONSE_GET => trim_inner(inner, |bytes| root::<api::GetResult<'_>>(bytes).is_ok()),
        RESPONSE_SEARCH => trim_inner(inner, |bytes| root::<api::SearchResult<'_>>(bytes).is_ok()),
        RESPONSE_ANALYZE => {
            trim_inner(inner, |bytes| root::<api::AnalyzeResult<'_>>(bytes).is_ok())
        }
        RESPONSE_ERROR => trim_inner(inner, |bytes| root::<api::Error<'_>>(bytes).is_ok()),
        _ => inner,
    }
}

fn trim_inner(inner: &[u8], verifies: impl Fn(&[u8]) -> bool) -> &[u8] {
    let mut end = inner.len();
    while end > 4 && verifies(&inner[..end - 1]) {
        end -= 1;
    }
    &inner[..end]
}

fn frame(kind: u8, protocol_version: u32, inner: &[u8], identifier: &[u8; 4]) -> Vec<u8> {
    assert!(
        inner.len() >= 4,
        "inner FlatBuffer is missing its root offset"
    );
    let root_offset = u32::from_le_bytes(inner[..4].try_into().unwrap());
    let root_offset = usize::try_from(root_offset).unwrap();
    assert!(
        root_offset < inner.len(),
        "inner FlatBuffer root is out of bounds"
    );

    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let mut copied_inner = Vec::with_capacity(inner.len() + 4);
    copied_inner.resize(4, 0);
    copied_inner.extend_from_slice(inner);
    let inner_vector = builder.create_byte_string(&copied_inner);
    let payload_value = inner_vector
        .value()
        .checked_sub(4)
        .and_then(|value| value.checked_sub(4))
        .and_then(|value| value.checked_sub(root_offset as u32))
        .expect("inner root offset does not fit in builder offset");
    let payload = WIPOffset::<flatbuffers::UnionWIPOffset>::new(payload_value);

    let table = builder.start_table();
    builder.push_slot(
        ResponseOrRequestTable::PROTOCOL_VERSION,
        protocol_version,
        PROTOCOL_VERSION,
    );
    builder.push_slot(ResponseOrRequestTable::PAYLOAD_TYPE, kind, 0u8);
    builder.push_slot_always(ResponseOrRequestTable::PAYLOAD, payload);
    let root = builder.end_table(table);
    builder.finish(root, Some(std::str::from_utf8(identifier).unwrap()));
    let finished = builder.finished_data();
    // `create_byte_string` adds a terminator for string semantics. The
    // copied bytes are addressed as a table, not as a string, so that final
    // byte is not part of the transport and would otherwise make the final
    // one-byte truncation indistinguishable from a complete buffer.
    finished[..finished.len() - 1].to_vec()
}

struct ResponseOrRequestTable;

impl ResponseOrRequestTable {
    const PROTOCOL_VERSION: flatbuffers::VOffsetT = 4;
    const PAYLOAD_TYPE: flatbuffers::VOffsetT = 6;
    const PAYLOAD: flatbuffers::VOffsetT = 8;
}

#[cfg(all(test, feature = "ffi-test-support"))]
mod tests {
    use super::*;

    #[test]
    fn copied_inner_tables_remain_valid_inside_both_envelopes() {
        let mut builder = flatbuffers::FlatBufferBuilder::new();
        let inner =
            api::InsertRequest::create(&mut builder, &api::InsertRequestArgs { document: None });
        builder.finish(inner, None);
        let inner_bytes = builder.finished_data().to_vec();

        let request_bytes = frame_request(REQUEST_INSERT, PROTOCOL_VERSION, &inner_bytes);
        let request = parse_request(&request_bytes).expect("request envelope");
        assert_eq!(request.payload_type(), REQUEST_INSERT);
        assert!(request.insert().is_some());

        let result_bytes =
            searchkernel_core::insert_result_to_api(&searchkernel_core::InsertResult {
                id: "doc-1".to_owned(),
            });
        let response_bytes = frame_response(RESPONSE_INSERT, &result_bytes);
        let response = parse_response(&response_bytes).expect("response envelope");
        assert_eq!(response.payload_type(), RESPONSE_INSERT);
        assert_eq!(
            response.insert().and_then(|result| result.id()),
            Some("doc-1")
        );

        let mut fields = std::collections::BTreeMap::new();
        fields.insert(
            "title".to_owned(),
            searchkernel_core::Value::String("Wireless Headphones".to_owned()),
        );
        let result = searchkernel_core::search_result_to_api(&searchkernel_core::SearchResult {
            hits: vec![searchkernel_core::Hit {
                id: "doc-1".to_owned(),
                score: Some(1.25),
                fields,
                highlights: vec![searchkernel_core::HighlightResult {
                    field: "title".to_owned(),
                    fragments: vec![searchkernel_core::HighlightFragment {
                        text: "Wireless Headphones".to_owned(),
                        matches: vec![searchkernel_core::HighlightSpan {
                            start_byte: 9,
                            end_byte: 19,
                        }],
                    }],
                }],
            }],
            total_hits: searchkernel_core::TotalHits {
                value: 1,
                relation: searchkernel_core::TotalHitsRelation::Exact,
            },
            facets: vec![searchkernel_core::FacetResult {
                field: "brand".to_owned(),
                values: vec![searchkernel_core::FacetValue {
                    value: searchkernel_core::Value::String("Sony".to_owned()),
                    count: 1,
                }],
            }],
        });
        assert!(flatbuffers::root::<api::SearchResult>(&result).is_ok());
        let response_bytes = frame_response(RESPONSE_SEARCH, &result);
        let response = flatbuffers::root::<ResponseEnvelope>(&response_bytes)
            .unwrap_or_else(|error| panic!("search response envelope: {error:?}"));
        assert!(response.search().is_some());

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
        assert!(flatbuffers::root::<RequestEnvelope>(&framed).is_ok());
        for end in 0..framed.len() {
            assert!(
                flatbuffers::root::<RequestEnvelope>(&framed[..end]).is_err(),
                "valid prefix length {end}"
            );
        }
    }
}
