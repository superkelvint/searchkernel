use searchkernel_vespa::{
    ConfigCompiler, Engine, Field, FieldType, Schema, TextDocument, TextQuery,
};
use std::collections::BTreeMap;
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
    let engine = Engine::open(&data, &config).expect("open generated BM25 config");
    engine
        .put_text(TextDocument {
            id: "repeated",
            field: "title",
            text: "fox fox fox",
        })
        .expect("insert repeated-term document");
    engine
        .put_text(TextDocument {
            id: "single",
            field: "title",
            text: "fox",
        })
        .expect("insert single-term document");

    let result = engine
        .search_text(
            TextQuery::Term {
                field: "title",
                text: "fox",
            },
            "bm25",
        )
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
            "indexfield[0].interleavedfeatures true",
            "indexfield[0].interleavedfeatures false",
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

    assert!(engine
        .put_text(TextDocument {
            id: "wrong-text-type",
            field: "sku",
            text: "sku",
        })
        .is_err());
    assert!(engine
        .search_text(
            TextQuery::Term {
                field: "sku",
                text: "sku",
            },
            "bm25",
        )
        .is_err());
    assert!(engine.put("wrong-integer-type", "title", 1).is_err());
    assert!(engine.search("title", 1, 10).is_err());

    engine.close().expect("close semantic test engine");
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
        .put_text(TextDocument {
            id: "index-a",
            field: "title",
            text: "alpha",
        })
        .expect("write first independent index");
    engine_b
        .put_text(TextDocument {
            id: "index-b",
            field: "title",
            text: "beta",
        })
        .expect("write second independent index");
    assert_eq!(
        engine_a
            .search_text(
                TextQuery::Term {
                    field: "title",
                    text: "alpha",
                },
                "bm25"
            )
            .expect("search first independent index")
            .hits
            .len(),
        1
    );
    assert_eq!(
        engine_b
            .search_text(
                TextQuery::Term {
                    field: "title",
                    text: "beta",
                },
                "bm25"
            )
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
