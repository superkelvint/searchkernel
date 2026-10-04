#![cfg(feature = "native")]
use searchkernel_protocol::json::{request_json_to_flatbuffer, response_flatbuffer_to_json};
use searchkerneld::IndexRegistry;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Root(std::path::PathBuf);
impl Root {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "configuration-workflow-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        Self(root)
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn ordered_json(value: &Value) -> String {
    match value {
        Value::Object(fields) => {
            let mut fields: Vec<_> = fields.iter().collect();
            fields.sort_by_key(|(key, _)| (!key.ends_with("_type"), *key));
            format!(
                "{{{}}}",
                fields
                    .into_iter()
                    .map(|(key, value)| format!(
                        "{}:{}",
                        serde_json::to_string(key).unwrap(),
                        ordered_json(value)
                    ))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        }
        Value::Array(values) => format!(
            "[{}]",
            values
                .iter()
                .map(ordered_json)
                .collect::<Vec<_>>()
                .join(",")
        ),
        _ => value.to_string(),
    }
}
fn execute(
    registry: &IndexRegistry,
    operation: &str,
    target: &str,
    generation: Option<&str>,
    payload: Value,
) -> Value {
    let request = json!({"protocol_version":1, "target":{"name":target,"generation":generation},
        "payload_type":format!("SearchKernel_Wire_{operation}"), "payload":payload});
    let request = request_json_to_flatbuffer(ordered_json(&request).as_bytes()).unwrap();
    let response = registry.dispatch(&request).unwrap();
    serde_json::from_slice(&response_flatbuffer_to_json(&response).unwrap()).unwrap()
}
fn definition(expression: &str) -> Value {
    json!({"name":"products", "fields":[{"name":"title", "data_type":{"kind_type":"PrimitiveDataType","kind":{"kind":"String"}},
        "stored":{"value":true},"searchable":{"value":true}}],
        "rank_profiles":[{"name":"product_rank","first_phase":{"expression":expression}}]})
}
fn plan(
    registry: &IndexRegistry,
    target: &str,
    generation: Option<&str>,
    definition: Value,
) -> Value {
    execute(
        registry,
        "PlanIndexChangeRequest",
        target,
        generation,
        json!({"definition":definition}),
    )
}
fn apply(registry: &IndexRegistry, target: &str, generation: Option<&str>, token: &str) -> Value {
    execute(
        registry,
        "ApplyIndexChangeRequest",
        target,
        generation,
        json!({"plan_id":token}),
    )
}
fn assert_success(response: &Value, result: &str) {
    assert_eq!(
        response["payload_type"],
        format!("SearchKernel_Wire_{result}"),
        "{response}"
    );
}

#[test]
fn canonical_plan_create_rank_edit_retains_documents_and_reopens() {
    let root = Root::new();
    let registry = IndexRegistry::new(&root.0).unwrap();
    let initial = plan(&registry, "catalog", None, definition("1"));
    assert_success(&initial, "PlanIndexChangeResult");
    assert_eq!(initial["payload"]["supported"], true);
    assert!(
        !root.0.join("catalog").exists(),
        "validation published selected index"
    );
    let created = apply(
        &registry,
        "catalog",
        None,
        initial["payload"]["plan_id"].as_str().unwrap(),
    );
    assert_success(&created, "ApplyIndexChangeResult");
    let generation = created["payload"]["generation"].as_str().unwrap();
    let upsert = execute(
        &registry,
        "UpsertRequest",
        "catalog",
        Some(generation),
        json!({"document":{"id":"one","fields":[{"key":"title","value":{"kind_type":"StringValue","kind":{"value":"orchid"}}}]}}),
    );
    assert_success(&upsert, "UpsertResult");
    let changed = plan(&registry, "catalog", Some(generation), definition("2"));
    assert_success(&changed, "PlanIndexChangeResult");
    assert_eq!(changed["payload"]["requires_reopen"], true);
    let other_plan = plan(&registry, "catalog", Some(generation), definition("3"));
    assert_success(&other_plan, "PlanIndexChangeResult");
    let applied = apply(
        &registry,
        "catalog",
        Some(generation),
        changed["payload"]["plan_id"].as_str().unwrap(),
    );
    assert_success(&applied, "ApplyIndexChangeResult");
    assert_success(
        &apply(
            &registry,
            "catalog",
            Some(generation),
            other_plan["payload"]["plan_id"].as_str().unwrap(),
        ),
        "Error",
    );
    assert!(!std::fs::read_dir(&root.0).unwrap().any(|entry| entry
        .unwrap()
        .file_name()
        .to_string_lossy()
        .starts_with(".configuration-probe-")));
    let new_generation = applied["payload"]["generation"].as_str().unwrap();
    assert_ne!(generation, new_generation);
    let stale = execute(
        &registry,
        "GetRequest",
        "catalog",
        Some(generation),
        json!({"id":"one"}),
    );
    assert_success(&stale, "Error");
    let document = execute(
        &registry,
        "GetRequest",
        "catalog",
        Some(new_generation),
        json!({"id":"one"}),
    );
    assert_success(&document, "GetResult");
    assert_eq!(document["payload"]["document"]["id"], "one");
    registry.close_all().unwrap();
    let reopened = IndexRegistry::new(&root.0).unwrap();
    let open = execute(&reopened, "OpenIndexRequest", "catalog", None, json!({}));
    assert_success(&open, "OpenIndexResult");
    assert_eq!(
        open["payload"]["index"]["rank_profiles"][0]["first_phase"]["expression"],
        "2"
    );
    let document = execute(
        &reopened,
        "GetRequest",
        "catalog",
        open["payload"]["generation"].as_str(),
        json!({"id":"one"}),
    );
    assert_success(&document, "GetResult");
    assert_eq!(document["payload"]["document"]["id"], "one");
    reopened.close_all().unwrap();
}

#[test]
fn plans_reject_schema_changes_wrong_target_stale_generation_replay_and_invalid_expression() {
    let root = Root::new();
    let registry = IndexRegistry::new(&root.0).unwrap();
    let missing = execute(
        &registry,
        "PlanIndexChangeRequest",
        "catalog",
        None,
        json!({}),
    );
    assert_success(&missing, "Error");
    assert!(missing.to_string().contains("plan_index_change.definition"));
    assert_success(
        &execute(
            &registry,
            "ApplyIndexChangeRequest",
            "catalog",
            None,
            json!({}),
        ),
        "Error",
    );
    let validated = plan(&registry, "catalog", None, definition("1"));
    assert_success(&validated, "PlanIndexChangeResult");
    let token = validated["payload"]["plan_id"].as_str().unwrap();
    assert_success(&apply(&registry, "other", None, token), "Error");
    let created = apply(&registry, "catalog", None, token);
    assert_success(&created, "ApplyIndexChangeResult");
    assert_success(&apply(&registry, "catalog", None, token), "Error");
    let generation = created["payload"]["generation"].as_str().unwrap();
    assert_success(
        &plan(&registry, "catalog", Some("stale"), definition("2")),
        "Error",
    );
    let mut different = definition("1");
    different["fields"][0]["name"] = json!("body");
    let unsupported = plan(&registry, "catalog", Some(generation), different);
    assert_success(&unsupported, "PlanIndexChangeResult");
    assert!(!unsupported["payload"]["supported"]
        .as_bool()
        .unwrap_or(false));
    assert_eq!(unsupported["payload"]["plan_id"].as_str().unwrap_or(""), "");
    assert_success(
        &plan(
            &registry,
            "catalog",
            Some(generation),
            definition("invalid_function("),
        ),
        "Error",
    );
    let still_open = execute(
        &registry,
        "DescribeRequest",
        "catalog",
        Some(generation),
        json!({}),
    );
    assert_success(&still_open, "DescribeResult");
    assert_eq!(
        still_open["payload"]["index"]["rank_profiles"][0]["first_phase"]["expression"],
        "1"
    );
    registry.close_all().unwrap();
}
