// These helpers are shared by the M14 driver, adapter, and error examples,
// which use different subsets of the module.
#![allow(dead_code)]

use searchkernel_vespa_engine::{
    ConfigCompiler, DistanceMetric, Document, DocumentUpdate, Engine, Field, FieldType, FieldValue,
    HnswIndexConfig, NearestNeighborOptions, Query, RankProfile, Schema, SearchRequest,
    TensorValue,
};
use serde_json::{json, Value as Json};
use std::{
    env, fs,
    path::{Path, PathBuf},
};

pub struct TraceReader {
    path: Option<PathBuf>,
    offset: usize,
}

impl TraceReader {
    pub fn new() -> Self {
        let path = env::var_os("SEARCHKERNEL_NATIVE_TRACE_FILE").map(PathBuf::from);
        let offset = path
            .as_ref()
            .and_then(|path| fs::metadata(path).ok())
            .map(|metadata| usize::try_from(metadata.len()).expect("trace file is too large"))
            .unwrap_or(0);
        Self { path, offset }
    }

    pub fn next(&mut self) -> Option<serde_json::Value> {
        let Some(path) = &self.path else {
            return None;
        };
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return None,
            Err(error) => panic!("read native trace observation: {error}"),
        };
        if text.len() < self.offset {
            panic!("native trace observation file was truncated");
        }
        let appended = &text[self.offset..];
        self.offset = text.len();
        appended
            .lines()
            .rfind(|line| !line.trim().is_empty())
            .map(|line| serde_json::from_str(line).expect("native trace observation is not JSON"))
    }
}

pub fn load(path: &Path) -> Json {
    serde_json::from_slice(&fs::read(path).expect("read M14 JSON")).expect("parse M14 JSON")
}

fn tensor(type_spec: &str, values: &Json) -> TensorValue {
    TensorValue::dense(
        type_spec,
        values
            .as_array()
            .expect("tensor values")
            .iter()
            .map(|v| v.as_f64().expect("finite tensor cell"))
            .collect::<Vec<_>>(),
    )
}

pub fn schema(root: &Json) -> Schema {
    let field = &root["tensor_field"];
    let hnsw = &field["hnsw"];
    Schema::new(root["document_type"].as_str().expect("document_type"))
        .field(
            Field::new(
                field["name"].as_str().expect("field name"),
                FieldType::Tensor(field["type_spec"].as_str().expect("type spec").to_owned()),
            )
            .attribute()
            .distance_metric(DistanceMetric::Euclidean)
            .hnsw(
                HnswIndexConfig::new()
                    .max_links_per_node(
                        u32::try_from(hnsw["max_links_per_node"].as_u64().expect("max links"))
                            .unwrap(),
                    )
                    .neighbors_to_explore_at_insert(
                        u32::try_from(
                            hnsw["neighbors_to_explore_at_insert"]
                                .as_u64()
                                .expect("neighbors at insert"),
                        )
                        .unwrap(),
                    ),
            ),
        )
        .rank_profile(
            RankProfile::new("semantic")
                .first_phase("closeness(embedding)")
                .query_input("query_embedding", field["type_spec"].as_str().unwrap()),
        )
}

pub fn compile(root: &Json, config: &Path) -> searchkernel_vespa_engine::Result<()> {
    ConfigCompiler.compile_to(&schema(root), config)
}

fn document(root: &Json, doc: &Json) -> Document {
    let field = root["tensor_field"]["name"].as_str().unwrap();
    let type_spec = root["tensor_field"]["type_spec"].as_str().unwrap();
    Document::new(doc["id"].as_str().unwrap()).field(field, tensor(type_spec, &doc["values"]))
}

pub fn feed(engine: &Engine, root: &Json) -> searchkernel_vespa_engine::Result<()> {
    let field = root["tensor_field"]["name"].as_str().unwrap();
    let type_spec = root["tensor_field"]["type_spec"].as_str().unwrap();
    let options = json!({
        "target_hits": 1,
        "approximate": true,
        "explore_additional_hits": 4,
        "distance_threshold": null,
        "limit": 1
    });
    for doc in root["documents"].as_array().expect("documents") {
        engine.put(document(root, doc))?;
        let id = doc["id"].as_str().unwrap();
        let expected = FieldValue::Tensor(tensor(type_spec, &doc["values"]));
        let fetched = engine
            .get(id)?
            .expect("new HNSW document is visible to GET");
        assert_eq!(fetched.fields().get(field), Some(&expected));
        let result = engine.search(query_request(root, &options, &doc["values"]))?;
        assert!(
            result.hits.iter().any(|hit| hit.document_id == id),
            "new HNSW document {id} is absent from its immediate nearest-neighbor search"
        );
    }
    Ok(())
}

fn query_request(root: &Json, options: &Json, query_values: &Json) -> SearchRequest {
    let field = root["tensor_field"]["name"].as_str().unwrap();
    let type_spec = root["tensor_field"]["type_spec"].as_str().unwrap();
    let nn = NearestNeighborOptions {
        target_num_hits: u32::try_from(options["target_hits"].as_u64().unwrap()).unwrap(),
        approximate: options["approximate"].as_bool().unwrap(),
        distance_threshold: options["distance_threshold"].as_f64(),
        explore_additional_hits: u32::try_from(
            options["explore_additional_hits"].as_u64().unwrap(),
        )
        .unwrap(),
    };
    SearchRequest::new(Query::NearestNeighborWithOptions {
        document_field: field.to_owned(),
        query_tensor: "query_embedding".to_owned(),
        options: nn,
    })
    .rank_profile("semantic")
    .query_tensor("query_embedding", tensor(type_spec, query_values))
    .limit(u32::try_from(options["limit"].as_u64().unwrap()).unwrap())
}

fn search_with_evidence(
    engine: &Engine,
    root: &Json,
    options: &Json,
    query_values: &Json,
    traces: &mut TraceReader,
) -> searchkernel_vespa_engine::Result<(Vec<String>, Option<Json>)> {
    let result = engine.search(query_request(root, options, query_values))?;
    let hits = result.hits.into_iter().map(|hit| hit.document_id).collect();
    Ok((hits, traces.next()))
}

fn threshold_probe(
    engine: &Engine,
    root: &Json,
    traces: &mut TraceReader,
) -> searchkernel_vespa_engine::Result<Json> {
    let options = json!({
        "target_hits": 3,
        "approximate": true,
        "distance_threshold": 0.01,
        "explore_additional_hits": 1,
        "limit": 3
    });
    let query = json!([0.0, 0.5, 0.5]);
    let (hits, trace) = search_with_evidence(engine, root, &options, &query, traces)?;
    Ok(json!({"id":"threshold-probe","trace":trace,"hit_count":hits.len()}))
}

pub fn run_query_controls(
    root: &Json,
    config: &Path,
    data: &Path,
) -> searchkernel_vespa_engine::Result<Json> {
    compile(root, config)?;
    let engine = Engine::open(data, config)?;
    feed(&engine, root)?;
    let mut traces = TraceReader::new();
    let mut steps = Vec::new();
    let mut evidence = Vec::new();
    for step in root["steps"].as_array().unwrap() {
        let (hits, trace) = search_with_evidence(&engine, root, step, &root["query"], &mut traces)?;
        steps.push(json!({"id":step["id"],"hits":hits}));
        evidence.push(json!({"id":step["id"],"trace":trace}));
    }
    evidence.push(threshold_probe(&engine, root, &mut traces)?);
    engine.close()?;
    Ok(json!({"id":root["id"],"steps":steps,"evidence":evidence}))
}

pub fn run_lifecycle(
    root: &Json,
    config: &Path,
    data: &Path,
) -> searchkernel_vespa_engine::Result<Json> {
    compile(root, config)?;
    let options = &root["query_options"];
    let field = root["tensor_field"]["name"].as_str().unwrap();
    let type_spec = root["tensor_field"]["type_spec"].as_str().unwrap();
    let update_id = root["mutation"]["update_id"].as_str().unwrap();
    let delete_id = root["mutation"]["delete_id"].as_str().unwrap();

    let engine = Engine::open(data, config)?;
    feed(&engine, root)?;
    let mut traces = TraceReader::new();
    let mut evidence = Vec::new();
    let (before_update, trace) =
        search_with_evidence(&engine, root, options, &root["query"], &mut traces)?;
    evidence.push(json!({"id":"before_update","trace":trace}));
    evidence.push(threshold_probe(&engine, root, &mut traces)?);
    engine.update(
        DocumentUpdate::new(update_id)
            .assign(field, tensor(type_spec, &root["mutation"]["update_values"])),
    )?;
    let expected = FieldValue::Tensor(tensor(type_spec, &root["mutation"]["update_values"]));
    let fetched = engine
        .get(update_id)?
        .expect("updated HNSW document remains visible to GET");
    assert_eq!(fetched.fields().get(field), Some(&expected));
    let (after_update, trace) =
        search_with_evidence(&engine, root, options, &root["query"], &mut traces)?;
    evidence.push(json!({"id":"after_update","trace":trace}));
    engine.close()?;

    let engine = Engine::open(data, config)?;
    let (after_reopen, trace) =
        search_with_evidence(&engine, root, options, &root["query"], &mut traces)?;
    evidence.push(json!({"id":"after_reopen","trace":trace}));
    let deleted = engine.delete(delete_id)?;
    assert!(deleted, "M14 delete did not find document");
    assert!(
        engine.get(delete_id)?.is_none(),
        "deleted HNSW document remains visible to GET"
    );
    let (after_delete, trace) =
        search_with_evidence(&engine, root, options, &root["query"], &mut traces)?;
    evidence.push(json!({"id":"after_delete","trace":trace}));
    engine.close()?;

    let engine = Engine::open(data, config)?;
    let (after_delete_reopen, trace) =
        search_with_evidence(&engine, root, options, &root["query"], &mut traces)?;
    evidence.push(json!({"id":"after_delete_reopen","trace":trace}));
    engine.close()?;

    Ok(json!({
        "id":root["id"],
        "before_update":before_update,
        "after_update":after_update,
        "after_reopen":after_reopen,
        "after_delete":after_delete,
        "after_delete_reopen":after_delete_reopen,
        "evidence":evidence
    }))
}

pub fn run_low_level(
    root: &Json,
    config: &Path,
    data: &Path,
) -> searchkernel_vespa_engine::Result<Json> {
    match root["id"].as_str() {
        Some("hnsw-query-controls") => run_query_controls(root, config, data),
        Some("hnsw-mutation-reopen") => run_lifecycle(root, config, data),
        _ => panic!("unknown M14 scenario"),
    }
}
