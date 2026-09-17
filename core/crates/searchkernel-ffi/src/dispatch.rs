use crate::handle::HandleState;
use crate::status;
use crate::transport;

pub(crate) fn dispatch(state: &HandleState, request: &[u8]) -> Result<Vec<u8>, u32> {
    let envelope = transport::parse_request(request).map_err(|_| status::INVALID_REQUEST)?;
    if envelope.protocol_version() != transport::PROTOCOL_VERSION {
        return Err(status::UNSUPPORTED_PROTOCOL);
    }

    #[cfg(feature = "c7-test-support")]
    if let Some(response) = &state.response_override {
        return Ok(response.clone());
    }

    match envelope.payload_type().0 {
        transport::REQUEST_INSERT => {
            let payload = envelope
                .payload_as_search_kernel_v1_insert_request()
                .ok_or(status::INVALID_REQUEST)?;
            let result = execute_backend(state, |context, backend| {
                context
                    .insert_to_ir(payload)
                    .and_then(|request| backend.insert(request))
            })?;
            let response = encode_result(result, transport::RESPONSE_INSERT, |builder, result| {
                searchkernel_ir::insert_result_offset(builder, result).as_union_value()
            });
            Ok(response)
        }
        transport::REQUEST_UPSERT => {
            let payload = envelope
                .payload_as_search_kernel_v1_upsert_request()
                .ok_or(status::INVALID_REQUEST)?;
            let result = execute_backend(state, |context, backend| {
                context
                    .upsert_to_ir(payload)
                    .and_then(|request| backend.upsert(request))
            })?;
            let response = encode_result(result, transport::RESPONSE_UPSERT, |builder, result| {
                searchkernel_ir::upsert_result_offset(builder, result).as_union_value()
            });
            Ok(response)
        }
        transport::REQUEST_PATCH => {
            let payload = envelope
                .payload_as_search_kernel_v1_patch_request()
                .ok_or(status::INVALID_REQUEST)?;
            let result = execute_backend(state, |context, backend| {
                context
                    .patch_to_ir(payload)
                    .and_then(|request| backend.patch(request))
            })?;
            let response = encode_result(result, transport::RESPONSE_PATCH, |builder, result| {
                searchkernel_ir::patch_result_offset(builder, result).as_union_value()
            });
            Ok(response)
        }
        transport::REQUEST_DELETE => {
            let payload = envelope
                .payload_as_search_kernel_v1_delete_request()
                .ok_or(status::INVALID_REQUEST)?;
            let result = execute_backend(state, |context, backend| {
                context
                    .delete_to_ir(payload)
                    .and_then(|request| backend.delete(request))
            })?;
            let response = encode_result(result, transport::RESPONSE_DELETE, |builder, result| {
                searchkernel_ir::delete_result_offset(builder, result).as_union_value()
            });
            Ok(response)
        }
        transport::REQUEST_GET => {
            let payload = envelope
                .payload_as_search_kernel_v1_get_request()
                .ok_or(status::INVALID_REQUEST)?;
            let result = execute_backend(state, |context, backend| {
                context
                    .get_to_ir(payload)
                    .and_then(|request| backend.get(request))
            })?;
            let response = encode_result(result, transport::RESPONSE_GET, |builder, result| {
                searchkernel_ir::get_result_offset(builder, result).as_union_value()
            });
            Ok(response)
        }
        transport::REQUEST_SEARCH => {
            let payload = envelope
                .payload_as_search_kernel_v1_search_request()
                .ok_or(status::INVALID_REQUEST)?;
            let result = execute_backend(state, |context, backend| {
                context
                    .search_to_ir(payload)
                    .and_then(|request| backend.search(request))
            })?;
            let response = encode_result(result, transport::RESPONSE_SEARCH, |builder, result| {
                searchkernel_ir::search_result_offset(builder, result).as_union_value()
            });
            Ok(response)
        }
        transport::REQUEST_ANALYZE => {
            let payload = envelope
                .payload_as_search_kernel_v1_analyze_request()
                .ok_or(status::INVALID_REQUEST)?;
            let result = execute_backend(state, |context, backend| {
                context
                    .analyze_to_ir(payload)
                    .and_then(|request| backend.analyze(request))
            })?;
            let response = encode_result(result, transport::RESPONSE_ANALYZE, |builder, result| {
                searchkernel_ir::analyze_result_offset(builder, result).as_union_value()
            });
            Ok(response)
        }
        transport::REQUEST_DESCRIBE => {
            let payload = envelope
                .payload_as_search_kernel_v1_describe_request()
                .ok_or(status::INVALID_REQUEST)?;
            let _ = payload;
            let result = execute_context(state, |context| Ok(context.describe()))?;
            Ok(encode_result(
                result,
                transport::RESPONSE_DESCRIBE,
                |builder, result| {
                    searchkernel_ir::describe_result_offset(builder, result).as_union_value()
                },
            ))
        }
        transport::REQUEST_INSPECT_QUERY => {
            let payload = envelope
                .payload_as_search_kernel_v1_inspect_query_request()
                .ok_or(status::INVALID_REQUEST)?;
            let request = payload.request().ok_or(status::INVALID_REQUEST)?;
            let result = execute_context(state, |context| context.inspect_query(request))?;
            Ok(encode_result(
                result,
                transport::RESPONSE_INSPECT_QUERY,
                |builder, result| {
                    searchkernel_ir::inspect_query_result_offset(builder, result).as_union_value()
                },
            ))
        }
        0 => Err(status::INVALID_REQUEST),
        _ => Err(status::UNSUPPORTED_OPERATION),
    }
}

fn execute_backend<T>(
    state: &HandleState,
    callback: impl FnOnce(
        &searchkernel_ir::IndexContext,
        &dyn searchkernel_ir::Backend,
    ) -> searchkernel_ir::Result<T>,
) -> Result<searchkernel_ir::Result<T>, u32> {
    state
        .session
        .execute(|backend| callback(&state.context, backend))
        .map_err(|searchkernel_ir::SessionError::Closed| status::INDEX_CLOSED)
}

fn execute_context<T>(
    state: &HandleState,
    callback: impl FnOnce(&searchkernel_ir::IndexContext) -> searchkernel_ir::Result<T>,
) -> Result<searchkernel_ir::Result<T>, u32> {
    state
        .session
        .execute(|_| callback(&state.context))
        .map_err(|searchkernel_ir::SessionError::Closed| status::INDEX_CLOSED)
}

fn encode_result<T>(
    result: searchkernel_ir::Result<T>,
    success_kind: u8,
    encode: impl for<'a> FnOnce(
        &mut flatbuffers::FlatBufferBuilder<'a>,
        &T,
    ) -> flatbuffers::WIPOffset<flatbuffers::UnionWIPOffset>,
) -> Vec<u8> {
    transport::frame_response(
        match result {
            Ok(_) => success_kind,
            Err(_) => transport::RESPONSE_ERROR,
        },
        |builder| match result {
            Ok(result) => encode(builder, &result),
            Err(error) => searchkernel_ir::error_offset(builder, &error).as_union_value(),
        },
    )
}

#[cfg(all(test, feature = "ffi-test-support"))]
mod tests {
    use super::*;
    use crate::test_support::RecordingBackend;
    use searchkernel_ir::{api, IndexContext};
    use std::sync::Arc;

    fn test_state() -> HandleState {
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
        let bytes = builder.finished_data().to_vec();
        let root = searchkernel_ir::__flatbuffers::root::<api::IndexDefinition>(&bytes)
            .expect("test definition");
        HandleState {
            context: Arc::new(IndexContext::from_api(root).expect("test context")),
            session: searchkernel_ir::BackendSession::from_backend(Arc::new(
                RecordingBackend::new(),
            )),
            #[cfg(feature = "c7-test-support")]
            response_override: None,
        }
    }

    fn describe_request() -> Vec<u8> {
        let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
        let request =
            api::DescribeRequest::create(&mut builder, &api::DescribeRequestArgs::default());
        builder.finish(request, None);
        builder.finished_data().to_vec()
    }

    #[test]
    fn dispatch_rejects_invalid_protocol_and_operation_discriminants() {
        let state = test_state();
        let inner = describe_request();
        let request = transport::frame_request(transport::REQUEST_DESCRIBE, 99, &inner);
        assert_eq!(
            dispatch(&state, &request),
            Err(status::UNSUPPORTED_PROTOCOL)
        );

        let request = transport::frame_request(0xfe, transport::PROTOCOL_VERSION, &inner);
        assert_eq!(
            dispatch(&state, &request),
            Err(status::UNSUPPORTED_OPERATION)
        );

        let request = transport::frame_request(0, transport::PROTOCOL_VERSION, &inner);
        assert_eq!(dispatch(&state, &request), Err(status::INVALID_REQUEST));
        assert_eq!(
            dispatch(&state, b"not a request"),
            Err(status::INVALID_REQUEST)
        );
    }

    #[test]
    fn dispatch_rejects_operations_after_session_close() {
        let state = test_state();
        state.session.close().expect("close test session");
        assert_eq!(
            dispatch(
                &state,
                &transport::frame_request(
                    transport::REQUEST_DESCRIBE,
                    transport::PROTOCOL_VERSION,
                    &describe_request(),
                ),
            ),
            Err(status::INDEX_CLOSED)
        );
    }

    #[test]
    fn context_dispatch_helper_holds_lease_until_callback_returns() {
        let state = Arc::new(test_state());
        let (started_sender, started_receiver) = std::sync::mpsc::channel();
        let (release_sender, release_receiver) = std::sync::mpsc::channel();

        let worker_state = Arc::clone(&state);
        let worker = std::thread::spawn(move || {
            execute_context(&worker_state, |_| {
                started_sender.send(()).expect("signal describe callback");
                release_receiver.recv().expect("wait for describe release");
                Ok(())
            })
        });
        started_receiver
            .recv()
            .expect("describe callback must start before close");

        let close_session = Arc::clone(&state.session);
        let closer = std::thread::spawn(move || close_session.close());
        release_sender.send(()).expect("release describe callback");

        assert_eq!(worker.join().expect("describe worker"), Ok(Ok(())));
        closer.join().expect("close worker").expect("close session");
    }
}
