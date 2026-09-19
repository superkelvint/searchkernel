use super::*;
use crate::{Analyzer, Field, SearchField};
use searchkernel_ir::{FieldCardinality, FieldKind};
use searchkernel_testkit::RecordingBackend;
use std::time::{SystemTime, UNIX_EPOCH};

fn temporary_path(label: &str) -> std::path::PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("searchkernel-index-test-{label}-{nonce}"))
}

fn write_config(config: &std::path::Path) {
    std::fs::create_dir_all(config).unwrap();
    for name in [
        "searchkernel-schema.json",
        "indexschema.cfg",
        "attributes.cfg",
        "summary.cfg",
        "rank-profiles.cfg",
        "juniperrc.cfg",
        "proton.cfg",
    ] {
        std::fs::write(config.join(name), format!("{name}\n")).unwrap();
    }
}

fn manifest_definition() -> IndexDefinition {
    IndexDefinition::new()
        .name("catalog")
        .field(
            Field::text("title")
                .multi()
                .stored(true)
                .searchable(true)
                .filterable(false)
                .sortable(false)
                .facetable(false)
                .analyzer(Analyzer::standard_v1()),
        )
        .field(
            Field::keyword("brand")
                .stored(false)
                .searchable(false)
                .filterable(true)
                .sortable(true)
                .facetable(true),
        )
        .field(Field::int64("quantity"))
        .field(Field::float64("price"))
        .field(Field::bool("enabled"))
        .default_search_fields([SearchField::new("title")])
}

#[test]
fn schema_options_and_default_search_fields_reach_core() {
    let definition = IndexDefinition::new()
        .name("catalog")
        .field(
            Field::text("title")
                .stored(false)
                .searchable(true)
                .filterable(false)
                .sortable(false)
                .facetable(false)
                .analyzer(Analyzer::standard_v1()),
        )
        .field(
            Field::keyword("brand")
                .stored(false)
                .searchable(false)
                .filterable(true)
                .sortable(true)
                .facetable(true),
        )
        .field(
            Field::int64("quantity")
                .stored(true)
                .searchable(false)
                .filterable(true)
                .sortable(true)
                .facetable(true),
        )
        .field(
            Field::float64("price")
                .stored(false)
                .searchable(false)
                .filterable(true)
                .sortable(false)
                .facetable(false),
        )
        .field(
            Field::bool("enabled")
                .multi()
                .stored(true)
                .searchable(false)
                .filterable(true)
                .sortable(false)
                .facetable(true),
        )
        .default_search_fields([SearchField::new("title").weight(0.0)]);
    let index = IndexRuntime::from_backend(definition, Arc::new(RecordingBackend::new()))
        .unwrap()
        .index();
    let description = index.inner.context.describe();

    assert_eq!(description.name().unwrap().as_str(), "catalog");
    assert_eq!(description.fields().len(), 5);
    assert_eq!(description.default_search_fields().len(), 1);
    assert_eq!(description.default_search_fields()[0].weight().get(), 0.0);

    let fields = description.fields();
    assert_eq!(fields[0].name().as_str(), "title");
    assert_eq!(fields[0].kind(), FieldKind::Text);
    assert_eq!(fields[0].cardinality(), FieldCardinality::Single);
    assert!(fields[0]
        .analyzer()
        .is_some_and(|analyzer| analyzer.name == "standard" && analyzer.version == 1));
    assert!(!fields[0].capabilities().stored());
    assert!(fields[0].capabilities().searchable());

    assert_eq!(fields[1].kind(), FieldKind::Keyword);
    assert!(!fields[1].capabilities().stored());
    assert!(fields[1].capabilities().filterable());
    assert!(fields[1].capabilities().sortable());
    assert!(fields[1].capabilities().facetable());

    assert_eq!(fields[2].kind(), FieldKind::Int64);
    assert!(fields[2].capabilities().facetable());
    assert_eq!(fields[3].kind(), FieldKind::Float64);
    assert!(!fields[3].capabilities().stored());
    assert_eq!(fields[4].kind(), FieldKind::Bool);
    assert_eq!(fields[4].cardinality(), FieldCardinality::Multi);
    assert!(!fields[4].capabilities().sortable());
}

#[test]
fn manifest_round_trips_definition_and_config_identity() {
    let root = temporary_path("manifest-round-trip");
    let config = root.join("config");
    write_config(&config);
    let definition = manifest_definition();

    let resolved = definition_from_ir(&definition_to_ir(&definition).unwrap());
    let manifest = IndexManifest::for_definitions(
        &crate::portable_definition::encode_portable_definition(&definition),
        &resolved,
        &config,
    )
    .unwrap();
    manifest.write(&root).unwrap();
    let loaded = IndexManifest::read(&root).unwrap();
    loaded.validate_config(&config).unwrap();

    let expected = definition_from_ir(&definition_to_ir(&definition).unwrap());
    assert_eq!(loaded.definition().unwrap(), expected);
    assert_eq!(
        loaded.value["manifest_format_version"],
        MANIFEST_FORMAT_VERSION
    );
    assert_eq!(
        loaded.value["storage_compatibility_generation"],
        STORAGE_COMPATIBILITY_GENERATION
    );
    assert!(root.join("manifest/index-definition.bfbs").is_file());
    assert!(root
        .join("manifest/portable-index-definition.bfbs")
        .is_file());
    assert_eq!(
        loaded.value["canonical_index_definition"],
        "index-definition.bfbs"
    );
    assert_eq!(
        loaded.value["portable_index_definition"],
        "portable-index-definition.bfbs"
    );
    assert!(!root.join("manifest/.index-definition.bfbs.tmp").exists());
    assert!(!root
        .join("manifest/.portable-index-definition.bfbs.tmp")
        .exists());
    assert!(!root.join("manifest/.index.json.tmp").exists());

    std::fs::write(config.join("proton.cfg"), "changed\n").unwrap();
    assert!(matches!(
        loaded.validate_config(&config),
        Err(Error::Search { code: crate::ErrorCode::IncompatibleIndex, message, .. })
            if message == "incompatible index: resolved engine configuration differs"
    ));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn manifest_separates_portable_intent_from_resolved_definition() {
    let root = temporary_path("manifest-resolved-definition");
    let config = root.join("config");
    write_config(&config);
    let portable_definition = IndexDefinition::new().field(Field::text("title"));
    let resolved_ir = definition_to_ir(&portable_definition).unwrap();
    let resolved_definition = definition_from_ir(&resolved_ir);
    let portable_bytes =
        crate::portable_definition::encode_portable_definition(&portable_definition);
    let manifest =
        IndexManifest::for_definitions(&portable_bytes, &resolved_definition, &config).unwrap();

    manifest.write(&root).unwrap();
    let loaded = IndexManifest::read(&root).unwrap();
    assert_eq!(loaded.definition().unwrap(), resolved_definition);
    assert_eq!(loaded.portable_definition_bytes, portable_bytes);
    assert_ne!(
        loaded.portable_definition_bytes,
        loaded.resolved_definition_bytes
    );

    let portable_root = searchkernel_ir::__flatbuffers::root::<api::IndexDefinition>(
        &loaded.portable_definition_bytes,
    )
    .unwrap();
    let portable_field = portable_root.fields().unwrap().get(0);
    assert!(portable_field.stored().is_none());
    assert!(portable_field.analyzer().is_none());

    let resolved_root = searchkernel_ir::__flatbuffers::root::<api::IndexDefinition>(
        &loaded.resolved_definition_bytes,
    )
    .unwrap();
    let resolved_field = resolved_root.fields().unwrap().get(0);
    assert_eq!(
        resolved_field.stored().map(|value| value.value()),
        Some(true)
    );
    assert_eq!(
        resolved_field.searchable().map(|value| value.value()),
        Some(true)
    );
    assert_eq!(
        resolved_field.analyzer().map(|value| value.version()),
        Some(1)
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn manifest_uses_resolved_definition_when_portable_provenance_differs() {
    let root = temporary_path("manifest-mismatched-definition");
    let config = root.join("config");
    write_config(&config);
    let resolved_definition =
        definition_from_ir(&definition_to_ir(&manifest_definition()).unwrap());
    let portable_definition = IndexDefinition::new().field(Field::text("different"));
    let manifest = IndexManifest::for_definitions(
        &crate::portable_definition::encode_portable_definition(&portable_definition),
        &resolved_definition,
        &config,
    )
    .unwrap();
    manifest.write(&root).unwrap();

    let loaded = IndexManifest::read(&root).expect("portable provenance is not authoritative");
    assert_eq!(loaded.definition().unwrap(), resolved_definition);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn semantically_invalid_portable_provenance_does_not_block_reopen() {
    let root = temporary_path("manifest-portable-semantics");
    let config = root.join("config");
    write_config(&config);
    let resolved_definition =
        definition_from_ir(&definition_to_ir(&manifest_definition()).unwrap());
    let manifest = IndexManifest::for_definitions(
        &crate::portable_definition::encode_portable_definition(&manifest_definition()),
        &resolved_definition,
        &config,
    )
    .unwrap();
    manifest.write(&root).unwrap();

    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let invalid_name = builder.create_string("invalid");
    let field = api::FieldDefinition::create(
        &mut builder,
        &api::FieldDefinitionArgs {
            // A valid FlatBuffers table with a missing required semantic
            // field. Reopen must not reinterpret this provenance.
            name: Some(invalid_name),
            data_type: None,
            ..Default::default()
        },
    );
    let fields = builder.create_vector(&[field]);
    let portable = api::IndexDefinition::create(
        &mut builder,
        &api::IndexDefinitionArgs {
            fields: Some(fields),
            ..Default::default()
        },
    );
    builder.finish(portable, None);
    std::fs::write(
        root.join("manifest/portable-index-definition.bfbs"),
        builder.finished_data(),
    )
    .unwrap();

    let loaded = IndexManifest::read(&root).expect("portable provenance is structural only");
    assert_eq!(loaded.definition().unwrap(), resolved_definition);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
#[cfg(feature = "native")]
fn staged_creation_publishes_only_a_complete_index_directory() {
    let root = temporary_path("staged-create");
    let staging = create_staging_directory(&root).unwrap();
    std::fs::write(staging.join("in-progress"), b"not yet public").unwrap();
    assert!(!root.exists());

    publish_staging_directory(&staging, &root).unwrap();
    assert!(root.join("in-progress").is_file());
    assert!(!staging.exists());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
#[cfg(feature = "native")]
fn parent_fsync_failure_does_not_turn_publication_into_create_failure() {
    let root = temporary_path("staged-create-fsync");
    let staging = create_staging_directory(&root).unwrap();
    std::fs::write(staging.join("in-progress"), b"complete before publication").unwrap();
    FAIL_NEXT_PARENT_FSYNC.with(|flag| flag.set(true));

    publish_staging_directory(&staging, &root).expect("parent fsync is advisory after publication");
    assert!(root.join("in-progress").is_file());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
#[cfg(feature = "native")]
fn failed_post_publication_open_reports_recoverable_published_index() {
    let root = temporary_path("published-open-failure");
    FAIL_NEXT_BACKEND_OPEN.with(|flag| flag.set(true));
    FAIL_NEXT_PUBLISHED_CLEANUP.with(|flag| flag.set(true));

    let error = match Index::create(&root, IndexDefinition::new().field(Field::text("title"))) {
        Ok(index) => {
            index.close().unwrap();
            panic!("injected backend open failure unexpectedly succeeded")
        }
        Err(error) => error,
    };
    assert_eq!(error.code(), crate::ErrorCode::Unavailable);
    assert_eq!(error.details()[0].code, "index_published");
    assert_eq!(error.details()[0].path.as_deref(), root.to_str());
    assert!(root.join("manifest/index.json").is_file());

    let reopened = Index::open(&root).expect("published index remains recoverable");
    reopened.close().unwrap();
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn interrupted_staging_does_not_block_a_retry() {
    let root = temporary_path("interrupted-staging");
    let staging = create_staging_directory(&root).unwrap();
    std::fs::write(staging.join("partial-state"), b"incomplete").unwrap();
    // The process may disappear here. The final path is still absent, so
    // a later creator can publish a fresh complete tree.
    assert!(!root.exists());

    let index = Index::create(&root, IndexDefinition::new().field(Field::text("title")))
        .expect("retry after interrupted staging");
    index.close().unwrap();
    assert!(root.join("manifest/index.json").is_file());
    assert!(staging.exists());
    let _ = std::fs::remove_dir_all(staging);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn legacy_manifest_with_old_abi_is_still_readable() {
    let root = temporary_path("legacy-manifest");
    let config = root.join("config");
    write_config(&config);
    let definition = serde_json::json!({
        "name": "legacy",
        "fields": [{"name": "title", "kind": "Text"}],
        "default_search_fields": [{"field": "title", "weight": 1.0}]
    });
    let value = serde_json::json!({
        "format_version": LEGACY_MANIFEST_FORMAT_VERSION,
        "searchkernel_version": "0.1.0",
        "native_abi_version": 4,
        "vespa_revision": "2300ef0412bdb76f227ae3b497c8ca5e73c5da4e",
        "portable_index_definition": definition.clone(),
        "resolved_index_definition": definition,
        "resolved_engine_config_identity": config_identity(&config).unwrap(),
    });
    std::fs::create_dir_all(root.join("manifest")).unwrap();
    std::fs::write(
        root.join("manifest/index.json"),
        serde_json::to_vec_pretty(&value).unwrap(),
    )
    .unwrap();

    let loaded = IndexManifest::read(&root).unwrap();
    assert_eq!(loaded.definition().unwrap().index_name(), Some("legacy"));

    let stable_definition = serde_json::json!({
        "fields": [{
            "name": "title",
            "kind": "text",
            "analyzer": "standard-v1"
        }]
    });
    assert_eq!(
        definition_from_json(&stable_definition).unwrap().fields()[0],
        Field::text("title").analyzer(Analyzer::standard_v1())
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn canonical_definition_corruption_is_rejected() {
    let root = temporary_path("canonical-definition-corruption");
    let config = root.join("config");
    write_config(&config);
    let definition = manifest_definition();
    let resolved = definition_from_ir(&definition_to_ir(&definition).unwrap());
    let manifest = IndexManifest::for_definitions(
        &crate::portable_definition::encode_portable_definition(&definition),
        &resolved,
        &config,
    )
    .unwrap();
    manifest.write(&root).unwrap();
    std::fs::write(root.join("manifest/index-definition.bfbs"), b"invalid").unwrap();

    assert!(matches!(
        IndexManifest::read(&root),
        Err(Error::Search {
            code: crate::ErrorCode::CorruptIndex,
            ..
        })
    ));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn portable_definition_corruption_is_rejected() {
    let root = temporary_path("portable-definition-corruption");
    let config = root.join("config");
    write_config(&config);
    let definition = manifest_definition();
    let resolved = definition_from_ir(&definition_to_ir(&definition).unwrap());
    let manifest = IndexManifest::for_definitions(
        &crate::portable_definition::encode_portable_definition(&definition),
        &resolved,
        &config,
    )
    .unwrap();
    manifest.write(&root).unwrap();
    std::fs::write(
        root.join("manifest/portable-index-definition.bfbs"),
        b"invalid",
    )
    .unwrap();

    assert!(matches!(
        IndexManifest::read(&root),
        Err(Error::Search {
            code: crate::ErrorCode::CorruptIndex,
            ..
        })
    ));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn manifest_validation_rejects_identity_and_structure_corruption() {
    let root = temporary_path("manifest-validation");
    let config = root.join("config");
    write_config(&config);
    let definition = manifest_definition();
    let resolved = definition_from_ir(&definition_to_ir(&definition).unwrap());
    let manifest = IndexManifest::for_definitions(
        &crate::portable_definition::encode_portable_definition(&definition),
        &resolved,
        &config,
    )
    .unwrap();

    let mut provenance = manifest.clone();
    provenance.value["provenance"]["created_by_searchkernel_version"] = serde_json::json!("0.0.0");
    provenance.value["provenance"]["created_by_native_abi"] = serde_json::json!(4);
    provenance.value["provenance"]["created_by_vespa_revision"] =
        serde_json::json!("older-revision");
    assert!(provenance.validate_identity().is_ok());

    let mut generation = manifest.clone();
    generation.value["storage_compatibility_generation"] = serde_json::json!(999);
    assert!(matches!(
        generation.validate_identity(),
        Err(Error::Search {
            code: crate::ErrorCode::IncompatibleIndex,
            ..
        })
    ));

    let mut malformed_config = manifest.clone();
    malformed_config.value["resolved_engine_config_identity"] = serde_json::json!("invalid");
    assert!(matches!(
        malformed_config.validate_identity(),
        Err(Error::Search {
            code: crate::ErrorCode::CorruptIndex,
            ..
        })
    ));

    let mut malformed_provenance = manifest.clone();
    malformed_provenance.value["provenance"]["created_by_native_abi"] = serde_json::json!("5");
    assert!(matches!(
        malformed_provenance.validate_identity(),
        Err(Error::Search {
            code: crate::ErrorCode::CorruptIndex,
            ..
        })
    ));

    for value in [serde_json::json!(null), serde_json::json!({})] {
        let result = IndexManifest {
            value,
            portable_definition_bytes: Vec::new(),
            resolved_definition_bytes: Vec::new(),
        }
        .validate_identity();
        assert!(matches!(
            result,
            Err(Error::Search {
                code: crate::ErrorCode::CorruptIndex,
                ..
            })
        ));
    }

    let malformed = root.join("malformed");
    std::fs::create_dir_all(malformed.join("manifest")).unwrap();
    std::fs::write(malformed.join("manifest/index.json"), b"not-json").unwrap();
    assert!(matches!(
        IndexManifest::read(&malformed),
        Err(Error::Search {
            code: crate::ErrorCode::CorruptIndex,
            ..
        })
    ));

    let missing = root.join("missing");
    std::fs::create_dir_all(&missing).unwrap();
    assert!(matches!(
        IndexManifest::read(&missing),
        Err(Error::Search {
            code: crate::ErrorCode::CorruptIndex,
            ..
        })
    ));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn manifest_definition_decoder_rejects_malformed_fields() {
    let invalid_definitions = [
        serde_json::json!(null),
        serde_json::json!({}),
        serde_json::json!({"fields": [null]}),
        serde_json::json!({"fields": [{"name": "title"}]}),
        serde_json::json!({"fields": [{"name": "title", "kind": "Unknown"}]}),
        serde_json::json!({
            "fields": [{"name": "title", "kind": "Text", "analyzer": 1}]
        }),
        serde_json::json!({
            "fields": [{"name": "title", "kind": "Text", "analyzer": "Unknown"}]
        }),
        serde_json::json!({
            "fields": [{"name": "title", "kind": "Text"}],
            "default_search_fields": [null]
        }),
        serde_json::json!({
            "fields": [{"name": "title", "kind": "Text"}],
            "default_search_fields": [{}]
        }),
    ];

    for value in invalid_definitions {
        assert!(matches!(
            definition_from_json(&value),
            Err(Error::Search {
                code: crate::ErrorCode::CorruptIndex,
                ..
            })
        ));
    }
}

#[test]
fn failed_creation_removes_the_claimed_index_directory() {
    let root = temporary_path("failed-create-cleanup");
    let error = create_directory_transaction(&root, |root| {
        std::fs::write(root.join("partial-state"), b"incomplete")
            .map_err(|error| Error::Infrastructure(error.to_string()))?;
        Err::<(), _>(Error::Infrastructure(
            "forced initialization failure".into(),
        ))
    })
    .unwrap_err();

    assert!(
        matches!(error, Error::Infrastructure(message) if message == "forced initialization failure")
    );
    assert!(!root.exists());
}

#[test]
fn invalid_native_schema_is_rejected_before_directory_creation() {
    let root = temporary_path("invalid-native-schema");
    let error = match Index::create(
        &root,
        IndexDefinition::new().field(Field::text("title").searchable(false)),
    ) {
        Ok(_) => panic!("invalid native schema was accepted"),
        Err(error) => error,
    };

    assert_eq!(error.code(), crate::ErrorCode::Unsupported);
    assert!(!root.exists());
}
