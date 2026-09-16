use searchkernel_vespa_engine::{
    Aggregation, ConfigCompiler, Document, DocumentUpdate, Engine, FacetRequest, Field, FieldType,
    FieldValue, GroupingOrder, GroupingRequest, LinguisticsOptions, Query, ScalarValue, Schema,
    SearchRequest, SortField, SummaryRequest,
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
fn native_get_preserves_empty_attribute_values_and_unset_boolean_fields() {
    let config = temporary_path("source-config");
    let data = temporary_path("source-data");
    let schema = Schema::new("typea")
        .field(Field::new("title", FieldType::Text).indexed().summary())
        .field(Field::new("keyword", FieldType::Text).indexed().summary())
        .field(Field::new("qty", FieldType::Integer).attribute().summary())
        .field(
            Field::new("active", FieldType::Boolean)
                .attribute()
                .summary(),
        );
    ConfigCompiler
        .compile_to(&schema, &config)
        .expect("compile source-document config");

    let engine = Engine::open(&data, &config).expect("open source-document engine");
    engine
        .put(
            Document::new("source-1")
                .field("title", "source")
                .field("keyword", "")
                .field("qty", 1_i64),
        )
        .expect("put partial source document");

    let document = engine
        .get("source-1")
        .expect("get source document")
        .expect("source document exists");
    assert_eq!(
        document.fields().get("keyword"),
        Some(&FieldValue::Scalar(ScalarValue::String(String::new())))
    );
    assert_eq!(
        document.fields().get("active"),
        Some(&FieldValue::Scalar(ScalarValue::Boolean(false)))
    );

    engine.close().expect("close source-document engine");
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
        .search(
            SearchRequest::new(Query::True)
                .limit(10)
                .offset(0)
                .sort(SortField::descending("qty"))
                .summary(SummaryRequest::Fields(vec![
                    "title".to_owned(),
                    "qty".to_owned(),
                ])),
        )
        .expect("execute local search");
    assert_eq!(result.total_hits, 2);
    assert_eq!(result.hits[0].document_id, "local-2");
    assert_eq!(
        result.hits[0].fields["title"],
        FieldValue::Scalar(ScalarValue::String("beta".to_owned()))
    );
    assert_eq!(
        result.hits[0].fields["qty"],
        FieldValue::Scalar(ScalarValue::Integer(7))
    );

    engine
        .put(Document::new("local-1").field("qty", 11_i64))
        .expect("put attribute document");
    engine
        .update(DocumentUpdate::new("local-1").assign("qty", 13_i64))
        .expect("update attribute document");

    let grouped = engine
        .group(
            &GroupingRequest::new(Query::True, "qty")
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
    engine
        .put(Document::new("text-1").field("title", "quick fox"))
        .expect("put analyzed text");
    engine
        .update(DocumentUpdate::new("text-1").assign("title", "quick fox jumps"))
        .expect("update analyzed text");

    let term = engine
        .search(SearchRequest::new(Query::term("title", "jumps")))
        .expect("search text term");
    assert_eq!(
        term.hits
            .iter()
            .map(|hit| hit.document_id.as_str())
            .collect::<Vec<_>>(),
        ["text-1"]
    );

    let phrase = engine
        .search(SearchRequest::new(Query::phrase("title", "quick fox")))
        .expect("search text phrase");
    assert_eq!(phrase.hits.len(), 1);

    let typed = engine
        .search(SearchRequest::new(Query::term("title", "jumps")))
        .expect("search typed query");
    let yql = engine
        .search(SearchRequest::new(
            Query::from_yql("select * from typea where title contains \"jumps\"")
                .expect("parse YQL query"),
        ))
        .expect("search YQL query");
    assert_eq!(typed, yql);

    engine.close().expect("close text engine");
    let _ = fs::remove_dir_all(config);
    let _ = fs::remove_dir_all(data);
}

#[test]
fn native_generic_search_and_grouping_honor_language_sort_limits_and_order() {
    let config = temporary_path("generic-config");
    let data = temporary_path("generic-data");
    let schema = Schema::new("typea")
        .field(Field::new("title", FieldType::Text).indexed().summary())
        .field(
            Field::new("category", FieldType::String)
                .attribute()
                .summary(),
        )
        .field(
            Field::new("price", FieldType::Integer)
                .attribute()
                .summary(),
        );
    ConfigCompiler
        .compile_to(&schema, &config)
        .expect("compile generic search config");
    let engine = Engine::open(&data, &config).expect("open generic search engine");

    for (id, title, category, price) in [
        ("generic-1", "wireless alpha", "books", Some(20_i64)),
        ("generic-2", "wireless beta", "books", Some(10_i64)),
        ("generic-3", "wireless gamma", "games", None),
    ] {
        let mut document = Document::new(id)
            .field("title", title)
            .field("category", category);
        if let Some(price) = price {
            document = document.field("price", price);
        }
        engine.put(document).expect("put generic search document");
    }

    let sorted = engine
        .search(
            SearchRequest::new(Query::term("title", "wireless"))
                .language(LinguisticsOptions::default())
                .sort(SortField::ascending("price").missing_first())
                .limit(1)
                .fields(["title"]),
        )
        .expect("execute generic search");
    assert_eq!(sorted.total_hits, 3);
    assert_eq!(sorted.hits[0].document_id, "generic-3");

    let faceted = engine
        .search(
            SearchRequest::new(Query::True)
                .limit(1)
                .facet(FacetRequest::new(
                    "category",
                    2,
                    GroupingOrder::CountDescending,
                ))
                .facet(FacetRequest::new("price", 2, GroupingOrder::ValueAscending)),
        )
        .expect("execute composite search and facets");
    assert_eq!(faceted.hits.len(), 1);
    assert_eq!(faceted.facets.len(), 2);
    assert_eq!(
        faceted.facets[0].groups[0].key,
        searchkernel_vespa_engine::Value::String("books".into())
    );
    assert_eq!(faceted.facets[0].groups[0].count, 2);
    assert_eq!(
        faceted.facets[1].groups[0].key,
        searchkernel_vespa_engine::Value::Integer(10)
    );
    assert_eq!(
        faceted.facets[1].groups[1].key,
        searchkernel_vespa_engine::Value::Integer(20)
    );

    let count_desc = engine
        .group(
            &GroupingRequest::new(Query::True, "category")
                .max_groups(2)
                .order(GroupingOrder::CountDescending),
        )
        .expect("execute count-desc grouping");
    assert_eq!(count_desc.groups, faceted.facets[0].groups);

    let limited = engine
        .group(
            &GroupingRequest::new(Query::True, "category")
                .max_groups(1)
                .order(GroupingOrder::CountDescending),
        )
        .expect("execute limited count-desc grouping");
    assert_eq!(limited.groups.len(), 1);
    assert_eq!(
        limited.groups[0].key,
        searchkernel_vespa_engine::Value::String("books".into())
    );
    assert_eq!(limited.groups[0].count, 2);

    let value_asc = engine
        .group(
            &GroupingRequest::new(Query::True, "category")
                .max_groups(2)
                .order(GroupingOrder::ValueAscending),
        )
        .expect("execute value-asc grouping");
    assert_eq!(
        value_asc.groups[0].key,
        searchkernel_vespa_engine::Value::String("books".into())
    );
    assert_eq!(
        value_asc.groups[1].key,
        searchkernel_vespa_engine::Value::String("games".into())
    );

    engine.close().expect("close generic search engine");
    let _ = fs::remove_dir_all(config);
    let _ = fs::remove_dir_all(data);
}
