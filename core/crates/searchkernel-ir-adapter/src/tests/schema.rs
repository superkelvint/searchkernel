use super::*;

#[test]
fn multi_valued_schema_is_rejected_before_m_configuration() {
    let definition = IndexDefinitionIr::new(
        None,
        vec![FieldIr::new(
            field("tags"),
            DataTypeIr::Array(Box::new(DataTypeIr::Primitive(PrimitiveDataTypeIr::Int64))),
            None,
            FieldCapabilities::new(false, false, true, false, false),
        )],
        Vec::new(),
    )
    .unwrap();
    let error = validate_index_definition(&definition).unwrap_err();
    assert_eq!(error.code, searchkernel_ir::ErrorCode::Unsupported);
}

#[test]
fn document_type_drives_native_schema_identity() {
    let definition = {
        let field = FieldIr::new(
            field("title"),
            DataTypeIr::Primitive(PrimitiveDataTypeIr::String),
            None,
            FieldCapabilities::new(true, true, false, false, false),
        );
        let mut definition = IndexDefinitionIr::new(
            Some(IndexName::parse("index_name").unwrap()),
            vec![field],
            Vec::new(),
        )
        .unwrap();
        definition.set_document_type("document_type");
        definition
    };
    let root = std::env::temp_dir().join(format!(
        "searchkernel-adapter-document-type-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    compile_index_definition(&definition, &root).expect("schema compiles");
    let manifest =
        std::fs::read_to_string(root.join("searchkernel-schema.json")).expect("manifest");
    assert!(manifest.contains("\"document_type\":\"document_type\""));
    assert!(!manifest.contains("\"document_type\":\"index_name\""));
    std::fs::remove_dir_all(root).expect("remove temporary schema");
}

#[test]
fn portable_dense_tensor_definition_compiles_and_opens_through_adapter() {
    let mut definition = IndexDefinitionIr::new(
        None,
        vec![FieldIr::new(
            field("embedding"),
            DataTypeIr::Tensor {
                type_spec: "tensor<float>(x[2])".to_owned(),
            },
            None,
            FieldCapabilities::new(true, false, false, false, false),
        )],
        Vec::new(),
    )
    .unwrap();
    definition.set_document_type("portable_tensor_test");
    let root = std::env::temp_dir().join(format!(
        "searchkernel-adapter-portable-tensor-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let config = root.join("config");
    let data = root.join("data");

    compile_index_definition(&definition, &config).expect("tensor schema compiles");
    let backend = VespaBackend::open(&data, &config).expect("tensor schema opens");
    Box::new(backend).close().expect("tensor backend closes");
    std::fs::remove_dir_all(root).expect("remove temporary tensor schema");
}

#[test]
fn portable_hnsw_tensor_definition_lowers_to_native_attribute_config() {
    let mut embedding = FieldIr::new(
        field("embedding"),
        DataTypeIr::Tensor {
            type_spec: "tensor<float>(x[3])".to_owned(),
        },
        None,
        FieldCapabilities::new(true, true, false, false, false),
    );
    embedding.set_index_config(Some(IndexConfigIr {
        enabled: Some(true),
        hnsw: Some(HnswIndexConfigIr {
            max_links_per_node: Some(4),
            neighbors_to_explore_at_insert: Some(16),
            options: std::collections::BTreeMap::new(),
        }),
        options: std::collections::BTreeMap::new(),
    }));
    embedding.set_attribute_config(Some(AttributeConfigIr {
        enabled: Some(true),
        fast_search: None,
        fast_access: None,
        distance_metric: DistanceMetricIr::Euclidean,
        options: std::collections::BTreeMap::new(),
    }));
    let mut definition = IndexDefinitionIr::new(None, vec![embedding], Vec::new()).unwrap();
    definition.set_document_type("portable_hnsw_test");
    let root = std::env::temp_dir().join(format!(
        "searchkernel-adapter-hnsw-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    compile_index_definition(&definition, &root).expect("HNSW schema compiles");
    let attributes = std::fs::read_to_string(root.join("attributes.cfg")).unwrap();
    assert!(attributes.contains("attribute[0].index.hnsw.enabled true"));
    assert!(attributes.contains("attribute[0].distancemetric EUCLIDEAN"));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn typed_attribute_settings_are_lowered_and_contradictions_fail_closed() {
    let mut value_field = FieldIr::new(
        field("value"),
        DataTypeIr::Primitive(PrimitiveDataTypeIr::Int64),
        None,
        FieldCapabilities::new(true, false, true, false, false),
    );
    value_field.set_attribute_config(Some(AttributeConfigIr {
        enabled: Some(true),
        fast_search: Some(true),
        fast_access: None,
        distance_metric: searchkernel_ir::DistanceMetricIr::Unspecified,
        options: std::collections::BTreeMap::new(),
    }));
    let mut definition = IndexDefinitionIr::new(None, vec![value_field], Vec::new()).unwrap();
    definition.set_document_type("typed_attribute_test");
    validate_index_definition(&definition).expect("typed attribute settings are supported");
    let root = std::env::temp_dir().join(format!(
        "searchkernel-adapter-attribute-settings-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    compile_index_definition(&definition, &root).expect("schema compiles");
    let attributes =
        std::fs::read_to_string(root.join("attributes.cfg")).expect("attributes config");
    assert!(attributes.contains("fastsearch true"));
    std::fs::remove_dir_all(root).expect("remove temporary schema");

    let mut contradictory = FieldIr::new(
        field("value"),
        DataTypeIr::Primitive(PrimitiveDataTypeIr::Int64),
        None,
        FieldCapabilities::new(true, false, true, false, false),
    );
    contradictory.set_attribute_config(Some(AttributeConfigIr {
        enabled: Some(false),
        fast_search: None,
        fast_access: None,
        distance_metric: searchkernel_ir::DistanceMetricIr::Unspecified,
        options: std::collections::BTreeMap::new(),
    }));
    let definition = IndexDefinitionIr::new(None, vec![contradictory], Vec::new()).unwrap();
    let error = validate_index_definition(&definition).expect_err("contradiction rejected");
    assert_eq!(error.code, searchkernel_ir::ErrorCode::InvalidArgument);
    assert_eq!(error.details[0].code, "invalid_capability");
}

#[test]
fn adapter_consumes_canonical_resolved_document_identity() {
    let title = FieldIr::new(
        field("title"),
        DataTypeIr::Primitive(PrimitiveDataTypeIr::String),
        None,
        FieldCapabilities::new(true, true, false, false, false),
    );
    let definition = IndexDefinitionIr::new(None, vec![title], Vec::new()).unwrap();
    assert_eq!(definition.document_type(), None);
    assert_eq!(definition.resolved_document_type(), "searchkernel");

    let config_dir = std::env::temp_dir().join(format!(
        "searchkernel-resolved-document-type-{}",
        std::process::id()
    ));
    compile_index_definition(&definition, &config_dir)
        .expect("adapter accepts identity resolved by canonical IR");
    let manifest = std::fs::read_to_string(config_dir.join("searchkernel-schema.json"))
        .expect("compiled schema manifest");
    assert!(manifest.contains(&format!(
        "\"document_type\":\"{}\"",
        definition.resolved_document_type()
    )));
    std::fs::remove_dir_all(config_dir).expect("remove generated config");
}

#[test]
fn unsupported_schema_capabilities_are_rejected_before_m_configuration() {
    for (kind, capabilities, expected_path) in [
        (
            FieldKind::Text,
            FieldCapabilities::new(true, false, false, false, false),
            "fields[0].searchable",
        ),
        (
            FieldKind::Float64,
            FieldCapabilities::new(true, false, true, true, true),
            "fields[0].facetable",
        ),
    ] {
        let definition = IndexDefinitionIr::new(
            None,
            vec![FieldIr::new(
                field("value"),
                match kind {
                    FieldKind::Text | FieldKind::Keyword => {
                        DataTypeIr::Primitive(PrimitiveDataTypeIr::String)
                    }
                    FieldKind::Float64 => DataTypeIr::Primitive(PrimitiveDataTypeIr::Float64),
                    FieldKind::Int64 => DataTypeIr::Primitive(PrimitiveDataTypeIr::Int64),
                    FieldKind::Bool => DataTypeIr::Primitive(PrimitiveDataTypeIr::Bool),
                },
                None,
                capabilities,
            )],
            Vec::new(),
        )
        .unwrap();
        let error = validate_index_definition(&definition).unwrap_err();
        assert_eq!(error.code, searchkernel_ir::ErrorCode::Unsupported);
        assert_eq!(error.details[0].code, "unsupported_field_capability");
        assert_eq!(error.details[0].path.as_deref(), Some(expected_path));
    }

    let definition = IndexDefinitionIr::new(
        None,
        vec![FieldIr::new(
            field("active"),
            DataTypeIr::Primitive(PrimitiveDataTypeIr::Bool),
            None,
            FieldCapabilities::new(true, false, true, true, true),
        )],
        Vec::new(),
    )
    .unwrap();
    validate_index_definition(&definition)
        .expect("single-valued boolean capabilities are supported");
}

#[test]
fn nearest_neighbor_lowers_dense_tensor_input_to_native_request_input() {
    let request = request(Vec::new(), Vec::new(), None)
        .with_query(typed_expression(QueryExprKindIr::NearestNeighbor {
            document_field: typed_field("embedding"),
            query_tensor: "query_embedding".to_owned(),
        }))
        .with_inputs(std::collections::BTreeMap::from([(
            "query_embedding".to_owned(),
            QueryInputIr::Literal(RequestValueIr::Tensor(RequestTensorIr {
                type_spec: "tensor<float>(x[2])".to_owned(),
                payload: RequestTensorPayloadIr::Dense(vec![0.1, 0.2]),
            })),
        )]));
    validate_search_request(&request).expect("dense nearest-neighbor input validates");
    let query = lower_search_query(&request, analysis_options(request.language()).unwrap())
        .expect("nearest-neighbor query lowers");
    let lowered = lower_search_options(
        &request,
        query,
        analysis_options(request.language()).unwrap(),
    )
    .expect("nearest-neighbor request lowers");
    assert_eq!(
        lowered.query_inputs,
        vec![(
            "query_embedding".to_owned(),
            searchkernel_vespa_engine::QueryInput::Literal(
                searchkernel_vespa_engine::TensorValue::Dense {
                    type_spec: "tensor<float>(x[2])".to_owned(),
                    values: vec![0.1, 0.2],
                }
            )
        )]
    );
}
