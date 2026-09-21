use searchkernel_vespa_engine::{
    ConfigCompiler, DistanceMetric, Document, DocumentUpdate, Engine, Field, FieldType,
    HnswIndexConfig, NearestNeighborOptions, Query, RankProfile, Schema, SearchRequest, TensorValue,
};
use serde_json::{Value as Json, json};
use std::{fs, path::Path};

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
                    .max_links_per_node(u32::try_from(hnsw["max_links_per_node"].as_u64().expect("max links")).unwrap())
                    .neighbors_to_explore_at_insert(u32::try_from(hnsw["neighbors_to_explore_at_insert"].as_u64().expect("neighbors at insert")).unwrap()),
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
    for doc in root["documents"].as_array().expect("documents") {
        engine.put(document(root, doc))?;
    }
    Ok(())
}

fn query_request(root: &Json, options: &Json) -> SearchRequest {
    let field = root["tensor_field"]["name"].as_str().unwrap();
    let type_spec = root["tensor_field"]["type_spec"].as_str().unwrap();
    let nn = NearestNeighborOptions {
        target_num_hits: u32::try_from(options["target_hits"].as_u64().unwrap()).unwrap(),
        approximate: options["approximate"].as_bool().unwrap(),
        distance_threshold: options["distance_threshold"].as_f64(),
        explore_additional_hits: u32::try_from(options["explore_additional_hits"].as_u64().unwrap()).unwrap(),
    };
    SearchRequest::new(Query::NearestNeighborWithOptions {
        document_field: field.to_owned(),
        query_tensor: "query_embedding".to_owned(),
        options: nn,
    })
    .rank_profile("semantic")
    .query_tensor("query_embedding", tensor(type_spec, &root["query"]))
    .limit(u32::try_from(options["limit"].as_u64().unwrap()).unwrap())
}

pub fn search(engine: &Engine, root: &Json, options: &Json) -> searchkernel_vespa_engine::Result<Vec<String>> {
    Ok(engine
        .search(query_request(root, options))?
        .hits
        .into_iter()
        .map(|hit| hit.document_id)
        .collect())
}

pub fn run_query_controls(root: &Json, config: &Path, data: &Path) -> searchkernel_vespa_engine::Result<Json> {
    compile(root, config)?;
    let engine = Engine::open(data, config)?;
    feed(&engine, root)?;
    let mut steps = Vec::new();
    for step in root["steps"].as_array().unwrap() {
        steps.push(json!({"id":step["id"],"hits":search(&engine,root,step)?}));
    }
    engine.close()?;
    Ok(json!({"id":root["id"],"steps":steps}))
}

pub fn run_lifecycle(root: &Json, config: &Path, data: &Path) -> searchkernel_vespa_engine::Result<Json> {
    compile(root, config)?;
    let options=&root["query_options"];
    let field=root["tensor_field"]["name"].as_str().unwrap();
    let type_spec=root["tensor_field"]["type_spec"].as_str().unwrap();
    let update_id=root["mutation"]["update_id"].as_str().unwrap();
    let delete_id=root["mutation"]["delete_id"].as_str().unwrap();

    let engine=Engine::open(data,config)?;
    feed(&engine,root)?;
    let before_update=search(&engine,root,options)?;
    engine.update(DocumentUpdate::new(update_id).assign(field,tensor(type_spec,&root["mutation"]["update_values"])))?;
    let after_update=search(&engine,root,options)?;
    engine.close()?;

    let engine=Engine::open(data,config)?;
    let after_reopen=search(&engine,root,options)?;
    let deleted=engine.delete(delete_id)?;
    assert!(deleted, "M14 delete did not find document");
    let after_delete=search(&engine,root,options)?;
    engine.close()?;

    let engine=Engine::open(data,config)?;
    let after_delete_reopen=search(&engine,root,options)?;
    engine.close()?;

    Ok(json!({
        "id":root["id"],
        "before_update":before_update,
        "after_update":after_update,
        "after_reopen":after_reopen,
        "after_delete":after_delete,
        "after_delete_reopen":after_delete_reopen
    }))
}

pub fn run_low_level(root: &Json, config: &Path, data: &Path) -> searchkernel_vespa_engine::Result<Json> {
    match root["id"].as_str() {
        Some("hnsw-query-controls") => run_query_controls(root,config,data),
        Some("hnsw-mutation-reopen") => run_lifecycle(root,config,data),
        _ => panic!("unknown M14 scenario"),
    }
}
