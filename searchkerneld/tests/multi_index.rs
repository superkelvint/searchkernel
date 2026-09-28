#![cfg(feature = "native")]

use flatbuffers::root;
use searchkernel_protocol::{protocol::v2 as wire, v2 as api, PROTOCOL_VERSION};
use searchkerneld::{IndexRegistry, RegistryDispatchError};
use std::{
    fs,
    path::PathBuf,
    sync::{Arc, Barrier},
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

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
    let name = builder.create_string(name);
    api::IndexTarget::create(builder, &api::IndexTargetArgs { name: Some(name) })
}

fn create_request(target_name: &str, field_name: &str) -> Vec<u8> {
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
    let definition_name = builder.create_string(target_name);
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
            payload_type: wire::RequestPayload::SearchKernel_V2_CreateIndexRequest,
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
            payload_type: wire::RequestPayload::SearchKernel_V2_OpenIndexRequest,
            payload: Some(payload.as_union_value()),
        },
    );
    wire::finish_request_envelope_buffer(&mut builder, envelope);
    builder.finished_data().to_vec()
}

fn upsert_request(target_name: &str, id: &str, field_name: &str, text: &str) -> Vec<u8> {
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
    let target = target(&mut builder, target_name);
    let envelope = wire::RequestEnvelope::create(
        &mut builder,
        &wire::RequestEnvelopeArgs {
            protocol_version: PROTOCOL_VERSION,
            target: Some(target),
            payload_type: wire::RequestPayload::SearchKernel_V2_UpsertRequest,
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
            payload_type: wire::RequestPayload::SearchKernel_V2_SearchRequest,
            payload: Some(payload.as_union_value()),
        },
    );
    wire::finish_request_envelope_buffer(&mut builder, envelope);
    builder.finished_data().to_vec()
}

fn response(bytes: &[u8]) -> wire::ResponseEnvelope<'_> {
    root::<wire::ResponseEnvelope<'_>>(bytes).expect("response envelope")
}

fn assert_lifecycle(bytes: &[u8], kind: wire::ResponsePayload) {
    assert_eq!(response(bytes).payload_type(), kind);
}

fn assert_upsert(bytes: &[u8]) {
    assert_eq!(
        response(bytes).payload_type(),
        wire::ResponsePayload::SearchKernel_V2_UpsertResult
    );
}

fn total_hits(bytes: &[u8]) -> u64 {
    let envelope = response(bytes);
    assert_eq!(
        envelope.payload_type(),
        wire::ResponsePayload::SearchKernel_V2_SearchResult
    );
    envelope
        .payload_as_search_kernel_v2_search_result()
        .expect("search result")
        .total_hits()
        .expect("total hits")
        .value()
}

#[test]
fn routes_two_different_indexes_without_cross_index_leakage_and_reopens_them() {
    let temp = TempDir::new("isolation");
    let registry = IndexRegistry::new(&temp.0).expect("registry");

    assert_lifecycle(
        &registry
            .dispatch(&create_request("products", "title"))
            .expect("create products"),
        wire::ResponsePayload::SearchKernel_V2_CreateIndexResult,
    );
    assert_lifecycle(
        &registry
            .dispatch(&create_request("articles", "body"))
            .expect("create articles"),
        wire::ResponsePayload::SearchKernel_V2_CreateIndexResult,
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
        wire::ResponsePayload::SearchKernel_V2_OpenIndexResult,
    );
    assert_lifecycle(
        &reopened
            .dispatch(&open_request(Some("articles")))
            .expect("open articles"),
        wire::ResponsePayload::SearchKernel_V2_OpenIndexResult,
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
            wire::ResponsePayload::SearchKernel_V2_CreateIndexResult => created += 1,
            wire::ResponsePayload::SearchKernel_V2_Error => {
                let error = envelope
                    .payload_as_search_kernel_v2_error()
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
        wire::ResponsePayload::SearchKernel_V2_CreateIndexResult
    );

    let open_bytes = opener.join().expect("opener");
    let open_envelope = response(&open_bytes);
    match open_envelope.payload_type() {
        wire::ResponsePayload::SearchKernel_V2_OpenIndexResult => {}
        wire::ResponsePayload::SearchKernel_V2_Error => {
            let error = open_envelope
                .payload_as_search_kernel_v2_error()
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
        wire::ResponsePayload::SearchKernel_V2_OpenIndexResult,
    );
    registry.close_all().expect("close registry");
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
        wire::ResponsePayload::SearchKernel_V2_Error
    );
    let error = envelope
        .payload_as_search_kernel_v2_error()
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
fn rejects_missing_and_path_like_targets_before_mapping_the_data_root() {
    let temp = TempDir::new("invalid-targets");
    let registry = IndexRegistry::new(&temp.0).expect("registry");

    assert_eq!(
        registry.dispatch(&open_request(None)),
        Err(RegistryDispatchError::MissingTarget)
    );

    for invalid in ["../escape", "/absolute", "nested/path", "dot.name", "Upper"] {
        assert!(
            matches!(
                registry.dispatch(&open_request(Some(invalid))),
                Err(RegistryDispatchError::InvalidTarget(_))
            ),
            "target {invalid:?} should fail closed"
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
