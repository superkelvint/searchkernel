use super::*;
use crate::{DataType, SchemaStructField};

#[test]
fn describe_exposes_schema_fields_publicly() {
    let (canonical, described) = public_described_canonical_fixture();
    assert_eq!(
        described.schema_fields().len(),
        canonical.schema_fields().len()
    );
    assert_eq!(
        described.schema_fields()[0].name(),
        canonical.schema_fields()[0].name().as_str()
    );
}

#[test]
fn custom_analyzer_configuration_is_shared_portable_schema_state() {
    use searchkernel_ir::{
        AnalyzerIr, DataTypeIr, FieldCapabilities, FieldIr, FieldName, PrimitiveDataTypeIr,
        SchemaPropertiesIr,
    };

    let mut options = SchemaPropertiesIr::new();
    options.insert(
        "tokenizer".to_owned(),
        searchkernel_ir::RequestValueIr::String("custom".to_owned()),
    );
    let field = FieldIr::new(
        FieldName::parse("title").expect("valid field name"),
        DataTypeIr::Primitive(PrimitiveDataTypeIr::String),
        Some(AnalyzerIr {
            name: "custom".to_owned(),
            version: 2,
            options,
        }),
        FieldCapabilities::new(true, true, false, false, false),
    );
    let canonical = searchkernel_ir::IndexDefinitionIr::new(None, vec![field], Vec::new())
        .expect("rich analyzer is valid portable schema state");

    let index = index_from_canonical_ir(canonical.clone());
    let described = index
        .describe()
        .expect("describe should preserve rich analyzer state");
    let rust = definition_to_ir(&described).expect("Rust ingress should preserve rich analyzer");

    let bytes = searchkernel_ir::index_definition_ir_to_api(&canonical);
    let portable = searchkernel_ir::__flatbuffers::root::<api::IndexDefinition>(&bytes)
        .expect("portable schema should decode");
    let portable = searchkernel_ir::wire::v1::index_definition_to_ir(portable)
        .expect("portable ingress should preserve rich analyzer");

    assert_eq!(
        rust.fields()[0].analyzer(),
        canonical.fields()[0].analyzer()
    );
    assert_eq!(
        portable.fields()[0].analyzer(),
        canonical.fields()[0].analyzer()
    );
    assert_eq!(rust.fields()[0].analyzer(), portable.fields()[0].analyzer());
}

#[test]
fn described_field_preserves_custom_analyzer_options() {
    let mut options = searchkernel_ir::SchemaPropertiesIr::new();
    options.insert(
        "tokenizer".to_owned(),
        searchkernel_ir::RequestValueIr::String("custom".to_owned()),
    );
    let mut field = searchkernel_ir::FieldIr::new(
        searchkernel_ir::FieldName::parse("title").expect("valid field name"),
        searchkernel_ir::DataTypeIr::Primitive(searchkernel_ir::PrimitiveDataTypeIr::String),
        Some(searchkernel_ir::AnalyzerIr {
            name: "custom".to_owned(),
            version: 2,
            options,
        }),
        searchkernel_ir::FieldCapabilities::new(true, true, false, false, false),
    );
    let mut matching_options = searchkernel_ir::SchemaPropertiesIr::new();
    matching_options.insert(
        "match-key".to_owned(),
        searchkernel_ir::RequestValueIr::String("value".to_owned()),
    );
    field.set_matching_config(Some(searchkernel_ir::MatchingConfigIr {
        mode: searchkernel_ir::MatchingModeIr::Text,
        options: matching_options,
    }));
    let definition = searchkernel_ir::IndexDefinitionIr::new(None, vec![field], Vec::new())
        .expect("valid analyzer definition");

    let index = index_from_canonical_ir(definition);
    let described = index.describe().expect("describe should succeed");
    let analyzer = described.fields()[0]
        .analyzer_config()
        .expect("described custom analyzer should remain publicly visible");
    assert_eq!(analyzer.name, "custom");
    assert_eq!(analyzer.version, 2);
    assert_eq!(
        analyzer.options.get("tokenizer"),
        Some(&crate::schema::SchemaValue::String("custom".to_owned()))
    );
    assert_eq!(
        described.fields()[0].matching(),
        Some(crate::schema::MatchingConfig {
            mode: crate::schema::MatchingMode::Text,
            options: [(
                "match-key".to_owned(),
                crate::schema::SchemaValue::String("value".to_owned()),
            )]
            .into_iter()
            .collect(),
        })
    );

    let public_field = described.fields()[0].clone();
    let reused = IndexDefinition::new().field(public_field);
    let resolved = definition_to_ir(&reused).expect("reused field should resolve");
    let analyzer_ir = searchkernel_ir::AnalyzerIr {
        name: analyzer.name.clone(),
        version: analyzer.version,
        options: analyzer
            .options
            .iter()
            .map(|(key, value)| (key.clone(), value.to_ir()))
            .collect(),
    };
    assert_eq!(resolved.fields()[0].analyzer(), Some(&analyzer_ir));
    assert_eq!(
        resolved.fields()[0].matching(),
        Some(&searchkernel_ir::MatchingConfigIr {
            mode: searchkernel_ir::MatchingModeIr::Text,
            options: [(
                "match-key".to_owned(),
                searchkernel_ir::RequestValueIr::String("value".to_owned())
            )]
            .into_iter()
            .collect(),
        })
    );
}

#[test]
fn browse_executes_match_all_and_describe_returns_schema() {
    let definition = scripted_definition();
    let context = context_for(&definition);
    let (_probe_runtime, probe) =
        runtime_index(definition.clone(), Arc::new(RecordingBackend::new()));
    let expected = expected_search(&context, &probe.browse());
    let backend = Arc::new(ScriptedBackend::new([ExpectedCall::success(
        OperationIr::Search(expected),
        ScriptedResult::Search(Box::new(searchkernel_ir::SearchResult::empty(
            searchkernel_ir::TotalHits {
                value: 0,
                relation: searchkernel_ir::TotalHitsRelation::Exact,
            },
        ))),
    )]));
    let (_runtime, index) = runtime_index(definition, backend.clone());

    let described = index.describe().unwrap();
    assert_eq!(described.index_name(), Some("catalog"));
    assert_eq!(described.fields().len(), 5);
    assert_eq!(described.fields()[0].name(), "title");
    assert_eq!(described.fields()[1].name(), "brand");

    let result: crate::SearchResult<serde_json::Value> = index.browse().execute().unwrap();
    assert!(result.hits.is_empty());
    assert_eq!(result.total_hits.value, 0);
    backend.finish().unwrap();
}

#[test]
fn canonical_fixture_covers_the_complete_schema_surface() {
    let definition = canonical_fixture_ir();
    assert_eq!(
        definition.name().map(|value| value.as_str()),
        Some("complete_schema")
    );
    assert_eq!(definition.fields().len(), 17);
    assert_eq!(definition.default_search_fields().len(), 2);
    assert_eq!(
        definition.default_search_fields()[0].field().as_str(),
        "title"
    );
    assert_eq!(definition.default_search_fields()[0].weight().get(), 0.75);
    assert_eq!(
        definition.default_search_fields()[1].field().as_str(),
        "tags"
    );
    assert_eq!(definition.default_search_fields()[1].weight().get(), 0.25);
    assert_eq!(definition.properties().len(), 2);
    assert_eq!(definition.document_type(), Some("complete_document"));
    assert_eq!(
        definition.schema_inherits(),
        ["base_schema", "common_schema"]
    );
    assert_eq!(
        definition.document_inherits(),
        ["base_document", "audited_document"]
    );
    assert_eq!(definition.structs().len(), 1);
    assert_eq!(definition.structs()[0].name, "address");
    assert_eq!(definition.structs()[0].fields.len(), 2);
    assert_eq!(definition.structs()[0].properties.len(), 1);
    assert_eq!(definition.fieldsets().len(), 1);
    assert_eq!(definition.fieldsets()[0].name, "all_text");
    assert_eq!(definition.fieldsets()[0].fields.len(), 2);
    assert_eq!(
        definition.fieldsets()[0].query_commands,
        ["stem", "normalize"]
    );
    assert_eq!(definition.schema_fields().len(), 1);
    assert_eq!(definition.schema_fields()[0].name().as_str(), "derived");
    assert_eq!(definition.imported_fields().len(), 1);
    assert_eq!(
        definition.imported_fields()[0].alias.as_str(),
        "parent_name"
    );
    assert_eq!(definition.document_summaries().len(), 1);
    assert_eq!(definition.document_summaries()[0].fields.len(), 1);
    assert_eq!(definition.document_summaries()[0].from_disk, Some(false));
    assert_eq!(
        definition.document_summaries()[0].omit_summary_features,
        Some(true)
    );
    assert_eq!(definition.rank_profiles().len(), 3);
    assert_eq!(definition.constants().len(), 2);
    assert_eq!(definition.onnx_models().len(), 1);

    let data_types = definition
        .fields()
        .iter()
        .map(|field| (field.name().as_str(), field.data_type()))
        .collect::<BTreeMap<_, _>>();
    assert!(matches!(
        data_types["title"],
        searchkernel_ir::DataTypeIr::Primitive(searchkernel_ir::PrimitiveDataTypeIr::String)
    ));
    assert!(matches!(
        data_types["flag"],
        searchkernel_ir::DataTypeIr::Primitive(searchkernel_ir::PrimitiveDataTypeIr::Bool)
    ));
    assert!(matches!(
        data_types["byte_value"],
        searchkernel_ir::DataTypeIr::Primitive(searchkernel_ir::PrimitiveDataTypeIr::Byte)
    ));
    assert!(matches!(
        data_types["count32"],
        searchkernel_ir::DataTypeIr::Primitive(searchkernel_ir::PrimitiveDataTypeIr::Int32)
    ));
    assert!(matches!(
        data_types["count64"],
        searchkernel_ir::DataTypeIr::Primitive(searchkernel_ir::PrimitiveDataTypeIr::Int64)
    ));
    assert!(matches!(
        data_types["ratio32"],
        searchkernel_ir::DataTypeIr::Primitive(searchkernel_ir::PrimitiveDataTypeIr::Float32)
    ));
    assert!(matches!(
        data_types["ratio64"],
        searchkernel_ir::DataTypeIr::Primitive(searchkernel_ir::PrimitiveDataTypeIr::Float64)
    ));
    assert!(matches!(
        data_types["payload"],
        searchkernel_ir::DataTypeIr::Primitive(searchkernel_ir::PrimitiveDataTypeIr::Raw)
    ));
    assert!(matches!(
        data_types["embedding"],
        searchkernel_ir::DataTypeIr::Tensor { .. }
    ));
    assert!(matches!(
        data_types["tags"],
        searchkernel_ir::DataTypeIr::Array(_)
    ));
    assert!(matches!(
        data_types["scores"],
        searchkernel_ir::DataTypeIr::Map { .. }
    ));
    assert!(matches!(
        data_types["labels"],
        searchkernel_ir::DataTypeIr::WeightedSet(_)
    ));
    assert!(matches!(
        data_types["inline"],
        searchkernel_ir::DataTypeIr::Struct { .. }
    ));
    assert!(matches!(
        data_types["address"],
        searchkernel_ir::DataTypeIr::StructReference { .. }
    ));
    assert!(matches!(
        data_types["predicate"],
        searchkernel_ir::DataTypeIr::Predicate
    ));
    assert!(matches!(
        data_types["location"],
        searchkernel_ir::DataTypeIr::Position
    ));
    assert!(matches!(
        data_types["parent"],
        searchkernel_ir::DataTypeIr::Reference { .. }
    ));

    let title = &definition.fields()[0];
    assert!(title.analyzer().is_some());
    assert!(title.matching().is_some());
    assert!(title.index().is_some());
    assert!(title.attribute().is_some());
    assert!(title.summary().is_some());
    assert_eq!(title.indexing_script(), None);
    assert_eq!(title.properties().len(), 2);
    assert_eq!(title.struct_fields().len(), 1);
    assert!(title.capabilities().stored());
    assert!(title.capabilities().searchable());
    assert!(!title.capabilities().filterable());
    assert!(!title.capabilities().sortable());
    assert!(!title.capabilities().groupable());
    assert_eq!(
        definition.schema_fields()[0].indexing_script(),
        Some("input title | attribute(derived)")
    );

    let complete = &definition.rank_profiles()[0];
    assert_eq!(complete.name, "complete_profile");
    assert_eq!(complete.inherits, ["base_profile"]);
    assert_eq!(complete.inputs.len(), 2);
    assert_eq!(complete.functions.len(), 3);
    assert!(complete.first_phase.is_some());
    assert!(complete.second_phase.is_some());
    assert!(complete.global_phase.is_some());
    assert_eq!(complete.rank_features.len(), 2);
    assert_eq!(complete.match_features.len(), 2);
    assert_eq!(complete.summary_features.len(), 1);
    assert_eq!(complete.sort_features.len(), 1);
    assert_eq!(complete.constants.len(), 1);
    assert_eq!(complete.onnx_models.len(), 1);
    assert_eq!(complete.rank_properties.len(), 1);
    assert_eq!(complete.strict, Some(true));
    assert_eq!(complete.properties.len(), 1);
    assert_eq!(
        complete.inputs[0].default_value,
        searchkernel_ir::RequestValueIr::Float64(1.5)
    );
    assert!(matches!(
        complete.inputs[1].default_value,
        searchkernel_ir::RequestValueIr::Tensor(_)
    ));
    assert_eq!(
        complete.inputs[1].default_value,
        searchkernel_ir::RequestValueIr::Tensor(searchkernel_ir::RequestTensorIr {
            type_spec: "tensor<float>(x[3])".to_owned(),
            payload: searchkernel_ir::RequestTensorPayloadIr::Dense(vec![1.0, 2.0, 3.0]),
        })
    );
    assert_eq!(complete.functions[0].inline, Some(true));
    assert_eq!(complete.functions[1].inline, Some(false));
    assert_eq!(complete.functions[2].inline, None);
    assert_eq!(
        complete.first_phase.as_ref().unwrap().keep_rank_count,
        Some(5)
    );
    assert_eq!(
        complete.second_phase.as_ref().unwrap().rerank_count,
        Some(10)
    );
    assert_eq!(
        complete
            .global_phase
            .as_ref()
            .unwrap()
            .rank_score_drop_limit,
        Some(0.5)
    );
    assert_eq!(
        definition.rank_profiles()[1]
            .first_phase
            .as_ref()
            .unwrap()
            .total_keep_rank_count,
        Some(6)
    );
    assert_eq!(
        definition.rank_profiles()[1]
            .second_phase
            .as_ref()
            .unwrap()
            .total_rerank_count,
        Some(11)
    );
    assert!(definition.rank_profiles()[2].first_phase.is_none());
    assert!(definition.rank_profiles()[2].second_phase.is_none());
    assert!(definition.rank_profiles()[2].global_phase.is_none());
}

#[test]
fn portable_definition_preserves_explicit_vs_omitted_capability_presence() {
    let definition = IndexDefinition::new()
        .field(
            Field::text("explicit_defaults")
                .stored(true)
                .searchable(true),
        )
        .field(Field::text("omitted_defaults"));
    let bytes = crate::portable_definition::encode_portable_definition(&definition);
    let portable = searchkernel_ir::__flatbuffers::root::<api::IndexDefinition>(&bytes)
        .expect("portable definition should parse");
    let fields = portable
        .fields()
        .expect("portable definition should contain fields");
    let explicit = fields
        .iter()
        .find(|field| field.name() == "explicit_defaults")
        .expect("explicit field should be encoded");
    let omitted = fields
        .iter()
        .find(|field| field.name() == "omitted_defaults")
        .expect("omitted field should be encoded");

    assert_eq!(explicit.stored().map(|value| value.value()), Some(true));
    assert_eq!(explicit.searchable().map(|value| value.value()), Some(true));
    assert!(omitted.stored().is_none());
    assert!(omitted.searchable().is_none());

    // Canonical resolution deliberately erases this presence distinction:
    // both fields resolve to the same default-true stored/searchable behavior.
    let resolved = searchkernel_ir::wire::v1::index_definition_to_ir(portable)
        .expect("portable definition should resolve");
    for name in ["explicit_defaults", "omitted_defaults"] {
        let field = resolved
            .fields()
            .iter()
            .find(|field| field.name().as_str() == name)
            .expect("resolved field should exist");
        assert!(field.capabilities().stored());
        assert!(field.capabilities().searchable());
    }
}

#[test]
fn describe_preserves_field_properties() {
    let (canonical, described) = described_canonical_fixture();
    assert_eq!(
        canonical.fields()[0].properties(),
        described.fields()[0].properties()
    );
}

#[test]
fn describe_preserves_all_document_data_types() {
    let (canonical, described) = described_canonical_fixture();
    let expected = canonical
        .fields()
        .iter()
        .map(|field| field.data_type())
        .collect::<Vec<_>>();
    let actual = described
        .fields()
        .iter()
        .map(|field| field.data_type())
        .collect::<Vec<_>>();
    assert_eq!(expected, actual);
}

#[test]
fn describe_exposes_canonical_field_data_types_publicly() {
    let (canonical, described) = public_described_canonical_fixture();
    for name in [
        "byte_value",
        "count32",
        "ratio32",
        "payload",
        "scores",
        "parent",
    ] {
        let expected = canonical
            .fields()
            .iter()
            .find(|field| field.name().as_str() == name)
            .expect("fixture field should exist");
        let actual = described
            .fields()
            .iter()
            .find(|field| field.name() == name)
            .expect("described field should exist");
        assert_eq!(
            actual.data_type().to_ir(),
            *expected.data_type(),
            "field {name}"
        );
    }
}

#[test]
fn described_field_exposes_exact_canonical_capabilities() {
    let (canonical, described) = public_described_canonical_fixture();
    let expected = canonical
        .fields()
        .iter()
        .find(|field| field.name().as_str() == "location")
        .expect("canonical location field should exist")
        .capabilities();
    let actual = described
        .fields()
        .iter()
        .find(|field| field.name() == "location")
        .expect("described location field should exist")
        .capabilities()
        .expect("described fields should expose resolved capabilities");

    assert_eq!(actual.stored(), expected.stored());
    assert_eq!(actual.searchable(), expected.searchable());
    assert_eq!(actual.filterable(), expected.filterable());
    assert_eq!(actual.sortable(), expected.sortable());
    assert_eq!(actual.groupable(), expected.groupable());
}

#[test]
fn describe_preserves_complete_field_configuration() {
    let (canonical, described) = described_canonical_fixture();
    assert_eq!(canonical.fields()[0], described.fields()[0]);
}

#[test]
fn describe_preserves_struct_field_configuration() {
    let (canonical, described) = described_canonical_fixture();
    assert_eq!(
        canonical.fields()[0].struct_fields(),
        described.fields()[0].struct_fields()
    );
}

#[test]
fn describe_preserves_default_search_fields_and_weights() {
    let (canonical, described) = described_canonical_fixture();
    assert_eq!(
        canonical.default_search_fields(),
        described.default_search_fields()
    );
}

#[test]
fn described_field_exposes_index_and_attribute_configuration_publicly() {
    let (canonical, described) = public_described_canonical_fixture();
    let described_field = described
        .fields()
        .iter()
        .find(|field| field.name() == "embedding")
        .expect("embedding field should be described");
    let canonical_field = canonical
        .fields()
        .iter()
        .find(|field| field.name().as_str() == "embedding")
        .expect("canonical embedding field should exist");

    assert_eq!(
        described_field.index_config.as_ref(),
        canonical_field.index()
    );
    assert_eq!(
        described_field.attribute_config.as_ref(),
        canonical_field.attribute()
    );
}

#[test]
fn tensor_builder_populates_public_index_and_attribute_configuration() {
    let field = Field::tensor("embedding", "tensor<float>(x[3])")
        .distance_metric(crate::DistanceMetric::Angular)
        .hnsw(crate::HnswIndexConfig::new().max_links_per_node(8));

    assert_eq!(
        field
            .attribute_config
            .as_ref()
            .map(|config| config.distance_metric),
        Some(searchkernel_ir::DistanceMetricIr::Angular)
    );
    let hnsw = field
        .index_config
        .as_ref()
        .and_then(|config| config.hnsw.as_ref())
        .expect("builder HNSW config should be public");
    assert_eq!(hnsw.max_links_per_node, Some(8));
}

#[test]
fn described_field_exposes_summary_script_and_properties() {
    let (canonical, described) = public_described_canonical_fixture();
    let expected = &canonical.fields()[0];
    let actual = &described.fields()[0];
    assert_eq!(actual.summary_config.as_ref(), expected.summary());
    assert_eq!(
        actual.indexing_script.as_deref(),
        expected.indexing_script()
    );
    assert_eq!(&actual.properties, expected.properties());
}

#[test]
fn describe_preserves_schema_only_fields() {
    let (canonical, described) = described_canonical_fixture();
    assert_eq!(canonical.schema_fields(), described.schema_fields());
}

#[test]
fn derived_default_search_fields_include_schema_only_fields() {
    let definition = IndexDefinition::new()
        .field(Field::text("title"))
        .schema_field(Field::text("derived"));
    let resolved = definition_to_ir(&definition).expect("schema should resolve");

    assert_eq!(
        resolved
            .default_search_fields()
            .iter()
            .map(|field| field.field().as_str())
            .collect::<Vec<_>>(),
        ["title", "derived"]
    );
}

#[test]
fn public_array_of_struct_field_lowers_to_shared_array_schema() {
    let people = Field::array_of_struct(
        "people",
        [
            SchemaStructField {
                name: "first".to_owned(),
                data_type: DataType::Primitive(PrimitiveDataType::String),
            },
            SchemaStructField {
                name: "last".to_owned(),
                data_type: DataType::Primitive(PrimitiveDataType::String),
            },
        ],
    )
    .stored(false)
    .searchable(false);
    let resolved = definition_to_ir(&IndexDefinition::new().field(people))
        .expect("public array-of-struct schema should resolve");
    assert!(matches!(
        resolved.fields()[0].data_type(),
        searchkernel_ir::DataTypeIr::Array(inner)
            if matches!(inner.as_ref(), searchkernel_ir::DataTypeIr::Struct { fields }
                if fields.iter().map(|field| field.name.as_str()).collect::<Vec<_>>() == ["first", "last"])
    ));
}
