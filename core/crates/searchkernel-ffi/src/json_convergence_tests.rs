use crate::test_support::{build_component_transport_case, RecordingBackend, TransportCase};
use crate::{dispatch, handle};
use searchkernel_ir::{api, IndexContext};
use searchkernel_json::{
    request_flatbuffer_to_json, request_json_to_flatbuffer, response_flatbuffer_to_json,
    response_json_to_flatbuffer,
};
use searchkernel_testkit::{ExpectedCall, ScriptedBackend, ScriptedResult};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn context() -> Arc<IndexContext> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let name = builder.create_string("title");
    let stored = api::OptionalBool::new(true);
    let searchable = api::OptionalBool::new(true);
    let analyzer_name = builder.create_string("standard");
    let analyzer = api::Analyzer::create(
        &mut builder,
        &api::AnalyzerArgs {
            name: Some(analyzer_name),
            version: 1,
            options: None,
        },
    );
    let field = api::FieldDefinition::create(
        &mut builder,
        &api::FieldDefinitionArgs {
            name: Some(name),
            kind: api::FieldKind::Text,
            cardinality: api::FieldCardinality::Single,
            stored: Some(&stored),
            searchable: Some(&searchable),
            analyzer: Some(analyzer),
            ..Default::default()
        },
    );
    let extra_fields = [
        (
            "brand",
            api::FieldKind::Keyword,
            api::FieldCardinality::Single,
        ),
        (
            "sku",
            api::FieldKind::Keyword,
            api::FieldCardinality::Single,
        ),
        (
            "price",
            api::FieldKind::Float64,
            api::FieldCardinality::Single,
        ),
        (
            "popularity",
            api::FieldKind::Int64,
            api::FieldCardinality::Single,
        ),
        (
            "in_stock",
            api::FieldKind::Bool,
            api::FieldCardinality::Single,
        ),
        (
            "image_url",
            api::FieldKind::Keyword,
            api::FieldCardinality::Single,
        ),
        (
            "description",
            api::FieldKind::Text,
            api::FieldCardinality::Single,
        ),
        (
            "category",
            api::FieldKind::Keyword,
            api::FieldCardinality::Single,
        ),
        (
            "tags",
            api::FieldKind::Keyword,
            api::FieldCardinality::Multi,
        ),
    ];
    let mut field_offsets = vec![field];
    for (name, kind, cardinality) in extra_fields {
        let name = builder.create_string(name);
        field_offsets.push(api::FieldDefinition::create(
            &mut builder,
            &api::FieldDefinitionArgs {
                name: Some(name),
                kind,
                cardinality,
                ..Default::default()
            },
        ));
    }
    let fields = builder.create_vector(&field_offsets);
    let definition = api::IndexDefinition::create(
        &mut builder,
        &api::IndexDefinitionArgs {
            fields: Some(fields),
            ..Default::default()
        },
    );
    builder.finish(definition, None);
    let bytes = builder.finished_data().to_vec();
    Arc::new(
        IndexContext::from_api(flatbuffers::root::<api::IndexDefinition>(&bytes).unwrap()).unwrap(),
    )
}

fn request_fixtures() -> Vec<(String, PathBuf)> {
    let directory =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../searchkernel-json/tests/fixtures/requests");
    let mut paths = std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "skrq")
        })
        .collect::<Vec<_>>();
    paths.sort();
    paths
        .into_iter()
        .map(|path| {
            let name = path.file_stem().unwrap().to_string_lossy().into_owned();
            (name, path)
        })
        .collect()
}

#[test]
fn native_and_json_requests_converge_at_the_canonical_dispatcher() {
    let fixtures = request_fixtures();
    assert_eq!(fixtures.len(), 77);
    let context = context();
    let native_backend = Arc::new(RecordingBackend::new());
    let json_backend = Arc::new(RecordingBackend::new());
    let native_handle =
        crate::test_support::register_index(context.clone(), native_backend.clone());
    let json_handle = crate::test_support::register_index(context, json_backend.clone());
    let native_state = handle::lookup(native_handle).unwrap();
    let json_state = handle::lookup(json_handle).unwrap();

    for (name, path) in fixtures {
        native_backend.clear();
        json_backend.clear();
        let native_request = std::fs::read(path).unwrap();
        let json = request_flatbuffer_to_json(&native_request)
            .unwrap_or_else(|error| panic!("{name}: native to JSON failed: {error}"));
        let json_request = request_json_to_flatbuffer(&json)
            .unwrap_or_else(|error| panic!("{name}: JSON to native failed: {error}"));

        let native_result = dispatch::dispatch(&native_state, &native_request);
        let json_result = dispatch::dispatch(&json_state, &json_request);
        assert_eq!(native_backend.calls(), json_backend.calls(), "{name}: IR");
        assert_eq!(
            native_result.as_ref().map(Vec::as_slice),
            json_result.as_ref().map(Vec::as_slice),
            "{name}: dispatch status"
        );
        if let (Ok(native_response), Ok(json_response)) = (native_result, json_result) {
            assert_eq!(
                response_flatbuffer_to_json(&native_response).unwrap(),
                response_flatbuffer_to_json(&json_response).unwrap(),
                "{name}: response"
            );
        }
    }

    assert!(handle::close(native_handle));
    assert!(handle::close(json_handle));
}

#[test]
fn successful_dispatch_encodes_each_backend_operation() {
    let cases = [
        "write.insert.skrq",
        "write.upsert.skrq",
        "write.patch-set.skrq",
        "write.delete.skrq",
        "write.get.skrq",
        "search.defaults.skrq",
        "analyze.text-request.skrq",
    ];
    let directory =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../searchkernel-json/tests/fixtures/requests");

    for case in cases {
        let request = fs::read(directory.join(case)).expect("read representative request");
        let recording = Arc::new(RecordingBackend::new());
        let recording_handle = crate::test_support::register_index(context(), recording.clone());
        let recording_state = handle::lookup(recording_handle).unwrap();
        assert!(dispatch::dispatch(&recording_state, &request).is_ok());
        let operation = recording
            .take_calls()
            .into_iter()
            .next()
            .unwrap_or_else(|| panic!("{case} did not reach a backend operation"));
        assert!(handle::close(recording_handle));

        let expected_response = match &operation {
            searchkernel_ir::OperationIr::Insert(request) => {
                ScriptedResult::Insert(searchkernel_ir::InsertResult {
                    id: request.document().id().as_str().to_owned(),
                })
            }
            searchkernel_ir::OperationIr::Upsert(request) => {
                ScriptedResult::Upsert(searchkernel_ir::UpsertResult {
                    id: request.document().id().as_str().to_owned(),
                })
            }
            searchkernel_ir::OperationIr::Patch(request) => {
                ScriptedResult::Patch(searchkernel_ir::PatchResult {
                    id: request.id().as_str().to_owned(),
                })
            }
            searchkernel_ir::OperationIr::Delete(request) => {
                ScriptedResult::Delete(searchkernel_ir::DeleteResult {
                    id: request.id().as_str().to_owned(),
                    deleted: true,
                })
            }
            searchkernel_ir::OperationIr::Get(_) => {
                ScriptedResult::Get(searchkernel_ir::GetResult { document: None })
            }
            searchkernel_ir::OperationIr::Search(_) => {
                ScriptedResult::Search(searchkernel_ir::SearchResult {
                    hits: Vec::new(),
                    total_hits: searchkernel_ir::TotalHits {
                        value: 0,
                        relation: searchkernel_ir::TotalHitsRelation::Exact,
                    },
                    facets: Vec::new(),
                })
            }
            searchkernel_ir::OperationIr::Analyze(_) => {
                ScriptedResult::Analyze(searchkernel_ir::AnalyzeResult { tokens: Vec::new() })
            }
        };
        let backend = Arc::new(ScriptedBackend::new([ExpectedCall::success(
            operation,
            expected_response,
        )]));
        let handle_value = crate::test_support::register_index(context(), backend);
        let state = handle::lookup(handle_value).unwrap();
        let response = dispatch::dispatch(&state, &request).unwrap();
        assert!(matches!(
            crate::test_support::decode_response(&response),
            Ok(crate::test_support::DecodedResponse::Insert(_))
                | Ok(crate::test_support::DecodedResponse::Upsert(_))
                | Ok(crate::test_support::DecodedResponse::Patch(_))
                | Ok(crate::test_support::DecodedResponse::Delete(_))
                | Ok(crate::test_support::DecodedResponse::Get(_))
                | Ok(crate::test_support::DecodedResponse::Search(_))
                | Ok(crate::test_support::DecodedResponse::Analyze(_))
        ));
        assert!(handle::close(handle_value));
    }
}

#[test]
fn component_corpus_is_embedded_in_transport_roots_for_json_roundtrip() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../reference/api-conformance");
    let cases: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("cases.json")).expect("read API case manifest"),
    )
    .expect("parse API case manifest");
    let transport_operations = [
        "search_to_ir",
        "insert_to_ir",
        "upsert_to_ir",
        "patch_to_ir",
        "delete_to_ir",
        "get_to_ir",
        "analyze_to_ir",
        "describe",
        "inspect_query",
        "write_to_ir",
    ];
    let cases = cases["cases"].as_array().expect("cases array");
    let mut component_count = 0;
    for entry in cases {
        let operation = entry["operation"].as_str().expect("case operation");
        if transport_operations.contains(&operation) {
            continue;
        }
        component_count += 1;
        let case_path = root.join(entry["case"].as_str().expect("case path"));
        let case: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(case_path).unwrap()).unwrap();
        let embedded = build_component_transport_case(operation, &case["input"])
            .unwrap_or_else(|error| panic!("{}: {error}", entry["id"]));
        match embedded {
            TransportCase::Request(bytes) => {
                let json = match request_flatbuffer_to_json(&bytes) {
                    Ok(json) => json,
                    Err(error) => {
                        let golden_path = root.join(entry["golden"].as_str().expect("golden path"));
                        let golden: serde_json::Value =
                            serde_json::from_str(&std::fs::read_to_string(golden_path).unwrap())
                                .unwrap();
                        assert!(
                            golden.get("error").is_some(),
                            "{}: unexpected request JSON conversion failure: {error}",
                            entry["id"]
                        );
                        continue;
                    }
                };
                let round_tripped = request_json_to_flatbuffer(&json).unwrap();
                assert_eq!(
                    request_flatbuffer_to_json(&round_tripped).unwrap(),
                    json,
                    "{}",
                    entry["id"]
                );
            }
            TransportCase::Response(bytes) => {
                let json = match response_flatbuffer_to_json(&bytes) {
                    Ok(json) => json,
                    Err(error) => {
                        let golden_path = root.join(entry["golden"].as_str().expect("golden path"));
                        let golden: serde_json::Value =
                            serde_json::from_str(&std::fs::read_to_string(golden_path).unwrap())
                                .unwrap();
                        assert!(
                            golden.get("error").is_some(),
                            "{}: unexpected response JSON conversion failure: {error}",
                            entry["id"]
                        );
                        continue;
                    }
                };
                let round_tripped = response_json_to_flatbuffer(&json).unwrap();
                assert_eq!(
                    response_flatbuffer_to_json(&round_tripped).unwrap(),
                    json,
                    "{}",
                    entry["id"]
                );
            }
        }
    }
    assert_eq!(component_count, 127);
}
