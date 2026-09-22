use super::*;
use crate::{Analyzer, DistanceMetric, Field, HnswIndexConfig, RankProfile, SearchField};
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

fn resolved_context(definition: &IndexDefinition) -> IndexContext {
    IndexContext::from_definition_ir(definition_to_ir(definition).unwrap())
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
fn rust_schema_round_trip_preserves_hnsw_and_distance_metric() {
    let definition = IndexDefinition::new().field(
        Field::tensor("embedding", "tensor<float>(x[3])")
            .distance_metric(DistanceMetric::Euclidean)
            .hnsw(
                HnswIndexConfig::new()
                    .max_links_per_node(4)
                    .neighbors_to_explore_at_insert(16),
            ),
    );
    let resolved = definition_to_ir(&definition).expect("resolve HNSW schema");
    let field = &resolved.fields()[0];
    assert_eq!(
        field.attribute().map(|value| value.distance_metric),
        Some(searchkernel_ir::DistanceMetricIr::Euclidean)
    );
    let hnsw = field
        .index()
        .and_then(|value| value.hnsw.as_ref())
        .expect("HNSW config in IR");
    assert_eq!(hnsw.max_links_per_node, Some(4));
    assert_eq!(hnsw.neighbors_to_explore_at_insert, Some(16));

    let described = definition_from_ir(&resolved);
    let described_ir = definition_to_ir(&described).expect("re-resolve described schema");
    let described_field = &described_ir.fields()[0];
    assert_eq!(
        described_field
            .attribute()
            .map(|value| value.distance_metric),
        Some(searchkernel_ir::DistanceMetricIr::Euclidean)
    );
    assert_eq!(
        described_field
            .index()
            .and_then(|value| value.hnsw.as_ref())
            .and_then(|value| value.max_links_per_node),
        Some(4)
    );
    assert_eq!(
        described_field
            .index()
            .and_then(|value| value.hnsw.as_ref())
            .and_then(|value| value.neighbors_to_explore_at_insert),
        Some(16)
    );
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
        &resolved_context(&resolved),
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
    assert_eq!(
        loaded.value["resolved_schema_snapshot_version"],
        RESOLVED_SCHEMA_SNAPSHOT_VERSION
    );
    assert!(loaded.value["resolved_schema_snapshot"].is_object());
    assert_eq!(
        loaded.value["resolved_schema_snapshot"],
        searchkernel_ir::snapshot::index_definition(loaded.resolved_context().unwrap().definition())
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
    let manifest = IndexManifest::for_definitions(
        &portable_bytes,
        &resolved_context(&resolved_definition),
        &config,
    )
    .unwrap();

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
fn manifest_preserves_rich_resolved_ir() {
    let root = temporary_path("manifest-rich-resolved-ir");
    let config = root.join("config");
    write_config(&config);
    let definition = IndexDefinition::new().field(Field::text("title"));
    let mut resolved_ir = definition_to_ir(&definition).unwrap();
    resolved_ir.set_rank_profiles(vec![searchkernel_ir::RankProfileDefinitionIr {
        name: "product_search".to_owned(),
        inherits: Vec::new(),
        inputs: Vec::new(),
        functions: Vec::new(),
        first_phase: Some(searchkernel_ir::RankPhaseDefinitionIr {
            expression: Some("bm25(title) + query(boost)".to_owned()),
            rerank_count: None,
            rank_score_drop_limit: None,
            properties: std::collections::BTreeMap::new(),
            total_rerank_count: None,
            keep_rank_count: None,
            total_keep_rank_count: None,
        }),
        second_phase: None,
        global_phase: None,
        rank_features: Vec::new(),
        match_features: vec!["bm25(title)".to_owned(), "query(boost)".to_owned()],
        summary_features: Vec::new(),
        sort_features: Vec::new(),
        constants: Vec::new(),
        onnx_models: Vec::new(),
        rank_properties: std::collections::BTreeMap::new(),
        strict: None,
        properties: std::collections::BTreeMap::new(),
    }]);
    let context = IndexContext::from_definition_ir(resolved_ir);
    let manifest = IndexManifest::for_definitions(
        &crate::portable_definition::encode_portable_definition(&definition),
        &context,
        &config,
    )
    .unwrap();
    manifest.write(&root).unwrap();

    let loaded = IndexManifest::read(&root).unwrap();
    let loaded_context = loaded.resolved_context().unwrap();
    assert_eq!(
        loaded.value["resolved_schema_snapshot"],
        searchkernel_ir::snapshot::index_definition(loaded_context.definition())
    );
    let profile = &loaded_context.definition().rank_profiles()[0];
    assert_eq!(profile.name, "product_search");
    assert_eq!(
        profile.first_phase.as_ref().unwrap().expression.as_deref(),
        Some("bm25(title) + query(boost)")
    );
    assert_eq!(
        profile.match_features,
        vec!["bm25(title)".to_owned(), "query(boost)".to_owned()]
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn manifest_preserves_rich_described_schema_provenance() {
    let root = temporary_path("manifest-rich-described-schema");
    let config = root.join("config");
    write_config(&config);
    let (canonical, described) = super::api_tests::public_described_canonical_fixture();
    let portable_bytes = crate::portable_definition::encode_portable_definition(&described);
    let canonical_bytes = searchkernel_ir::index_definition_ir_to_api(&canonical);

    let manifest =
        IndexManifest::for_definitions(&portable_bytes, &resolved_context(&described), &config)
            .unwrap();
    manifest.write(&root).unwrap();
    let loaded = IndexManifest::read(&root).unwrap();

    assert_eq!(loaded.portable_definition_bytes, canonical_bytes);
    assert_eq!(loaded.resolved_definition_bytes, canonical_bytes);
    assert_eq!(loaded.definition().unwrap(), described);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn public_rank_profile_preserves_second_phase_and_rerank_count() {
    let definition = IndexDefinition::new()
        .field(Field::int64("rating"))
        .field(Field::int64("inventory"))
        .rank_profile(
            RankProfile::new("two_phase")
                .first_phase("attribute(rating)")
                .second_phase("attribute(inventory)")
                .rerank_count(3),
        );

    let resolved = definition_to_ir(&definition).unwrap();
    let profile = &resolved.rank_profiles()[0];
    assert_eq!(
        profile.first_phase.as_ref().unwrap().expression.as_deref(),
        Some("attribute(rating)")
    );
    let second_phase = profile.second_phase.as_ref().unwrap();
    assert_eq!(
        second_phase.expression.as_deref(),
        Some("attribute(inventory)")
    );
    assert_eq!(second_phase.rerank_count, Some(3));
}

#[test]
fn public_describe_preserves_rank_profiles_from_resolved_ir() {
    let definition = IndexDefinition::new()
        .field(Field::text("title"))
        .rank_profile(
            RankProfile::new("two_phase")
                .first_phase("bm25(title)")
                .second_phase("attribute(score)")
                .rerank_count(7)
                .match_feature("bm25(title)")
                .query_input("query_vector", "tensor<float>(x[3])"),
        );
    let index = IndexRuntime::from_backend(definition, Arc::new(RecordingBackend::new())).unwrap();

    let described = index.describe().unwrap();
    assert_eq!(described.rank_profiles().len(), 1);
    let profile = &described.rank_profiles()[0];
    assert_eq!(profile.name(), "two_phase");
    assert_eq!(
        profile
            .first_phase
            .as_ref()
            .and_then(|phase| phase.expression.as_deref()),
        Some("bm25(title)")
    );
    assert_eq!(
        profile
            .second_phase
            .as_ref()
            .and_then(|phase| phase.expression.as_deref()),
        Some("attribute(score)")
    );
    assert_eq!(profile.rerank_count, Some(7));
    assert_eq!(profile.match_features, vec!["bm25(title)"]);
    assert_eq!(
        profile.query_inputs,
        vec![("query_vector".to_owned(), "tensor<float>(x[3])".to_owned())]
    );
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
        &resolved_context(&resolved_definition),
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
        &resolved_context(&resolved_definition),
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
fn previous_v3_manifest_reopens_from_persisted_resolved_definition() {
    let root = temporary_path("manifest-v3-reopen");
    let config = root.join("config");
    write_config(&config);
    let definition = manifest_definition();
    let resolved = definition_from_ir(&definition_to_ir(&definition).unwrap());
    let manifest = IndexManifest::for_definitions(
        &crate::portable_definition::encode_portable_definition(&definition),
        &resolved_context(&resolved),
        &config,
    )
    .unwrap();
    manifest.write(&root).unwrap();

    let manifest_path = root.join("manifest/index.json");
    let mut value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&manifest_path).unwrap()).unwrap();
    value["manifest_format_version"] = serde_json::json!(3);
    value
        .as_object_mut()
        .unwrap()
        .remove("resolved_schema_snapshot_version");
    value
        .as_object_mut()
        .unwrap()
        .remove("resolved_schema_snapshot");
    std::fs::write(&manifest_path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();

    let loaded = IndexManifest::read(&root).expect("v3 manifest should remain reopenable");
    assert_eq!(loaded.definition().unwrap(), resolved);
    loaded.validate_config(&config).unwrap();
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn sparse_resolved_snapshot_is_rejected_before_caller_intent_resolution() {
    let root = temporary_path("manifest-sparse-resolved-snapshot");
    let config = root.join("config");
    write_config(&config);
    let definition = manifest_definition();
    let resolved = definition_from_ir(&definition_to_ir(&definition).unwrap());
    let manifest = IndexManifest::for_definitions(
        &crate::portable_definition::encode_portable_definition(&definition),
        &resolved_context(&resolved),
        &config,
    )
    .unwrap();
    manifest.write(&root).unwrap();

    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let field_name = builder.create_string("title");
    let primitive = searchkernel_ir::api::PrimitiveDataType::create(
        &mut builder,
        &searchkernel_ir::api::PrimitiveDataTypeArgs {
            kind: searchkernel_ir::api::PrimitiveDataTypeKind::String,
        },
    );
    let data_type = searchkernel_ir::api::DataType::create(
        &mut builder,
        &searchkernel_ir::api::DataTypeArgs {
            kind_type: searchkernel_ir::api::DataTypeKind::PrimitiveDataType,
            kind: Some(primitive.as_union_value()),
        },
    );
    let field = searchkernel_ir::api::FieldDefinition::create(
        &mut builder,
        &searchkernel_ir::api::FieldDefinitionArgs {
            name: Some(field_name),
            data_type: Some(data_type),
            ..Default::default()
        },
    );
    let fields = builder.create_vector(&[field]);
    let sparse_snapshot = searchkernel_ir::api::IndexDefinition::create(
        &mut builder,
        &searchkernel_ir::api::IndexDefinitionArgs {
            fields: Some(fields),
            ..Default::default()
        },
    );
    builder.finish(sparse_snapshot, None);
    std::fs::write(
        root.join("manifest/index-definition.bfbs"),
        builder.finished_data(),
    )
    .unwrap();

    assert!(matches!(
        IndexManifest::read(&root),
        Err(Error::Search {
            code: crate::ErrorCode::IncompatibleIndex,
            message,
            ..
        }) if message == "incompatible index: resolved schema snapshot is not compatible"
    ));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn semantically_invalid_resolved_snapshot_is_reported_as_compatibility_error() {
    let root = temporary_path("manifest-invalid-resolved-snapshot");
    let config = root.join("config");
    write_config(&config);
    let definition = manifest_definition();
    let resolved = definition_from_ir(&definition_to_ir(&definition).unwrap());
    let manifest = IndexManifest::for_definitions(
        &crate::portable_definition::encode_portable_definition(&definition),
        &resolved_context(&resolved),
        &config,
    )
    .unwrap();
    manifest.write(&root).unwrap();

    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let field_name = builder.create_string("title");
    let field = searchkernel_ir::api::FieldDefinition::create(
        &mut builder,
        &searchkernel_ir::api::FieldDefinitionArgs {
            name: Some(field_name),
            ..Default::default()
        },
    );
    let fields = builder.create_vector(&[field]);
    let invalid_snapshot = searchkernel_ir::api::IndexDefinition::create(
        &mut builder,
        &searchkernel_ir::api::IndexDefinitionArgs {
            fields: Some(fields),
            ..Default::default()
        },
    );
    builder.finish(invalid_snapshot, None);
    std::fs::write(
        root.join("manifest/index-definition.bfbs"),
        builder.finished_data(),
    )
    .unwrap();

    assert!(matches!(
        IndexManifest::read(&root),
        Err(Error::Search {
            code: crate::ErrorCode::IncompatibleIndex,
            message,
            ..
        }) if message == "incompatible index: resolved schema snapshot is not compatible"
    ));
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
fn parent_fsync_failure_is_reported_after_publication_without_rollback() {
    let root = temporary_path("staged-create-fsync");
    let staging = create_staging_directory(&root).unwrap();
    std::fs::write(staging.join("in-progress"), b"complete before publication").unwrap();
    FAIL_NEXT_PARENT_FSYNC.with(|flag| flag.set(true));

    let error = publish_staging_directory(&staging, &root)
        .expect_err("parent fsync failure after rename must be observable");
    assert_eq!(error.code(), crate::ErrorCode::Unavailable);
    assert_eq!(
        error.details()[0].code,
        "index_published_durability_uncertain"
    );
    assert_eq!(error.details()[0].path.as_deref(), root.to_str());
    assert!(root.join("in-progress").is_file());
    assert!(!staging.exists());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
#[cfg(feature = "native")]
fn create_fsync_failure_preserves_recoverable_root_and_rejects_destructive_retry() {
    let root = temporary_path("create-fsync-recovery");
    FAIL_NEXT_PARENT_FSYNC.with(|flag| flag.set(true));

    let error = match Index::create(&root, IndexDefinition::new().field(Field::text("title"))) {
        Ok(index) => {
            index.close().unwrap();
            panic!("injected parent fsync failure unexpectedly reported create success")
        }
        Err(error) => error,
    };
    assert_eq!(error.code(), crate::ErrorCode::Unavailable);
    assert_eq!(
        error.details()[0].code,
        "index_published_durability_uncertain"
    );
    assert_eq!(error.details()[0].path.as_deref(), root.to_str());
    assert!(root.join("manifest/index.json").is_file());

    let retry = match Index::create(&root, IndexDefinition::new().field(Field::text("title"))) {
        Ok(index) => {
            index.close().unwrap();
            panic!("retry unexpectedly overwrote published root")
        }
        Err(error) => error,
    };
    assert_eq!(retry.code(), crate::ErrorCode::AlreadyExists);
    assert!(root.join("manifest/index.json").is_file());

    FAIL_NEXT_BACKEND_OPEN.with(|flag| flag.set(true));
    let reopen_error = match Index::open(&root) {
        Ok(index) => {
            index.close().unwrap();
            panic!("injected recovery open failure unexpectedly succeeded")
        }
        Err(error) => error,
    };
    assert_eq!(reopen_error.code(), crate::ErrorCode::Unavailable);
    assert!(
        reopen_error.to_string().contains("injected backend open failure"),
        "recovery open must preserve backend failure context"
    );
    assert!(
        root.join("manifest/index.json").is_file(),
        "failed recovery open must not remove the published root"
    );

    let reopened = Index::open(&root).expect("published index remains recoverable");
    reopened.close().expect("close recovered index");
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
fn failed_create_after_publication_cleans_up_the_partial_runtime() {
    let root = temporary_path("failed-create-backend-open-cleanup");
    FAIL_NEXT_BACKEND_OPEN.with(|flag| flag.set(true));

    let error = match Index::create(&root, IndexDefinition::new().field(Field::text("title"))) {
        Ok(index) => {
            index.close().unwrap();
            panic!("injected backend open failure unexpectedly succeeded")
        }
        Err(error) => error,
    };
    assert_eq!(error.code(), crate::ErrorCode::Unavailable);
    assert!(!root.exists(), "failed create must not leave a usable root");

    let index = Index::create(&root, IndexDefinition::new().field(Field::text("title")))
        .expect("a later valid create must be possible");
    index.close().expect("close later valid create");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn failed_open_releases_lifecycle_lock_without_exposing_a_partial_runtime() {
    let root = temporary_path("failed-open-partial-runtime");
    let config = root.join("config");
    std::fs::create_dir_all(root.join("data")).unwrap();
    write_config(&config);
    let definition = manifest_definition();
    let resolved = definition_from_ir(&definition_to_ir(&definition).unwrap());
    let manifest = IndexManifest::for_definitions(
        &crate::portable_definition::encode_portable_definition(&definition),
        &resolved_context(&resolved),
        &config,
    )
    .unwrap();
    manifest.write(&root).unwrap();
    std::fs::remove_file(config.join("indexschema.cfg")).unwrap();

    let error = match Index::open(&root) {
        Ok(index) => {
            index.close().unwrap();
            panic!("corrupt config unexpectedly opened")
        }
        Err(error) => error,
    };
    assert_eq!(error.code(), crate::ErrorCode::CorruptIndex);
    assert!(root.join("data").is_dir());
    assert!(!root.join("config/indexschema.cfg").exists());

    // The failed opener did not publish a handle or retain the lifecycle lock;
    // a fresh create can claim the path after the corrupt tree is removed.
    std::fs::remove_dir_all(&root).unwrap();
    let recreated = Index::create(&root, IndexDefinition::new().field(Field::text("title")))
        .expect("failed open must not poison future initialization");
    recreated.close().expect("close recreated index");
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
        &resolved_context(&resolved),
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
        &resolved_context(&resolved),
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
        &resolved_context(&resolved),
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

    let mut snapshot_version = manifest.clone();
    snapshot_version.value["resolved_schema_snapshot_version"] = serde_json::json!(999);
    assert!(matches!(
        snapshot_version.validate_identity(),
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
