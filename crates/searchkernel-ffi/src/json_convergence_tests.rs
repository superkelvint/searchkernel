use crate::test_support::RecordingBackend;
use crate::{dispatch, handle};
use searchkernel_core::{api, IndexContext};
use searchkernel_json::{
    request_flatbuffer_to_json, request_json_to_flatbuffer, response_flatbuffer_to_json,
};
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
