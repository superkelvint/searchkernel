use searchkernel_vespa_engine::{
    ConfigCompiler, Document, Engine, Field, FieldType, Query, RangeBound, RankProfile, Schema,
    SearchRequest,
};
use std::collections::BTreeMap;
use std::ffi::CString;
use std::fs;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn temporary_path(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("searchkernel-native-contract-{label}-{nonce}"))
}

fn text_schema() -> Schema {
    Schema::new("typea")
        .field(Field::new("title", FieldType::Text).indexed().summary())
        .field(Field::new("qty", FieldType::Integer).attribute().summary())
}

fn compile_config(label: &str, schema: &Schema) -> (PathBuf, PathBuf) {
    let config = temporary_path(&format!("{label}-config"));
    let data = temporary_path(&format!("{label}-data"));
    ConfigCompiler
        .compile_to(schema, &config)
        .expect("compile generated config");
    (config, data)
}

unsafe extern "C" {
    fn searchkernel_query_outlives_engine_regression(
        data_dir: *const std::os::raw::c_char,
        config_dir: *const std::os::raw::c_char,
    ) -> i32;
    fn searchkernel_query_feature_locale_regression(
        data_dir: *const std::os::raw::c_char,
        config_dir: *const std::os::raw::c_char,
    ) -> i32;
}

fn replace_once(path: &Path, before: &str, after: &str) {
    let content = fs::read_to_string(path).expect("read generated config");
    let occurrences = content.matches(before).count();
    assert_eq!(occurrences, 1, "expected one occurrence of {before:?}");
    fs::write(path, content.replacen(before, after, 1)).expect("write mutated config");
}

#[test]
fn generated_text_configuration_enables_real_bm25() {
    let (config, data) = compile_config(
        "bm25",
        &Schema::new("typea").field(Field::new("title", FieldType::Text).indexed()),
    );
    replace_once(
        &config.join("indexschema.cfg"),
        "indexfield[0].interleavedfeatures false",
        "indexfield[0].interleavedfeatures true",
    );
    let engine = Engine::open(&data, &config).expect("open generated BM25 config");
    engine
        .put(Document::new("repeated").field("title", "fox fox fox"))
        .expect("insert repeated-term document");
    engine
        .put(Document::new("single").field("title", "fox"))
        .expect("insert single-term document");

    let result = engine
        .search(SearchRequest::new(Query::Term {
            field: "title".into(),
            text: "fox".into(),
        }))
        .expect("search generated BM25 config");
    let scores: BTreeMap<_, _> = result
        .hits
        .into_iter()
        .map(|hit| (hit.document_id, hit.score))
        .collect();
    assert!(scores.contains_key("repeated"));
    assert!(scores.contains_key("single"));
    assert!(scores["repeated"] > scores["single"]);
    engine.close().expect("close BM25 engine");

    let _ = fs::remove_dir_all(config);
    let _ = fs::remove_dir_all(data);
}

#[test]
fn native_accepts_custom_profiles_that_resemble_legacy_profiles() {
    let (text_config, text_data) = compile_config(
        "custom-bm25-profile",
        &Schema::new("typea")
            .field(Field::new("title", FieldType::Text).indexed())
            .rank_profile(RankProfile::new("bm25").first_phase("bm25(title)")),
    );
    let text_engine = Engine::open(&text_data, &text_config)
        .expect("a custom profile named bm25 uses the generic configuration shape");
    text_engine
        .close()
        .expect("close custom text profile engine");

    let (attribute_config, attribute_data) = compile_config(
        "custom-attribute-profile",
        &Schema::new("typea")
            .field(Field::new("popularity", FieldType::Integer).attribute())
            .rank_profile(RankProfile::new("popular").first_phase("attribute(popularity)")),
    );
    let attribute_engine = Engine::open(&attribute_data, &attribute_config)
        .expect("a custom attribute-only profile uses the generic configuration shape");
    attribute_engine
        .close()
        .expect("close custom attribute profile engine");

    for path in [text_config, text_data, attribute_config, attribute_data] {
        let _ = fs::remove_dir_all(path);
    }
}

#[test]
fn native_rejects_every_owned_generated_configuration_setting_when_mutated() {
    let mutations = [
        (
            "indexschema.cfg",
            "indexfield[0].name \"title\"",
            "indexfield[0].name \"other\"",
        ),
        (
            "indexschema.cfg",
            "indexfield[0].datatype STRING",
            "indexfield[0].datatype INT32",
        ),
        (
            "indexschema.cfg",
            "indexfield[0].collectiontype SINGLE",
            "indexfield[0].collectiontype ARRAY",
        ),
        (
            "indexschema.cfg",
            "indexfield[0].prefix false",
            "indexfield[0].prefix true",
        ),
        (
            "indexschema.cfg",
            "indexfield[0].phrases false",
            "indexfield[0].phrases true",
        ),
        (
            "indexschema.cfg",
            "indexfield[0].positions true",
            "indexfield[0].positions false",
        ),
        (
            "indexschema.cfg",
            "indexfield[0].averageelementlen 512",
            "indexfield[0].averageelementlen 256",
        ),
        (
            "indexschema.cfg",
            "indexfield[0].interleavedfeatures false",
            "indexfield[0].interleavedfeatures true",
        ),
        (
            "attributes.cfg",
            "attribute[0].name \"qty\"",
            "attribute[0].name \"other\"",
        ),
        (
            "attributes.cfg",
            "attribute[0].datatype INT32",
            "attribute[0].datatype DOUBLE",
        ),
        (
            "attributes.cfg",
            "attribute[0].collectiontype SINGLE",
            "attribute[0].collectiontype ARRAY",
        ),
        ("summary.cfg", "defaultsummaryid 1", "defaultsummaryid 2"),
        ("summary.cfg", "classes[1]", "classes[2]"),
        ("summary.cfg", "classes[0].id 1", "classes[0].id 2"),
        (
            "summary.cfg",
            "classes[0].name \"default\"",
            "classes[0].name \"other\"",
        ),
        (
            "summary.cfg",
            "classes[0].fields[5]",
            "classes[0].fields[6]",
        ),
        ("rank-profiles.cfg", "rankprofile[2]", "rankprofile[1]"),
        (
            "rank-profiles.cfg",
            "rankprofile[0].name \"default\"",
            "rankprofile[0].name \"other\"",
        ),
        (
            "rank-profiles.cfg",
            "rankprofile[1].name \"bm25\"",
            "rankprofile[1].name \"other\"",
        ),
        (
            "rank-profiles.cfg",
            "rankprofile[1].fef.property[1]",
            "rankprofile[1].fef.property[2]",
        ),
        (
            "rank-profiles.cfg",
            "rankprofile[1].fef.property[0].name \"vespa.rank.firstphase\"",
            "rankprofile[1].fef.property[0].name \"other\"",
        ),
        (
            "rank-profiles.cfg",
            "rankprofile[1].fef.property[0].value \"bm25(title)\"",
            "rankprofile[1].fef.property[0].value \"other\"",
        ),
    ];

    for (index, (file, before, after)) in mutations.into_iter().enumerate() {
        let (config, data) = compile_config(&format!("config-mutation-{index}"), &text_schema());
        replace_once(&config.join(file), before, after);
        assert!(
            Engine::open(&data, &config).is_err(),
            "native accepted mutation {file}: {before:?} -> {after:?}"
        );
        let _ = fs::remove_dir_all(config);
        let _ = fs::remove_dir_all(data);
    }
}

#[test]
fn runtime_operations_enforce_manifest_field_semantics() {
    let schema = Schema::new("typea")
        .field(Field::new("title", FieldType::Text).indexed())
        .field(Field::new("sku", FieldType::String).attribute())
        .field(Field::new("qty", FieldType::Integer).attribute());
    let (config, data) = compile_config("field-semantics", &schema);
    let engine = Engine::open(&data, &config).expect("open semantic test config");

    engine
        .put(Document::new("wrong-text-type").field("sku", "sku"))
        .expect("generic documents may contain string attributes");
    assert!(engine
        .search(SearchRequest::new(Query::Term {
            field: "sku".into(),
            text: "sku".into(),
        }))
        .is_err());
    assert!(engine
        .put(Document::new("wrong-integer-type").field("title", 1_i64))
        .is_err());
    assert!(engine
        .search(SearchRequest::new(Query::equals("title", 1)).limit(10))
        .is_err());

    engine.close().expect("close semantic test engine");
    let _ = fs::remove_dir_all(config);
    let _ = fs::remove_dir_all(data);
}

#[test]
fn low_level_query_validates_fields_before_text_analysis() {
    let (config, data) = compile_config("query-field-validation", &text_schema());
    let engine = Engine::open(&data, &config).expect("open query validation config");

    let unknown = engine
        .search(SearchRequest::new(Query::Term {
            field: "missing".to_owned(),
            text: "two words".to_owned(),
        }))
        .expect_err("unknown query field must be rejected");
    assert!(unknown.to_string().contains("unknown"));

    let unknown_yql = engine
        .search(SearchRequest::new(
            Query::from_yql("select * from sources * where missing contains \"two words\"")
                .expect("parse YQL query"),
        ))
        .expect_err("YQL query fields must be validated by Engine::search");
    assert!(unknown_yql.to_string().contains("unknown"));

    let wrong_type = engine
        .search(SearchRequest::new(Query::Term {
            field: "qty".to_owned(),
            text: "two words".to_owned(),
        }))
        .expect_err("numeric query field must reject text operators");
    assert!(wrong_type.to_string().contains("text"));

    let nested_unknown = engine
        .search(SearchRequest::new(Query::And(vec![
            Query::Term {
                field: "title".to_owned(),
                text: "two words".to_owned(),
            },
            Query::Term {
                field: "missing".to_owned(),
                text: "fox".to_owned(),
            },
        ])))
        .expect_err("nested unknown fields must be rejected before text analysis");
    assert!(nested_unknown.to_string().contains("unknown"));

    engine.close().expect("close query validation engine");
    let _ = fs::remove_dir_all(config);
    let _ = fs::remove_dir_all(data);
}

#[test]
fn yql_search_uses_the_same_query_as_typed_search() {
    let schema = Schema::new("typea").field(Field::new("title", FieldType::Text).indexed());
    let (config, data) = compile_config("query-source-equivalence", &schema);
    let engine = Engine::open(&data, &config).expect("open query source config");
    engine
        .put(Document::new("fox").field("title", "fox"))
        .expect("insert source-equivalence document");

    let typed = Query::Term {
        field: "title".to_owned(),
        text: "fox".to_owned(),
    };
    let typed_result = engine
        .search(SearchRequest::new(typed.clone()))
        .expect("typed source-equivalence query");
    let yql_result = engine
        .search(SearchRequest::new(
            Query::from_yql("select * from typea where title contains \"fox\"")
                .expect("parse supported YQL"),
        ))
        .expect("the schema document type is a supported YQL source");
    assert_eq!(typed_result, yql_result);

    engine.close().expect("close source-equivalence engine");
    let _ = fs::remove_dir_all(config);
    let _ = fs::remove_dir_all(data);
}

#[test]
fn native_query_can_outlive_engine() {
    let (config_path, data_path) = compile_config(
        "query-lifetime",
        &Schema::new("typea").field(Field::new("title", FieldType::Text).indexed()),
    );
    let config = CString::new(config_path.to_str().expect("config path is UTF-8")).unwrap();
    let data = CString::new(data_path.to_str().expect("data path is UTF-8")).unwrap();
    let result =
        unsafe { searchkernel_query_outlives_engine_regression(data.as_ptr(), config.as_ptr()) };
    assert_eq!(
        result, 0,
        "query lifetime regression failed with code {result}"
    );
    let _ = fs::remove_dir_all(config_path);
    let _ = fs::remove_dir_all(data_path);
}

#[test]
fn native_scalar_query_features_ignore_the_process_global_locale() {
    let (config_path, data_path) = compile_config(
        "query-feature-locale",
        &Schema::new("typea")
            .field(Field::new("title", FieldType::Text).indexed())
            .rank_profile(RankProfile::new("locale").first_phase("query(locale_probe)")),
    );
    let engine = Engine::open(&data_path, &config_path).expect("open locale test config");
    engine
        .put(Document::new("locale-1").field("title", "fox"))
        .expect("insert locale test document");
    engine.close().expect("close locale test engine");

    let config = CString::new(config_path.to_str().expect("config path is UTF-8")).unwrap();
    let data = CString::new(data_path.to_str().expect("data path is UTF-8")).unwrap();
    let result =
        unsafe { searchkernel_query_feature_locale_regression(data.as_ptr(), config.as_ptr()) };
    assert_eq!(
        result, 0,
        "query-feature locale regression failed with code {result}"
    );
    let _ = fs::remove_dir_all(config_path);
    let _ = fs::remove_dir_all(data_path);
}

#[test]
fn low_level_query_range_accepts_full_signed_i64_domain() {
    let (config, data) = compile_config(
        "query-signed-range",
        &Schema::new("typea").field(Field::new("qty", FieldType::Long).attribute()),
    );
    let engine = Engine::open(&data, &config).expect("open signed range config");
    engine
        .put(Document::new("minimum").field("qty", i64::MIN + 1))
        .expect("insert minimum integer");
    let result = engine
        .search(SearchRequest::new(Query::Range {
            field: "qty".to_owned(),
            lower: RangeBound::Included(i64::MIN + 1),
            upper: RangeBound::Included(i64::MIN + 1),
        }))
        .expect("search minimum integer");
    assert_eq!(result.hits.len(), 1);
    assert_eq!(result.hits[0].document_id, "minimum");

    engine.close().expect("close signed range engine");
    let _ = fs::remove_dir_all(config);
    let _ = fs::remove_dir_all(data);
}

#[test]
fn data_directories_are_exclusive_and_schema_bound() {
    let schema = text_schema();
    let (config_a, data_a) = compile_config("ownership-a", &schema);
    let (config_b, data_b) = compile_config("ownership-b", &schema);
    // If the historical fixed port is available, occupy it while opening the
    // engine. A successful open proves startup does not depend on that port.
    let historical_port = TcpListener::bind(("127.0.0.1", 9014)).ok();
    let engine_a = Engine::open(&data_a, &config_a).expect("open first index");
    drop(historical_port);

    assert!(
        Engine::open(&data_a, &config_a).is_err(),
        "the same data directory must have one owner"
    );
    let engine_b = Engine::open(&data_b, &config_b).expect("open independent index");
    engine_a
        .put(Document::new("index-a").field("title", "alpha"))
        .expect("write first independent index");
    engine_b
        .put(Document::new("index-b").field("title", "beta"))
        .expect("write second independent index");
    assert_eq!(
        engine_a
            .search(SearchRequest::new(Query::Term {
                field: "title".into(),
                text: "alpha".into(),
            }))
            .expect("search first independent index")
            .hits
            .len(),
        1
    );
    assert_eq!(
        engine_b
            .search(SearchRequest::new(Query::Term {
                field: "title".into(),
                text: "beta".into(),
            }))
            .expect("search second independent index")
            .hits
            .len(),
        1
    );
    engine_a.close().expect("close first index");
    engine_b.close().expect("close independent index");

    let different_schema =
        Schema::new("other_type").field(Field::new("title", FieldType::Text).indexed().summary());
    let (config_c, data_c) = compile_config("ownership-c", &different_schema);
    let result = Engine::open(&data_a, &config_c);
    assert!(
        result.is_err(),
        "reopening data with a different schema must fail"
    );

    for path in [config_a, config_b, config_c, data_a, data_b, data_c] {
        let _ = fs::remove_dir_all(path);
    }
}

#[test]
fn existing_data_without_schema_identity_cannot_be_rebound() {
    let original_schema = text_schema();
    let (config_a, data_a) = compile_config("missing-identity", &original_schema);
    let engine = Engine::open(&data_a, &config_a).expect("open original index");
    engine
        .put(Document::new("persisted").field("title", "persisted data"))
        .expect("write persisted data");
    engine.close().expect("close original index");

    fs::remove_file(data_a.join(".searchkernel-schema")).expect("remove schema identity");
    let replacement_schema = Schema::new("replacement_type")
        .field(Field::new("title", FieldType::Text).indexed().summary());
    let (config_b, _data_b) = compile_config("replacement-identity", &replacement_schema);

    assert!(
        Engine::open(&data_a, &config_b).is_err(),
        "existing engine state without an identity must not be rebound"
    );

    for path in [config_a, config_b, data_a] {
        let _ = fs::remove_dir_all(path);
    }
}

#[test]
fn stale_schema_identity_temp_file_does_not_block_new_index() {
    let (config, data) = compile_config(
        "stale-identity-temp",
        &Schema::new("typea").field(Field::new("title", FieldType::Text).indexed()),
    );
    fs::create_dir_all(&data).expect("create data directory");
    fs::write(data.join(".searchkernel-schema.tmp"), b"partial identity")
        .expect("create stale identity temporary file");

    let engine = Engine::open(&data, &config)
        .expect("a stale schema identity temporary file is not engine state");
    engine.close().expect("close engine");

    assert!(data.join(".searchkernel-schema").is_file());
    let _ = fs::remove_dir_all(config);
    let _ = fs::remove_dir_all(data);
}

#[test]
fn schema_identity_covers_physical_generated_configuration() {
    let schema = text_schema();
    let (config, data) = compile_config("physical-identity", &schema);
    let engine = Engine::open(&data, &config).expect("open index");
    engine.close().expect("close index");

    fs::write(
        config.join("juniperrc.cfg"),
        "changed physical configuration\n",
    )
    .expect("mutate physical configuration");
    assert!(
        Engine::open(&data, &config).is_err(),
        "schema identity must include physical generated configuration"
    );

    let _ = fs::remove_dir_all(config);
    let _ = fs::remove_dir_all(data);
}

#[test]
fn native_validates_manifest_and_configuration_location_before_side_effects() {
    let (config, data) = compile_config("manifest-preflight", &text_schema());
    fs::remove_file(config.join("searchkernel-schema.json")).expect("remove manifest");
    assert!(Engine::open(&data, &config).is_err());
    assert!(
        !data.exists(),
        "invalid configuration must not create data state"
    );
    let _ = fs::remove_dir_all(&config);

    let (nested_config, nested_data) = compile_config("nested-config", &text_schema());
    let config_parent = temporary_path("config-parent");
    fs::create_dir_all(&config_parent).expect("create config parent");
    fs::rename(&nested_config, config_parent.join("nested")).expect("nest config");
    assert!(Engine::open(&nested_data, &config_parent).is_err());
    assert!(
        !nested_data.exists(),
        "configuration discovery must not create data state"
    );
    let _ = fs::remove_dir_all(config_parent);
    let _ = fs::remove_dir_all(nested_data);
}

#[test]
fn native_rejects_invalid_manifest_values_at_the_boundary() {
    let mutations = [
        (
            "\"document_type\":\"typea\"",
            "\"document_type\":\"../escape\"",
        ),
        ("{\"name\":\"title\"", "{\"name\":\"../escape\""),
        ("\"type\":\"text\"", "\"type\":\"unknown\""),
        ("\"index\":true", "\"index\":\"true\""),
        ("\"version\":1", "\"version\":2"),
    ];

    for (index, (before, after)) in mutations.into_iter().enumerate() {
        let (config, data) = compile_config(&format!("manifest-mutation-{index}"), &text_schema());
        replace_once(&config.join("searchkernel-schema.json"), before, after);
        assert!(
            Engine::open(&data, &config).is_err(),
            "native accepted invalid manifest mutation {before:?} -> {after:?}"
        );
        assert!(
            !data.exists(),
            "invalid manifest must be rejected before data setup"
        );
        let _ = fs::remove_dir_all(config);
        let _ = fs::remove_dir_all(data);
    }
}

#[test]
fn failed_native_initialization_releases_the_data_directory_owner() {
    let (config, data) = compile_config("failed-initialization", &text_schema());
    fs::create_dir_all(&data).expect("create data directory");
    fs::write(data.join("typea"), b"not a directory").expect("create conflicting document type");

    assert!(Engine::open(&data, &config).is_err());

    fs::remove_file(data.join("typea")).expect("remove conflicting document type");
    let engine = Engine::open(&data, &config)
        .expect("failed initialization must release its data-directory owner");
    engine.close().expect("close recovered engine");

    let _ = fs::remove_dir_all(config);
    let _ = fs::remove_dir_all(data);
}

#[test]
fn reopening_after_a_second_write_preserves_the_first_document() {
    let (config, data) = compile_config("reopen-second-write", &text_schema());
    for cycle in 0..2 {
        let engine = Engine::open(&data, &config).expect("open persistent engine");
        engine
            .put(Document::new(format!("reopen-{cycle:04}")).field("qty", i64::from(cycle)))
            .expect("persist document");
        engine.close().expect("close persistent engine");

        let engine = Engine::open(&data, &config).expect("reopen persistent engine");
        for document in 0..=cycle {
            let id = format!("reopen-{document:04}");
            let hits = engine
                .search(
                    SearchRequest::new(Query::equals("qty", i64::from(document))).limit(1_000_000),
                )
                .expect("search document")
                .hits;
            assert!(
                hits.iter().any(|hit| hit.document_id == id),
                "reopen cycle {cycle} lost {id}"
            );
        }
        engine.close().expect("close reopened engine");
    }

    let _ = fs::remove_dir_all(config);
    let _ = fs::remove_dir_all(data);
}
