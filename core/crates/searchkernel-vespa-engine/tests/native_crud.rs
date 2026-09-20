use searchkernel_vespa_engine::{
    ConfigCompiler, Document, DocumentUpdate, Engine, FacetRequest, Field, FieldType, FieldValue,
    GroupingAggregation, GroupingExpression, GroupingLevel, GroupingOrder, GroupingPlan,
    InQueryValue, LinguisticsOptions, Query, QueryInput, RankProfile, ScalarValue, Schema,
    SearchRequest, SortField, SummaryRequest, TensorValue, Value,
};
use std::collections::BTreeSet;
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
fn native_nearest_neighbor_uses_a_tensor_query_input_and_rank_profile() {
    let config = temporary_path("nearest-config");
    let data = temporary_path("nearest-data");
    let schema = Schema::new("typea")
        .field(
            Field::new(
                "embedding",
                FieldType::Tensor("tensor<float>(x[2])".to_owned()),
            )
            .attribute(),
        )
        .rank_profile(
            RankProfile::new("semantic")
                .query_input("query_embedding", "tensor<float>(x[2])")
                .first_phase("closeness(embedding)"),
        );
    ConfigCompiler
        .compile_to(&schema, &config)
        .expect("compile nearest-neighbor config");

    let engine = Engine::open(&data, &config).expect("open nearest-neighbor engine");
    engine
        .put(Document::new("near").field(
            "embedding",
            TensorValue::dense("tensor<float>(x[2])", [1.0, 0.0]),
        ))
        .expect("put nearest document");
    engine
        .put(Document::new("far").field(
            "embedding",
            TensorValue::dense("tensor<float>(x[2])", [0.0, 1.0]),
        ))
        .expect("put far document");

    let result = engine
        .search(
            SearchRequest::new(Query::nearest_neighbor("embedding", "query_embedding"))
                .rank_profile("semantic")
                .input(
                    "query_embedding",
                    QueryInput::Literal(TensorValue::dense("tensor<float>(x[2])", [0.9, 0.1])),
                )
                .limit(2),
        )
        .expect("execute nearest-neighbor search");
    assert_eq!(result.total_hits, 2);
    assert_eq!(
        result.total_hits_relation,
        searchkernel_vespa_engine::TotalHitsRelation::Unknown
    );
    assert_eq!(result.hits[0].document_id, "near");
    assert!(result.hits[0].score > result.hits[1].score);

    engine.close().expect("close nearest-neighbor engine");
    let _ = fs::remove_dir_all(config);
    let _ = fs::remove_dir_all(data);
}

#[test]
fn native_dense_tensor_document_crud_persists_and_updates_nearest_neighbor() {
    let config = temporary_path("tensor-crud-config");
    let data = temporary_path("tensor-crud-data");
    let tensor_type = "tensor<float>(x[2])";
    let schema = Schema::new("typea")
        .field(Field::new("embedding", FieldType::Tensor(tensor_type.to_owned())).attribute())
        .rank_profile(
            RankProfile::new("semantic")
                .query_input("query_embedding", tensor_type)
                .first_phase("closeness(embedding)"),
        );
    ConfigCompiler
        .compile_to(&schema, &config)
        .expect("compile tensor CRUD config");

    let tensor_a = TensorValue::dense(tensor_type, [1.0, 0.0]);
    let tensor_b = TensorValue::dense(tensor_type, [0.0, 1.0]);
    let tensor_replacement = TensorValue::dense(tensor_type, [0.0, 2.0]);
    let tensor_c = TensorValue::dense(tensor_type, [0.25, 0.75]);
    let engine = Engine::open(&data, &config).expect("open tensor CRUD engine");
    engine
        .put(Document::new("doc-a").field("embedding", tensor_a.clone()))
        .expect("insert tensor document");
    engine
        .put(Document::new("doc-b").field("embedding", tensor_b.clone()))
        .expect("insert second tensor document");

    let assert_tensor = |document: &Document, expected: &TensorValue| {
        assert_eq!(
            document.fields().get("embedding"),
            Some(&FieldValue::Tensor(expected.clone()))
        );
    };
    assert_tensor(
        &engine
            .get("doc-a")
            .expect("get inserted tensor")
            .expect("inserted tensor exists"),
        &tensor_a,
    );

    // A second complete put is the low-level upsert operation and must replace
    // the whole Vespa document value.
    engine
        .put(Document::new("doc-a").field("embedding", tensor_b.clone()))
        .expect("upsert replacement tensor");
    assert_tensor(
        &engine
            .get("doc-a")
            .expect("get upserted tensor")
            .expect("upserted tensor exists"),
        &tensor_b,
    );

    engine
        .update(DocumentUpdate::new("doc-a").assign("embedding", tensor_a.clone()))
        .expect("assign whole tensor");
    assert_tensor(
        &engine
            .get("doc-a")
            .expect("get assigned tensor")
            .expect("assigned tensor exists"),
        &tensor_a,
    );

    let initial = engine
        .search(
            SearchRequest::new(Query::nearest_neighbor("embedding", "query_embedding"))
                .rank_profile("semantic")
                .input(
                    "query_embedding",
                    QueryInput::Literal(TensorValue::dense(tensor_type, [1.0, 0.0])),
                )
                .limit(2),
        )
        .expect("search before tensor replacement");
    assert_eq!(initial.hits[0].document_id, "doc-a");
    assert_eq!(initial.hits[1].document_id, "doc-b");

    engine
        .update(DocumentUpdate::new("doc-a").assign("embedding", tensor_replacement.clone()))
        .expect("replace tensor used by nearest-neighbor search");
    let replaced = engine
        .search(
            SearchRequest::new(Query::nearest_neighbor("embedding", "query_embedding"))
                .rank_profile("semantic")
                .input(
                    "query_embedding",
                    QueryInput::Literal(TensorValue::dense(tensor_type, [1.0, 0.0])),
                )
                .limit(2),
        )
        .expect("search after tensor replacement");
    assert_eq!(replaced.hits[0].document_id, "doc-b");
    assert_eq!(replaced.hits[1].document_id, "doc-a");

    engine
        .update(DocumentUpdate::new("doc-a").clear("embedding"))
        .expect("unset tensor field");
    let cleared = engine
        .get("doc-a")
        .expect("get document after tensor unset")
        .expect("document remains after tensor unset");
    assert!(!cleared.fields().contains_key("embedding"));

    engine
        .update(DocumentUpdate::new("doc-a").assign("embedding", tensor_c.clone()))
        .expect("reassign tensor after unset");
    assert_tensor(
        &engine
            .get("doc-a")
            .expect("get reassigned tensor")
            .expect("reassigned tensor exists"),
        &tensor_c,
    );
    engine.close().expect("close tensor CRUD engine");

    let reopened = Engine::open(&data, &config).expect("reopen tensor CRUD engine");
    assert_tensor(
        &reopened
            .get("doc-a")
            .expect("get persisted tensor")
            .expect("persisted tensor exists"),
        &tensor_c,
    );
    assert!(reopened.delete("doc-a").expect("delete tensor document"));
    assert!(reopened
        .get("doc-a")
        .expect("get deleted tensor document")
        .is_none());
    reopened.close().expect("close reopened tensor CRUD engine");

    let _ = fs::remove_dir_all(config);
    let _ = fs::remove_dir_all(data);
}

#[test]
fn native_dense_tensor_document_validation_is_shared_by_put_and_patch() {
    let config = temporary_path("tensor-errors-config");
    let data = temporary_path("tensor-errors-data");
    let tensor_type = "tensor<float>(x[2])";
    let schema = Schema::new("typea")
        .field(Field::new("embedding", FieldType::Tensor(tensor_type.to_owned())).attribute())
        .field(Field::new("qty", FieldType::Integer).attribute());
    ConfigCompiler
        .compile_to(&schema, &config)
        .expect("compile tensor validation config");
    let engine = Engine::open(&data, &config).expect("open tensor validation engine");

    let assert_put_error = |value: FieldValue, label: &str| {
        let error = engine
            .put(Document::new(format!("invalid-{label}")).field("embedding", value))
            .expect_err("invalid tensor put unexpectedly succeeded");
        assert!(
            !error.to_string().is_empty(),
            "{label} returned no diagnostic"
        );
    };
    assert_put_error(
        FieldValue::Scalar(ScalarValue::Integer(1)),
        "scalar-for-tensor",
    );
    assert_put_error(
        FieldValue::Tensor(TensorValue::dense("not-a-tensor", [1.0, 0.0])),
        "malformed-type-spec",
    );
    assert_put_error(
        FieldValue::Tensor(TensorValue::dense("tensor<float>(x[0])", [])),
        "zero-indexed-dimension",
    );
    assert_put_error(
        FieldValue::Tensor(TensorValue::dense("tensor<float>(x[2],y[2])", [1.0; 4])),
        "multiple-indexed-dimensions",
    );
    assert_put_error(
        FieldValue::Tensor(TensorValue::dense("tensor<float>(m{},x[2])", [1.0, 0.0])),
        "mapped-dimension",
    );
    assert_put_error(
        FieldValue::Tensor(TensorValue::dense("tensor<float>(x[2])", [1.0])),
        "cell-count-mismatch",
    );
    assert_put_error(
        FieldValue::Tensor(TensorValue::dense("tensor<float>(x[2])", [f64::NAN, 0.0])),
        "non-finite-cell",
    );
    assert_put_error(
        FieldValue::Tensor(TensorValue::Sparse {
            type_spec: "tensor<float>(m{},x[2])".to_owned(),
            cells: Vec::new(),
        }),
        "sparse",
    );
    assert_put_error(
        FieldValue::Tensor(TensorValue::Mixed {
            type_spec: "tensor<float>(m{},x[2])".to_owned(),
            blocks: Vec::new(),
        }),
        "mixed",
    );

    let scalar_error = engine
        .put(
            Document::new("invalid-scalar-for-tensor")
                .field("qty", TensorValue::dense(tensor_type, [1.0, 0.0])),
        )
        .expect_err("tensor value supplied to scalar field unexpectedly succeeded");
    assert!(!scalar_error.to_string().is_empty());

    engine
        .put(
            Document::new("patch-errors")
                .field("embedding", TensorValue::dense(tensor_type, [1.0, 0.0])),
        )
        .expect("insert patch validation document");
    for (label, value) in [
        (
            "incompatible-type",
            TensorValue::dense("tensor<double>(x[2])", [1.0, 0.0]),
        ),
        (
            "incompatible-dimension",
            TensorValue::dense("tensor<float>(x[3])", [1.0, 0.0, 0.0]),
        ),
    ] {
        let error = engine
            .update(DocumentUpdate::new("patch-errors").assign("embedding", value))
            .expect_err("incompatible tensor patch unexpectedly succeeded");
        assert!(
            !error.to_string().is_empty(),
            "{label} returned no diagnostic"
        );
    }

    engine.close().expect("close tensor validation engine");
    let _ = fs::remove_dir_all(config);
    let _ = fs::remove_dir_all(data);
}

#[test]
fn native_in_supports_string_and_integer_values_without_approximating_invalid_input() {
    let config = temporary_path("in-config");
    let data = temporary_path("in-data");
    let schema = Schema::new("typea")
        .field(Field::new("code", FieldType::String).attribute())
        .field(Field::new("qty", FieldType::Integer).attribute());
    ConfigCompiler
        .compile_to(&schema, &config)
        .expect("compile in-query config");
    let engine = Engine::open(&data, &config).expect("open in-query engine");
    for (id, code, qty) in [
        ("in-a", "alpha", 1_i64),
        ("in-b", "beta", 2),
        ("in-c", "gamma", 3),
    ] {
        engine
            .put(Document::new(id).field("code", code).field("qty", qty))
            .expect("put in-query document");
    }

    let strings = engine
        .search(SearchRequest::new(Query::In {
            field: "code".into(),
            values: vec![
                InQueryValue::String("alpha".into()),
                InQueryValue::String("gamma".into()),
            ],
        }))
        .expect("execute string in query");
    let mut string_ids: Vec<_> = strings
        .hits
        .iter()
        .map(|hit| hit.document_id.as_str())
        .collect();
    string_ids.sort_unstable();
    assert_eq!(string_ids, ["in-a", "in-c"]);

    let integers = engine
        .search(SearchRequest::new(Query::In {
            field: "qty".into(),
            values: vec![InQueryValue::Integer(2), InQueryValue::Integer(99)],
        }))
        .expect("execute integer in query");
    assert_eq!(
        integers
            .hits
            .iter()
            .map(|hit| hit.document_id.as_str())
            .collect::<Vec<_>>(),
        vec!["in-b"]
    );

    for values in [
        Vec::new(),
        vec![
            InQueryValue::String("alpha".into()),
            InQueryValue::Integer(1),
        ],
        vec![InQueryValue::Integer(i64::MIN)],
    ] {
        let error = engine
            .search(SearchRequest::new(Query::In {
                field: "code".into(),
                values,
            }))
            .expect_err("invalid in query unexpectedly executed");
        assert!(!error.to_string().is_empty());
    }

    engine.close().expect("close in-query engine");
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
            &Query::True,
            &GroupingPlan::new().level(
                GroupingLevel::new(GroupingExpression::attribute("qty"))
                    .aggregation(GroupingAggregation::Count)
                    .aggregation(GroupingAggregation::Average(GroupingExpression::attribute(
                        "qty",
                    ))),
            ),
        )
        .expect("execute local grouping");
    assert!(!grouped.groups.is_empty());

    engine.close().expect("close local search engine");
    let _ = fs::remove_dir_all(config);
    let _ = fs::remove_dir_all(data);
}

#[test]
fn native_attribute_exists_matches_keyword_and_boolean_vespa_semantics() {
    let config = temporary_path("exists-config");
    let data = temporary_path("exists-data");
    let schema = Schema::new("typea")
        .field(Field::new("title", FieldType::Text).indexed().summary())
        .field(
            Field::new("keyword", FieldType::String)
                .attribute()
                .summary(),
        )
        .field(
            Field::new("active", FieldType::Boolean)
                .attribute()
                .summary(),
        );
    ConfigCompiler
        .compile_to(&schema, &config)
        .expect("compile exists config");
    let engine = Engine::open(&data, &config).expect("open exists engine");
    engine
        .put(Document::new("absent").field("title", "absent"))
        .expect("put absent document");
    engine
        .put(
            Document::new("empty-false")
                .field("keyword", "")
                .field("active", false),
        )
        .expect("put empty and false document");
    engine
        .put(
            Document::new("value-true")
                .field("keyword", "present")
                .field("active", true),
        )
        .expect("put value and true document");

    let ids = |field: &str| {
        engine
            .search(SearchRequest::new(Query::AttributeRange {
                field: field.to_owned(),
                lower: searchkernel_vespa_engine::AttributeBound::Unbounded,
                upper: searchkernel_vespa_engine::AttributeBound::Unbounded,
            }))
            .expect("execute exists query")
            .hits
            .into_iter()
            .map(|hit| hit.document_id)
            .collect::<BTreeSet<_>>()
    };
    assert_eq!(ids("keyword"), BTreeSet::from(["value-true".to_owned()]));
    // Vespa BOOL attributes expose false for an absent source value, so
    // existence follows the attribute view rather than source-field presence.
    assert_eq!(
        ids("active"),
        BTreeSet::from([
            "absent".to_owned(),
            "empty-false".to_owned(),
            "value-true".to_owned(),
        ])
    );

    engine.close().expect("close exists engine");
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

    engine.close().expect("close generic search engine");
    let _ = fs::remove_dir_all(config);
    let _ = fs::remove_dir_all(data);
}

#[test]
fn native_boolean_attributes_support_sorting_and_faceting() {
    let config = temporary_path("boolean-capability-config");
    let data = temporary_path("boolean-capability-data");
    let schema = Schema::new("typea").field(
        Field::new("active", FieldType::Boolean)
            .attribute()
            .summary(),
    );
    ConfigCompiler
        .compile_to(&schema, &config)
        .expect("compile boolean capability config");
    let engine = Engine::open(&data, &config).expect("open boolean capability engine");

    engine
        .put(Document::new("false-1").field("active", false))
        .expect("put false boolean document");
    engine
        .put(Document::new("true-1").field("active", true))
        .expect("put true boolean document");
    engine
        .put(Document::new("false-2").field("active", false))
        .expect("put second false boolean document");

    let ascending = engine
        .search(SearchRequest::new(Query::True).sort(SortField::ascending("active")))
        .expect("sort boolean attribute ascending");
    assert_eq!(ascending.hits[0].document_id, "false-1");
    assert_eq!(ascending.hits[1].document_id, "false-2");
    assert_eq!(ascending.hits[2].document_id, "true-1");

    let faceted = engine
        .search(SearchRequest::new(Query::True).facet(FacetRequest::new(
            "active",
            2,
            GroupingOrder::ValueAscending,
        )))
        .expect("facet boolean attribute");
    assert_eq!(faceted.facets.len(), 1);
    assert_eq!(faceted.facets[0].groups.len(), 2);
    assert_eq!(faceted.facets[0].groups[0].key, Value::Boolean(false));
    assert_eq!(faceted.facets[0].groups[0].count, 2);
    assert_eq!(faceted.facets[0].groups[1].key, Value::Boolean(true));
    assert_eq!(faceted.facets[0].groups[1].count, 1);

    engine.close().expect("close boolean capability engine");
    let _ = fs::remove_dir_all(config);
    let _ = fs::remove_dir_all(data);
}

#[test]
fn native_grouping_preserves_native_order_and_recursive_children() {
    let config = temporary_path("group-order-config");
    let data = temporary_path("group-order-data");
    let schema = Schema::new("typea")
        .field(
            Field::new("category", FieldType::String)
                .attribute()
                .summary(),
        )
        .field(Field::new("qty", FieldType::Integer).attribute().summary());
    ConfigCompiler
        .compile_to(&schema, &config)
        .expect("compile group-order config");
    let engine = Engine::open(&data, &config).expect("open group-order engine");

    for (id, category) in [
        ("group-order-1", "zulu"),
        ("group-order-2", "zulu"),
        ("group-order-3", "alpha"),
    ] {
        engine
            .put(
                Document::new(id)
                    .field("category", category)
                    .field("qty", 1_i64),
            )
            .expect("put group-order document");
    }

    let result = engine
        .group(
            &Query::True,
            &GroupingPlan::new()
                .level(
                    GroupingLevel::new(GroupingExpression::attribute("category"))
                        .aggregation(GroupingAggregation::Count),
                )
                .level(
                    GroupingLevel::new(GroupingExpression::attribute("qty"))
                        .aggregation(GroupingAggregation::Count),
                ),
        )
        .expect("execute group-order grouping");
    assert_eq!(
        result
            .groups
            .iter()
            .map(|group| (&group.key, group.groups.len()))
            .collect::<Vec<_>>(),
        vec![
            (
                &Some(searchkernel_vespa_engine::GroupingResultValue::String(
                    "alpha".into()
                )),
                1
            ),
            (
                &Some(searchkernel_vespa_engine::GroupingResultValue::String(
                    "zulu".into()
                )),
                1
            ),
        ]
    );
    assert_eq!(
        result.groups[0].groups[0].key,
        Some(searchkernel_vespa_engine::GroupingResultValue::Integer(1))
    );
    assert_eq!(result.groups[0].groups[0].outputs.len(), 1);

    engine.close().expect("close group-order engine");
    let _ = fs::remove_dir_all(config);
    let _ = fs::remove_dir_all(data);
}

#[test]
fn native_grouping_supports_float_keys() {
    let config = temporary_path("float-group-config");
    let data = temporary_path("float-group-data");
    let schema =
        Schema::new("typea").field(Field::new("rating", FieldType::Float).attribute().summary());
    ConfigCompiler
        .compile_to(&schema, &config)
        .expect("compile float-group config");
    let engine = Engine::open(&data, &config).expect("open float-group engine");
    for (id, rating) in [("float-group-1", 1.5), ("float-group-2", 2.5)] {
        engine
            .put(Document::new(id).field("rating", rating))
            .expect("put float-group document");
    }

    let result = engine
        .group(
            &Query::True,
            &GroupingPlan::new().level(
                GroupingLevel::new(GroupingExpression::attribute("rating"))
                    .aggregation(GroupingAggregation::Count),
            ),
        )
        .expect("execute float-key grouping");
    assert_eq!(
        result
            .groups
            .iter()
            .map(|group| group.key.clone())
            .collect::<Vec<_>>(),
        vec![
            Some(searchkernel_vespa_engine::GroupingResultValue::Float(1.5)),
            Some(searchkernel_vespa_engine::GroupingResultValue::Float(2.5)),
        ]
    );

    engine.close().expect("close float-group engine");
    let _ = fs::remove_dir_all(config);
    let _ = fs::remove_dir_all(data);
}

#[test]
fn native_grouping_decodes_a_valid_empty_root() {
    let config = temporary_path("empty-group-config");
    let data = temporary_path("empty-group-data");
    let schema = Schema::new("typea").field(
        Field::new("category", FieldType::String)
            .attribute()
            .summary(),
    );
    ConfigCompiler
        .compile_to(&schema, &config)
        .expect("compile empty-group config");
    let engine = Engine::open(&data, &config).expect("open empty-group engine");
    engine
        .put(Document::new("empty-group-1").field("category", "present"))
        .expect("put empty-group document");

    let result = engine
        .group(
            &Query::AttributeEquals {
                field: "category".into(),
                value: Value::String("absent".into()),
            },
            &GroupingPlan::new().level(
                GroupingLevel::new(GroupingExpression::attribute("category"))
                    .aggregation(GroupingAggregation::Count),
            ),
        )
        .expect("execute empty grouping");
    assert_eq!(result.key, None);
    assert!(result.outputs.is_empty());
    assert!(result.groups.is_empty());

    engine.close().expect("close empty-group engine");
    let _ = fs::remove_dir_all(config);
    let _ = fs::remove_dir_all(data);
}
