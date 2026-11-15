use super::*;

#[cfg(test)]
#[test]
fn inspect_query_encoding_preserves_tensors_empty_objects_query_type_and_nested_grouping() {
    use std::collections::BTreeMap;

    let mut inputs = BTreeMap::new();
    inputs.insert(
        "dense".to_owned(),
        ir::QueryInputIr::Literal(ir::RequestValueIr::Tensor(ir::RequestTensorIr {
            type_spec: "tensor(x[2])".to_owned(),
            payload: ir::RequestTensorPayloadIr::Dense(vec![1.0, 2.0]),
        })),
    );
    inputs.insert(
        "sparse".to_owned(),
        ir::QueryInputIr::Literal(ir::RequestValueIr::Tensor(ir::RequestTensorIr {
            type_spec: "tensor(label{})".to_owned(),
            payload: ir::RequestTensorPayloadIr::Sparse(vec![ir::RequestTensorCellIr {
                address: BTreeMap::from([("label".to_owned(), "a".to_owned())]),
                value: 3.0,
            }]),
        })),
    );
    inputs.insert(
        "mixed".to_owned(),
        ir::QueryInputIr::Literal(ir::RequestValueIr::Tensor(ir::RequestTensorIr {
            type_spec: "tensor(label{},x[2])".to_owned(),
            payload: ir::RequestTensorPayloadIr::Mixed(vec![ir::RequestTensorBlockIr {
                address: BTreeMap::from([("label".to_owned(), "a".to_owned())]),
                values: vec![4.0, 5.0],
            }]),
        })),
    );
    let request = ir::SearchRequestIr {
        diagnostic_source: false,
        query: ir::QueryIr::MatchAll,
        search_fields: Vec::new(),
        filter: None,
        sort: Vec::new(),
        grouping: vec![ir::GroupingRequestIr {
            root: ir::GroupingAllIr {
                operations: vec![ir::GroupingOperationIr::All(ir::GroupingAllIr {
                    operations: vec![ir::GroupingOperationIr::Group(
                        ir::GroupingExpressionIr::Attribute("brand".to_owned()),
                    )],
                    label: Some("nested".to_owned()),
                })],
                label: None,
            },
            continuations: Vec::new(),
        }],
        ranking_request: ir::RankingRequestIr {
            profile: "default".to_owned(),
            properties: BTreeMap::new(),
        },
        inputs,
        limit: 20,
        offset: 0,
        projection: ir::ProjectionIr::new(Vec::new()),
        highlighting: ir::HighlightingIr::Disabled,
        language: None,
        query_model: ir::QueryModelIr {
            query_type: Some(ir::QueryModelTypeIr {
                composite: Some("rank".to_owned()),
                tokenization: Some("word".to_owned()),
                syntax: Some("advanced".to_owned()),
                profile: Some("default".to_owned()),
                is_yql_default: Some(false),
            }),
            ..Default::default()
        },
        presentation: ir::PresentationIr::default(),
        properties: BTreeMap::from([(
            "empty".to_owned(),
            ir::RequestValueIr::Object(BTreeMap::new()),
        )]),
        query_profile: None,
        search_chain: None,
        timeout: None,
        prefix_policy: ir::PrefixPolicy::Disabled,
        typo_policy: ir::TypoPolicy::Disabled,
    };

    let bytes = inspect_query_result_to_api(&request);
    let result =
        flatbuffers::root::<api::InspectQueryResult>(&bytes).expect("valid inspect query result");
    let resolved = result.resolved_request().expect("resolved request");
    let inputs = resolved.inputs().expect("query inputs");
    assert_eq!(inputs.len(), 3);
    assert_eq!(
        inputs
            .get(0)
            .input_as_literal_query_input()
            .unwrap()
            .value()
            .unwrap()
            .kind_type(),
        api::ValueKind::TensorValue
    );
    for (index, expected) in [
        api::TensorPayloadKind::DenseTensorValues,
        api::TensorPayloadKind::MixedTensorBlocks,
        api::TensorPayloadKind::SparseTensorCells,
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(
            inputs
                .get(index)
                .input_as_literal_query_input()
                .unwrap()
                .value()
                .unwrap()
                .kind_as_tensor_value()
                .unwrap()
                .payload_type(),
            expected
        );
    }
    let properties = resolved.properties().expect("properties");
    let empty = properties
        .get(0)
        .value()
        .unwrap()
        .kind_as_value_object()
        .unwrap();
    assert_eq!(empty.fields().map(|fields| fields.len()), Some(0));
    let query_type = resolved.query_model().unwrap().type_().unwrap();
    assert_eq!(query_type.composite(), Some("rank"));
    let grouping = resolved.grouping().expect("grouping");
    let nested = grouping.get(0).root().unwrap().operations().unwrap().get(0);
    assert!(nested.operation_as_grouping_all_operation().is_some());
}

#[cfg(test)]
#[test]
fn rich_v2_index_definition_round_trips_through_ir() {
    use crate::{
        AttributeConfigIr, ConstantDefinitionIr, ConstantSourceIr, DataTypeIr,
        DocumentSummaryDefinitionIr, DocumentSummaryFieldDefinitionIr, FieldCapabilities, FieldIr,
        FieldName, FieldSetDefinitionIr, ImportedFieldDefinitionIr, IndexDefinitionIr,
        MatchingConfigIr, MatchingModeIr, NamedStructDefinitionIr, OnnxModelDefinitionIr,
        PrimitiveDataTypeIr, RankProfileDefinitionIr, StructFieldIr,
    };
    use std::collections::BTreeMap;

    let field = |name: &str, data_type| {
        FieldIr::new(
            FieldName::parse(name).unwrap(),
            data_type,
            None,
            FieldCapabilities::new(true, false, true, false, false),
        )
    };
    let mut text = field("text", DataTypeIr::Primitive(PrimitiveDataTypeIr::String));
    text.matching = Some(MatchingConfigIr {
        mode: MatchingModeIr::Exact,
        options: BTreeMap::from([("case".to_owned(), ir::RequestValueIr::Bool(true))]),
    });
    text.attribute = Some(AttributeConfigIr {
        enabled: Some(true),
        fast_search: Some(true),
        fast_access: None,
        distance_metric: crate::DistanceMetricIr::Unspecified,
        options: BTreeMap::new(),
    });
    let mut definition = IndexDefinitionIr::new(
        None,
        vec![
            text,
            field("byte", DataTypeIr::Primitive(PrimitiveDataTypeIr::Byte)),
            field("int32", DataTypeIr::Primitive(PrimitiveDataTypeIr::Int32)),
            field("raw", DataTypeIr::Primitive(PrimitiveDataTypeIr::Raw)),
            field(
                "tensor",
                DataTypeIr::Tensor {
                    type_spec: "tensor(x[2])".to_owned(),
                },
            ),
            field(
                "array",
                DataTypeIr::Array(Box::new(DataTypeIr::Primitive(PrimitiveDataTypeIr::Int64))),
            ),
            field(
                "map",
                DataTypeIr::Map {
                    key_type: Box::new(DataTypeIr::Primitive(PrimitiveDataTypeIr::String)),
                    value_type: Box::new(DataTypeIr::Primitive(PrimitiveDataTypeIr::Float32)),
                },
            ),
            {
                let mut weighted = field(
                    "weighted",
                    DataTypeIr::WeightedSet(Box::new(DataTypeIr::Primitive(
                        PrimitiveDataTypeIr::String,
                    ))),
                );
                weighted.analyzer = Some(crate::AnalyzerIr {
                    name: "standard".to_owned(),
                    version: 1,
                    options: BTreeMap::new(),
                });
                weighted
            },
            field(
                "inline",
                DataTypeIr::Struct {
                    fields: vec![StructFieldIr {
                        name: "part".to_owned(),
                        data_type: DataTypeIr::Primitive(PrimitiveDataTypeIr::Bool),
                    }],
                },
            ),
            field(
                "named",
                DataTypeIr::StructReference {
                    name: "address".to_owned(),
                },
            ),
            field("predicate", DataTypeIr::Predicate),
            field("position", DataTypeIr::Position),
            field(
                "parent",
                DataTypeIr::Reference {
                    document_type: "parent_doc".to_owned(),
                },
            ),
        ],
        Vec::new(),
    )
    .unwrap();
    definition.set_document_type("doc");
    definition.schema_inherits = vec!["base".to_owned()];
    definition.structs = vec![NamedStructDefinitionIr {
        name: "address".to_owned(),
        fields: vec![StructFieldIr {
            name: "city".to_owned(),
            data_type: DataTypeIr::Primitive(PrimitiveDataTypeIr::String),
        }],
        properties: BTreeMap::new(),
    }];
    definition.fieldsets = vec![FieldSetDefinitionIr {
        name: "all_text".to_owned(),
        fields: vec![FieldName::parse("text").unwrap()],
        matching: None,
        query_commands: vec!["stem".to_owned()],
        properties: BTreeMap::new(),
    }];
    let mut derived = field(
        "derived",
        DataTypeIr::Primitive(PrimitiveDataTypeIr::String),
    );
    derived.capabilities.filterable = false;
    derived.analyzer = Some(crate::AnalyzerIr {
        name: "standard".to_owned(),
        version: 1,
        options: BTreeMap::new(),
    });
    definition.schema_fields = vec![derived];
    definition.imported_fields = vec![ImportedFieldDefinitionIr {
        reference_field: FieldName::parse("parent").unwrap(),
        target_field: "name".to_owned(),
        alias: FieldName::parse("parent_name").unwrap(),
        properties: BTreeMap::new(),
    }];
    definition.document_summaries = vec![DocumentSummaryDefinitionIr {
        name: "short".to_owned(),
        inherits: Vec::new(),
        fields: vec![DocumentSummaryFieldDefinitionIr {
            name: "text".to_owned(),
            source: "text".to_owned(),
            data_type: DataTypeIr::Primitive(PrimitiveDataTypeIr::String),
            struct_fields: Vec::new(),
            properties: BTreeMap::new(),
        }],
        from_disk: Some(false),
        omit_summary_features: Some(true),
        properties: BTreeMap::new(),
    }];
    definition.constants = vec![ConstantDefinitionIr {
        name: "bias".to_owned(),
        data_type: DataTypeIr::Primitive(PrimitiveDataTypeIr::Float64),
        source: ConstantSourceIr::Literal(ir::RequestValueIr::Float64(1.0)),
        properties: BTreeMap::new(),
    }];
    definition.onnx_models = vec![OnnxModelDefinitionIr {
        name: "model".to_owned(),
        file: "model.onnx".to_owned(),
        inputs: BTreeMap::from([("input".to_owned(), "query(q)".to_owned())]),
        outputs: BTreeMap::new(),
        gpu_device: Some(0),
        intraop_threads: Some(1),
        interop_threads: None,
        properties: BTreeMap::new(),
    }];
    definition.rank_profiles = vec![RankProfileDefinitionIr {
        name: "rank".to_owned(),
        inherits: Vec::new(),
        inputs: Vec::new(),
        functions: Vec::new(),
        first_phase: None,
        second_phase: None,
        global_phase: None,
        rank_features: Vec::new(),
        match_features: vec!["score".to_owned()],
        summary_features: Vec::new(),
        sort_features: Vec::new(),
        constants: Vec::new(),
        onnx_models: Vec::new(),
        rank_properties: BTreeMap::from([("foo".to_owned(), "bar".to_owned())]),
        strict: Some(true),
        properties: BTreeMap::new(),
    }];

    let IndexDefinitionIr {
        name: _,
        fields: _,
        default_search_fields: _,
        properties: _,
        document_type: _,
        resolved_document_type: _,
        schema_inherits: _,
        document_inherits: _,
        structs: _,
        fieldsets: _,
        schema_fields: _,
        imported_fields: _,
        document_summaries: _,
        rank_profiles: _,
        constants: _,
        onnx_models: _,
    } = &definition;

    let bytes = index_definition_ir_to_api(&definition);
    let api = flatbuffers::root::<api::IndexDefinition>(&bytes).unwrap();
    let first = crate::wire::v1::index_definition_to_ir(api).unwrap();
    assert_eq!(
        definition, first,
        "independently constructed canonical IR must survive its first FlatBuffers round trip"
    );
    let bytes = index_definition_ir_to_api(&first);
    let api = flatbuffers::root::<api::IndexDefinition>(&bytes).unwrap();
    let second = crate::wire::v1::index_definition_to_ir(api).unwrap();
    assert_eq!(first, second);
}

#[cfg(test)]
#[test]
fn indexing_script_only_field_round_trips_through_v2_without_typed_capabilities() {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let name = builder.create_string("derived");
    let primitive = api::PrimitiveDataType::create(
        &mut builder,
        &api::PrimitiveDataTypeArgs {
            kind: api::PrimitiveDataTypeKind::String,
        },
    );
    let data_type = api::DataType::create(
        &mut builder,
        &api::DataTypeArgs {
            kind_type: api::DataTypeKind::PrimitiveDataType,
            kind: Some(primitive.as_union_value()),
        },
    );
    let indexing_script = builder.create_string("input title | attribute(derived)");
    let field = api::FieldDefinition::create(
        &mut builder,
        &api::FieldDefinitionArgs {
            name: Some(name),
            data_type: Some(data_type),
            indexing_script: Some(indexing_script),
            ..Default::default()
        },
    );
    let fields = builder.create_vector(&[field]);
    let definition = api::IndexDefinition::create(
        &mut builder,
        &api::IndexDefinitionArgs {
            fields: Some(fields),
            ..Default::default()
        },
    );
    builder.finish(definition, None);
    let first = crate::wire::v1::index_definition_to_ir(
        flatbuffers::root::<api::IndexDefinition>(builder.finished_data()).unwrap(),
    )
    .unwrap();
    assert_eq!(
        first.fields()[0].indexing_script(),
        Some("input title | attribute(derived)")
    );

    let bytes = index_definition_ir_to_api(&first);
    let second = crate::wire::v1::index_definition_to_ir(
        flatbuffers::root::<api::IndexDefinition>(&bytes).unwrap(),
    )
    .unwrap();
    assert_eq!(first, second);
}

#[cfg(test)]
#[test]
fn indexing_script_with_unrelated_metadata_round_trips_losslessly() {
    fn round_trip(include_properties: bool, include_struct_fields: bool) {
        let mut builder = flatbuffers::FlatBufferBuilder::new();
        let name = builder.create_string("derived");
        let primitive = api::PrimitiveDataType::create(
            &mut builder,
            &api::PrimitiveDataTypeArgs {
                kind: api::PrimitiveDataTypeKind::String,
            },
        );
        let data_type = api::DataType::create(
            &mut builder,
            &api::DataTypeArgs {
                kind_type: api::DataTypeKind::PrimitiveDataType,
                kind: Some(primitive.as_union_value()),
            },
        );
        let indexing_script = builder.create_string("input title | attribute(derived)");
        let properties = include_properties.then(|| {
            request_value_entries_offset(
                &mut builder,
                &std::collections::BTreeMap::from([(
                    "some.unrelated.setting".to_owned(),
                    ir::RequestValueIr::String("preserve".to_owned()),
                )]),
            )
            .expect("property entry")
        });
        let struct_fields = include_struct_fields.then(|| {
            let path = builder.create_string("nested");
            let field = api::StructFieldConfig::create(
                &mut builder,
                &api::StructFieldConfigArgs {
                    path: Some(path),
                    ..Default::default()
                },
            );
            builder.create_vector(&[field])
        });
        let field = api::FieldDefinition::create(
            &mut builder,
            &api::FieldDefinitionArgs {
                name: Some(name),
                data_type: Some(data_type),
                indexing_script: Some(indexing_script),
                properties,
                struct_fields,
                ..Default::default()
            },
        );
        let fields = builder.create_vector(&[field]);
        let definition = api::IndexDefinition::create(
            &mut builder,
            &api::IndexDefinitionArgs {
                fields: Some(fields),
                ..Default::default()
            },
        );
        builder.finish(definition, None);

        let first = crate::wire::v1::index_definition_to_ir(
            flatbuffers::root::<api::IndexDefinition>(builder.finished_data()).unwrap(),
        )
        .unwrap();
        let bytes = index_definition_ir_to_api(&first);
        let second = crate::wire::v1::index_definition_to_ir(
            flatbuffers::root::<api::IndexDefinition>(&bytes).unwrap(),
        )
        .unwrap();
        assert_eq!(first, second);
    }

    round_trip(true, false);
    round_trip(false, true);
}

#[cfg(test)]
#[test]
fn rank_phase_controls_are_validated_against_their_phase() {
    use crate::{
        DataTypeIr, FieldCapabilities, FieldIr, FieldName, IndexDefinitionIr, PrimitiveDataTypeIr,
        RankPhaseDefinitionIr, RankProfileDefinitionIr,
    };
    use std::collections::BTreeMap;

    let field = FieldIr::new(
        FieldName::parse("title").unwrap(),
        DataTypeIr::Primitive(PrimitiveDataTypeIr::String),
        None,
        FieldCapabilities::new(true, true, false, false, false),
    );
    let mut definition = IndexDefinitionIr::new(None, vec![field], Vec::new()).unwrap();
    definition.rank_profiles = vec![RankProfileDefinitionIr {
        name: "invalid".to_owned(),
        inherits: Vec::new(),
        inputs: Vec::new(),
        functions: Vec::new(),
        first_phase: Some(RankPhaseDefinitionIr {
            expression: Some("nativeRank(title)".to_owned()),
            rerank_count: Some(10),
            rank_score_drop_limit: None,
            properties: BTreeMap::new(),
            total_rerank_count: None,
            keep_rank_count: None,
            total_keep_rank_count: None,
        }),
        second_phase: None,
        global_phase: None,
        rank_features: Vec::new(),
        match_features: Vec::new(),
        summary_features: Vec::new(),
        sort_features: Vec::new(),
        constants: Vec::new(),
        onnx_models: Vec::new(),
        rank_properties: BTreeMap::new(),
        strict: None,
        properties: BTreeMap::new(),
    }];

    let bytes = index_definition_ir_to_api(&definition);
    let error = crate::wire::v1::index_definition_to_ir(
        flatbuffers::root::<api::IndexDefinition>(&bytes).unwrap(),
    )
    .unwrap_err();
    assert_eq!(error.details[0].code, "invalid_rank_phase");
    assert_eq!(
        error.details[0].path.as_deref(),
        Some("rank_profiles[0].first_phase")
    );
}
