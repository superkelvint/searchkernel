#![allow(clippy::too_many_lines)]

#[cfg(test)]
mod tests {
    use flatbuffers::{root, FlatBufferBuilder, WIPOffset};
    use searchkernel_ir::{api, Backend, IndexContext, OperationIr, PortableError};
    use searchkernel_ffi::test_support::{
        decode_response, live_buffer_count, live_handle_count, register_index, wrap_request,
        wrap_request_with_protocol, wrap_unknown_operation, DecodedResponse, RequestKind,
    };
    use searchkernel_testkit::{ExpectedCall, RecordingBackend, ScriptedBackend, ScriptedResult};
    use std::collections::BTreeMap;
    use std::ptr;
    use std::sync::{Arc, Barrier, Mutex};
    use std::thread;

    const SK_STATUS_OK: u32 = 0;
    const SK_STATUS_INVALID_ARGUMENT: u32 = 1;
    const SK_STATUS_INVALID_HANDLE: u32 = 2;
    const SK_STATUS_INVALID_REQUEST: u32 = 3;
    const SK_STATUS_UNSUPPORTED_PROTOCOL: u32 = 4;
    const SK_STATUS_UNSUPPORTED_OPERATION: u32 = 5;
    const SK_STATUS_PANIC: u32 = 7;

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    extern "C" {
        fn sk_abi_version() -> u32;
        fn sk_call(
            handle: u64,
            request: *const u8,
            request_len: usize,
            out_data: *mut *mut u8,
            out_len: *mut usize,
            out_allocation_id: *mut u64,
        ) -> u32;
        fn sk_close(handle: u64, error: *mut u8, error_capacity: usize) -> u32;
        fn sk_buffer_free(data: *mut u8, allocation_id: u64);
    }

    type RawResponse = (*mut u8, usize, u64);

    fn raw_call(handle: u64, request: &[u8]) -> (u32, RawResponse) {
        let mut data = ptr::null_mut();
        let mut len = 0;
        let mut allocation_id = 0;
        let request_ptr = if request.is_empty() {
            ptr::null()
        } else {
            request.as_ptr()
        };
        let status = unsafe {
            sk_call(
                handle,
                request_ptr,
                request.len(),
                &mut data,
                &mut len,
                &mut allocation_id,
            )
        };
        (status, (data, len, allocation_id))
    }

    fn take_response((data, len, allocation_id): RawResponse) -> Vec<u8> {
        let bytes = if data.is_null() || len == 0 {
            Vec::new()
        } else {
            unsafe { std::slice::from_raw_parts(data, len) }.to_vec()
        };
        unsafe { sk_buffer_free(data, allocation_id) };
        bytes
    }

    fn call_ok(handle: u64, request: &[u8]) -> DecodedResponse {
        let (status, out) = raw_call(handle, request);
        assert_eq!(status, SK_STATUS_OK);
        assert!(!out.0.is_null());
        assert!(out.1 > 0);
        let bytes = take_response(out);
        decode_response(&bytes).expect("valid response envelope")
    }

    fn analyzer_offset<'a>(
        builder: &mut FlatBufferBuilder<'a>,
    ) -> WIPOffset<api::Analyzer<'a>> {
        let name = builder.create_string("standard");
        api::Analyzer::create(
            builder,
            &api::AnalyzerArgs {
                name: Some(name),
                version: 1,
                options: None,
            },
        )
    }

    fn field_offset<'a>(
        builder: &mut FlatBufferBuilder<'a>,
        name: &str,
        kind: api::FieldKind,
        searchable: bool,
        filterable: bool,
        sortable: bool,
        facetable: bool,
        analyzer: bool,
    ) -> WIPOffset<api::FieldDefinition<'a>> {
        let name = builder.create_string(name);
        let stored = api::OptionalBool::new(true);
        let searchable_value = api::OptionalBool::new(searchable);
        let filterable_value = api::OptionalBool::new(filterable);
        let sortable_value = api::OptionalBool::new(sortable);
        let facetable_value = api::OptionalBool::new(facetable);
        let analyzer = analyzer.then(|| analyzer_offset(builder));
        api::FieldDefinition::create(
            builder,
            &api::FieldDefinitionArgs {
                name: Some(name),
                kind,
                cardinality: api::FieldCardinality::Single,
                stored: Some(&stored),
                searchable: Some(&searchable_value),
                filterable: Some(&filterable_value),
                sortable: Some(&sortable_value),
                facetable: Some(&facetable_value),
                analyzer,
            },
        )
    }

    fn definition_bytes() -> Vec<u8> {
        let mut builder = FlatBufferBuilder::new();
        let title = field_offset(
            &mut builder,
            "title",
            api::FieldKind::Text,
            true,
            false,
            false,
            false,
            true,
        );
        let brand = field_offset(
            &mut builder,
            "brand",
            api::FieldKind::Keyword,
            false,
            true,
            true,
            true,
            false,
        );
        let price = field_offset(
            &mut builder,
            "price",
            api::FieldKind::Float64,
            false,
            true,
            true,
            false,
            false,
        );
        let fields = builder.create_vector(&[title, brand, price]);

        let title_name = builder.create_string("title");
        let weight = api::OptionalDouble::new(1.0);
        let search_field = api::SearchField::create(
            &mut builder,
            &api::SearchFieldArgs {
                field: Some(title_name),
                weight: Some(&weight),
            },
        );
        let search_fields = builder.create_vector(&[search_field]);
        let selection = api::SearchFieldSelection::create(
            &mut builder,
            &api::SearchFieldSelectionArgs {
                fields: Some(search_fields),
            },
        );
        let name = builder.create_string("products");
        let definition = api::IndexDefinition::create(
            &mut builder,
            &api::IndexDefinitionArgs {
                name: Some(name),
                fields: Some(fields),
                default_search_fields: Some(selection),
            },
        );
        builder.finish(definition, None);
        builder.finished_data().to_vec()
    }

    fn context() -> Arc<IndexContext> {
        let bytes = definition_bytes();
        let definition = root::<api::IndexDefinition>(&bytes).expect("definition parses");
        Arc::new(IndexContext::from_api(definition).expect("definition is valid"))
    }

    fn string_value<'a>(
        builder: &mut FlatBufferBuilder<'a>,
        value: &str,
    ) -> WIPOffset<api::Value<'a>> {
        let value = builder.create_string(value);
        let string = api::StringValue::create(
            builder,
            &api::StringValueArgs { value: Some(value) },
        );
        api::Value::create(
            builder,
            &api::ValueArgs {
                kind_type: api::ValueKind::StringValue,
                kind: Some(string.as_union_value()),
            },
        )
    }

    fn float_value<'a>(
        builder: &mut FlatBufferBuilder<'a>,
        value: f64,
    ) -> WIPOffset<api::Value<'a>> {
        let value = api::Float64Value::create(builder, &api::Float64ValueArgs { value });
        api::Value::create(
            builder,
            &api::ValueArgs {
                kind_type: api::ValueKind::Float64Value,
                kind: Some(value.as_union_value()),
            },
        )
    }

    fn document_offset<'a>(
        builder: &mut FlatBufferBuilder<'a>,
    ) -> WIPOffset<api::Document<'a>> {
        let id = builder.create_string("doc-1");
        let title_key = builder.create_string("title");
        let title_value = string_value(builder, "Wireless Headphones");
        let title = api::ValueEntry::create(
            builder,
            &api::ValueEntryArgs {
                key: Some(title_key),
                value: Some(title_value),
            },
        );
        let brand_key = builder.create_string("brand");
        let brand_value = string_value(builder, "Sony");
        let brand = api::ValueEntry::create(
            builder,
            &api::ValueEntryArgs {
                key: Some(brand_key),
                value: Some(brand_value),
            },
        );
        let price_key = builder.create_string("price");
        let price_value = float_value(builder, 199.0);
        let price = api::ValueEntry::create(
            builder,
            &api::ValueEntryArgs {
                key: Some(price_key),
                value: Some(price_value),
            },
        );
        let fields = builder.create_vector(&[title, brand, price]);
        api::Document::create(
            builder,
            &api::DocumentArgs {
                id: Some(id),
                fields: Some(fields),
            },
        )
    }

    fn insert_bytes() -> Vec<u8> {
        let mut builder = FlatBufferBuilder::new();
        let document = document_offset(&mut builder);
        let request = api::InsertRequest::create(
            &mut builder,
            &api::InsertRequestArgs {
                document: Some(document),
            },
        );
        builder.finish(request, None);
        builder.finished_data().to_vec()
    }

    fn upsert_bytes() -> Vec<u8> {
        let mut builder = FlatBufferBuilder::new();
        let document = document_offset(&mut builder);
        let request = api::UpsertRequest::create(
            &mut builder,
            &api::UpsertRequestArgs {
                document: Some(document),
            },
        );
        builder.finish(request, None);
        builder.finished_data().to_vec()
    }

    fn patch_bytes() -> Vec<u8> {
        let mut builder = FlatBufferBuilder::new();
        let id = builder.create_string("doc-1");
        let field = builder.create_string("price");
        let value = float_value(&mut builder, 149.0);
        let set = api::SetField::create(
            &mut builder,
            &api::SetFieldArgs {
                field: Some(field),
                value: Some(value),
            },
        );
        let operation = api::PatchOperation::create(
            &mut builder,
            &api::PatchOperationArgs {
                operation_type: api::PatchOperationKind::SetField,
                operation: Some(set.as_union_value()),
            },
        );
        let operations = builder.create_vector(&[operation]);
        let request = api::PatchRequest::create(
            &mut builder,
            &api::PatchRequestArgs {
                id: Some(id),
                operations: Some(operations),
            },
        );
        builder.finish(request, None);
        builder.finished_data().to_vec()
    }

    fn delete_bytes() -> Vec<u8> {
        let mut builder = FlatBufferBuilder::new();
        let id = builder.create_string("doc-1");
        let request = api::DeleteRequest::create(
            &mut builder,
            &api::DeleteRequestArgs { id: Some(id) },
        );
        builder.finish(request, None);
        builder.finished_data().to_vec()
    }

    fn get_bytes() -> Vec<u8> {
        let mut builder = FlatBufferBuilder::new();
        let id = builder.create_string("doc-1");
        let request = api::GetRequest::create(
            &mut builder,
            &api::GetRequestArgs {
                id: Some(id),
                projection: None,
            },
        );
        builder.finish(request, None);
        builder.finished_data().to_vec()
    }

    fn search_bytes() -> Vec<u8> {
        let mut builder = FlatBufferBuilder::new();
        let query = builder.create_string("headphones");
        let brand = builder.create_string("brand");
        let sony = string_value(&mut builder, "Sony");
        let comparison = api::FilterComparison::create(
            &mut builder,
            &api::FilterComparisonArgs {
                field: Some(brand),
                op: api::ComparisonOp::Eq,
                value: Some(sony),
            },
        );
        let filter = api::FilterExpr::create(
            &mut builder,
            &api::FilterExprArgs {
                expr_type: api::FilterExprKind::FilterComparison,
                expr: Some(comparison.as_union_value()),
            },
        );
        let limit = api::OptionalUInt32::new(5);
        let request = api::SearchRequest::create(
            &mut builder,
            &api::SearchRequestArgs {
                query: Some(query),
                filter: Some(filter),
                limit: Some(&limit),
                ..Default::default()
            },
        );
        builder.finish(request, None);
        builder.finished_data().to_vec()
    }

    fn invalid_search_bytes() -> Vec<u8> {
        let mut builder = FlatBufferBuilder::new();
        let query = builder.create_string("headphones");
        let name = builder.create_string("not_a_field");
        let field = api::SearchField::create(
            &mut builder,
            &api::SearchFieldArgs {
                field: Some(name),
                weight: None,
            },
        );
        let fields = builder.create_vector(&[field]);
        let selection = api::SearchFieldSelection::create(
            &mut builder,
            &api::SearchFieldSelectionArgs {
                fields: Some(fields),
            },
        );
        let request = api::SearchRequest::create(
            &mut builder,
            &api::SearchRequestArgs {
                query: Some(query),
                search_fields: Some(selection),
                ..Default::default()
            },
        );
        builder.finish(request, None);
        builder.finished_data().to_vec()
    }

    fn analyze_bytes() -> Vec<u8> {
        let mut builder = FlatBufferBuilder::new();
        let field_name = builder.create_string("title");
        let target = api::AnalyzeFieldTarget::create(
            &mut builder,
            &api::AnalyzeFieldTargetArgs {
                field: Some(field_name),
            },
        );
        let text = builder.create_string("Running");
        let request = api::AnalyzeRequest::create(
            &mut builder,
            &api::AnalyzeRequestArgs {
                target_type: api::AnalyzeTarget::AnalyzeFieldTarget,
                target: Some(target.as_union_value()),
                text: Some(text),
                language: None,
            },
        );
        builder.finish(request, None);
        builder.finished_data().to_vec()
    }

    fn expected_operation(context: &IndexContext, kind: RequestKind, bytes: &[u8]) -> OperationIr {
        match kind {
            RequestKind::Insert => OperationIr::Insert(
                context
                    .insert_to_ir(root::<api::InsertRequest>(bytes).unwrap())
                    .unwrap(),
            ),
            RequestKind::Upsert => OperationIr::Upsert(
                context
                    .upsert_to_ir(root::<api::UpsertRequest>(bytes).unwrap())
                    .unwrap(),
            ),
            RequestKind::Patch => OperationIr::Patch(
                context
                    .patch_to_ir(root::<api::PatchRequest>(bytes).unwrap())
                    .unwrap(),
            ),
            RequestKind::Delete => OperationIr::Delete(
                context
                    .delete_to_ir(root::<api::DeleteRequest>(bytes).unwrap())
                    .unwrap(),
            ),
            RequestKind::Get => OperationIr::Get(
                context
                    .get_to_ir(root::<api::GetRequest>(bytes).unwrap())
                    .unwrap(),
            ),
            RequestKind::Search => OperationIr::Search(
                context
                    .search_to_ir(root::<api::SearchRequest>(bytes).unwrap())
                    .unwrap(),
            ),
            RequestKind::Analyze => OperationIr::Analyze(
                context
                    .analyze_to_ir(root::<api::AnalyzeRequest>(bytes).unwrap())
                    .unwrap(),
            ),
            RequestKind::Describe | RequestKind::InspectQuery => {
                panic!("extended workspace operations are outside C5")
            }
        }
    }

    fn document_result() -> searchkernel_ir::DocumentResult {
        let mut fields = BTreeMap::new();
        fields.insert(
            "title".to_owned(),
            searchkernel_ir::Value::String("Wireless Headphones".to_owned()),
        );
        searchkernel_ir::DocumentResult {
            id: "doc-1".to_owned(),
            fields,
        }
    }

    fn search_result() -> searchkernel_ir::SearchResult {
        let doc = document_result();
        searchkernel_ir::SearchResult {
            hits: vec![searchkernel_ir::Hit {
                id: doc.id.clone(),
                score: Some(1.25),
                fields: doc.fields,
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
        }
    }

    fn analyze_result() -> searchkernel_ir::AnalyzeResult {
        searchkernel_ir::AnalyzeResult {
            tokens: vec![searchkernel_ir::AnalyzedToken {
                surface: "Running".to_owned(),
                term: "running".to_owned(),
                position: 0,
                start_byte: 0,
                end_byte: 7,
            }],
        }
    }

    fn close_ok(handle: u64) {
        assert_eq!(unsafe { sk_close(handle, ptr::null_mut(), 0) }, SK_STATUS_OK);
    }

    #[test]
    fn abi_version_is_frozen() {
        let _guard = TEST_LOCK.lock().unwrap();
        assert_eq!(unsafe { sk_abi_version() }, 2);
    }

    #[test]
    fn every_operation_dispatches_exact_ir_and_round_trips_result() {
        let _guard = TEST_LOCK.lock().unwrap();
        assert_eq!(live_handle_count(), 0);
        assert_eq!(live_buffer_count(), 0);

        let cases: Vec<(RequestKind, Vec<u8>, ScriptedResult)> = vec![
            (
                RequestKind::Insert,
                insert_bytes(),
                ScriptedResult::Insert(searchkernel_ir::InsertResult {
                    id: "doc-1".to_owned(),
                }),
            ),
            (
                RequestKind::Upsert,
                upsert_bytes(),
                ScriptedResult::Upsert(searchkernel_ir::UpsertResult {
                    id: "doc-1".to_owned(),
                }),
            ),
            (
                RequestKind::Patch,
                patch_bytes(),
                ScriptedResult::Patch(searchkernel_ir::PatchResult {
                    id: "doc-1".to_owned(),
                }),
            ),
            (
                RequestKind::Delete,
                delete_bytes(),
                ScriptedResult::Delete(searchkernel_ir::DeleteResult {
                    id: "doc-1".to_owned(),
                    deleted: true,
                }),
            ),
            (
                RequestKind::Get,
                get_bytes(),
                ScriptedResult::Get(searchkernel_ir::GetResult {
                    document: Some(document_result()),
                }),
            ),
            (
                RequestKind::Search,
                search_bytes(),
                ScriptedResult::Search(search_result()),
            ),
            (
                RequestKind::Analyze,
                analyze_bytes(),
                ScriptedResult::Analyze(analyze_result()),
            ),
        ];

        for (kind, inner, scripted_result) in cases {
            let context = context();
            let expected = expected_operation(&context, kind, &inner);
            let backend = Arc::new(ScriptedBackend::new(vec![ExpectedCall::success(
                expected,
                scripted_result.clone(),
            )]));
            let handle = register_index(context, backend.clone());
            assert_ne!(handle, 0);
            let envelope = wrap_request(kind, &inner);
            let before = envelope.clone();
            let decoded = call_ok(handle, &envelope);
            assert_eq!(envelope, before, "sk_call modified request bytes");

            match (scripted_result, decoded) {
                (ScriptedResult::Insert(expected), DecodedResponse::Insert(actual)) => {
                    assert_eq!(actual, expected)
                }
                (ScriptedResult::Upsert(expected), DecodedResponse::Upsert(actual)) => {
                    assert_eq!(actual, expected)
                }
                (ScriptedResult::Patch(expected), DecodedResponse::Patch(actual)) => {
                    assert_eq!(actual, expected)
                }
                (ScriptedResult::Delete(expected), DecodedResponse::Delete(actual)) => {
                    assert_eq!(actual, expected)
                }
                (ScriptedResult::Get(expected), DecodedResponse::Get(actual)) => {
                    assert_eq!(actual, expected)
                }
                (ScriptedResult::Search(expected), DecodedResponse::Search(actual)) => {
                    assert_eq!(actual, expected)
                }
                (ScriptedResult::Analyze(expected), DecodedResponse::Analyze(actual)) => {
                    assert_eq!(actual, expected)
                }
                (expected, actual) => panic!("wrong response variant: {expected:?} -> {actual:?}"),
            }

            backend.finish().unwrap();
            close_ok(handle);
            assert_eq!(live_handle_count(), 0);
            assert_eq!(live_buffer_count(), 0);
        }
    }

    #[test]
    fn semantic_error_is_portable_response_and_never_reaches_backend() {
        let _guard = TEST_LOCK.lock().unwrap();
        let context = context();
        let inner = invalid_search_bytes();
        let expected_error = context
            .search_to_ir(root::<api::SearchRequest>(&inner).unwrap())
            .unwrap_err();
        let backend = Arc::new(RecordingBackend::new());
        let handle = register_index(context, backend.clone());
        let envelope = wrap_request(RequestKind::Search, &inner);
        let decoded = call_ok(handle, &envelope);
        match decoded {
            DecodedResponse::Error(actual) => assert_eq!(actual, expected_error),
            other => panic!("expected portable error, got {other:?}"),
        }
        assert!(backend.calls().is_empty());
        close_ok(handle);
    }

    #[test]
    fn backend_portable_error_is_status_ok_and_preserved_exactly() {
        let _guard = TEST_LOCK.lock().unwrap();
        let context = context();
        let inner = search_bytes();
        let expected_request = expected_operation(&context, RequestKind::Search, &inner);
        let mut metadata = BTreeMap::new();
        metadata.insert("source".to_owned(), "scripted".to_owned());
        let expected_error = PortableError {
            code: searchkernel_ir::ErrorCode::Unavailable,
            message: "backend temporarily unavailable".to_owned(),
            details: vec![searchkernel_ir::ErrorDetail {
                code: "backend_unavailable".to_owned(),
                path: Some("search".to_owned()),
                message: "backend temporarily unavailable".to_owned(),
                metadata,
            }],
            retryable: true,
        };
        let backend = Arc::new(ScriptedBackend::new(vec![ExpectedCall::error(
            expected_request,
            expected_error.clone(),
        )]));
        let handle = register_index(context, backend.clone());
        let envelope = wrap_request(RequestKind::Search, &inner);
        let decoded = call_ok(handle, &envelope);
        match decoded {
            DecodedResponse::Error(actual) => assert_eq!(actual, expected_error),
            other => panic!("expected backend portable error, got {other:?}"),
        }
        backend.finish().unwrap();
        close_ok(handle);
    }

    #[test]
    fn result_edge_variants_round_trip_without_reinterpretation() {
        let _guard = TEST_LOCK.lock().unwrap();
        let context = context();
        let delete_inner = delete_bytes();
        let get_inner = get_bytes();
        let search_inner = search_bytes();
        let script = vec![
            ExpectedCall::success(
                expected_operation(&context, RequestKind::Delete, &delete_inner),
                ScriptedResult::Delete(searchkernel_ir::DeleteResult {
                    id: "doc-1".to_owned(),
                    deleted: false,
                }),
            ),
            ExpectedCall::success(
                expected_operation(&context, RequestKind::Get, &get_inner),
                ScriptedResult::Get(searchkernel_ir::GetResult { document: None }),
            ),
            ExpectedCall::success(
                expected_operation(&context, RequestKind::Search, &search_inner),
                ScriptedResult::Search(searchkernel_ir::SearchResult {
                    hits: vec![],
                    total_hits: searchkernel_ir::TotalHits {
                        value: 100,
                        relation: searchkernel_ir::TotalHitsRelation::LowerBound,
                    },
                    facets: vec![searchkernel_ir::FacetResult {
                        field: "brand".to_owned(),
                        values: vec![searchkernel_ir::FacetValue {
                            value: searchkernel_ir::Value::String("Sony".to_owned()),
                            count: 7,
                        }],
                    }],
                }),
            ),
        ];
        let backend = Arc::new(ScriptedBackend::new(script));
        let handle = register_index(context, backend.clone());

        match call_ok(handle, &wrap_request(RequestKind::Delete, &delete_inner)) {
            DecodedResponse::Delete(actual) => assert!(!actual.deleted),
            other => panic!("wrong delete response: {other:?}"),
        }
        match call_ok(handle, &wrap_request(RequestKind::Get, &get_inner)) {
            DecodedResponse::Get(actual) => assert!(actual.document.is_none()),
            other => panic!("wrong get response: {other:?}"),
        }
        match call_ok(handle, &wrap_request(RequestKind::Search, &search_inner)) {
            DecodedResponse::Search(actual) => {
                assert_eq!(actual.total_hits.value, 100);
                assert_eq!(
                    actual.total_hits.relation,
                    searchkernel_ir::TotalHitsRelation::LowerBound
                );
                assert_eq!(actual.facets.len(), 1);
                assert_eq!(actual.facets[0].field, "brand");
                assert_eq!(actual.facets[0].values[0].count, 7);
            }
            other => panic!("wrong search response: {other:?}"),
        }

        backend.finish().unwrap();
        close_ok(handle);
    }

    #[test]
    fn malformed_and_truncated_envelopes_never_reach_backend() {
        let _guard = TEST_LOCK.lock().unwrap();
        let context = context();
        let backend = Arc::new(RecordingBackend::new());
        let handle = register_index(context, backend.clone());
        let valid = wrap_request(RequestKind::Search, &search_bytes());

        for end in 0..valid.len() {
            let (status, out) = raw_call(handle, &valid[..end]);
            assert_eq!(status, SK_STATUS_INVALID_REQUEST, "prefix length {end}");
            assert!(out.0.is_null());
            assert_eq!(out.1, 0);
        }

        for garbage in [vec![0u8], vec![0u8; 16], b"not-flatbuffers".to_vec()] {
            let (status, out) = raw_call(handle, &garbage);
            assert_eq!(status, SK_STATUS_INVALID_REQUEST);
            assert!(out.0.is_null());
            assert_eq!(out.1, 0);
        }

        assert!(backend.calls().is_empty());
        close_ok(handle);
    }

    #[test]
    fn protocol_and_operation_errors_are_abi_errors() {
        let _guard = TEST_LOCK.lock().unwrap();
        let context = context();
        let backend = Arc::new(RecordingBackend::new());
        let handle = register_index(context, backend.clone());
        let inner = search_bytes();

        let wrong_version = wrap_request_with_protocol(RequestKind::Search, &inner, 2);
        let (status, out) = raw_call(handle, &wrong_version);
        assert_eq!(status, SK_STATUS_UNSUPPORTED_PROTOCOL);
        assert!(out.0.is_null());
        assert_eq!(out.1, 0);

        let unknown = wrap_unknown_operation(250, &inner);
        let (status, out) = raw_call(handle, &unknown);
        assert_eq!(status, SK_STATUS_UNSUPPORTED_OPERATION);
        assert!(out.0.is_null());
        assert_eq!(out.1, 0);

        assert!(backend.calls().is_empty());
        close_ok(handle);
    }

    #[test]
    fn response_envelope_cannot_be_reused_as_request() {
        let _guard = TEST_LOCK.lock().unwrap();
        let index_context = context();
        let inner = search_bytes();
        let expected = expected_operation(&index_context, RequestKind::Search, &inner);
        let backend = Arc::new(ScriptedBackend::new(vec![ExpectedCall::success(
            expected,
            ScriptedResult::Search(search_result()),
        )]));
        let handle = register_index(index_context, backend.clone());
        let request = wrap_request(RequestKind::Search, &inner);
        let (status, out) = raw_call(handle, &request);
        assert_eq!(status, SK_STATUS_OK);
        let response_bytes = take_response(out);
        backend.finish().unwrap();
        close_ok(handle);

        let recorder = Arc::new(RecordingBackend::new());
        let other = register_index(context(), recorder.clone());
        let (status, out) = raw_call(other, &response_bytes);
        assert_eq!(status, SK_STATUS_INVALID_REQUEST);
        assert!(out.0.is_null());
        assert_eq!(out.1, 0);
        assert!(recorder.calls().is_empty());
        close_ok(other);
    }

    #[test]
    fn output_is_write_only_and_pointer_rules_are_frozen() {
        let _guard = TEST_LOCK.lock().unwrap();
        assert_eq!(
            unsafe {
                sk_call(
                    0,
                    ptr::null(),
                    0,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                )
            },
            SK_STATUS_INVALID_ARGUMENT
        );

        let mut partial_len = usize::MAX;
        let mut partial_allocation_id = u64::MAX;
        assert_eq!(
            unsafe {
                sk_call(
                    0,
                    ptr::null(),
                    0,
                    ptr::null_mut(),
                    &mut partial_len,
                    &mut partial_allocation_id,
                )
            },
            SK_STATUS_INVALID_ARGUMENT
        );
        assert_eq!(partial_len, 0);
        assert_eq!(partial_allocation_id, 0);

        let context = context();
        let backend = Arc::new(RecordingBackend::new());
        let handle = register_index(context, backend);

        let mut sentinel = 0x5au8;
        let mut data = &mut sentinel as *mut u8;
        let mut len = 1usize;
        let mut allocation_id = 0u64;
        let status = unsafe {
            sk_call(
                0,
                ptr::null(),
                0,
                &mut data,
                &mut len,
                &mut allocation_id,
            )
        };
        assert_eq!(status, SK_STATUS_INVALID_HANDLE);
        assert!(data.is_null());
        assert_eq!(len, 0);
        assert_eq!(allocation_id, 0);
        assert_eq!(sentinel, 0x5a);

        let mut data = ptr::null_mut();
        let mut len = 0;
        let mut allocation_id = 0;
        let status = unsafe {
            sk_call(
                handle,
                ptr::null(),
                1,
                &mut data,
                &mut len,
                &mut allocation_id,
            )
        };
        assert_eq!(status, SK_STATUS_INVALID_ARGUMENT);
        assert!(data.is_null());
        assert_eq!(len, 0);
        assert_eq!(allocation_id, 0);

        let status = unsafe {
            sk_call(
                handle,
                ptr::null(),
                0,
                &mut data,
                &mut len,
                &mut allocation_id,
            )
        };
        assert_eq!(status, SK_STATUS_INVALID_REQUEST);
        assert!(data.is_null());
        assert_eq!(len, 0);
        assert_eq!(allocation_id, 0);

        close_ok(handle);
    }

    #[cfg(unix)]
    #[test]
    fn request_memory_may_be_read_only_and_is_not_retained() {
        let _guard = TEST_LOCK.lock().unwrap();
        let index_context = context();
        let inner = search_bytes();
        let expected = expected_operation(&index_context, RequestKind::Search, &inner);
        let backend = Arc::new(ScriptedBackend::new(vec![ExpectedCall::success(
            expected,
            ScriptedResult::Search(search_result()),
        )]));
        let handle = register_index(index_context, backend.clone());
        let request = wrap_request(RequestKind::Search, &inner);

        let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as usize;
        assert!(page_size > 0);
        let allocation_len = request.len().div_ceil(page_size) * page_size;
        let mapping = unsafe {
            libc::mmap(
                ptr::null_mut(),
                allocation_len,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE | libc::MAP_ANON,
                -1,
                0,
            )
        };
        assert_ne!(mapping, libc::MAP_FAILED);
        unsafe {
            ptr::copy_nonoverlapping(request.as_ptr(), mapping.cast::<u8>(), request.len());
            assert_eq!(libc::mprotect(mapping, allocation_len, libc::PROT_READ), 0);
        }

        let mut data = ptr::null_mut();
        let mut len = 0;
        let mut allocation_id = 0;
        let status = unsafe {
            sk_call(
                handle,
                mapping.cast::<u8>(),
                request.len(),
                &mut data,
                &mut len,
                &mut allocation_id,
            )
        };
        assert_eq!(status, SK_STATUS_OK);
        let bytes = take_response((data, len, allocation_id));
        assert!(matches!(
            decode_response(&bytes).unwrap(),
            DecodedResponse::Search(_)
        ));

        // The call is complete; caller memory may disappear immediately.
        assert_eq!(unsafe { libc::munmap(mapping, allocation_len) }, 0);

        backend.finish().unwrap();
        close_ok(handle);

        // Prove later activity does not need the now-unmapped request bytes.
        let later = register_index(context(), Arc::new(RecordingBackend::new()));
        assert_ne!(later, 0);
        close_ok(later);
    }

    #[test]
    fn buffer_free_is_idempotent_and_accounted() {
        let _guard = TEST_LOCK.lock().unwrap();
        let context = context();
        let inner = search_bytes();
        let expected = expected_operation(&context, RequestKind::Search, &inner);
        let backend = Arc::new(ScriptedBackend::new(vec![
            ExpectedCall::success(expected.clone(), ScriptedResult::Search(search_result())),
            ExpectedCall::success(expected, ScriptedResult::Search(search_result())),
        ]));
        let handle = register_index(context, backend.clone());
        let request = wrap_request(RequestKind::Search, &inner);
        let (status, out) = raw_call(handle, &request);
        assert_eq!(status, SK_STATUS_OK);
        let stale = out;
        let stale_id = out.2;
        assert_eq!(live_buffer_count(), 1);
        unsafe { sk_buffer_free(out.0, out.2) };
        assert_eq!(live_buffer_count(), 0);
        unsafe { sk_buffer_free(out.0, out.2) };
        assert_eq!(live_buffer_count(), 0);
        unsafe { sk_buffer_free(ptr::null_mut(), 0) };
        assert_eq!(live_buffer_count(), 0);

        // A stale token cannot release a later buffer, even when presented
        // with the later buffer's pointer.
        let (status, later) = raw_call(handle, &request);
        assert_eq!(status, SK_STATUS_OK);
        let later_id = later.2;
        unsafe { sk_buffer_free(stale.0, stale.2) };
        assert_eq!(later.2, later_id);
        assert_eq!(live_buffer_count(), 1);
        unsafe { sk_buffer_free(later.0, stale_id) };
        assert_eq!(later.2, later_id);
        assert_eq!(live_buffer_count(), 1);
        unsafe { sk_buffer_free(later.0, later.2) };
        assert_eq!(live_buffer_count(), 0);
        backend.finish().unwrap();
        close_ok(handle);
    }

    #[test]
    fn handles_are_never_reused_and_close_is_final() {
        let _guard = TEST_LOCK.lock().unwrap();
        let first = register_index(context(), Arc::new(RecordingBackend::new()));
        assert_ne!(first, 0);
        assert_eq!(unsafe { sk_close(first, ptr::null_mut(), 0) }, SK_STATUS_OK);
        assert_eq!(unsafe { sk_close(first, ptr::null_mut(), 0) }, SK_STATUS_INVALID_HANDLE);

        let second = register_index(context(), Arc::new(RecordingBackend::new()));
        assert_ne!(second, 0);
        assert_ne!(first, second, "handle values must never be reused");

        let request = wrap_request(RequestKind::Search, &search_bytes());
        let (status, out) = raw_call(first, &request);
        assert_eq!(status, SK_STATUS_INVALID_HANDLE);
        assert!(out.0.is_null());
        assert_eq!(out.1, 0);

        close_ok(second);
        assert_eq!(live_handle_count(), 0);
    }

    struct PanicBackend;

    impl Backend for PanicBackend {
        fn insert(&self, _: searchkernel_ir::InsertRequestIr) -> searchkernel_ir::Result<searchkernel_ir::InsertResult> { panic!("intentional verifier panic") }
        fn upsert(&self, _: searchkernel_ir::UpsertRequestIr) -> searchkernel_ir::Result<searchkernel_ir::UpsertResult> { panic!("intentional verifier panic") }
        fn patch(&self, _: searchkernel_ir::PatchRequestIr) -> searchkernel_ir::Result<searchkernel_ir::PatchResult> { panic!("intentional verifier panic") }
        fn delete(&self, _: searchkernel_ir::DeleteRequestIr) -> searchkernel_ir::Result<searchkernel_ir::DeleteResult> { panic!("intentional verifier panic") }
        fn get(&self, _: searchkernel_ir::GetRequestIr) -> searchkernel_ir::Result<searchkernel_ir::GetResult> { panic!("intentional verifier panic") }
        fn search(&self, _: searchkernel_ir::SearchRequestIr) -> searchkernel_ir::Result<searchkernel_ir::SearchResult> { panic!("intentional verifier panic") }
        fn analyze(&self, _: searchkernel_ir::AnalyzeRequestIr) -> searchkernel_ir::Result<searchkernel_ir::AnalyzeResult> { panic!("intentional verifier panic") }
    }

    #[test]
    fn panic_never_crosses_ffi_boundary() {
        let _guard = TEST_LOCK.lock().unwrap();
        let handle = register_index(context(), Arc::new(PanicBackend));
        let request = wrap_request(RequestKind::Search, &search_bytes());
        let (status, out) = raw_call(handle, &request);
        assert_eq!(status, SK_STATUS_PANIC);
        assert!(out.0.is_null());
        assert_eq!(out.1, 0);
        close_ok(handle);

        let usable = register_index(context(), Arc::new(RecordingBackend::new()));
        assert_ne!(usable, 0);
        close_ok(usable);
    }

    struct BlockingBackend {
        entered: Arc<Barrier>,
        release: Arc<Barrier>,
    }

    impl Backend for BlockingBackend {
        fn insert(&self, _: searchkernel_ir::InsertRequestIr) -> searchkernel_ir::Result<searchkernel_ir::InsertResult> { unreachable!() }
        fn upsert(&self, _: searchkernel_ir::UpsertRequestIr) -> searchkernel_ir::Result<searchkernel_ir::UpsertResult> { unreachable!() }
        fn patch(&self, _: searchkernel_ir::PatchRequestIr) -> searchkernel_ir::Result<searchkernel_ir::PatchResult> { unreachable!() }
        fn delete(&self, _: searchkernel_ir::DeleteRequestIr) -> searchkernel_ir::Result<searchkernel_ir::DeleteResult> { unreachable!() }
        fn get(&self, _: searchkernel_ir::GetRequestIr) -> searchkernel_ir::Result<searchkernel_ir::GetResult> { unreachable!() }
        fn analyze(&self, _: searchkernel_ir::AnalyzeRequestIr) -> searchkernel_ir::Result<searchkernel_ir::AnalyzeResult> { unreachable!() }
        fn search(&self, _: searchkernel_ir::SearchRequestIr) -> searchkernel_ir::Result<searchkernel_ir::SearchResult> {
            self.entered.wait();
            self.release.wait();
            Ok(search_result())
        }
    }

    #[test]
    fn close_does_not_cancel_inflight_call_but_blocks_future_calls() {
        let _guard = TEST_LOCK.lock().unwrap();
        let entered = Arc::new(Barrier::new(2));
        let release = Arc::new(Barrier::new(2));
        let backend = Arc::new(BlockingBackend {
            entered: entered.clone(),
            release: release.clone(),
        });
        let handle = register_index(context(), backend);
        let request = wrap_request(RequestKind::Search, &search_bytes());
        let thread_request = request.clone();

        let worker = thread::spawn(move || {
            let (status, out) = raw_call(handle, &thread_request);
            let bytes = if status == SK_STATUS_OK {
                take_response(out)
            } else {
                Vec::new()
            };
            (status, bytes)
        });

        entered.wait();
        assert_eq!(unsafe { sk_close(handle, ptr::null_mut(), 0) }, SK_STATUS_OK);

        let (status, out) = raw_call(handle, &request);
        assert_eq!(status, SK_STATUS_INVALID_HANDLE);
        assert!(out.0.is_null());
        assert_eq!(out.1, 0);

        release.wait();
        let (status, bytes) = worker.join().unwrap();
        assert_eq!(status, SK_STATUS_OK);
        assert!(matches!(
            decode_response(&bytes).unwrap(),
            DecodedResponse::Search(_)
        ));
        assert_eq!(live_handle_count(), 0);
        assert_eq!(live_buffer_count(), 0);
    }

    #[test]
    fn repeated_call_free_cycles_leave_no_live_buffers() {
        let _guard = TEST_LOCK.lock().unwrap();
        for _ in 0..256 {
            let context = context();
            let inner = search_bytes();
            let expected = expected_operation(&context, RequestKind::Search, &inner);
            let backend = Arc::new(ScriptedBackend::new(vec![ExpectedCall::success(
                expected,
                ScriptedResult::Search(search_result()),
            )]));
            let handle = register_index(context, backend.clone());
            let request = wrap_request(RequestKind::Search, &inner);
            let _ = call_ok(handle, &request);
            backend.finish().unwrap();
            close_ok(handle);
        }
        assert_eq!(live_handle_count(), 0);
        assert_eq!(live_buffer_count(), 0);
    }
}
