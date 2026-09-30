use serde_json::{Value as Json, json};
use searchkernel_vespa_engine::{
    ConfigCompiler, Document, DocumentUpdate, Engine, Field, FieldType, Query, QueryInput,
    RankProfile, Schema, SearchRequest, TensorValue,
};
use std::{fs, path::Path};

pub fn load(path: &Path) -> Json {
    serde_json::from_slice(&fs::read(path).expect("read M13 scenario")).expect("parse M13 scenario")
}

pub fn type_spec(root: &Json) -> &str {
    root["tensor_field"]["type_spec"]
        .as_str()
        .expect("tensor type spec")
}

pub fn tensor(root: &Json, name: &str) -> TensorValue {
    TensorValue::dense(
        type_spec(root),
        root["tensors"][name]
            .as_array()
            .expect("tensor cells")
            .iter()
            .map(|value| value.as_f64().expect("finite numeric tensor cell"))
            .collect::<Vec<_>>(),
    )
}

pub fn schema(root: &Json) -> Schema {
    let tensor_type = type_spec(root).to_owned();
    Schema::new(
        root["document_type"]
            .as_str()
            .expect("document type"),
    )
    .field(
        Field::new(
            root["tensor_field"]["name"]
                .as_str()
                .expect("tensor field name"),
            FieldType::Tensor(tensor_type.clone()),
        )
        .attribute(),
    )
    .field(Field::new("score", FieldType::Float).attribute().summary())
    .rank_profile(
        RankProfile::new("semantic")
            .query_input("query_embedding", tensor_type)
            .first_phase("closeness(embedding)"),
    )
}

pub fn compile_and_open(root: &Json, config: &Path, data: &Path) -> Engine {
    ConfigCompiler
        .compile_to(&schema(root), config)
        .expect("compile M13 native schema");
    Engine::open(data, config).expect("open M13 native engine")
}

pub fn dense_json(value: &TensorValue) -> Json {
    match value {
        TensorValue::Dense { type_spec, values } => {
            json!({"type_spec": type_spec, "values": values})
        }
        _ => panic!("M13 expects a dense tensor"),
    }
}

pub fn document_tensor(engine: &Engine, id: &str, field: &str) -> Json {
    let document = engine
        .get(id)
        .expect("get M13 document")
        .expect("M13 document exists");
    match document.fields().get(field).expect("tensor field present") {
        searchkernel_vespa_engine::FieldValue::Tensor(value) => dense_json(value),
        _ => panic!("M13 field is not a tensor"),
    }
}

pub fn run_low_level(root: &Json, config: &Path, data: &Path) -> Json {
    let field = root["tensor_field"]["name"].as_str().expect("tensor field");
    let id_a = "id:ns:typea::a";
    let id_b = "id:ns:typea::b";
    let engine = compile_and_open(root, config, data);
    engine
        .put(Document::new(id_a).field(field, tensor(root, "a")))
        .expect("insert tensor A");
    engine
        .put(Document::new(id_b).field(field, tensor(root, "b")))
        .expect("insert tensor B");
    let inserted = document_tensor(&engine, id_a, field);
    let query = QueryInput::Literal(tensor(root, "query"));
    let search = |engine: &Engine| {
        engine
            .search(
                SearchRequest::new(Query::nearest_neighbor(field, "query_embedding"))
                    .rank_profile("semantic")
                    .input("query_embedding", query.clone())
                    .limit(2),
            )
            .expect("M13 nearest-neighbor search")
            .hits
            .into_iter()
            .map(|hit| hit.document_id)
            .collect::<Vec<_>>()
    };
    let nn_before_patch = search(&engine);

    engine
        .put(Document::new(id_a).field(field, tensor(root, "b")))
        .expect("upsert tensor B");
    let upserted = document_tensor(&engine, id_a, field);
    let nn_before_update = search(&engine);

    engine
        .update(DocumentUpdate::new(id_a).assign(field, tensor(root, "replacement")))
        .expect("patch tensor replacement");
    let patched = document_tensor(&engine, id_a, field);
    let nn_after_patch_immediate = search(&engine);
    if nn_after_patch_immediate == nn_before_update {
        panic!("tensor UPDATE returned before immediate NN search reflected the replacement");
    }
    engine.close().expect("close after patched tensor");

    let engine = Engine::open(data, config).expect("reopen after patched tensor");
    let persisted_patch = document_tensor(&engine, id_a, field);
    let nn_after_patch = search(&engine);
    assert_eq!(
        nn_after_patch_immediate, nn_after_patch,
        "immediate tensor NN state disagreed with the persisted post-update state"
    );

    engine
        .update(DocumentUpdate::new(id_a).clear(field))
        .expect("unset tensor");
    let unset_fields = engine
        .get(id_a)
        .expect("get after tensor unset")
        .expect("document remains after tensor unset")
        .fields()
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    engine
        .update(DocumentUpdate::new(id_a).assign(field, tensor(root, "reassign")))
        .expect("reassign tensor");
    let reassigned = document_tensor(&engine, id_a, field);
    engine.close().expect("close after tensor reassign");

    let engine = Engine::open(data, config).expect("reopen after tensor reassign");
    let persisted_reassign = document_tensor(&engine, id_a, field);
    let deleted = engine.delete(id_a).expect("delete tensor document");
    let deleted_get = engine.get(id_a).expect("get deleted tensor document").is_none();
    engine.close().expect("final close M13 engine");

    json!({
        "id": root["id"],
        "inserted": inserted,
        "upserted": upserted,
        "patched": patched,
        "persisted_patch": persisted_patch,
        "nn_before_patch": nn_before_patch,
        "nn_after_patch": nn_after_patch,
        "unset_fields": unset_fields,
        "reassigned": reassigned,
        "persisted_reassign": persisted_reassign,
        "deleted": deleted,
        "deleted_get": deleted_get
    })
}
