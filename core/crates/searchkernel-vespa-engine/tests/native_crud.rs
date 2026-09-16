use searchkernel_vespa_engine::{
    Aggregation, AttributeDocument, AttributeUpdate, ConfigCompiler, Document, DocumentUpdate,
    Engine, Field, FieldType, FieldValue, LocalGroupingRequest, LocalSearchRequest, Query,
    ScalarValue, Schema, SortField, SummaryRequest, TextDocument, TextQuery, Value,
};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn temporary_path(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("searchkernel-native-crud-{label}-{nonce}"))
}

#[test]
fn native_document_crud_round_trip_exercises_lifecycle_and_value_conversion() {
    let config = temporary_path("config");
    let data = temporary_path("data");
    let schema = Schema::new("typea")
        .field(Field::new("title", FieldType::Text).indexed().summary())
        .field(Field::new("qty", FieldType::Integer).attribute().summary())
        .field(Field::new("rating", FieldType::Float).attribute().summary())
        .field(
            Field::new("active", FieldType::Boolean)
                .attribute()
                .summary(),
        );
    ConfigCompiler
        .compile_to(&schema, &config)
        .expect("compile native CRUD config");

    let engine = Engine::open(&data, &config).expect("open native CRUD engine");
    engine
        .put(
            Document::new("crud-1")
                .field("title", "before")
                .field("qty", 3_i64)
                .field("rating", 4.5_f64)
                .field("active", true),
        )
        .expect("put complete document");

    let document = engine
        .get("crud-1")
        .expect("get complete document")
        .expect("document exists after put");
    assert_eq!(document.id(), "crud-1");
    assert_eq!(document.fields().len(), 4);
    assert_eq!(
        document.fields()["title"],
        FieldValue::Scalar(ScalarValue::String("before".to_owned()))
    );
    assert_eq!(
        document.fields()["qty"],
        FieldValue::Scalar(ScalarValue::Integer(3))
    );
    assert_eq!(
        document.fields()["rating"],
        FieldValue::Scalar(ScalarValue::Float(4.5))
    );
    assert_eq!(
        document.fields()["active"],
        FieldValue::Scalar(ScalarValue::Boolean(true))
    );

    engine
        .update(
            DocumentUpdate::new("crud-1")
                .assign("title", "after")
                .assign("qty", 7_i64)
                .assign("rating", 2.5_f64)
                .assign("active", false),
        )
        .expect("update complete document");
    let updated = engine
        .get("crud-1")
        .expect("get updated document")
        .expect("updated document exists");
    assert_eq!(
        updated.fields()["title"],
        FieldValue::Scalar(ScalarValue::String("after".to_owned()))
    );
    assert_eq!(
        updated.fields()["qty"],
        FieldValue::Scalar(ScalarValue::Integer(7))
    );
    assert_eq!(
        updated.fields()["rating"],
        FieldValue::Scalar(ScalarValue::Float(2.5))
    );
    assert_eq!(
        updated.fields()["active"],
        FieldValue::Scalar(ScalarValue::Boolean(false))
    );

    assert!(engine.delete("crud-1").expect("delete existing document"));
    assert!(!engine.delete("crud-1").expect("delete missing document"));
    assert!(engine
        .get("crud-1")
        .expect("get deleted document")
        .is_none());

    engine.close().expect("close native CRUD engine");
    let _ = fs::remove_dir_all(config);
    let _ = fs::remove_dir_all(data);
}

#[test]
fn native_local_search_and_attribute_operations_round_trip() {
    let config = temporary_path("local-config");
    let data = temporary_path("local-data");
    let schema = Schema::new("typea")
        .field(Field::new("title", FieldType::Text).indexed().summary())
        .field(Field::new("qty", FieldType::Integer).attribute().summary());
    ConfigCompiler
        .compile_to(&schema, &config)
        .expect("compile local search config");

    let engine = Engine::open(&data, &config).expect("open local search engine");
    engine
        .put(
            Document::new("local-1")
                .field("title", "alpha")
                .field("qty", 3_i64),
        )
        .expect("put local search document");
    engine
        .put(
            Document::new("local-2")
                .field("title", "beta")
                .field("qty", 7_i64),
        )
        .expect("put second local search document");

    let result = engine
        .search_local(
            &LocalSearchRequest::new(Query::True)
                .limit(10)
                .offset(0)
                .sort_by(SortField::descending("qty"))
                .summary(SummaryRequest::Fields(vec![
                    "title".to_owned(),
                    "qty".to_owned(),
                ])),
        )
        .expect("execute local search");
    assert_eq!(result.total_hit_count, 2);
    assert_eq!(result.hits[0].document_id, "local-2");
    assert_eq!(
        result.hits[0].summary["title"],
        Value::String("beta".to_owned())
    );
    assert_eq!(result.hits[0].summary["qty"], Value::Integer(7));

    engine
        .put_attributes(&AttributeDocument::new("local-1").field("qty", Value::Integer(11)))
        .expect("put attribute document");
    engine
        .update_attributes(&AttributeUpdate::new("local-1").assign("qty", Value::Integer(13)))
        .expect("update attribute document");

    let grouped = engine
        .group_local(
            &LocalGroupingRequest::new(Query::True, "qty")
                .aggregation(Aggregation::Sum("qty".to_owned()))
                .aggregation(Aggregation::Min("qty".to_owned()))
                .aggregation(Aggregation::Max("qty".to_owned()))
                .aggregation(Aggregation::Average("qty".to_owned())),
        )
        .expect("execute local grouping");
    assert!(!grouped.groups.is_empty());

    engine.close().expect("close local search engine");
    let _ = fs::remove_dir_all(config);
    let _ = fs::remove_dir_all(data);
}

#[test]
fn native_text_crud_and_query_forms_round_trip() {
    let config = temporary_path("text-config");
    let data = temporary_path("text-data");
    let schema = Schema::new("typea").field(Field::new("title", FieldType::Text).indexed());
    ConfigCompiler
        .compile_to(&schema, &config)
        .expect("compile text config");

    let engine = Engine::open(&data, &config).expect("open text engine");
    let document = TextDocument {
        id: "text-1",
        field: "title",
        text: "quick fox",
    };
    engine.put_text(document).expect("put analyzed text");
    engine
        .update_text(TextDocument {
            text: "quick fox jumps",
            ..document
        })
        .expect("update analyzed text");

    let term = engine
        .search_text(
            TextQuery::Term {
                field: "title",
                text: "jumps",
            },
            "default",
        )
        .expect("search text term");
    assert_eq!(
        term.hits
            .iter()
            .map(|hit| hit.document_id.as_str())
            .collect::<Vec<_>>(),
        ["text-1"]
    );

    let phrase = engine
        .search_text(
            TextQuery::Phrase {
                field: "title",
                text: "quick fox",
            },
            "default",
        )
        .expect("search text phrase");
    assert_eq!(phrase.hits.len(), 1);

    let typed = engine
        .search_query(&Query::term("title", "jumps"), "default")
        .expect("search typed query");
    let yql = engine
        .search_yql(
            "select * from typea where title contains \"jumps\"",
            "default",
        )
        .expect("search YQL query");
    assert_eq!(typed, yql);

    engine.close().expect("close text engine");
    let _ = fs::remove_dir_all(config);
    let _ = fs::remove_dir_all(data);
}
