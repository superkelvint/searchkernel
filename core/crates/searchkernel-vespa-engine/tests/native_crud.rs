#![cfg(feature = "native")]

use searchkernel_vespa_engine::{
    ArithmeticOperator, ArrayValue, AttributeDocument, AttributeUpdate, ConfigCompiler, Document,
    DocumentUpdate, Engine, FacetRequest, Field, FieldType, FieldValue, GroupingAggregation,
    GroupingExpression, GroupingFilter, GroupingLevel, GroupingOrder, GroupingPlan, InQueryValue,
    LinguisticsOptions, NearestNeighborOptions, PositionValue, PredicateValue, Query, QueryInput,
    RankProfile, ScalarValue, Schema, SearchRequest, SortField, StructField, StructFieldType,
    SummaryRequest, TensorValue, Value,
};
use std::collections::{BTreeMap, BTreeSet};
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
fn native_arithmetic_updates_survive_read_and_reopen() {
    let config = temporary_path("arithmetic-config");
    let data = temporary_path("arithmetic-data");
    let schema =
        Schema::new("typea").field(Field::new("rating", FieldType::Float).attribute().summary());
    ConfigCompiler
        .compile_to(&schema, &config)
        .expect("compile arithmetic config");
    let engine = Engine::open(&data, &config).expect("open arithmetic engine");
    engine
        .put(Document::new("arithmetic-1").field("rating", 4.0_f64))
        .expect("put arithmetic baseline");

    for (operator, operand) in [
        (ArithmeticOperator::Add, 1.5),
        (ArithmeticOperator::Subtract, 0.5),
        (ArithmeticOperator::Multiply, 2.0),
        (ArithmeticOperator::Divide, 4.0),
    ] {
        engine
            .update(DocumentUpdate::new("arithmetic-1").arithmetic("rating", operator, operand))
            .expect("apply arithmetic update");
    }
    let updated = engine
        .get("arithmetic-1")
        .expect("read arithmetic update")
        .expect("arithmetic document exists");
    assert_eq!(
        updated.fields()["rating"],
        FieldValue::Scalar(ScalarValue::Float(2.5))
    );
    engine
        .update(
            DocumentUpdate::new("arithmetic-created")
                .assign("rating", 7.0_f64)
                .create_if_nonexistent(true),
        )
        .expect("create-if-nonexistent update");
    assert_eq!(
        engine
            .get("arithmetic-created")
            .expect("read created document")
            .expect("created document exists")
            .fields()["rating"],
        FieldValue::Scalar(ScalarValue::Float(7.0))
    );
    engine.close().expect("close arithmetic engine");

    let reopened = Engine::open(&data, &config).expect("reopen arithmetic engine");
    let persisted = reopened
        .get("arithmetic-1")
        .expect("read persisted arithmetic update")
        .expect("persisted arithmetic document exists");
    assert_eq!(
        persisted.fields()["rating"],
        FieldValue::Scalar(ScalarValue::Float(2.5))
    );
    reopened.close().expect("close reopened arithmetic engine");
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
fn native_nearest_neighbor_options_accept_zero_explore_additional_hits() {
    let config = temporary_path("nearest-options-config");
    let data = temporary_path("nearest-options-data");
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
        .expect("compile nearest-neighbor options config");
    let engine = Engine::open(&data, &config).expect("open nearest-neighbor options engine");
    engine
        .put(Document::new("near-options").field(
            "embedding",
            TensorValue::dense("tensor<float>(x[2])", [1.0, 0.0]),
        ))
        .expect("put nearest-neighbor options document");

    let result = engine
        .search(
            SearchRequest::new(Query::NearestNeighborWithOptions {
                document_field: "embedding".to_owned(),
                query_tensor: "query_embedding".to_owned(),
                options: NearestNeighborOptions {
                    target_num_hits: 1,
                    approximate: true,
                    distance_threshold: None,
                    explore_additional_hits: 0,
                },
            })
            .rank_profile("semantic")
            .input(
                "query_embedding",
                QueryInput::Literal(TensorValue::dense("tensor<float>(x[2])", [1.0, 0.0])),
            )
            .limit(1),
        )
        .expect("execute nearest-neighbor options search");
    assert_eq!(result.hits.len(), 1);
    assert_eq!(result.hits[0].document_id, "near-options");

    engine
        .close()
        .expect("close nearest-neighbor options engine");
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

    let explicit_default = engine
        .search(
            SearchRequest::new(Query::True)
                .summary_class("default")
                .fields(["title"]),
        )
        .expect("explicit default summary class should execute");
    assert_eq!(explicit_default.total_hits, 2);
    let explicit_default_local_1 = explicit_default
        .hits
        .iter()
        .find(|hit| hit.document_id == "local-1")
        .expect("explicit default result should include local-1");
    assert_eq!(explicit_default_local_1.fields.len(), 1);
    assert_eq!(
        explicit_default_local_1.fields["title"],
        FieldValue::Scalar(ScalarValue::String("alpha".to_owned()))
    );

    let nonexistent = engine
        .search(
            SearchRequest::new(Query::True)
                .summary_class("definitely_nonexistent_summary_class")
                .fields(["title"]),
        )
        .expect_err("unknown result class must not fall back to default docsums");
    assert!(nonexistent
        .to_string()
        .contains("docsum returned no result"));

    let empty_class = engine
        .search(
            SearchRequest::new(Query::True)
                .summary_class("")
                .fields(["title"]),
        )
        .expect_err("an empty summary class must be rejected");
    assert!(empty_class
        .to_string()
        .contains("summary class must not be empty"));

    let error = engine
        .put_attributes(&AttributeDocument::new("local-1").field("title", "not-an-attribute"))
        .expect_err("attribute puts must reject indexed-only fields");
    assert!(error.to_string().contains("not an attribute"));

    engine
        .put_attributes(&AttributeDocument::new("local-1").field("qty", 11_i64))
        .expect("put attribute document");
    engine
        .update_attributes(&AttributeUpdate::new("local-1").assign("qty", 13_i64))
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

// SKC-unverified: result.highlighting/basic kinds=e2e runtime=native
// SKC-unverified: result.highlighting/literal-html kinds=e2e runtime=native
#[test]
fn native_dynamic_teaser_highlighting_uses_vespa_docsums() {
    let config = temporary_path("highlight-config");
    let data = temporary_path("highlight-data");
    let schema = Schema::new("typea")
        .field(Field::new("title", FieldType::Text).indexed().summary())
        .field(
            Field::new("description", FieldType::Text)
                .indexed()
                .summary(),
        )
        .field(Field::new("markup", FieldType::Text).indexed().summary())
        .field(
            Field::new("category", FieldType::String)
                .attribute()
                .summary(),
        );
    ConfigCompiler
        .compile_to(&schema, &config)
        .expect("compile highlight config");
    let engine = Engine::open(&data, &config).expect("open highlight engine");
    engine
        .put(
            Document::new("highlight-1")
                .field("title", "The quick brown fox jumps over the lazy dog")
                .field("description", "A brown fox appears near the quiet river")
                .field("markup", "A <b>stored text</b> remains literal")
                .field("category", "animal"),
        )
        .expect("put highlight document");

    let result = engine
        .search(
            SearchRequest::new(Query::And(vec![
                Query::term("title", "brown"),
                Query::term("title", "fox"),
                Query::term("description", "fox"),
                Query::term("markup", "literal"),
            ]))
            .summary_class("default")
            .fields(["category"])
            .highlight(["title", "description", "markup"]),
        )
        .expect("execute highlight search");
    assert_eq!(result.hits.len(), 1);
    assert_eq!(
        result.hits[0].fields["category"],
        FieldValue::Scalar(ScalarValue::String("animal".to_owned()))
    );
    assert_eq!(result.hits[0].fields.len(), 1);
    assert_eq!(result.hits[0].highlights.len(), 3);
    assert_eq!(result.hits[0].highlights[0].field, "title");
    assert_eq!(
        result.hits[0].highlights[0].fragments[0].text,
        "The quick brown fox jumps over the lazy dog"
    );
    assert_eq!(
        result.hits[0].highlights[0].fragments[0].matches,
        vec![
            searchkernel_vespa_engine::HighlightSpan {
                start_byte: 10,
                end_byte: 15,
            },
            searchkernel_vespa_engine::HighlightSpan {
                start_byte: 16,
                end_byte: 19,
            },
        ]
    );
    assert_eq!(result.hits[0].highlights[1].field, "description");
    assert_eq!(
        result.hits[0].highlights[1].fragments[0].matches,
        vec![searchkernel_vespa_engine::HighlightSpan {
            start_byte: 8,
            end_byte: 11,
        }]
    );
    assert_eq!(result.hits[0].highlights[2].field, "markup");
    assert_eq!(
        result.hits[0].highlights[2].fragments[0].text,
        "A <b>stored text</b> remains literal"
    );
    let markup_fragment = &result.hits[0].highlights[2].fragments[0];
    assert_eq!(markup_fragment.matches.len(), 1);
    let markup_match = markup_fragment.matches[0];
    assert_eq!(
        &markup_fragment.text[markup_match.start_byte as usize..markup_match.end_byte as usize],
        "literal"
    );
    assert!(result.hits[0]
        .highlights
        .iter()
        .filter(|highlight| highlight.field != "markup")
        .all(|highlight| {
            highlight.fragments.iter().any(|fragment| {
                !fragment.matches.is_empty()
                    && fragment
                        .matches
                        .iter()
                        .all(|span| span.end_byte as usize <= fragment.text.len())
            })
        }));

    let empty_projection = engine
        .search(
            SearchRequest::new(Query::term("title", "brown"))
                .summary(SummaryRequest::None)
                .highlight(["title"]),
        )
        .expect("highlight with empty projection should execute");
    assert_eq!(empty_projection.hits.len(), 1);
    assert!(empty_projection.hits[0].fields.is_empty());
    assert_eq!(empty_projection.hits[0].highlights.len(), 1);

    let no_matching_field = engine
        .search(SearchRequest::new(Query::term("title", "quick")).highlight(["description"]))
        .expect("highlight without a matching requested field");
    assert_eq!(no_matching_field.hits.len(), 1);
    assert_eq!(no_matching_field.hits[0].highlights.len(), 1);
    assert_eq!(
        no_matching_field.hits[0].highlights[0].fragments[0].text,
        "A brown fox appears near the quiet river"
    );
    assert!(no_matching_field.hits[0].highlights[0].fragments[0]
        .matches
        .is_empty());

    assert!(engine
        .search(SearchRequest::new(Query::term("title", "brown")).highlight(["category"]))
        .is_err());
    assert!(engine
        .search(SearchRequest::new(Query::term("title", "brown")).highlight(["title", "title"]))
        .is_err());

    engine.close().expect("close highlight engine");
    let _ = fs::remove_dir_all(config);
    let _ = fs::remove_dir_all(data);
}

// SKC-unverified: result.highlighting/continuation-fragments kinds=e2e runtime=native
#[test]
fn native_dynamic_teaser_highlighting_preserves_continuation_fragments() {
    let config = temporary_path("highlight-continuation-config");
    let data = temporary_path("highlight-continuation-data");
    let schema = Schema::new("highlight_continuation")
        .field(Field::new("title", FieldType::Text).indexed().summary());
    ConfigCompiler
        .compile_to(&schema, &config)
        .expect("compile continuation highlight config");
    fs::write(
        config.join("juniperrc.cfg"),
        "length 128\nmax_matches 3\nmin_length 0\nsurround_max 24\n",
    )
    .expect("write continuation highlight Juniper configuration");
    let engine = Engine::open(&data, &config).expect("open continuation highlight engine");

    let title = format!(
        "brown {} fox {} café",
        "unrelated ".repeat(60),
        "context ".repeat(60)
    );
    engine
        .put(Document::new("highlight-continuation-1").field("title", title))
        .expect("put continuation highlight document");

    let result = engine
        .search(
            SearchRequest::new(Query::Or(vec![
                Query::term("title", "brown"),
                Query::term("title", "fox"),
                Query::term("title", "café"),
            ]))
            .highlight(["title"]),
        )
        .expect("execute continuation highlight search");
    let highlight = &result.hits[0].highlights[0];
    assert!(
        highlight.fragments.len() >= 2,
        "expected continuation fragments, got {:#?}",
        highlight.fragments
    );
    assert!(highlight.fragments.iter().all(|fragment| {
        !fragment.text.contains('\u{1e}')
            && fragment.matches.iter().all(|span| {
                let start = span.start_byte as usize;
                let end = span.end_byte as usize;
                start < end
                    && end <= fragment.text.len()
                    && fragment.text.is_char_boundary(start)
                    && fragment.text.is_char_boundary(end)
            })
    }));
    assert!(highlight.fragments.iter().any(|fragment| {
        fragment
            .matches
            .iter()
            .any(|span| &fragment.text[span.start_byte as usize..span.end_byte as usize] == "brown")
    }));
    assert!(highlight.fragments.iter().any(|fragment| {
        fragment
            .matches
            .iter()
            .any(|span| &fragment.text[span.start_byte as usize..span.end_byte as usize] == "café")
    }));

    engine.close().expect("close continuation highlight engine");
    let _ = fs::remove_dir_all(config);
    let _ = fs::remove_dir_all(data);
}

#[test]
fn native_juniper_highlighting_is_field_aware_for_unsupported_query_shapes() {
    let config = temporary_path("highlight-query-shape-config");
    let data = temporary_path("highlight-query-shape-data");
    let schema = Schema::new("typea")
        .field(Field::new("title", FieldType::Text).indexed().summary())
        .field(
            Field::new("category", FieldType::String)
                .attribute()
                .summary(),
        );
    ConfigCompiler
        .compile_to(&schema, &config)
        .expect("compile highlight query-shape config");
    let engine = Engine::open(&data, &config).expect("open highlight query-shape engine");
    engine
        .put(
            Document::new("highlight-query-shape-1")
                .field("title", "The quick brown fox")
                .field("category", "animal"),
        )
        .expect("put highlight query-shape document");

    for query in [
        Query::Fuzzy {
            field: "title".to_owned(),
            text: "brown".to_owned(),
            max_edit_distance: 1,
            prefix_lock_length: 0,
            prefix_match: false,
        },
        Query::Matches {
            field: "title".to_owned(),
            pattern: "brown".to_owned(),
        },
    ] {
        let result = engine
            .search(SearchRequest::new(query).highlight(["title"]))
            .expect("same-field unsupported query shape should execute natively");
        assert_eq!(result.hits.len(), 1);
        assert_eq!(result.hits[0].highlights.len(), 1);
        assert!(result.hits[0].highlights[0]
            .fragments
            .iter()
            .all(|fragment| fragment.matches.is_empty()));
    }

    let mixed = engine
        .search(
            SearchRequest::new(Query::And(vec![
                Query::term("title", "brown"),
                Query::In {
                    field: "category".to_owned(),
                    values: vec![InQueryValue::String("animal".to_owned())],
                },
            ]))
            .highlight(["title"]),
        )
        .expect("an unrelated attribute query must preserve title highlighting");
    assert_eq!(mixed.hits.len(), 1);
    assert_eq!(mixed.hits[0].highlights.len(), 1);
    assert_eq!(
        mixed.hits[0].highlights[0].fragments[0].matches,
        vec![searchkernel_vespa_engine::HighlightSpan {
            start_byte: 10,
            end_byte: 15,
        }]
    );

    engine.close().expect("close highlight query-shape engine");
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
        .update(DocumentUpdate::new("text-1").assign("title", "slow hare"))
        .expect("update analyzed text");

    let term = engine
        .search(SearchRequest::new(Query::term("title", "hare")))
        .expect("search text term");
    assert_eq!(
        term.hits
            .iter()
            .map(|hit| hit.document_id.as_str())
            .collect::<Vec<_>>(),
        ["text-1"]
    );

    let phrase = engine
        .search(SearchRequest::new(Query::phrase("title", "slow hare")))
        .expect("search text phrase");
    assert_eq!(phrase.hits.len(), 1);

    let removed = engine
        .search(SearchRequest::new(Query::term("title", "quick")))
        .expect("search removed text term");
    assert!(removed.hits.is_empty());

    let typed = engine
        .search(SearchRequest::new(Query::term("title", "hare")))
        .expect("search typed query");
    let yql = engine
        .search(SearchRequest::new(
            Query::from_yql("select * from typea where title contains \"hare\"")
                .expect("parse YQL query"),
        ))
        .expect("search YQL query");
    assert_eq!(typed, yql);

    engine.close().expect("close text engine");
    let _ = fs::remove_dir_all(config);
    let _ = fs::remove_dir_all(data);
}

#[test]
fn native_same_element_query_keeps_struct_members_in_one_array_element() {
    let config = temporary_path("same-element-config");
    let data = temporary_path("same-element-data");
    let schema = Schema::new("typea").field(
        Field::new(
            "people",
            FieldType::ArrayStruct(vec![
                StructField {
                    name: "first".to_owned(),
                    field_type: StructFieldType::String,
                },
                StructField {
                    name: "last".to_owned(),
                    field_type: StructFieldType::String,
                },
            ]),
        )
        .attribute(),
    );
    ConfigCompiler
        .compile_to(&schema, &config)
        .expect("compile same-element config");
    let engine = Engine::open(&data, &config).expect("open same-element engine");
    let people = |pairs: &[(&str, &str)]| {
        FieldValue::Array(ArrayValue::Structs(
            pairs
                .iter()
                .map(|(first, last)| {
                    BTreeMap::from([
                        ("first".to_owned(), ScalarValue::String((*first).to_owned())),
                        ("last".to_owned(), ScalarValue::String((*last).to_owned())),
                    ])
                })
                .collect(),
        ))
    };
    engine
        .put(Document::new("same-1").field(
            "people",
            people(&[("Ada", "Lovelace"), ("Grace", "Hopper")]),
        ))
        .expect("put matching array-of-struct document");
    engine
        .put(Document::new("same-2").field("people", people(&[("Ada", "Hopper")])))
        .expect("put non-matching array-of-struct document");

    let result = engine
        .search(SearchRequest::new(Query::SameElement {
            field: "people".to_owned(),
            children: vec![
                Query::Term {
                    field: "people.first".to_owned(),
                    text: "Ada".to_owned(),
                },
                Query::Term {
                    field: "people.last".to_owned(),
                    text: "Lovelace".to_owned(),
                },
            ],
            element_filter: Vec::new(),
        }))
        .expect("execute same-element query");
    assert_eq!(
        result
            .hits
            .iter()
            .map(|hit| hit.document_id.as_str())
            .collect::<Vec<_>>(),
        ["same-1"]
    );

    let filtered = |element_filter| {
        engine
            .search(SearchRequest::new(Query::SameElement {
                field: "people".to_owned(),
                children: vec![
                    Query::Term {
                        field: "people.first".to_owned(),
                        text: "Ada".to_owned(),
                    },
                    Query::Term {
                        field: "people.last".to_owned(),
                        text: "Lovelace".to_owned(),
                    },
                ],
                element_filter,
            }))
            .expect("execute filtered same-element query")
            .hits
            .into_iter()
            .map(|hit| hit.document_id)
            .collect::<Vec<_>>()
    };
    assert_eq!(filtered(vec![0]), ["same-1"]);
    assert!(filtered(vec![1]).is_empty());

    engine.close().expect("close same-element engine");
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
fn native_grouping_supports_boolean_keys() {
    let config = temporary_path("boolean-group-config");
    let data = temporary_path("boolean-group-data");
    let schema = Schema::new("typea").field(
        Field::new("active", FieldType::Boolean)
            .attribute()
            .summary(),
    );
    ConfigCompiler
        .compile_to(&schema, &config)
        .expect("compile boolean-group config");
    let engine = Engine::open(&data, &config).expect("open boolean-group engine");
    for (id, active) in [("boolean-group-false", false), ("boolean-group-true", true)] {
        engine
            .put(Document::new(id).field("active", active))
            .expect("put boolean-group document");
    }

    let result = engine
        .group(
            &Query::True,
            &GroupingPlan::new().level(
                GroupingLevel::new(GroupingExpression::Literal(
                    searchkernel_vespa_engine::GroupingLiteral::Boolean(true),
                ))
                .aggregation(GroupingAggregation::Count),
            ),
        )
        .expect("execute boolean-key grouping");
    assert_eq!(
        result
            .groups
            .iter()
            .map(|group| group.key.clone())
            .collect::<Vec<_>>(),
        vec![Some(
            searchkernel_vespa_engine::GroupingResultValue::Boolean(true)
        )]
    );

    engine.close().expect("close boolean-group engine");
    let _ = fs::remove_dir_all(config);
    let _ = fs::remove_dir_all(data);
}

#[test]
fn native_raw_sort_spec_is_available_through_the_safe_request_wrapper() {
    let config = temporary_path("raw-sort-config");
    let data = temporary_path("raw-sort-data");
    let schema = Schema::new("typea").field(Field::new("category", FieldType::String).attribute());
    ConfigCompiler
        .compile_to(&schema, &config)
        .expect("compile raw sort config");
    let engine = Engine::open(&data, &config).expect("open raw sort engine");
    for (id, category) in [("raw-sort-b", "B"), ("raw-sort-a", "A")] {
        engine
            .put(Document::new(id).field("category", category))
            .expect("put raw sort document");
    }
    let result = engine
        .search(SearchRequest::new(Query::True).raw_sort_spec("+category"))
        .expect("execute raw sort search");
    let ids: Vec<_> = result
        .hits
        .iter()
        .map(|hit| hit.document_id.as_str())
        .collect();
    assert_eq!(ids, ["raw-sort-a", "raw-sort-b"]);

    engine.close().expect("close raw sort engine");
    let _ = fs::remove_dir_all(config);
    let _ = fs::remove_dir_all(data);
}

#[test]
fn native_grouping_decodes_quantiles_and_summary_hits() {
    let config = temporary_path("group-extensions-config");
    let data = temporary_path("group-extensions-data");
    let schema = Schema::new("typea")
        .field(
            Field::new("category", FieldType::String)
                .attribute()
                .summary(),
        )
        .field(Field::new("qty", FieldType::Integer).attribute().summary())
        .field(Field::new("rating", FieldType::Float).attribute().summary())
        .field(Field::new("title", FieldType::Text).indexed().summary());
    ConfigCompiler
        .compile_to(&schema, &config)
        .expect("compile grouping extension config");
    let engine = Engine::open(&data, &config).expect("open grouping extension engine");
    for (id, qty, rating, title) in [
        ("group-extension-1", 1_i64, 10.0_f64, "first"),
        ("group-extension-2", 2_i64, 20.0_f64, "second"),
    ] {
        engine
            .put(
                Document::new(id)
                    .field("category", "all")
                    .field("qty", qty)
                    .field("rating", rating)
                    .field("title", title),
            )
            .expect("put grouping extension document");
    }
    let level = GroupingLevel::new(GroupingExpression::attribute("category"))
        .aggregation(GroupingAggregation::Count)
        .aggregation(GroupingAggregation::Sum(GroupingExpression::attribute(
            "rating",
        )))
        .aggregation(GroupingAggregation::Min(GroupingExpression::attribute(
            "rating",
        )))
        .aggregation(GroupingAggregation::Max(GroupingExpression::attribute(
            "rating",
        )))
        .aggregation(GroupingAggregation::StandardDeviation(
            GroupingExpression::attribute("rating"),
        ))
        .aggregation(GroupingAggregation::Xor(GroupingExpression::attribute(
            "qty",
        )))
        .aggregation(GroupingAggregation::Quantiles {
            expression: GroupingExpression::attribute("rating"),
            quantiles: vec![0.5, 1.0],
        })
        .aggregation(GroupingAggregation::Summary {
            summary_class: "default".to_owned(),
            max_hits: 2,
        });
    let result = engine
        .group(&Query::True, &GroupingPlan::new().level(level))
        .expect("execute grouping extensions");
    assert_eq!(result.groups.len(), 1);
    assert_eq!(result.groups[0].outputs.len(), 8);
    let hits = result.groups[0]
        .outputs
        .iter()
        .find_map(|output| match output {
            searchkernel_vespa_engine::GroupingOutput::HitList(hits) => Some(hits),
            searchkernel_vespa_engine::GroupingOutput::Value(_) => None,
        })
        .expect("summary aggregation should return grouped hits");
    assert_eq!(
        hits.iter()
            .map(|hit| hit.document_id.as_str())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["group-extension-1", "group-extension-2"])
    );
    let first = hits
        .iter()
        .find(|hit| hit.document_id == "group-extension-1")
        .expect("first grouped hit");
    assert!(matches!(
        first.fields.get("title"),
        Some(FieldValue::Scalar(ScalarValue::String(value))) if value == "first"
    ));
    assert!(result.groups[0].outputs.iter().any(|output| matches!(
        output,
        searchkernel_vespa_engine::GroupingOutput::Value(
            searchkernel_vespa_engine::GroupingResultValue::Quantiles(values)
        ) if values.len() == 2
    )));
    engine.close().expect("close grouping extension engine");
    let _ = fs::remove_dir_all(config);
    let _ = fs::remove_dir_all(data);
}

#[test]
fn native_grouping_filter_survives_serialization_and_filters_groups() {
    let config = temporary_path("group-filter-config");
    let data = temporary_path("group-filter-data");
    let schema = Schema::new("typea").field(
        Field::new("category", FieldType::String)
            .attribute()
            .summary(),
    );
    ConfigCompiler
        .compile_to(&schema, &config)
        .expect("compile grouping filter config");
    let engine = Engine::open(&data, &config).expect("open grouping filter engine");
    for (id, category) in [("group-filter-a", "A"), ("group-filter-b", "B")] {
        engine
            .put(Document::new(id).field("category", category))
            .expect("put grouping filter document");
    }

    let filtered = GroupingLevel::new(GroupingExpression::attribute("category"))
        .aggregation(GroupingAggregation::Count)
        .filter(GroupingFilter::Regex {
            field: "category".to_owned(),
            pattern: "^A$".to_owned(),
        });
    let result = engine
        .group(&Query::True, &GroupingPlan::new().level(filtered))
        .expect("execute positive grouping filter");
    assert_eq!(result.groups.len(), 1);
    assert_eq!(
        result.groups[0].key,
        Some(searchkernel_vespa_engine::GroupingResultValue::String(
            "A".to_owned(),
        ))
    );

    let rejected = GroupingLevel::new(GroupingExpression::attribute("category"))
        .aggregation(GroupingAggregation::Count)
        .filter(GroupingFilter::Regex {
            field: "category".to_owned(),
            pattern: "^Z$".to_owned(),
        });
    let result = engine
        .group(&Query::True, &GroupingPlan::new().level(rejected))
        .expect("execute negative grouping filter");
    assert!(result.groups.is_empty());

    engine.close().expect("close grouping filter engine");
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

#[test]
fn native_get_round_trips_composite_values_across_reopen() {
    let config = temporary_path("composite-get-config");
    let data = temporary_path("composite-get-data");
    let schema = Schema::new("typea")
        .field(Field::new("tags", FieldType::ArrayString).attribute())
        .field(
            Field::new(
                "people",
                FieldType::ArrayStruct(vec![
                    StructField {
                        name: "name".to_owned(),
                        field_type: StructFieldType::String,
                    },
                    StructField {
                        name: "age".to_owned(),
                        field_type: StructFieldType::Long,
                    },
                    StructField {
                        name: "active".to_owned(),
                        field_type: StructFieldType::Boolean,
                    },
                    StructField {
                        name: "score".to_owned(),
                        field_type: StructFieldType::Float,
                    },
                ]),
            )
            .attribute(),
        )
        .field(Field::new("location", FieldType::Position).attribute())
        .field(
            Field::new("feature_access", FieldType::Predicate)
                .attribute()
                .predicate_arity(8),
        )
        .field(
            Field::new("expression_access", FieldType::Predicate)
                .attribute()
                .predicate_arity(8),
        );
    ConfigCompiler
        .compile_to(&schema, &config)
        .expect("compile composite GET config");

    let tags = FieldValue::Array(ArrayValue::Strings(vec![
        "alpha".to_owned(),
        "".to_owned(),
        "omega".to_owned(),
    ]));
    let people = FieldValue::Array(ArrayValue::Structs(vec![
        BTreeMap::from([
            ("active".to_owned(), ScalarValue::Boolean(true)),
            ("age".to_owned(), ScalarValue::Integer(37)),
            ("name".to_owned(), ScalarValue::String("Ada".to_owned())),
            ("score".to_owned(), ScalarValue::Float(1.23456789)),
        ]),
        BTreeMap::from([
            ("active".to_owned(), ScalarValue::Boolean(false)),
            ("age".to_owned(), ScalarValue::Integer(85)),
            ("name".to_owned(), ScalarValue::String("Grace".to_owned())),
            ("score".to_owned(), ScalarValue::Float(9.87654321)),
        ]),
    ]));
    let location = FieldValue::Position(PositionValue::new(37.774929, -122.419416));
    let feature_access = FieldValue::Predicate(PredicateValue::new(BTreeMap::from([(
        "hobby".to_owned(),
        "music".to_owned(),
    )])));
    let expression_access = FieldValue::Predicate(PredicateValue::from_expression(
        r#"{"type":4,"key":"hobby","feature_set":["music"]}"#,
    ));
    // Vespa normalizes predicate-expression JSON before storing it. Object
    // member order is not semantic, so assert the canonical native form
    // returned by GET rather than the source spelling sent to upsert.
    let expected_expression_access = FieldValue::Predicate(PredicateValue::from_expression(
        r#"{"type":4,"feature_set":["music"],"key":"hobby"}"#,
    ));
    let expected = BTreeMap::from([
        ("expression_access".to_owned(), expected_expression_access),
        ("feature_access".to_owned(), feature_access.clone()),
        ("location".to_owned(), location.clone()),
        ("people".to_owned(), people.clone()),
        ("tags".to_owned(), tags.clone()),
    ]);

    let engine = Engine::open(&data, &config).expect("open composite GET engine");
    engine
        .put(
            Document::new("composite-1")
                .field("tags", tags)
                .field("people", people)
                .field("location", location)
                .field("feature_access", feature_access)
                .field("expression_access", expression_access),
        )
        .expect("put composite document");

    let current = engine
        .get("composite-1")
        .expect("get composite document before close")
        .expect("composite document exists before close");
    assert_eq!(current.fields(), &expected);
    engine.close().expect("close composite GET engine");

    let reopened = Engine::open(&data, &config).expect("reopen composite GET engine");
    let persisted = reopened
        .get("composite-1")
        .expect("get composite document after reopen")
        .expect("composite document exists after reopen");
    assert_eq!(persisted.fields(), &expected);

    reopened
        .close()
        .expect("close reopened composite GET engine");
    let _ = fs::remove_dir_all(config);
    let _ = fs::remove_dir_all(data);
}

#[test]
fn native_get_preserves_explicit_empty_collections_across_reopen() {
    let config = temporary_path("empty-composite-get-config");
    let data = temporary_path("empty-composite-get-data");
    let schema = Schema::new("typea")
        .field(Field::new("marker", FieldType::String).attribute())
        .field(Field::new("tags", FieldType::ArrayString).attribute())
        .field(
            Field::new(
                "people",
                FieldType::ArrayStruct(vec![StructField {
                    name: "name".to_owned(),
                    field_type: StructFieldType::String,
                }]),
            )
            .attribute(),
        );
    ConfigCompiler
        .compile_to(&schema, &config)
        .expect("compile empty composite GET config");

    let engine = Engine::open(&data, &config).expect("open empty composite GET engine");
    engine
        .put(
            Document::new("explicit-empty")
                .field("marker", "present")
                .field("tags", FieldValue::Array(ArrayValue::Strings(Vec::new())))
                .field("people", FieldValue::Array(ArrayValue::Structs(Vec::new()))),
        )
        .expect("put explicit empty composite document");
    engine
        .put(Document::new("absent").field("marker", "present"))
        .expect("put document with absent collection fields");
    engine
        .put(
            Document::new("updated-empty")
                .field("marker", "present")
                .field(
                    "tags",
                    FieldValue::Array(ArrayValue::Strings(vec!["before".to_owned()])),
                ),
        )
        .expect("put document before assigning empty array");
    engine
        .update(
            DocumentUpdate::new("updated-empty")
                .assign("tags", FieldValue::Array(ArrayValue::Strings(Vec::new()))),
        )
        .expect("assign explicit empty string array");
    engine
        .put(
            Document::new("updated-empty-people")
                .field("marker", "present")
                .field(
                    "people",
                    FieldValue::Array(ArrayValue::Structs(vec![BTreeMap::from([(
                        "name".to_owned(),
                        ScalarValue::String("before".to_owned()),
                    )])])),
                ),
        )
        .expect("put document before assigning empty struct array");
    engine
        .update(
            DocumentUpdate::new("updated-empty-people")
                .assign("people", FieldValue::Array(ArrayValue::Structs(Vec::new()))),
        )
        .expect("assign explicit empty struct array");
    engine
        .put(
            Document::new("cleared-array")
                .field("marker", "present")
                .field(
                    "tags",
                    FieldValue::Array(ArrayValue::Strings(vec!["before".to_owned()])),
                ),
        )
        .expect("put document before clearing array");
    engine
        .update(DocumentUpdate::new("cleared-array").clear("tags"))
        .expect("clear string array");

    let assert_presence = |engine: &Engine| {
        let explicit = engine
            .get("explicit-empty")
            .expect("get explicit-empty document")
            .expect("explicit-empty document exists");
        assert_eq!(
            explicit.fields().get("tags"),
            Some(&FieldValue::Array(ArrayValue::Strings(Vec::new())))
        );
        assert_eq!(
            explicit.fields().get("people"),
            Some(&FieldValue::Array(ArrayValue::Structs(Vec::new())))
        );

        let absent = engine
            .get("absent")
            .expect("get absent document")
            .expect("absent document exists");
        assert!(!absent.fields().contains_key("tags"));
        assert!(!absent.fields().contains_key("people"));

        let updated = engine
            .get("updated-empty")
            .expect("get updated-empty document")
            .expect("updated-empty document exists");
        assert_eq!(
            updated.fields().get("tags"),
            Some(&FieldValue::Array(ArrayValue::Strings(Vec::new())))
        );

        let updated_people = engine
            .get("updated-empty-people")
            .expect("get updated-empty-people document")
            .expect("updated-empty-people document exists");
        assert_eq!(
            updated_people.fields().get("people"),
            Some(&FieldValue::Array(ArrayValue::Structs(Vec::new())))
        );

        let cleared = engine
            .get("cleared-array")
            .expect("get cleared-array document")
            .expect("cleared-array document exists");
        assert!(!cleared.fields().contains_key("tags"));
    };

    assert_presence(&engine);
    engine.close().expect("close empty composite GET engine");

    let reopened = Engine::open(&data, &config).expect("reopen empty composite GET engine");
    assert_presence(&reopened);
    reopened
        .close()
        .expect("close reopened empty composite GET engine");

    let _ = fs::remove_dir_all(config);
    let _ = fs::remove_dir_all(data);
}

#[test]
fn native_ffi_accepts_zero_byte_string_arrays_and_predicate_features() {
    let config = temporary_path("zero-byte-packed-config");
    let data = temporary_path("zero-byte-packed-data");
    let schema = Schema::new("typea")
        .field(Field::new("tags", FieldType::ArrayString).attribute())
        .field(
            Field::new("feature", FieldType::Predicate)
                .attribute()
                .predicate_arity(8),
        );
    ConfigCompiler
        .compile_to(&schema, &config)
        .expect("compile zero-byte packed config");
    let engine = Engine::open(&data, &config).expect("open zero-byte packed engine");

    for (id, values) in [
        ("one-empty-string", vec![String::new()]),
        ("two-empty-strings", vec![String::new(), String::new()]),
    ] {
        engine
            .put(Document::new(id).field("tags", FieldValue::Array(ArrayValue::Strings(values))))
            .expect("Rust string-array packing should cross the native ABI");
    }

    for (id, key, value) in [
        ("empty-key", "", "value"),
        ("empty-value", "key", ""),
        ("empty-key-value", "", ""),
    ] {
        engine
            .put(Document::new(id).field(
                "feature",
                FieldValue::Predicate(PredicateValue::new(BTreeMap::from([(
                    key.to_owned(),
                    value.to_owned(),
                )]))),
            ))
            .expect("Rust predicate packing should cross the native ABI");
    }

    engine
        .put(Document::new("update-one-empty").field(
            "tags",
            FieldValue::Array(ArrayValue::Strings(vec!["before".to_owned()])),
        ))
        .expect("put document before string-array update");
    engine
        .update(DocumentUpdate::new("update-one-empty").assign(
            "tags",
            FieldValue::Array(ArrayValue::Strings(vec![String::new()])),
        ))
        .expect("single empty string update should cross the native ABI");

    engine
        .put(Document::new("update-two-empty").field(
            "tags",
            FieldValue::Array(ArrayValue::Strings(vec!["before".to_owned()])),
        ))
        .expect("put document before multi-string-array update");
    engine
        .update(DocumentUpdate::new("update-two-empty").assign(
            "tags",
            FieldValue::Array(ArrayValue::Strings(vec![String::new(), String::new()])),
        ))
        .expect("multiple empty strings update should cross the native ABI");

    engine.close().expect("close zero-byte packed engine");
    let _ = fs::remove_dir_all(config);
    let _ = fs::remove_dir_all(data);
}

#[test]
fn diagnostic_tensor_decoder_preserves_general_vespa_tensor_shapes() {
    use searchkernel_vespa_engine::{RankProfile, TensorValue};

    let config = temporary_path("diagnostic-tensor-config");
    let data = temporary_path("diagnostic-tensor-data");
    let schema = Schema::new("typea")
        .field(Field::new("qty", FieldType::Integer).attribute())
        .rank_profile(
            RankProfile::new("tensor_features")
                .first_phase("1")
                .match_feature("tensor(x[2],y[2]):[[1,2],[3,4]]")
                .match_feature("tensor(tag{}):{alpha:5,beta:7}")
                .match_feature("tensor(tag{},x[2]):{alpha:[1,2],beta:[3,4]}"),
        );
    ConfigCompiler
        .compile_to(&schema, &config)
        .expect("compile diagnostic tensor fixture");
    let engine = Engine::open(&data, &config).expect("open diagnostic tensor fixture");
    engine
        .put(Document::new("doc-1").field("qty", 1_i64))
        .expect("insert diagnostic tensor fixture");

    let result = engine
        .search(
            SearchRequest::new(Query::True)
                .rank_profile("tensor_features")
                .limit(1),
        )
        .expect("execute tensor feature search");
    let hit = result.hits.first().expect("tensor feature hit");
    assert_eq!(hit.match_features.len(), 3);
    let encoded = hit
        .match_features
        .values()
        .map(|value| {
            value
                .as_data()
                .expect("ordinary search must preserve tensor features as raw DATA")
                .to_vec()
        })
        .collect::<Vec<_>>();
    let decoded = encoded
        .iter()
        .map(|bytes| {
            searchkernel_vespa_engine::decode_diagnostic_tensor(bytes, 32)
                .expect("diagnostic tensor should decode")
        })
        .collect::<Vec<_>>();

    let dense = decoded
        .iter()
        .find_map(|value| match value {
            TensorValue::Dense { type_spec, values }
                if type_spec == "tensor(x[2],y[2])" =>
            {
                Some(values)
            }
            _ => None,
        })
        .expect("multi-dimensional dense feature");
    assert_eq!(dense, &[1.0, 2.0, 3.0, 4.0]);

    let sparse = decoded
        .iter()
        .find_map(|value| match value {
            TensorValue::Sparse { type_spec, cells }
                if type_spec == "tensor(tag{})" =>
            {
                Some(cells)
            }
            _ => None,
        })
        .expect("mapped sparse feature");
    let mut sparse_values = sparse
        .iter()
        .map(|cell| {
            (
                cell.address
                    .iter()
                    .find(|entry| entry.dimension == "tag")
                    .expect("sparse tag address")
                    .label
                    .clone(),
                cell.value,
            )
        })
        .collect::<Vec<_>>();
    sparse_values.sort_by(|left, right| left.0.cmp(&right.0));
    assert_eq!(
        sparse_values,
        vec![("alpha".to_owned(), 5.0), ("beta".to_owned(), 7.0)]
    );

    let (mixed_bytes, mixed) = encoded
        .iter()
        .zip(decoded.iter())
        .find_map(|(bytes, value)| match value {
            TensorValue::Mixed { type_spec, blocks }
                if type_spec == "tensor(x[2],tag{})"
                    || type_spec == "tensor(tag{},x[2])" =>
            {
                Some((bytes, blocks))
            }
            _ => None,
        })
        .expect("mixed feature");
    let mut mixed_values = mixed
        .iter()
        .map(|block| {
            (
                block
                    .address
                    .iter()
                    .find(|entry| entry.dimension == "tag")
                    .expect("mixed tag address")
                    .label
                    .clone(),
                block.values.clone(),
            )
        })
        .collect::<Vec<_>>();
    mixed_values.sort_by(|left, right| left.0.cmp(&right.0));
    assert_eq!(
        mixed_values,
        vec![
            ("alpha".to_owned(), vec![1.0, 2.0]),
            ("beta".to_owned(), vec![3.0, 4.0]),
        ]
    );

    let budget = searchkernel_vespa_engine::decode_diagnostic_tensor(mixed_bytes, 3)
        .expect_err("cell budget must reject before materializing the full tensor");
    assert_eq!(
        budget.detail_code(),
        Some("diagnostic_tensor_cell_budget_exceeded")
    );
    let malformed = searchkernel_vespa_engine::decode_diagnostic_tensor(&[0xff, 0xff], 32)
        .expect_err("malformed tensor bytes must fail closed");
    assert_eq!(
        malformed.detail_code(),
        Some("diagnostic_tensor_malformed")
    );

    engine.close().expect("close diagnostic tensor fixture");
    let _ = fs::remove_dir_all(config);
    let _ = fs::remove_dir_all(data);
}

#[test]
fn final_matcher_request_diagnostics_are_lowered_private_and_bounded() {
    use searchkernel_vespa_engine::{
        RankProfile, SearchDiagnosticControls, SortField, TensorValue,
    };
    use std::time::Duration;

    let config = temporary_path("diagnostic-config");
    let data = temporary_path("diagnostic-data");
    let schema = Schema::new("typea")
        .field(Field::new("title", FieldType::Text).indexed())
        .field(Field::new("qty", FieldType::Integer).attribute())
        .rank_profile(
            RankProfile::new("diagnostic")
                .first_phase("query(weight)")
                .query_input("private_tensor", "tensor<float>(x[512])"),
        )
        .rank_profile(
            RankProfile::new("both_phases")
                .first_phase("query(weight)")
                .second_phase("query(weight)")
                .rerank_count(10),
        );
    ConfigCompiler
        .compile_to(&schema, &config)
        .expect("compile diagnostic fixture");
    let engine = Engine::open(&data, &config).expect("open diagnostic fixture");
    engine
        .put(
            Document::new("doc-1")
                .field("title", "secretterm")
                .field("qty", 7_i64),
        )
        .expect("insert diagnostic fixture");
    let request = SearchRequest::new(Query::Term {
        field: "title".into(),
        text: "secretterm".into(),
    })
    .rank_profile("diagnostic")
    .query_feature("weight", 2.75)
    .query_tensor(
        "private_tensor",
        TensorValue::dense("tensor<float>(x[512])", vec![9182.5; 512]),
    )
    .rank_property("private_property", "secret_value".repeat(16))
    .limit(3)
    .timeout(Duration::from_secs(30));

    let ordinary = engine.search(request.clone()).expect("ordinary search");
    assert!(ordinary.diagnostics.is_none());
    assert_eq!(ordinary.hits.len(), 1);
    assert_eq!(ordinary.hits[0].score, 2.75);

    let mut diagnostic = request.clone();
    diagnostic.diagnostics = Some(SearchDiagnosticControls {
        structure_depth: 16,
        // Even trace level zero must include the final request snapshot.
        trace_level: 0,
        matching_depth: 0,
        first_phase_depth: 0,
        second_phase_depth: 0,
        raw_bytes: 65_536,
    });
    let result = engine
        .search(diagnostic.clone())
        .expect("diagnostic search");
    assert_eq!(result.hits, ordinary.hits);
    let evidence = result.diagnostics.expect("native diagnostics");
    assert!(!evidence.truncated);
    assert!(evidence.trace.len() <= 65_536);
    let raw: serde_json::Value = serde_json::from_str(&evidence.trace).expect("raw JSON");
    let snapshot = &raw["final_matcher_request"];
    assert_eq!(snapshot["format"], "searchkernel.proton-request.v1");
    let manifest: serde_json::Value =
        serde_json::from_str(include_str!("../../../native/vespa/manifest.json")).unwrap();
    assert_eq!(snapshot["vespa_commit"], manifest["vespa_commit"]);
    assert_eq!(
        snapshot["native_abi_version"],
        manifest["native_abi_version"]
    );
    assert_eq!(snapshot["rank_profile"], "diagnostic");
    assert_eq!(snapshot["timeout_ms"], 30_000.0);
    assert!(snapshot["time_left_ms"].as_f64().unwrap() <= 30_000.0);
    assert_eq!(snapshot["maxhits"], 3);
    assert_eq!(snapshot["sort"], "");
    assert_eq!(snapshot["trace_level"], 0);
    assert_eq!(snapshot["grouping_bytes"], 0);
    assert!(!snapshot["query_tree"].as_array().unwrap().is_empty());
    assert_eq!(snapshot["query_tree"][0]["term"], "[redacted]");
    let rank = snapshot["properties"]["rank"]["entries"]
        .as_array()
        .unwrap();
    let features = snapshot["properties"]["feature_overrides"]["entries"]
        .as_array()
        .unwrap();
    let property = |entries: &[serde_json::Value], name: &str| {
        entries
            .iter()
            .find(|entry| entry["name"] == name)
            .unwrap_or_else(|| panic!("missing lowered property {name}: {entries:?}"))["value"]
            .clone()
    };
    assert_eq!(property(rank, "vespa.matching.numthreadspersearch"), "1");
    assert_eq!(property(rank, "private_property"), "[redacted]");
    assert_eq!(property(rank, "query(private_tensor)"), "[redacted]");
    assert_eq!(property(rank, "weight"), "[redacted]");
    assert!(features.is_empty());
    let snapshot_text = snapshot.to_string();
    for secret in ["secretterm", "secret_value", "9182.5", "8371.25", "2.75"] {
        assert!(!snapshot_text.contains(secret), "snapshot leaked {secret}");
    }

    let sorted_request = request.clone().sort(SortField::descending("qty"));
    let sorted_ordinary = engine
        .search(sorted_request)
        .expect("ordinary sorted search");
    let sorted = engine
        .search(diagnostic.clone().sort(SortField::descending("qty")))
        .expect("diagnostic sorted search");
    assert_eq!(sorted.hits, sorted_ordinary.hits);
    let sorted_raw: serde_json::Value =
        serde_json::from_str(&sorted.diagnostics.unwrap().trace).unwrap();
    assert_eq!(sorted_raw["final_matcher_request"]["sort"], "-qty");

    // Output limits and structural limits are independent and both explicit.
    // Raw bytes and semantic sections have independent acquisition budgets.
    let mut projected = diagnostic.clone();
    let controls = projected.diagnostics.as_mut().unwrap();
    controls.trace_level = 7;
    controls.matching_depth = 8;
    controls.first_phase_depth = 8;
    controls.second_phase_depth = 8;
    controls.raw_bytes = 128;
    let structured = engine
        .search(projected.clone())
        .unwrap()
        .diagnostics
        .unwrap();
    assert!(structured.truncated);
    use searchkernel_vespa_engine::{DiagnosticSection as Section, DiagnosticStatus as Status};
    for section in [
        Section::MatchingProfiler,
        Section::FirstPhaseProfiler,
        Section::IteratorTree,
        Section::Blueprint,
    ] {
        let item = structured
            .sections
            .iter()
            .find(|item| item.section == section)
            .unwrap();
        assert_eq!(item.status, Status::Available, "{section:?}");
        let projection = item.projection.as_ref().unwrap();
        assert_eq!(projection.engine, "Vespa");
        let pin: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../reference/vespa/PINNED_VESPA.json"
        ))
        .unwrap();
        assert_eq!(projection.revision, pin["commit"].as_str().unwrap());
        assert_eq!(
            projection.native_abi,
            pin["native_abi_version"].as_u64().unwrap() as u32
        );
        assert!(!projection.nodes.is_empty());
    }
    let second = structured
        .sections
        .iter()
        .find(|item| item.section == Section::SecondPhaseProfiler)
        .unwrap();
    assert_eq!(second.status, Status::UnavailableForQuery);

    projected.rank_profile = "both_phases".into();
    let ranked = engine.search(projected).unwrap().diagnostics.unwrap();
    let second = ranked
        .sections
        .iter()
        .find(|item| item.section == Section::SecondPhaseProfiler)
        .unwrap();
    assert_eq!(second.status, Status::Available);
    assert!(second
        .projection
        .as_ref()
        .unwrap()
        .nodes
        .iter()
        .any(|node| node.count.is_some()));

    diagnostic.diagnostics.as_mut().unwrap().raw_bytes = 128;
    let tiny = engine
        .search(diagnostic.clone())
        .expect("tiny diagnostic budget")
        .diagnostics
        .unwrap();
    assert!(tiny.truncated);
    assert!(tiny.trace.len() <= 128);
    diagnostic.diagnostics.as_mut().unwrap().raw_bytes = 65_536;
    for index in (0..40).rev() {
        diagnostic
            .rank_properties
            .push((format!("bounded_{index:02}"), "private".into()));
    }
    let bounded = engine
        .search(diagnostic.clone())
        .expect("bounded properties")
        .diagnostics
        .unwrap();
    assert!(bounded.truncated);
    let raw: serde_json::Value = serde_json::from_str(&bounded.trace).unwrap();
    let properties = &raw["final_matcher_request"]["properties"]["rank"];
    assert!(properties["key_count"].as_u64().unwrap() > 32);
    let entries = properties["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 32);
    assert_eq!(entries[0]["name"], "bounded_00");
    assert_eq!(entries[31]["name"], "bounded_31");
    diagnostic.rank_properties.reverse();
    let reordered = engine
        .search(diagnostic)
        .expect("reordered properties")
        .diagnostics
        .unwrap();
    let raw: serde_json::Value = serde_json::from_str(&reordered.trace).unwrap();
    assert_eq!(
        &raw["final_matcher_request"]["properties"]["rank"],
        properties
    );
    assert!(engine.search(request).unwrap().diagnostics.is_none());
    engine.close().expect("close diagnostic fixture");
    let _ = fs::remove_dir_all(config);
    let _ = fs::remove_dir_all(data);
}
