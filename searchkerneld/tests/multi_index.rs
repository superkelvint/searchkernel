#![cfg(feature = "native")]

use flatbuffers::root;
use http_body_util::{BodyExt, Full};
use hyper::{body::Bytes, Request as HttpRequest};
use hyper_util::{client::legacy::Client, rt::TokioExecutor};
use searchkernel::{ErrorCode as RustErrorCode, Field, IndexDefinition, RemoteClient};
use searchkernel_ir::{
    CREATE_INDEX_DEFINITION_PATH, INDEX_ALREADY_EXISTS_DETAIL_CODE, INDEX_NOT_FOUND_DETAIL_CODE,
    INVALID_INDEX_TARGET_DETAIL_CODE, MISSING_INDEX_DEFINITION_DETAIL_CODE,
    MISSING_INDEX_TARGET_DETAIL_CODE, STALE_INDEX_HANDLE_DETAIL_CODE, TARGET_NAME_PATH,
    TARGET_PATH,
};
use searchkernel_protocol::{
    json::{
        request_flatbuffer_to_json, request_json_to_flatbuffer, response_flatbuffer_to_json,
        response_json_to_flatbuffer,
    },
    protocol::v1::{self as wire, PROTOCOL_VERSION},
    wire::v1 as api,
};
use searchkerneld::{rpc, start_multi, IndexRegistry, ServerConfig, JSON_EXECUTE_PATH};
use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::{Arc, Barrier},
    thread,
    time::{SystemTime, UNIX_EPOCH},
};
use tonic::Request;

struct TempDir(PathBuf);

impl TempDir {
    fn new(label: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "searchkerneld-multi-index-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create test data root");
        Self(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn target<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    name: &str,
) -> flatbuffers::WIPOffset<api::IndexTarget<'a>> {
    target_with_generation(builder, name, None)
}

fn target_with_generation<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    name: &str,
    generation: Option<&str>,
) -> flatbuffers::WIPOffset<api::IndexTarget<'a>> {
    let name = builder.create_string(name);
    let generation = generation.map(|generation| builder.create_string(generation));
    api::IndexTarget::create(
        builder,
        &api::IndexTargetArgs {
            name: Some(name),
            generation,
        },
    )
}

fn create_request(target_name: &str, field_name: &str) -> Vec<u8> {
    create_request_with_definition_name(target_name, field_name, target_name)
}

fn create_request_with_definition_name(
    target_name: &str,
    field_name: &str,
    definition_name: &str,
) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let field_name = builder.create_string(field_name);
    let primitive = api::PrimitiveDataType::create(
        &mut builder,
        &api::PrimitiveDataTypeArgs {
            kind: api::PrimitiveDataTypeKind::String,
        },
    );
    let data_type = api::DataType::create(
        &mut builder,
        &api::DataTypeArgs {
            kind_type: api::DataTypeKind::PrimitiveDataType,
            kind: Some(primitive.as_union_value()),
        },
    );
    let stored = api::OptionalBool::new(true);
    let searchable = api::OptionalBool::new(true);
    let field = api::FieldDefinition::create(
        &mut builder,
        &api::FieldDefinitionArgs {
            name: Some(field_name),
            data_type: Some(data_type),
            stored: Some(&stored),
            searchable: Some(&searchable),
            ..Default::default()
        },
    );
    let fields = builder.create_vector(&[field]);
    let definition_name = builder.create_string(definition_name);
    let definition = api::IndexDefinition::create(
        &mut builder,
        &api::IndexDefinitionArgs {
            name: Some(definition_name),
            fields: Some(fields),
            ..Default::default()
        },
    );
    let payload = api::CreateIndexRequest::create(
        &mut builder,
        &api::CreateIndexRequestArgs {
            definition: Some(definition),
        },
    );
    let target = target(&mut builder, target_name);
    let envelope = wire::RequestEnvelope::create(
        &mut builder,
        &wire::RequestEnvelopeArgs {
            protocol_version: PROTOCOL_VERSION,
            target: Some(target),
            payload_type: wire::RequestPayload::SearchKernel_Wire_CreateIndexRequest,
            payload: Some(payload.as_union_value()),
        },
    );
    wire::finish_request_envelope_buffer(&mut builder, envelope);
    builder.finished_data().to_vec()
}

fn create_request_without_definition(target_name: &str) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let payload = api::CreateIndexRequest::create(
        &mut builder,
        &api::CreateIndexRequestArgs { definition: None },
    );
    let target = target(&mut builder, target_name);
    let envelope = wire::RequestEnvelope::create(
        &mut builder,
        &wire::RequestEnvelopeArgs {
            protocol_version: PROTOCOL_VERSION,
            target: Some(target),
            payload_type: wire::RequestPayload::SearchKernel_Wire_CreateIndexRequest,
            payload: Some(payload.as_union_value()),
        },
    );
    wire::finish_request_envelope_buffer(&mut builder, envelope);
    builder.finished_data().to_vec()
}

fn open_request(target_name: Option<&str>) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let payload =
        api::OpenIndexRequest::create(&mut builder, &api::OpenIndexRequestArgs::default());
    let target = target_name.map(|name| target(&mut builder, name));
    let envelope = wire::RequestEnvelope::create(
        &mut builder,
        &wire::RequestEnvelopeArgs {
            protocol_version: PROTOCOL_VERSION,
            target,
            payload_type: wire::RequestPayload::SearchKernel_Wire_OpenIndexRequest,
            payload: Some(payload.as_union_value()),
        },
    );
    wire::finish_request_envelope_buffer(&mut builder, envelope);
    builder.finished_data().to_vec()
}

fn delete_index_request(target_name: &str) -> Vec<u8> {
    delete_index_request_with_generation(target_name, None)
}

fn delete_index_request_with_generation(target_name: &str, generation: Option<&str>) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let payload =
        api::DeleteIndexRequest::create(&mut builder, &api::DeleteIndexRequestArgs::default());
    let target = target_with_generation(&mut builder, target_name, generation);
    let envelope = wire::RequestEnvelope::create(
        &mut builder,
        &wire::RequestEnvelopeArgs {
            protocol_version: PROTOCOL_VERSION,
            target: Some(target),
            payload_type: wire::RequestPayload::SearchKernel_Wire_DeleteIndexRequest,
            payload: Some(payload.as_union_value()),
        },
    );
    wire::finish_request_envelope_buffer(&mut builder, envelope);
    builder.finished_data().to_vec()
}

fn upsert_request(target_name: &str, id: &str, field_name: &str, text: &str) -> Vec<u8> {
    upsert_request_with_generation(target_name, id, field_name, text, None)
}

fn upsert_request_with_generation(
    target_name: &str,
    id: &str,
    field_name: &str,
    text: &str,
    generation: Option<&str>,
) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let text = builder.create_string(text);
    let string_value =
        api::StringValue::create(&mut builder, &api::StringValueArgs { value: Some(text) });
    let value = api::Value::create(
        &mut builder,
        &api::ValueArgs {
            kind_type: api::ValueKind::StringValue,
            kind: Some(string_value.as_union_value()),
        },
    );
    let key = builder.create_string(field_name);
    let entry = api::ValueEntry::create(
        &mut builder,
        &api::ValueEntryArgs {
            key: Some(key),
            value: Some(value),
        },
    );
    let fields = builder.create_vector(&[entry]);
    let id = builder.create_string(id);
    let document = api::Document::create(
        &mut builder,
        &api::DocumentArgs {
            id: Some(id),
            fields: Some(fields),
        },
    );
    let payload = api::UpsertRequest::create(
        &mut builder,
        &api::UpsertRequestArgs {
            document: Some(document),
            ..Default::default()
        },
    );
    let target = target_with_generation(&mut builder, target_name, generation);
    let envelope = wire::RequestEnvelope::create(
        &mut builder,
        &wire::RequestEnvelopeArgs {
            protocol_version: PROTOCOL_VERSION,
            target: Some(target),
            payload_type: wire::RequestPayload::SearchKernel_Wire_UpsertRequest,
            payload: Some(payload.as_union_value()),
        },
    );
    wire::finish_request_envelope_buffer(&mut builder, envelope);
    builder.finished_data().to_vec()
}

fn search_request(target_name: &str, text: &str) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let text = builder.create_string(text);
    let query = api::SearchKernelTextQuery::create(
        &mut builder,
        &api::SearchKernelTextQueryArgs { text: Some(text) },
    );
    let source = api::QuerySource::create(
        &mut builder,
        &api::QuerySourceArgs {
            source_type: api::QuerySourceKind::SearchKernelTextQuery,
            source: Some(query.as_union_value()),
        },
    );
    let payload = api::SearchRequest::create(
        &mut builder,
        &api::SearchRequestArgs {
            query: Some(source),
            ..Default::default()
        },
    );
    let target = target(&mut builder, target_name);
    let envelope = wire::RequestEnvelope::create(
        &mut builder,
        &wire::RequestEnvelopeArgs {
            protocol_version: PROTOCOL_VERSION,
            target: Some(target),
            payload_type: wire::RequestPayload::SearchKernel_Wire_SearchRequest,
            payload: Some(payload.as_union_value()),
        },
    );
    wire::finish_request_envelope_buffer(&mut builder, envelope);
    builder.finished_data().to_vec()
}

async fn execute_http_json_raw(
    address: std::net::SocketAddr,
    json: impl Into<Bytes>,
) -> (http::StatusCode, Vec<u8>) {
    let request = HttpRequest::builder()
        .method("POST")
        .uri(format!("http://{address}{JSON_EXECUTE_PATH}"))
        .header("content-type", "application/json")
        .body(Full::new(json.into()))
        .expect("build HTTP JSON request");
    let client = Client::builder(TokioExecutor::new()).build_http();
    let response = client.request(request).await.expect("HTTP JSON request");
    let status = response.status();
    let body = response
        .into_body()
        .collect()
        .await
        .expect("read HTTP JSON response")
        .to_bytes()
        .to_vec();
    (status, body)
}

async fn execute_http_json(address: std::net::SocketAddr, payload: &[u8]) -> Vec<u8> {
    let json = request_flatbuffer_to_json(payload).expect("encode request JSON");
    let (status, body) = execute_http_json_raw(address, Bytes::from(json)).await;
    assert_eq!(
        status,
        http::StatusCode::OK,
        "HTTP JSON request failed: {}",
        String::from_utf8_lossy(&body)
    );
    response_json_to_flatbuffer(&body).expect("decode response JSON")
}

fn response(bytes: &[u8]) -> wire::ResponseEnvelope<'_> {
    root::<wire::ResponseEnvelope<'_>>(bytes).expect("response envelope")
}

fn list_request() -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let payload = api::ListIndexesRequest::create(&mut builder, &Default::default());
    let envelope = wire::RequestEnvelope::create(
        &mut builder,
        &wire::RequestEnvelopeArgs {
            protocol_version: PROTOCOL_VERSION,
            target: None,
            payload_type: wire::RequestPayload::SearchKernel_Wire_ListIndexesRequest,
            payload: Some(payload.as_union_value()),
        },
    );
    wire::finish_request_envelope_buffer(&mut builder, envelope);
    builder.finished_data().to_vec()
}

fn listed_targets(bytes: &[u8]) -> Vec<String> {
    response(bytes)
        .payload_as_search_kernel_wire_list_indexes_result()
        .expect("list result")
        .targets()
        .expect("targets")
        .iter()
        .map(ToOwned::to_owned)
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn durable_discovery_without_opening_on_both_transports() {
    let temp = TempDir::new("discovery");
    let registry = IndexRegistry::new(&temp.0).unwrap();
    assert!(listed_targets(&registry.dispatch(&list_request()).unwrap()).is_empty());
    for name in ["zeta", "alpha"] {
        assert_lifecycle(
            &registry.dispatch(&create_request(name, "title")).unwrap(),
            wire::ResponsePayload::SearchKernel_Wire_CreateIndexResult,
        );
    }
    assert_eq!(
        listed_targets(&registry.dispatch(&list_request()).unwrap()),
        ["alpha", "zeta"]
    );
    registry.close_all().unwrap();
    drop(registry);
    fs::create_dir(temp.0.join("empty")).unwrap();
    fs::write(temp.0.join("file"), b"not an index").unwrap();
    fs::create_dir(temp.0.join("bad target")).unwrap();
    fs::create_dir(temp.0.join(".alpha.searchkernel-delete-123")).unwrap();
    std::os::unix::fs::symlink(temp.0.join("alpha"), temp.0.join("linked")).unwrap();
    // A plausible directory with a symlinked manifest must not be probed.
    fs::create_dir(temp.0.join("malformed")).unwrap();
    fs::create_dir(temp.0.join("malformed/data")).unwrap();
    fs::create_dir(temp.0.join("malformed/config")).unwrap();
    std::os::unix::fs::symlink(
        temp.0.join("alpha/manifest"),
        temp.0.join("malformed/manifest"),
    )
    .unwrap();

    let registry = Arc::new(IndexRegistry::new(&temp.0).unwrap());
    assert!(registry.loaded_targets().unwrap().is_empty());
    let server = start_multi(
        ServerConfig::new("127.0.0.1:0".parse().unwrap())
            .with_web_listen_addr("127.0.0.1:0".parse().unwrap()),
        registry.clone(),
    )
    .await
    .unwrap();
    let mut grpc = rpc::search_kernel_client::SearchKernelClient::connect(format!(
        "http://{}",
        server.tcp_addr().unwrap()
    ))
    .await
    .unwrap();
    let binary = grpc
        .execute(Request::new(rpc::FlatBufferFrame {
            payload: list_request(),
        }))
        .await
        .unwrap()
        .into_inner()
        .payload;
    assert_eq!(listed_targets(&binary), ["alpha", "zeta"]);
    let json = execute_http_json(server.web_tcp_addr().unwrap(), &list_request()).await;
    assert_eq!(listed_targets(&json), ["alpha", "zeta"]);
    assert!(
        registry.loaded_targets().unwrap().is_empty(),
        "discovery must never open targets"
    );
    registry.dispatch(&delete_index_request("alpha")).unwrap();
    assert_eq!(
        listed_targets(&registry.dispatch(&list_request()).unwrap()),
        ["zeta"]
    );
    assert_lifecycle(
        &registry.dispatch(&open_request(Some("zeta"))).unwrap(),
        wire::ResponsePayload::SearchKernel_Wire_OpenIndexResult,
    );
    server.shutdown().await.unwrap();
    registry.close_all().unwrap();
}

fn assert_lifecycle(bytes: &[u8], kind: wire::ResponsePayload) {
    let envelope = response(bytes);
    let error_message = envelope
        .payload_as_search_kernel_wire_error()
        .and_then(|error| error.message())
        .unwrap_or_default();
    assert_eq!(
        envelope.payload_type(),
        kind,
        "unexpected lifecycle response error: {error_message}"
    );
}

fn assert_portable_error(
    bytes: &[u8],
    code: api::ErrorCode,
    detail_code: &str,
    path: Option<&str>,
) {
    let envelope = response(bytes);
    assert_eq!(
        envelope.payload_type(),
        wire::ResponsePayload::SearchKernel_Wire_Error
    );
    let error = envelope
        .payload_as_search_kernel_wire_error()
        .expect("error payload");
    assert_eq!(error.code(), code);
    let details = error.details().expect("canonical error detail");
    assert_eq!(details.len(), 1);
    let detail = details.get(0);
    assert_eq!(detail.code(), Some(detail_code));
    assert_eq!(detail.path(), path);
}

fn portable_error_semantics(
    bytes: &[u8],
) -> (api::ErrorCode, String, Option<String>, String, String) {
    let envelope = response(bytes);
    assert_eq!(
        envelope.payload_type(),
        wire::ResponsePayload::SearchKernel_Wire_Error
    );
    let error = envelope
        .payload_as_search_kernel_wire_error()
        .expect("error payload");
    let details = error.details().expect("portable error details");
    assert_eq!(details.len(), 1);
    let detail = details.get(0);
    (
        error.code(),
        error.message().unwrap_or_default().to_owned(),
        detail.path().map(ToOwned::to_owned),
        detail.code().unwrap_or_default().to_owned(),
        detail.message().unwrap_or_default().to_owned(),
    )
}

fn ir_portable_error_semantics(
    error: searchkernel_ir::PortableError,
) -> (api::ErrorCode, String, Option<String>, String, String) {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let error = searchkernel_ir::wire::v1::error_offset(&mut builder, &error);
    builder.finish(error, None);
    let error = root::<api::Error<'_>>(builder.finished_data()).expect("canonical IR error");
    let details = error.details().expect("canonical IR error details");
    assert_eq!(details.len(), 1);
    let detail = details.get(0);
    (
        error.code(),
        error.message().unwrap_or_default().to_owned(),
        detail.path().map(ToOwned::to_owned),
        detail.code().unwrap_or_default().to_owned(),
        detail.message().unwrap_or_default().to_owned(),
    )
}

fn resolved_ir_request_error(request: &[u8]) -> searchkernel_ir::PortableError {
    match searchkernel_ir::wire::decode_control_plane_request(request) {
        Err(searchkernel_ir::wire::ControlPlaneDecodeError::Semantic { error, .. }) => error,
        result => panic!("expected canonical IR semantic error, got {result:?}"),
    }
}

fn assert_upsert(bytes: &[u8]) {
    assert_eq!(
        response(bytes).payload_type(),
        wire::ResponsePayload::SearchKernel_Wire_UpsertResult
    );
}

fn total_hits(bytes: &[u8]) -> u64 {
    let envelope = response(bytes);
    assert_eq!(
        envelope.payload_type(),
        wire::ResponsePayload::SearchKernel_Wire_SearchResult
    );
    envelope
        .payload_as_search_kernel_wire_search_result()
        .expect("search result")
        .total_hits()
        .expect("total hits")
        .value()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn php_emitted_create_index_json_reaches_multi_index_registry() {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .canonicalize()
        .expect("repository root");
    let script = repo_root.join("clients/php/tests/emit_bfbs_contract.php");
    let output = Command::new("php")
        .arg(&script)
        .current_dir(&repo_root)
        .output()
        .expect("run PHP BFBS contract emitter");
    assert!(
        output.status.success(),
        "PHP fixture emitter failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("PHP fixture output is UTF-8");
    let json = stdout
        .lines()
        .find_map(|line| {
            let mut parts = line.splitn(3, '\t');
            let schema = parts.next()?;
            let name = parts.next()?;
            let json = parts.next()?;
            (schema == "request" && name == "create-index-e2e-schema").then_some(json)
        })
        .expect("PHP emitter must produce exact E2E create-index request");

    let request = request_json_to_flatbuffer(json.as_bytes())
        .expect("PHP create-index JSON must convert to canonical request bytes");
    let envelope = root::<wire::RequestEnvelope<'_>>(&request)
        .expect("daemon request verifier must accept PHP create-index bytes");
    assert_eq!(
        envelope.payload_type(),
        wire::RequestPayload::SearchKernel_Wire_CreateIndexRequest
    );
    let payload = envelope
        .payload_as_search_kernel_wire_create_index_request()
        .expect("daemon must access PHP create-index union payload");
    let definition = payload.definition().expect("PHP create-index definition");
    assert_eq!(definition.fields().map(|fields| fields.len()), Some(13));
    assert_eq!(
        definition.rank_profiles().map(|profiles| profiles.len()),
        Some(6)
    );

    let temp = TempDir::new("php-create-json");
    let registry = Arc::new(IndexRegistry::new(&temp.0).expect("registry"));
    let server = start_multi(
        ServerConfig::new("127.0.0.1:0".parse().expect("gRPC address"))
            .with_web_listen_addr("127.0.0.1:0".parse().expect("HTTP address")),
        registry.clone(),
    )
    .await
    .expect("start multi-index daemon");
    let http_address = server.web_tcp_addr().expect("HTTP listener");

    let (status, body) =
        execute_http_json_raw(http_address, Bytes::copy_from_slice(json.as_bytes())).await;
    assert_eq!(
        status,
        http::StatusCode::OK,
        "PHP create-index HTTP request failed: {}",
        String::from_utf8_lossy(&body)
    );
    let response =
        response_json_to_flatbuffer(&body).expect("decode PHP create-index HTTP response JSON");
    assert_lifecycle(
        &response,
        wire::ResponsePayload::SearchKernel_Wire_CreateIndexResult,
    );

    let php_http_script = repo_root.join("clients/php/tests/create_index_http_smoke.php");
    let php_http = Command::new("php")
        .arg(&php_http_script)
        .arg(format!("http://{http_address}"))
        .arg("php-http-smoke")
        .current_dir(&repo_root)
        .output()
        .expect("run PHP HttpTransport create-index smoke");
    assert!(
        php_http.status.success(),
        "PHP HttpTransport create-index failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&php_http.stdout),
        String::from_utf8_lossy(&php_http.stderr)
    );

    let php_lifecycle = repo_root.join("clients/php/tests/lifecycle_http_smoke.php");
    let php_lifecycle_result = Command::new("php")
        .arg(&php_lifecycle)
        .arg(format!("http://{http_address}"))
        .current_dir(&repo_root)
        .output()
        .expect("run PHP lifecycle semantic smoke");
    assert!(
        php_lifecycle_result.status.success(),
        "PHP lifecycle semantics failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&php_lifecycle_result.stdout),
        String::from_utf8_lossy(&php_lifecycle_result.stderr)
    );

    server.shutdown().await.expect("shutdown daemon");
    registry.close_all().expect("close registry");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn grpc_and_http_json_share_one_registry() {
    let temp = TempDir::new("cross-transport");
    let registry = Arc::new(IndexRegistry::new(&temp.0).expect("registry"));
    let server = start_multi(
        ServerConfig::new("127.0.0.1:0".parse().expect("gRPC address"))
            .with_web_listen_addr("127.0.0.1:0".parse().expect("HTTP address")),
        registry.clone(),
    )
    .await
    .expect("start multi-index daemon");

    let grpc_address = server.tcp_addr().expect("gRPC listener");
    let http_address = server.web_tcp_addr().expect("HTTP listener");
    let mut grpc =
        rpc::search_kernel_client::SearchKernelClient::connect(format!("http://{grpc_address}"))
            .await
            .expect("connect gRPC client");

    let create = grpc
        .execute(Request::new(rpc::FlatBufferFrame {
            payload: create_request("products", "title"),
        }))
        .await
        .expect("create over gRPC")
        .into_inner()
        .payload;
    assert_lifecycle(
        &create,
        wire::ResponsePayload::SearchKernel_Wire_CreateIndexResult,
    );

    let upsert = execute_http_json(
        http_address,
        &upsert_request("products", "cross-transport", "title", "shared registry"),
    )
    .await;
    assert_upsert(&upsert);

    let search = grpc
        .execute(Request::new(rpc::FlatBufferFrame {
            payload: search_request("products", "shared"),
        }))
        .await
        .expect("search over gRPC")
        .into_inner()
        .payload;
    assert_eq!(total_hits(&search), 1);

    server.shutdown().await.expect("shutdown daemon");
    registry.close_all().expect("close registry");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn canonical_generation_rejects_stale_handles_on_grpc_and_http() {
    let temp = TempDir::new("canonical-generation");
    let registry = Arc::new(IndexRegistry::new(&temp.0).expect("registry"));
    let server = start_multi(
        ServerConfig::new("127.0.0.1:0".parse().expect("gRPC address"))
            .with_web_listen_addr("127.0.0.1:0".parse().expect("HTTP address")),
        registry.clone(),
    )
    .await
    .expect("start multi-index daemon");
    let mut grpc = rpc::search_kernel_client::SearchKernelClient::connect(format!(
        "http://{}",
        server.tcp_addr().expect("gRPC listener")
    ))
    .await
    .expect("connect gRPC client");
    let http_address = server.web_tcp_addr().expect("HTTP listener");

    let created = grpc
        .execute(Request::new(rpc::FlatBufferFrame {
            payload: create_request("reused", "title"),
        }))
        .await
        .expect("create over gRPC")
        .into_inner()
        .payload;
    let created_json: serde_json::Value = serde_json::from_slice(
        &response_flatbuffer_to_json(&created).expect("canonical create result JSON"),
    )
    .expect("create result JSON");
    let old_generation = created_json["payload"]["generation"]
        .as_str()
        .expect("canonical create result generation")
        .to_owned();

    assert_lifecycle(
        &execute_http_json(http_address, &delete_index_request("reused")).await,
        wire::ResponsePayload::SearchKernel_Wire_DeleteIndexResult,
    );
    let recreated = execute_http_json(http_address, &create_request("reused", "title")).await;
    assert_lifecycle(
        &recreated,
        wire::ResponsePayload::SearchKernel_Wire_CreateIndexResult,
    );
    let recreated_json: serde_json::Value = serde_json::from_slice(
        &response_flatbuffer_to_json(&recreated).expect("canonical recreate result JSON"),
    )
    .expect("recreate result JSON");
    let new_generation = recreated_json["payload"]["generation"]
        .as_str()
        .expect("canonical recreate result generation");
    assert_ne!(old_generation, new_generation);

    let stale_bytes =
        upsert_request_with_generation("reused", "doc", "title", "stale", Some(&old_generation));
    let stale_json =
        request_flatbuffer_to_json(&stale_bytes).expect("canonical stale request JSON");
    let stale_bytes = request_json_to_flatbuffer(&stale_json)
        .expect("canonical expected generation in request envelope");
    let grpc_stale = grpc
        .execute(Request::new(rpc::FlatBufferFrame {
            payload: stale_bytes,
        }))
        .await
        .expect("stale gRPC request")
        .into_inner()
        .payload;
    assert_portable_error(
        &grpc_stale,
        api::ErrorCode::FailedPrecondition,
        STALE_INDEX_HANDLE_DETAIL_CODE,
        Some("target"),
    );
    let (status, body) = execute_http_json_raw(http_address, Bytes::from(stale_json)).await;
    assert_eq!(status, http::StatusCode::OK);
    let http_stale = response_json_to_flatbuffer(&body).expect("stale HTTP response");
    assert_eq!(
        portable_error_semantics(&grpc_stale),
        portable_error_semantics(&http_stale)
    );

    let stale_delete = delete_index_request_with_generation("reused", Some(&old_generation));
    let stale_delete_json =
        request_flatbuffer_to_json(&stale_delete).expect("canonical stale delete JSON");
    let (status, body) = execute_http_json_raw(http_address, Bytes::from(stale_delete_json)).await;
    assert_eq!(status, http::StatusCode::OK);
    assert_portable_error(
        &response_json_to_flatbuffer(&body).expect("stale delete response"),
        api::ErrorCode::FailedPrecondition,
        STALE_INDEX_HANDLE_DETAIL_CODE,
        Some("target"),
    );

    let current =
        upsert_request_with_generation("reused", "doc", "title", "current", Some(new_generation));
    let current_json =
        request_flatbuffer_to_json(&current).expect("canonical current request JSON");
    let (status, body) = execute_http_json_raw(http_address, Bytes::from(current_json)).await;
    assert_eq!(status, http::StatusCode::OK);
    assert_upsert(&response_json_to_flatbuffer(&body).expect("current handle response"));

    server.shutdown().await.expect("shutdown daemon");
    registry.close_all().expect("close registry");
}

#[test]
fn routes_two_different_indexes_without_cross_index_leakage_and_reopens_them() {
    let temp = TempDir::new("isolation");
    let registry = IndexRegistry::new(&temp.0).expect("registry");

    assert_lifecycle(
        &registry
            .dispatch(&create_request("products", "title"))
            .expect("create products"),
        wire::ResponsePayload::SearchKernel_Wire_CreateIndexResult,
    );
    assert_lifecycle(
        &registry
            .dispatch(&create_request("articles", "body"))
            .expect("create articles"),
        wire::ResponsePayload::SearchKernel_Wire_CreateIndexResult,
    );

    assert_upsert(
        &registry
            .dispatch(&upsert_request("products", "shared", "title", "red fox"))
            .expect("upsert products"),
    );
    assert_upsert(
        &registry
            .dispatch(&upsert_request("articles", "shared", "body", "blue whale"))
            .expect("upsert articles"),
    );

    assert_eq!(
        total_hits(
            &registry
                .dispatch(&search_request("products", "red"))
                .expect("search products red")
        ),
        1
    );
    assert_eq!(
        total_hits(
            &registry
                .dispatch(&search_request("products", "blue"))
                .expect("search products blue")
        ),
        0
    );
    assert_eq!(
        total_hits(
            &registry
                .dispatch(&search_request("articles", "blue"))
                .expect("search articles blue")
        ),
        1
    );
    assert_eq!(
        total_hits(
            &registry
                .dispatch(&search_request("articles", "red"))
                .expect("search articles red")
        ),
        0
    );

    registry.close_all().expect("close first registry");
    assert!(registry
        .loaded_targets()
        .expect("loaded targets")
        .is_empty());
    registry.close_all().expect("second close is a no-op");
    drop(registry);

    let reopened = IndexRegistry::new(&temp.0).expect("reopened registry");
    assert_lifecycle(
        &reopened
            .dispatch(&open_request(Some("products")))
            .expect("open products"),
        wire::ResponsePayload::SearchKernel_Wire_OpenIndexResult,
    );
    assert_lifecycle(
        &reopened
            .dispatch(&open_request(Some("articles")))
            .expect("open articles"),
        wire::ResponsePayload::SearchKernel_Wire_OpenIndexResult,
    );
    assert_eq!(
        total_hits(
            &reopened
                .dispatch(&search_request("products", "red"))
                .expect("reopened products search")
        ),
        1
    );
    assert_eq!(
        total_hits(
            &reopened
                .dispatch(&search_request("articles", "blue"))
                .expect("reopened articles search")
        ),
        1
    );
    reopened.close_all().expect("close reopened registry");
}

#[test]
fn delete_unloads_removes_persistence_and_allows_recreate() {
    let temp = TempDir::new("delete-index");
    let registry = IndexRegistry::new(&temp.0).expect("registry");
    assert_lifecycle(
        &registry
            .dispatch(&create_request("products", "title"))
            .expect("create products"),
        wire::ResponsePayload::SearchKernel_Wire_CreateIndexResult,
    );
    assert!(temp.0.join("products").is_dir());
    assert_lifecycle(
        &registry
            .dispatch(&delete_index_request("products"))
            .expect("delete products"),
        wire::ResponsePayload::SearchKernel_Wire_DeleteIndexResult,
    );
    assert!(registry
        .loaded_targets()
        .expect("loaded targets")
        .is_empty());
    assert!(!temp.0.join("products").exists());
    assert_portable_error(
        &registry
            .dispatch(&open_request(Some("products")))
            .expect("deleted open"),
        api::ErrorCode::NotFound,
        INDEX_NOT_FOUND_DETAIL_CODE,
        None,
    );
    assert_lifecycle(
        &registry
            .dispatch(&create_request("products", "title"))
            .expect("recreate products"),
        wire::ResponsePayload::SearchKernel_Wire_CreateIndexResult,
    );
    registry.close_all().expect("close recreated registry");
}

#[test]
fn delete_persisted_but_unloaded_index() {
    let temp = TempDir::new("delete-unloaded");
    let first = IndexRegistry::new(&temp.0).expect("first registry");
    assert_lifecycle(
        &first
            .dispatch(&create_request("products", "title"))
            .expect("create products"),
        wire::ResponsePayload::SearchKernel_Wire_CreateIndexResult,
    );
    first.close_all().expect("close first registry");
    drop(first);
    let second = IndexRegistry::new(&temp.0).expect("second registry");
    assert_lifecycle(
        &second
            .dispatch(&delete_index_request("products"))
            .expect("delete persisted products"),
        wire::ResponsePayload::SearchKernel_Wire_DeleteIndexResult,
    );
    assert!(!temp.0.join("products").exists());
    assert_portable_error(
        &second
            .dispatch(&delete_index_request("products"))
            .expect("second delete"),
        api::ErrorCode::NotFound,
        INDEX_NOT_FOUND_DETAIL_CODE,
        None,
    );
}

#[test]
fn delete_rejects_storage_owned_by_another_runtime() {
    let temp = TempDir::new("delete-owned");
    let path = temp.0.join("products");
    let owner = searchkernel::Index::create(
        &path,
        searchkernel::IndexDefinition::new().field(
            searchkernel::Field::text("title")
                .stored(true)
                .searchable(true),
        ),
    )
    .expect("create externally owned index");

    let registry = IndexRegistry::new(&temp.0).expect("registry");
    assert_portable_error(
        &registry
            .dispatch(&delete_index_request("products"))
            .expect("delete owned target"),
        api::ErrorCode::Conflict,
        "index_in_use",
        None,
    );
    assert!(path.is_dir(), "failed delete must preserve owned storage");

    owner.close().expect("close external owner");
    assert_lifecycle(
        &registry
            .dispatch(&delete_index_request("products"))
            .expect("delete after owner close"),
        wire::ResponsePayload::SearchKernel_Wire_DeleteIndexResult,
    );
    assert!(!path.exists());
}

#[test]
fn open_rejects_storage_owned_by_another_runtime_as_conflict() {
    let temp = TempDir::new("open-owned");
    let path = temp.0.join("products");
    let owner = searchkernel::Index::create(
        &path,
        searchkernel::IndexDefinition::new().field(
            searchkernel::Field::text("title")
                .stored(true)
                .searchable(true),
        ),
    )
    .expect("create externally owned index");

    let registry = IndexRegistry::new(&temp.0).expect("registry");
    assert_portable_error(
        &registry
            .dispatch(&open_request(Some("products")))
            .expect("open owned target"),
        api::ErrorCode::Conflict,
        searchkernel_ir::INDEX_IN_USE_DETAIL_CODE,
        None,
    );
    assert!(path.is_dir(), "failed open must preserve owned storage");

    owner.close().expect("close external owner");
    assert_lifecycle(
        &registry
            .dispatch(&open_request(Some("products")))
            .expect("open after owner close"),
        wire::ResponsePayload::SearchKernel_Wire_OpenIndexResult,
    );
    registry.close_all().expect("close registry");
}

#[test]
fn concurrent_same_name_create_has_one_winner() {
    let temp = TempDir::new("create-race");
    let registry = Arc::new(IndexRegistry::new(&temp.0).expect("registry"));
    let request = Arc::new(create_request("products", "title"));

    let workers = (0..6)
        .map(|_| {
            let registry = registry.clone();
            let request = request.clone();
            thread::spawn(move || registry.dispatch(&request).expect("dispatch create"))
        })
        .collect::<Vec<_>>();

    let mut created = 0;
    let mut already_exists = 0;
    for worker in workers {
        let bytes = worker.join().expect("create worker");
        let envelope = response(&bytes);
        match envelope.payload_type() {
            wire::ResponsePayload::SearchKernel_Wire_CreateIndexResult => created += 1,
            wire::ResponsePayload::SearchKernel_Wire_Error => {
                let error = envelope
                    .payload_as_search_kernel_wire_error()
                    .expect("error payload");
                assert_eq!(error.code(), api::ErrorCode::AlreadyExists);
                already_exists += 1;
            }
            other => panic!("unexpected create-race response: {other:?}"),
        }
    }

    assert_eq!(created, 1);
    assert_eq!(already_exists, 5);
    registry.close_all().expect("close registry");
}

#[cfg(unix)]
#[test]
fn blocked_lifecycle_transition_does_not_stall_other_targets() {
    use std::os::fd::AsRawFd;

    let temp = TempDir::new("per-target-transition");
    let lock_path = temp.0.join(".products.searchkernel.lock");
    let lock_file = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&lock_path)
        .expect("open external lifecycle lock");
    assert_eq!(
        unsafe { libc::flock(lock_file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
        0,
        "acquire external lifecycle lock"
    );

    let registry = Arc::new(IndexRegistry::new(&temp.0).expect("registry"));
    let blocked_registry = registry.clone();
    let blocked = thread::spawn(move || {
        blocked_registry
            .dispatch(&create_request("products", "title"))
            .expect("blocked products create")
    });

    // Give the products transition time to enter the native lifecycle-lock
    // retry loop. A daemon-wide registry mutex would now stall every target.
    thread::sleep(std::time::Duration::from_millis(100));

    let (sender, receiver) = std::sync::mpsc::channel();
    let independent_registry = registry.clone();
    let independent = thread::spawn(move || {
        let response = independent_registry
            .dispatch(&create_request("articles", "body"))
            .expect("independent articles create");
        sender.send(response).expect("send independent response");
    });
    // Native index creation can contend with the rest of this real-engine suite.
    // Keep enough headroom for a slow but independent target. A daemon-wide
    // registry lock still fails deterministically because the products lifecycle
    // lock remains held until this unrelated request completes.
    let articles = receiver
        .recv_timeout(std::time::Duration::from_secs(10))
        .expect("unrelated target lifecycle must not wait for products");
    assert_lifecycle(
        &articles,
        wire::ResponsePayload::SearchKernel_Wire_CreateIndexResult,
    );

    assert_eq!(
        unsafe { libc::flock(lock_file.as_raw_fd(), libc::LOCK_UN) },
        0,
        "release external lifecycle lock"
    );
    assert_lifecycle(
        &blocked.join().expect("join blocked creator"),
        wire::ResponsePayload::SearchKernel_Wire_CreateIndexResult,
    );
    independent.join().expect("join independent creator");
    registry.close_all().expect("close registry");
}

#[cfg(unix)]
#[test]
fn close_all_waits_for_in_flight_create_and_prevents_republication() {
    use std::os::fd::AsRawFd;

    let temp = TempDir::new("close-create-race");
    let lock_path = temp.0.join(".products.searchkernel.lock");
    let lock_file = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&lock_path)
        .expect("open external lifecycle lock");
    assert_eq!(
        unsafe { libc::flock(lock_file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
        0,
        "acquire external lifecycle lock"
    );

    let registry = Arc::new(IndexRegistry::new(&temp.0).expect("registry"));
    let creating_registry = registry.clone();
    let creator = thread::spawn(move || {
        creating_registry
            .dispatch(&create_request("products", "title"))
            .expect("dispatch create")
    });

    thread::sleep(std::time::Duration::from_millis(100));

    let (closed_tx, closed_rx) = std::sync::mpsc::channel();
    let closing_registry = registry.clone();
    let closer = thread::spawn(move || {
        let result = closing_registry.close_all();
        closed_tx.send(result).expect("send close result");
    });

    assert!(
        closed_rx
            .recv_timeout(std::time::Duration::from_millis(200))
            .is_err(),
        "close_all must wait for the in-flight create"
    );

    assert_eq!(
        unsafe { libc::flock(lock_file.as_raw_fd(), libc::LOCK_UN) },
        0,
        "release external lifecycle lock"
    );
    assert_lifecycle(
        &creator.join().expect("join creator"),
        wire::ResponsePayload::SearchKernel_Wire_CreateIndexResult,
    );
    closed_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("close result after create")
        .expect("close registry");
    closer.join().expect("join closer");

    assert!(registry
        .loaded_targets()
        .expect("loaded targets")
        .is_empty());
    assert_eq!(
        registry.dispatch(&open_request(Some("products"))),
        Err(searchkerneld::RegistryDispatchError::Closed),
        "closed registry must not reopen a target after shutdown"
    );
}

#[test]
fn concurrent_create_and_open_are_linearizable() {
    let temp = TempDir::new("create-open-race");
    let registry = Arc::new(IndexRegistry::new(&temp.0).expect("registry"));
    let barrier = Arc::new(Barrier::new(2));

    let creator = {
        let registry = registry.clone();
        let barrier = barrier.clone();
        thread::spawn(move || {
            barrier.wait();
            registry
                .dispatch(&create_request("products", "title"))
                .expect("dispatch create")
        })
    };
    let opener = {
        let registry = registry.clone();
        let barrier = barrier.clone();
        thread::spawn(move || {
            barrier.wait();
            registry
                .dispatch(&open_request(Some("products")))
                .expect("dispatch open")
        })
    };

    let create_bytes = creator.join().expect("creator");
    assert_eq!(
        response(&create_bytes).payload_type(),
        wire::ResponsePayload::SearchKernel_Wire_CreateIndexResult
    );

    let open_bytes = opener.join().expect("opener");
    let open_envelope = response(&open_bytes);
    match open_envelope.payload_type() {
        wire::ResponsePayload::SearchKernel_Wire_OpenIndexResult => {}
        wire::ResponsePayload::SearchKernel_Wire_Error => {
            let error = open_envelope
                .payload_as_search_kernel_wire_error()
                .expect("open error payload");
            assert_eq!(
                error.code(),
                api::ErrorCode::NotFound,
                "an open that linearizes before create may only report NotFound"
            );
        }
        other => panic!("unexpected create/open race response: {other:?}"),
    }

    assert_eq!(
        registry.loaded_targets().expect("loaded targets"),
        vec!["products".to_owned()]
    );
    assert_lifecycle(
        &registry
            .dispatch(&open_request(Some("products")))
            .expect("final open"),
        wire::ResponsePayload::SearchKernel_Wire_OpenIndexResult,
    );
    registry.close_all().expect("close registry");
}

#[test]
fn rejects_non_directory_target_entry() {
    let temp = TempDir::new("non-directory-target");
    fs::write(temp.0.join("products"), b"not an index directory").expect("write target file");

    let registry = IndexRegistry::new(&temp.0).expect("registry");
    let bytes = registry
        .dispatch(&open_request(Some("products")))
        .expect("semantic lifecycle response");
    let envelope = response(&bytes);
    assert_eq!(
        envelope.payload_type(),
        wire::ResponsePayload::SearchKernel_Wire_Error
    );
    let error = envelope
        .payload_as_search_kernel_wire_error()
        .expect("error payload");
    assert_eq!(error.code(), api::ErrorCode::FailedPrecondition);
    assert_eq!(
        error.details().expect("error details").get(0).code(),
        Some("unsafe_index_path")
    );
}

#[cfg(unix)]
#[test]
fn rejects_symlinked_target_directory_without_following_it() {
    use std::os::unix::fs::symlink;

    let temp = TempDir::new("symlink-target");
    let outside = TempDir::new("symlink-outside");
    symlink(&outside.0, temp.0.join("products")).expect("create target symlink");

    let registry = IndexRegistry::new(&temp.0).expect("registry");
    let bytes = registry
        .dispatch(&open_request(Some("products")))
        .expect("semantic lifecycle response");
    let envelope = response(&bytes);
    assert_eq!(
        envelope.payload_type(),
        wire::ResponsePayload::SearchKernel_Wire_Error
    );
    let error = envelope
        .payload_as_search_kernel_wire_error()
        .expect("error payload");
    assert_eq!(error.code(), api::ErrorCode::FailedPrecondition);
    assert_eq!(
        error.details().expect("error details").get(0).code(),
        Some("unsafe_index_path")
    );
    assert!(
        fs::read_dir(&outside.0)
            .expect("read outside directory")
            .next()
            .is_none(),
        "daemon must not follow a target symlink outside its data root"
    );
}

#[test]
fn daemon_control_errors_use_the_canonical_portable_error_contract() {
    let temp = TempDir::new("portable-control-errors");
    let registry = IndexRegistry::new(&temp.0).expect("registry");

    assert_portable_error(
        &registry
            .dispatch(&open_request(None))
            .expect("missing target is a semantic response"),
        api::ErrorCode::InvalidArgument,
        MISSING_INDEX_TARGET_DETAIL_CODE,
        Some(TARGET_PATH),
    );

    assert_portable_error(
        &registry
            .dispatch(&open_request(Some("../escape")))
            .expect("invalid target is a semantic response"),
        api::ErrorCode::InvalidArgument,
        INVALID_INDEX_TARGET_DETAIL_CODE,
        Some(TARGET_NAME_PATH),
    );

    assert_portable_error(
        &registry
            .dispatch(&create_request_without_definition("products"))
            .expect("missing definition is a semantic response"),
        api::ErrorCode::InvalidArgument,
        MISSING_INDEX_DEFINITION_DETAIL_CODE,
        Some(CREATE_INDEX_DEFINITION_PATH),
    );

    assert_lifecycle(
        &registry
            .dispatch(&create_request("products", "title"))
            .expect("create products"),
        wire::ResponsePayload::SearchKernel_Wire_CreateIndexResult,
    );
    assert_portable_error(
        &registry
            .dispatch(&create_request("products", "title"))
            .expect("create collision is a semantic response"),
        api::ErrorCode::AlreadyExists,
        INDEX_ALREADY_EXISTS_DETAIL_CODE,
        None,
    );

    assert_portable_error(
        &registry
            .dispatch(&open_request(Some("missing")))
            .expect("open miss is a semantic response"),
        api::ErrorCode::NotFound,
        INDEX_NOT_FOUND_DETAIL_CODE,
        None,
    );

    registry.close_all().expect("close registry");
}

#[test]
fn rejects_missing_and_path_like_targets_before_mapping_the_data_root() {
    let temp = TempDir::new("invalid-targets");
    let registry = IndexRegistry::new(&temp.0).expect("registry");

    for invalid in [
        "",
        ".",
        "..",
        "../escape",
        "/absolute",
        "nested/path",
        r"nested\path",
        "dot.name",
        "Upper",
        "café",
    ] {
        assert_portable_error(
            &registry
                .dispatch(&open_request(Some(invalid)))
                .expect("invalid target is a semantic response"),
            api::ErrorCode::InvalidArgument,
            INVALID_INDEX_TARGET_DETAIL_CODE,
            Some(TARGET_NAME_PATH),
        );
    }

    assert!(
        fs::read_dir(&temp.0)
            .expect("read data root")
            .next()
            .is_none(),
        "invalid targets must not create filesystem entries"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn grpc_and_http_json_preserve_portable_control_errors() {
    let temp = TempDir::new("portable-error-transports");
    let registry = Arc::new(IndexRegistry::new(&temp.0).expect("registry"));
    let server = start_multi(
        ServerConfig::new("127.0.0.1:0".parse().expect("gRPC address"))
            .with_web_listen_addr("127.0.0.1:0".parse().expect("HTTP address")),
        registry.clone(),
    )
    .await
    .expect("start multi-index daemon");

    let grpc_address = server.tcp_addr().expect("gRPC listener");
    let http_address = server.web_tcp_addr().expect("HTTP listener");
    let mut grpc =
        rpc::search_kernel_client::SearchKernelClient::connect(format!("http://{grpc_address}"))
            .await
            .expect("connect gRPC client");

    let grpc_missing = grpc
        .execute(Request::new(rpc::FlatBufferFrame {
            payload: open_request(None),
        }))
        .await
        .expect("missing target must remain a SearchKernel response")
        .into_inner()
        .payload;
    assert_portable_error(
        &grpc_missing,
        api::ErrorCode::InvalidArgument,
        MISSING_INDEX_TARGET_DETAIL_CODE,
        Some(TARGET_PATH),
    );

    let http_invalid = execute_http_json(http_address, &open_request(Some("../escape"))).await;
    assert_portable_error(
        &http_invalid,
        api::ErrorCode::InvalidArgument,
        INVALID_INDEX_TARGET_DETAIL_CODE,
        Some(TARGET_NAME_PATH),
    );

    server.shutdown().await.expect("shutdown daemon");
    registry.close_all().expect("close registry");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn direct_grpc_and_http_share_lifecycle_semantics() {
    let temp = TempDir::new("lifecycle-semantic-parity");
    let registry = Arc::new(IndexRegistry::new(&temp.0).expect("registry"));
    let server = start_multi(
        ServerConfig::new("127.0.0.1:0".parse().expect("gRPC address"))
            .with_web_listen_addr("127.0.0.1:0".parse().expect("HTTP address")),
        registry.clone(),
    )
    .await
    .expect("start multi-index daemon");
    let grpc_address = server.tcp_addr().expect("gRPC listener");
    let http_address = server.web_tcp_addr().expect("HTTP listener");
    let mut grpc =
        rpc::search_kernel_client::SearchKernelClient::connect(format!("http://{grpc_address}"))
            .await
            .expect("connect gRPC client");

    let mut invalid_targets = vec![
        String::new(),
        "Products".to_owned(),
        "../products".to_owned(),
        "products/v2".to_owned(),
        "a".repeat(129),
    ];
    for invalid in invalid_targets.drain(..) {
        for request in [
            open_request(Some(&invalid)),
            create_request(&invalid, "title"),
            delete_index_request(&invalid),
        ] {
            let expected = ir_portable_error_semantics(resolved_ir_request_error(&request));
            let direct = registry
                .dispatch(&request)
                .expect("direct semantic response");
            let grpc_response = grpc
                .execute(Request::new(rpc::FlatBufferFrame {
                    payload: request.clone(),
                }))
                .await
                .expect("gRPC semantic response")
                .into_inner()
                .payload;
            let http = execute_http_json(http_address, &request).await;
            assert_eq!(
                portable_error_semantics(&direct),
                expected,
                "direct invalid target {invalid:?} must match IR semantics"
            );
            assert_eq!(
                portable_error_semantics(&grpc_response),
                expected,
                "gRPC invalid target {invalid:?} must match IR semantics"
            );
            assert_eq!(
                portable_error_semantics(&http),
                expected,
                "HTTP invalid target {invalid:?} must match IR semantics"
            );
        }
    }

    let missing = open_request(None);
    let missing_expected = ir_portable_error_semantics(resolved_ir_request_error(&missing));
    let direct = registry.dispatch(&missing).expect("direct missing target");
    let grpc_missing = grpc
        .execute(Request::new(rpc::FlatBufferFrame {
            payload: missing.clone(),
        }))
        .await
        .expect("gRPC missing target")
        .into_inner()
        .payload;
    let http_missing = execute_http_json(http_address, &missing).await;
    assert_eq!(portable_error_semantics(&direct), missing_expected);
    assert_eq!(portable_error_semantics(&grpc_missing), missing_expected);
    assert_eq!(portable_error_semantics(&http_missing), missing_expected);

    let boundary_target = "a".repeat(128);
    assert_lifecycle(
        &registry
            .dispatch(&create_request_with_definition_name(
                &boundary_target,
                "title",
                "boundary_schema",
            ))
            .expect("create exactly 128-byte target"),
        wire::ResponsePayload::SearchKernel_Wire_CreateIndexResult,
    );
    assert_lifecycle(
        &grpc
            .execute(Request::new(rpc::FlatBufferFrame {
                payload: open_request(Some(&boundary_target)),
            }))
            .await
            .expect("open boundary target via gRPC")
            .into_inner()
            .payload,
        wire::ResponsePayload::SearchKernel_Wire_OpenIndexResult,
    );
    assert_lifecycle(
        &execute_http_json(http_address, &delete_index_request(&boundary_target)).await,
        wire::ResponsePayload::SearchKernel_Wire_DeleteIndexResult,
    );

    assert_lifecycle(
        &registry
            .dispatch(&create_request_with_definition_name(
                "already-there",
                "title",
                "already_there",
            ))
            .expect("seed duplicate target"),
        wire::ResponsePayload::SearchKernel_Wire_CreateIndexResult,
    );
    let already_exists_expected =
        ir_portable_error_semantics(searchkernel_ir::already_exists_error());
    for request in [
        create_request_with_definition_name("already-there", "title", "already_there"),
        create_request_with_definition_name("already-there", "title", "already_there"),
        create_request_with_definition_name("already-there", "title", "already_there"),
    ] {
        let direct = registry
            .dispatch(&request)
            .expect("direct duplicate create");
        let grpc_response = grpc
            .execute(Request::new(rpc::FlatBufferFrame {
                payload: request.clone(),
            }))
            .await
            .expect("gRPC duplicate create")
            .into_inner()
            .payload;
        let http = execute_http_json(http_address, &request).await;
        assert_eq!(portable_error_semantics(&direct), already_exists_expected);
        assert_eq!(
            portable_error_semantics(&grpc_response),
            already_exists_expected
        );
        assert_eq!(portable_error_semantics(&http), already_exists_expected);
    }

    let not_found_expected = ir_portable_error_semantics(searchkernel_ir::not_found_error());
    for request in [
        open_request(Some("not-there")),
        delete_index_request("not-there"),
    ] {
        let direct = registry
            .dispatch(&request)
            .expect("direct missing lifecycle target");
        let grpc_response = grpc
            .execute(Request::new(rpc::FlatBufferFrame {
                payload: request.clone(),
            }))
            .await
            .expect("gRPC missing lifecycle target")
            .into_inner()
            .payload;
        let http = execute_http_json(http_address, &request).await;
        assert_eq!(portable_error_semantics(&direct), not_found_expected);
        assert_eq!(portable_error_semantics(&grpc_response), not_found_expected);
        assert_eq!(portable_error_semantics(&http), not_found_expected);
    }

    let owned_path = temp.0.join("external_owner");
    let owner = searchkernel::Index::create(
        &owned_path,
        searchkernel::IndexDefinition::new().field(
            searchkernel::Field::text("title")
                .stored(true)
                .searchable(true),
        ),
    )
    .expect("create externally owned index");
    let conflict_expected = ir_portable_error_semantics(searchkernel_ir::index_in_use_error());
    for request in [
        open_request(Some("external_owner")),
        delete_index_request("external_owner"),
    ] {
        let direct = registry
            .dispatch(&request)
            .expect("direct active-owner failure");
        let grpc_response = grpc
            .execute(Request::new(rpc::FlatBufferFrame {
                payload: request.clone(),
            }))
            .await
            .expect("gRPC active-owner failure")
            .into_inner()
            .payload;
        let http = execute_http_json(http_address, &request).await;
        assert_eq!(portable_error_semantics(&direct), conflict_expected);
        assert_eq!(portable_error_semantics(&grpc_response), conflict_expected);
        assert_eq!(portable_error_semantics(&http), conflict_expected);
    }
    owner.close().expect("close external owner");

    server.shutdown().await.expect("shutdown daemon");
    registry.close_all().expect("close registry");
}

#[derive(Debug, serde::Deserialize, serde::Serialize, PartialEq)]
struct RemoteLifecycleDocument {
    title: String,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
// SKC: daemon.multi_index_lifecycle/basic kinds=ergonomic,e2e runtime=grpc
async fn rust_remote_client_targets_independent_indexes_across_restart_and_delete() {
    let temp = TempDir::new("rust-remote-client-lifecycle");
    let registry = Arc::new(IndexRegistry::new(&temp.0).expect("registry"));
    let server = start_multi(
        ServerConfig::new("127.0.0.1:0".parse().expect("gRPC address")),
        registry.clone(),
    )
    .await
    .expect("start real multi-index daemon");
    let endpoint = server.tcp_addr().expect("gRPC listener").to_string();
    let client = RemoteClient::connect(&endpoint).expect("connect Rust remote client");
    let invalid = client
        .open_index("../invalid")
        .err()
        .expect("daemon rejects an invalid logical target");
    assert_eq!(invalid.code(), RustErrorCode::InvalidArgument);
    assert_eq!(invalid.details()[0].code, INVALID_INDEX_TARGET_DETAIL_CODE);
    let definition = |name: &str| {
        IndexDefinition::new()
            .name(name)
            .field(Field::text("title").stored(true).searchable(true))
            .default_search_fields(["title"])
    };

    let a = client
        .create_index("rust-a", definition("schema_a"))
        .expect("create A");
    let b = client
        .create_index("rust-b", definition("schema_b"))
        .expect("create B");
    let duplicate = client
        .create_index("rust-a", definition("schema_a"))
        .err()
        .expect("duplicate create error");
    assert_eq!(duplicate.code(), RustErrorCode::AlreadyExists);
    assert_eq!(
        duplicate.details()[0].code,
        INDEX_ALREADY_EXISTS_DETAIL_CODE
    );
    a.insert(
        "a-doc",
        &RemoteLifecycleDocument {
            title: "keyboard alpha".into(),
        },
    )
    .expect("insert A document");
    b.insert(
        "b-doc",
        &RemoteLifecycleDocument {
            title: "keyboard beta".into(),
        },
    )
    .expect("insert B document");
    a.close().expect("close A handle");
    b.close().expect("close B handle");
    drop(client);

    server.shutdown().await.expect("shutdown first daemon");
    registry.close_all().expect("close first registry");

    let registry = Arc::new(IndexRegistry::new(&temp.0).expect("reopen persisted registry"));
    let server = start_multi(
        ServerConfig::new("127.0.0.1:0".parse().expect("gRPC address")),
        registry.clone(),
    )
    .await
    .expect("restart multi-index daemon");
    let endpoint = server.tcp_addr().expect("gRPC listener").to_string();
    let client = RemoteClient::connect(&endpoint).expect("reconnect Rust remote client");

    let a = client.open_index("rust-a").expect("open A after restart");
    let b = client.open_index("rust-b").expect("open B after restart");
    assert_eq!(
        a.get::<RemoteLifecycleDocument>("a-doc").expect("get A"),
        Some(RemoteLifecycleDocument {
            title: "keyboard alpha".into()
        })
    );
    assert_eq!(
        b.get::<RemoteLifecycleDocument>("b-doc").expect("get B"),
        Some(RemoteLifecycleDocument {
            title: "keyboard beta".into()
        })
    );
    assert_eq!(
        a.get::<RemoteLifecycleDocument>("b-doc")
            .expect("A cannot read B"),
        None
    );
    assert_eq!(
        b.get::<RemoteLifecycleDocument>("a-doc")
            .expect("B cannot read A"),
        None
    );
    let a_hits = a
        .search("keyboard alpha")
        .execute::<RemoteLifecycleDocument>()
        .expect("search A");
    let b_hits = b
        .search("keyboard beta")
        .execute::<RemoteLifecycleDocument>()
        .expect("search B");
    assert_eq!(
        a_hits
            .hits
            .iter()
            .map(|hit| hit.id.as_str())
            .collect::<Vec<_>>(),
        ["a-doc"]
    );
    assert_eq!(
        b_hits
            .hits
            .iter()
            .map(|hit| hit.id.as_str())
            .collect::<Vec<_>>(),
        ["b-doc"]
    );
    a.close().expect("close reopened A");
    b.close().expect("close reopened B");

    // A handle opened through one RemoteClient must never silently retarget a
    // later incarnation created through another client.
    let stale_a = client
        .open_index("rust-a")
        .expect("open A handle before cross-client delete");
    let other_client =
        RemoteClient::connect(&endpoint).expect("connect independent Rust remote client");
    other_client.delete_index("rust-a").expect("delete A");

    let missing = stale_a
        .get::<RemoteLifecycleDocument>("a-doc")
        .expect_err("deleted target must no longer route the stale handle");
    assert_eq!(missing.code(), RustErrorCode::NotFound);

    let missing = client
        .open_index("rust-a")
        .err()
        .expect("deleted target error");
    assert_eq!(missing.code(), RustErrorCode::NotFound);
    assert_eq!(missing.details()[0].code, INDEX_NOT_FOUND_DETAIL_CODE);
    let repeated_delete = other_client
        .delete_index("rust-a")
        .expect_err("repeated delete error");
    assert_eq!(repeated_delete.code(), RustErrorCode::NotFound);
    assert_eq!(
        repeated_delete.details()[0].code,
        INDEX_NOT_FOUND_DETAIL_CODE
    );

    let replacement_a = other_client
        .create_index("rust-a", definition("schema_a_recreated"))
        .expect("recreate A through another client");
    let stale_error = stale_a
        .get::<RemoteLifecycleDocument>("a-doc")
        .expect_err("old generation must not address the recreated target");
    assert_eq!(stale_error.code(), RustErrorCode::FailedPrecondition);
    assert_eq!(
        stale_error.details()[0].code,
        STALE_INDEX_HANDLE_DETAIL_CODE
    );
    stale_a.close().expect("close stale A handle");
    replacement_a.close().expect("close replacement A");
    other_client
        .delete_index("rust-a")
        .expect("delete recreated A");
    drop(other_client);

    let b = client.open_index("rust-b").expect("B survives A deletion");
    assert_eq!(
        b.get::<RemoteLifecycleDocument>("b-doc")
            .expect("B document remains"),
        Some(RemoteLifecycleDocument {
            title: "keyboard beta".into()
        })
    );
    b.close().expect("close surviving B");

    // A successful delete through the same RemoteClient additionally closes
    // its local sessions immediately.
    let stale_b = client
        .open_index("rust-b")
        .expect("open B handle that local delete must invalidate");
    client.delete_index("rust-b").expect("delete B");
    let stale_error = stale_b
        .describe()
        .expect_err("same-client delete must close live local handles");
    assert_eq!(stale_error.code(), RustErrorCode::IndexClosed);

    drop(client);
    server.shutdown().await.expect("shutdown restarted daemon");
    registry.close_all().expect("close restarted registry");
}
