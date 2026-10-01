use super::super::support::*;
use super::super::*;
use searchkernel_protocol::wire::v1 as api;

macro_rules! fixture_field {
    (
        $builder:expr,
        $name:expr,
        $data_type:expr,
        $stored:expr,
        $searchable:expr,
        $filterable:expr,
        $sortable:expr,
        $facetable:expr,
        $analyzer:expr,
        $matching:expr,
        $index:expr,
        $attribute:expr,
        $summary:expr,
        $indexing_script:expr,
        $properties:expr,
        $struct_fields:expr $(,)?
    ) => {{
        let data_type = $data_type;
        let analyzer = $analyzer;
        let matching = $matching;
        let index = $index;
        let attribute = $attribute;
        let summary = $summary;
        let properties = $properties;
        let struct_fields = $struct_fields;
        fixture_field_with_struct_fields(
            $builder,
            $name,
            data_type,
            $stored,
            $searchable,
            $filterable,
            $sortable,
            $facetable,
            analyzer,
            matching,
            index,
            attribute,
            summary,
            $indexing_script,
            properties,
            struct_fields,
        )
    }};
}

pub(in crate::index::api_tests) fn canonical_fixture_ir() -> searchkernel_ir::IndexDefinitionIr {
    canonical_fixture_ir_named("complete_schema", 0.75, 0.25)
}

pub(in crate::index::api_tests) fn canonical_fixture_ir_named(
    name: &str,
    title_weight: f64,
    tags_weight: f64,
) -> searchkernel_ir::IndexDefinitionIr {
    let bytes = canonical_schema_fixture_named(name, title_weight, tags_weight);
    let root = searchkernel_ir::__flatbuffers::root::<api::IndexDefinition>(&bytes)
        .expect("canonical fixture should parse");
    searchkernel_ir::index_definition_to_ir(root)
        .expect("canonical fixture should resolve through FlatBuffers")
}

pub(in crate::index::api_tests) fn canonical_schema_fixture_named(
    name: &str,
    title_weight: f64,
    tags_weight: f64,
) -> Vec<u8> {
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();

    let title_data_type = primitive(&mut builder, api::PrimitiveDataTypeKind::String);
    let title_matching = matching(
        &mut builder,
        api::MatchingMode::Text,
        &[("matching.option", "title")],
    );
    let title_index = index_config(&mut builder, Some(true), None, &[("index.option", "title")]);
    let title_attribute = attribute(
        &mut builder,
        Some(true),
        Some(false),
        Some(true),
        api::DistanceMetric::Unspecified,
        &[("attribute.option", "title")],
    );
    let title_summary = summary(&mut builder, Some(true), &[("summary.option", "title")]);
    let title_analyzer = analyzer(&mut builder, "standard", 1, &[]);
    let nested_matching = matching(
        &mut builder,
        api::MatchingMode::Exact,
        &[("nested.matching", "exact")],
    );
    let nested_index = index_config(&mut builder, Some(true), None, &[("nested.index", "true")]);
    let nested_attribute = attribute(
        &mut builder,
        Some(true),
        Some(false),
        Some(true),
        api::DistanceMetric::Unspecified,
        &[("nested.attribute", "true")],
    );
    let nested_summary = summary(&mut builder, Some(true), &[("nested.summary", "true")]);
    let nested_config = struct_field_config(
        &mut builder,
        "value.first_name",
        Some(nested_matching),
        Some(nested_index),
        Some(nested_attribute),
        Some(nested_summary),
    );
    let title_struct_fields = builder.create_vector(&[nested_config]);
    let title_properties = string_properties(
        &mut builder,
        &[("field.property", "title"), ("field.second", "preserve")],
    );
    let title = fixture_field!(
        &mut builder,
        "title",
        title_data_type,
        Some(true),
        Some(true),
        Some(false),
        Some(false),
        Some(false),
        Some(title_analyzer),
        Some(title_matching),
        Some(title_index),
        Some(title_attribute),
        Some(title_summary),
        None,
        Some(title_properties),
        Some(title_struct_fields),
    );

    let flag = fixture_field!(
        &mut builder,
        "flag",
        primitive(&mut builder, api::PrimitiveDataTypeKind::Bool),
        Some(false),
        Some(false),
        Some(true),
        Some(true),
        Some(true),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    let byte_value = fixture_field!(
        &mut builder,
        "byte_value",
        primitive(&mut builder, api::PrimitiveDataTypeKind::Byte),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    let count32 = fixture_field!(
        &mut builder,
        "count32",
        primitive(&mut builder, api::PrimitiveDataTypeKind::Int32),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    let count64 = fixture_field!(
        &mut builder,
        "count64",
        primitive(&mut builder, api::PrimitiveDataTypeKind::Int64),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    let ratio32 = fixture_field!(
        &mut builder,
        "ratio32",
        primitive(&mut builder, api::PrimitiveDataTypeKind::Float32),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    let ratio64 = fixture_field!(
        &mut builder,
        "ratio64",
        primitive(&mut builder, api::PrimitiveDataTypeKind::Float64),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    let payload = fixture_field!(
        &mut builder,
        "payload",
        primitive(&mut builder, api::PrimitiveDataTypeKind::Raw),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );

    let embedding_hnsw = hnsw_config(&mut builder, 16, 100, &[("hnsw.option", "value")]);
    let embedding_index = index_config(
        &mut builder,
        Some(true),
        Some(embedding_hnsw),
        &[("embedding.index", "true")],
    );
    let embedding_attribute = attribute(
        &mut builder,
        Some(true),
        Some(false),
        Some(true),
        api::DistanceMetric::Angular,
        &[("embedding.attribute", "angular")],
    );
    let embedding = fixture_field!(
        &mut builder,
        "embedding",
        tensor(&mut builder, "tensor<float>(x[3])"),
        Some(false),
        Some(true),
        Some(false),
        Some(false),
        Some(false),
        None,
        None,
        Some(embedding_index),
        Some(embedding_attribute),
        None,
        None,
        None,
        None,
    );
    let tags_element_type = primitive(&mut builder, api::PrimitiveDataTypeKind::String);
    let tags_type = array(&mut builder, tags_element_type);
    let tags_analyzer = analyzer(&mut builder, "standard", 1, &[]);
    let tags_matching = matching(&mut builder, api::MatchingMode::Text, &[]);
    let tags = fixture_field!(
        &mut builder,
        "tags",
        tags_type,
        Some(true),
        Some(true),
        Some(false),
        Some(false),
        Some(false),
        Some(tags_analyzer),
        Some(tags_matching),
        None,
        None,
        None,
        None,
        None,
        None,
    );
    let scores_key_type = primitive(&mut builder, api::PrimitiveDataTypeKind::String);
    let scores_value_type = primitive(&mut builder, api::PrimitiveDataTypeKind::Float32);
    let scores_type = map(&mut builder, scores_key_type, scores_value_type);
    let scores = fixture_field!(
        &mut builder,
        "scores",
        scores_type,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    let labels_element_type = primitive(&mut builder, api::PrimitiveDataTypeKind::String);
    let labels_type = weighted_set(&mut builder, labels_element_type);
    let labels_matching = matching(&mut builder, api::MatchingMode::Exact, &[]);
    let labels = fixture_field!(
        &mut builder,
        "labels",
        labels_type,
        Some(true),
        Some(false),
        Some(true),
        Some(false),
        Some(true),
        None,
        Some(labels_matching),
        None,
        None,
        None,
        None,
        None,
        None,
    );
    let inline_part_type = primitive(&mut builder, api::PrimitiveDataTypeKind::Bool);
    let inline_value_type = primitive(&mut builder, api::PrimitiveDataTypeKind::Int32);
    let inline_type = struct_type(
        &mut builder,
        &[("part", inline_part_type), ("value", inline_value_type)],
    );
    let inline = fixture_field!(
        &mut builder,
        "inline",
        inline_type,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    let address = fixture_field!(
        &mut builder,
        "address",
        struct_reference(&mut builder, "address"),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    let predicate = fixture_field!(
        &mut builder,
        "predicate",
        empty_data_type(&mut builder, api::DataTypeKind::PredicateDataType),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    let location = fixture_field!(
        &mut builder,
        "location",
        empty_data_type(&mut builder, api::DataTypeKind::PositionDataType),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    let parent = fixture_field!(
        &mut builder,
        "parent",
        reference(&mut builder, "parent_doc"),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    let derived = fixture_field!(
        &mut builder,
        "derived",
        primitive(&mut builder, api::PrimitiveDataTypeKind::String),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        Some("input title | attribute(derived)"),
        Some(string_properties(
            &mut builder,
            &[("derived.property", "value")]
        )),
        None,
    );
    let fields = builder.create_vector(&[
        title, flag, byte_value, count32, count64, ratio32, ratio64, payload, embedding, tags,
        scores, labels, inline, address, predicate, location, parent,
    ]);
    let schema_fields = {
        let derived = api::SearchOnlyFieldDefinition::create(
            &mut builder,
            &api::SearchOnlyFieldDefinitionArgs {
                field: Some(derived),
            },
        );
        builder.create_vector(&[derived])
    };

    let address_fields = {
        let city = builder.create_string("city");
        let city_type = primitive(&mut builder, api::PrimitiveDataTypeKind::String);
        let city = api::StructFieldDefinition::create(
            &mut builder,
            &api::StructFieldDefinitionArgs {
                name: Some(city),
                data_type: Some(city_type),
            },
        );
        let zip = builder.create_string("zip");
        let zip_type = primitive(&mut builder, api::PrimitiveDataTypeKind::Int32);
        let zip = api::StructFieldDefinition::create(
            &mut builder,
            &api::StructFieldDefinitionArgs {
                name: Some(zip),
                data_type: Some(zip_type),
            },
        );
        builder.create_vector(&[city, zip])
    };
    let named_struct = {
        let name = builder.create_string("address");
        let properties = string_properties(&mut builder, &[("struct.property", "value")]);
        api::NamedStructDefinition::create(
            &mut builder,
            &api::NamedStructDefinitionArgs {
                name: Some(name),
                fields: Some(address_fields),
                properties: Some(properties),
            },
        )
    };
    let structs = builder.create_vector(&[named_struct]);

    let fieldset = {
        let name = builder.create_string("all_text");
        let field_names = strings(&mut builder, &["title", "tags"]);
        let matching = matching(
            &mut builder,
            api::MatchingMode::Word,
            &[("fieldset.matching", "word")],
        );
        let query_commands = strings(&mut builder, &["stem", "normalize"]);
        let properties = string_properties(&mut builder, &[("fieldset.property", "value")]);
        api::FieldSetDefinition::create(
            &mut builder,
            &api::FieldSetDefinitionArgs {
                name: Some(name),
                fields: Some(field_names),
                matching: Some(matching),
                query_commands: Some(query_commands),
                properties: Some(properties),
            },
        )
    };
    let fieldsets = builder.create_vector(&[fieldset]);

    let imported_field = {
        let reference_field = builder.create_string("parent");
        let target_field = builder.create_string("name");
        let alias = builder.create_string("parent_name");
        let properties = string_properties(&mut builder, &[("import.property", "value")]);
        api::ImportedFieldDefinition::create(
            &mut builder,
            &api::ImportedFieldDefinitionArgs {
                reference_field: Some(reference_field),
                target_field: Some(target_field),
                alias: Some(alias),
                properties: Some(properties),
            },
        )
    };
    let imported_fields = builder.create_vector(&[imported_field]);

    let summary_field = {
        let name = builder.create_string("title_summary");
        let source = builder.create_string("title");
        let data_type = primitive(&mut builder, api::PrimitiveDataTypeKind::String);
        let struct_fields = strings(&mut builder, &["value.first_name"]);
        let properties = string_properties(&mut builder, &[("summary.field", "value")]);
        api::DocumentSummaryFieldDefinition::create(
            &mut builder,
            &api::DocumentSummaryFieldDefinitionArgs {
                name: Some(name),
                source: Some(source),
                data_type: Some(data_type),
                struct_fields: Some(struct_fields),
                properties: Some(properties),
            },
        )
    };
    let summary_fields = builder.create_vector(&[summary_field]);
    let document_summary = {
        let name = builder.create_string("short");
        let inherits = strings(&mut builder, &["base_summary"]);
        let from_disk = api::OptionalBool::new(false);
        let omit_summary_features = api::OptionalBool::new(true);
        let properties = string_properties(&mut builder, &[("summary.property", "value")]);
        api::DocumentSummaryDefinition::create(
            &mut builder,
            &api::DocumentSummaryDefinitionArgs {
                name: Some(name),
                inherits: Some(inherits),
                fields: Some(summary_fields),
                from_disk: Some(&from_disk),
                omit_summary_features: Some(&omit_summary_features),
                properties: Some(properties),
            },
        )
    };
    let document_summaries = builder.create_vector(&[document_summary]);

    let input_scalar = {
        let name = builder.create_string("query_boost");
        let data_type = primitive(&mut builder, api::PrimitiveDataTypeKind::Float64);
        let default_value = float_value(&mut builder, 1.5);
        api::RankProfileInput::create(
            &mut builder,
            &api::RankProfileInputArgs {
                name: Some(name),
                data_type: Some(data_type),
                default_value: Some(default_value),
            },
        )
    };
    let input_tensor = {
        let name = builder.create_string("query_vector");
        let data_type = tensor(&mut builder, "tensor<float>(x[3])");
        let default_value = tensor_value(&mut builder, "tensor<float>(x[3])");
        api::RankProfileInput::create(
            &mut builder,
            &api::RankProfileInputArgs {
                name: Some(name),
                data_type: Some(data_type),
                default_value: Some(default_value),
            },
        )
    };
    let inputs = builder.create_vector(&[input_scalar, input_tensor]);
    let function_a = {
        let name = builder.create_string("boost");
        let arguments = strings(&mut builder, &["x", "y"]);
        let expression = builder.create_string("x + y");
        let inline = api::OptionalBool::new(true);
        api::RankFunctionDefinition::create(
            &mut builder,
            &api::RankFunctionDefinitionArgs {
                name: Some(name),
                arguments: Some(arguments),
                expression: Some(expression),
                inline: Some(&inline),
            },
        )
    };
    let function_b = {
        let name = builder.create_string("slow");
        let arguments = strings(&mut builder, &["x"]);
        let expression = builder.create_string("x * 2");
        let inline = api::OptionalBool::new(false);
        api::RankFunctionDefinition::create(
            &mut builder,
            &api::RankFunctionDefinitionArgs {
                name: Some(name),
                arguments: Some(arguments),
                expression: Some(expression),
                inline: Some(&inline),
            },
        )
    };
    let function_c = {
        let name = builder.create_string("default_inline");
        let arguments = strings(&mut builder, &["x"]);
        let expression = builder.create_string("x");
        api::RankFunctionDefinition::create(
            &mut builder,
            &api::RankFunctionDefinitionArgs {
                name: Some(name),
                arguments: Some(arguments),
                expression: Some(expression),
                inline: None,
            },
        )
    };
    let functions = builder.create_vector(&[function_a, function_b, function_c]);

    let complete_profile = {
        let name = builder.create_string("complete_profile");
        let inherits = strings(&mut builder, &["base_profile"]);
        let functions = Some(functions);
        let first_phase = rank_phase(
            &mut builder,
            "nativeRank(title)",
            None,
            Some(0.25),
            None,
            Some(5),
            None,
        );
        let second_phase = rank_phase(
            &mut builder,
            "rerank(embedding)",
            Some(10),
            None,
            None,
            None,
            None,
        );
        let global_phase = rank_phase(
            &mut builder,
            "globalPhaseScore",
            None,
            Some(0.5),
            None,
            None,
            None,
        );
        let rank_features = strings(&mut builder, &["nativeRank", "bm25(title)"]);
        let match_features = strings(
            &mut builder,
            &["attributeMatch(title)", "fieldMatch(title)"],
        );
        let summary_features = strings(&mut builder, &["summary(title)"]);
        let sort_features = strings(&mut builder, &["sort(title)"]);
        let profile_constant_type = primitive(&mut builder, api::PrimitiveDataTypeKind::Float64);
        let profile_constant_value = float_value(&mut builder, 2.0);
        let profile_constant = literal_constant(
            &mut builder,
            profile_constant_type,
            profile_constant_value,
            "profile_bias",
        );
        let constants = builder.create_vector(&[profile_constant]);
        let profile_model = onnx_model(&mut builder, "profile_model");
        let onnx_models = builder.create_vector(&[profile_model]);
        let rank_properties = string_entries(&mut builder, &[("rank.property", "value")]);
        let strict = api::OptionalBool::new(true);
        let properties = string_properties(&mut builder, &[("profile.property", "value")]);
        api::RankProfileDefinition::create(
            &mut builder,
            &api::RankProfileDefinitionArgs {
                name: Some(name),
                inherits: Some(inherits),
                inputs: Some(inputs),
                functions,
                first_phase: Some(first_phase),
                second_phase: Some(second_phase),
                global_phase: Some(global_phase),
                rank_features: Some(rank_features),
                match_features: Some(match_features),
                summary_features: Some(summary_features),
                sort_features: Some(sort_features),
                constants: Some(constants),
                onnx_models: Some(onnx_models),
                rank_properties: Some(rank_properties),
                strict: Some(&strict),
                properties: Some(properties),
            },
        )
    };
    let alternate_profile = {
        let name = builder.create_string("alternate_counts_profile");
        let first_phase = rank_phase(&mut builder, "first", None, None, None, None, Some(6));
        let second_phase = rank_phase(&mut builder, "second", None, None, Some(11), None, None);
        let global_phase = rank_phase(&mut builder, "global", Some(3), None, None, None, None);
        api::RankProfileDefinition::create(
            &mut builder,
            &api::RankProfileDefinitionArgs {
                name: Some(name),
                inherits: None,
                inputs: None,
                functions: None,
                first_phase: Some(first_phase),
                second_phase: Some(second_phase),
                global_phase: Some(global_phase),
                rank_features: None,
                match_features: None,
                summary_features: None,
                sort_features: None,
                constants: None,
                onnx_models: None,
                rank_properties: None,
                strict: None,
                properties: None,
            },
        )
    };
    let sparse_profile = {
        let name = builder.create_string("sparse_profile");
        api::RankProfileDefinition::create(
            &mut builder,
            &api::RankProfileDefinitionArgs {
                name: Some(name),
                ..Default::default()
            },
        )
    };
    let rank_profiles =
        builder.create_vector(&[complete_profile, alternate_profile, sparse_profile]);

    let schema_constant_type = primitive(&mut builder, api::PrimitiveDataTypeKind::Float64);
    let schema_constant_value = float_value(&mut builder, 1.0);
    let schema_constant = literal_constant(
        &mut builder,
        schema_constant_type,
        schema_constant_value,
        "schema_bias",
    );
    let file_constant_type = tensor(&mut builder, "tensor<float>(x[3])");
    let file_constant = file_constant(
        &mut builder,
        file_constant_type,
        "constants/tensor.json",
        "schema_tensor",
    );
    let constants = builder.create_vector(&[schema_constant, file_constant]);
    let schema_model = onnx_model(&mut builder, "schema_model");
    let onnx_models = builder.create_vector(&[schema_model]);

    let name = builder.create_string(name);
    let default_title = builder.create_string("title");
    let default_title_weight = api::OptionalDouble::new(title_weight);
    let default_title = api::SearchField::create(
        &mut builder,
        &api::SearchFieldArgs {
            field: Some(default_title),
            weight: Some(&default_title_weight),
        },
    );
    let default_tags = builder.create_string("tags");
    let default_tags_weight = api::OptionalDouble::new(tags_weight);
    let default_tags = api::SearchField::create(
        &mut builder,
        &api::SearchFieldArgs {
            field: Some(default_tags),
            weight: Some(&default_tags_weight),
        },
    );
    let default_search_fields = builder.create_vector(&[default_title, default_tags]);
    let default_search_fields = api::SearchFieldSelection::create(
        &mut builder,
        &api::SearchFieldSelectionArgs {
            fields: Some(default_search_fields),
        },
    );
    let properties = string_properties(
        &mut builder,
        &[("schema.property", "value"), ("schema.second", "preserve")],
    );
    let document_type = builder.create_string("complete_document");
    let schema_inherits = strings(&mut builder, &["base_schema", "common_schema"]);
    let document_inherits = strings(&mut builder, &["base_document", "audited_document"]);
    let definition = api::IndexDefinition::create(
        &mut builder,
        &api::IndexDefinitionArgs {
            name: Some(name),
            fields: Some(fields),
            default_search_fields: Some(default_search_fields),
            properties: Some(properties),
            document_type: Some(document_type),
            schema_inherits: Some(schema_inherits),
            document_inherits: Some(document_inherits),
            structs: Some(structs),
            fieldsets: Some(fieldsets),
            schema_fields: Some(schema_fields),
            imported_fields: Some(imported_fields),
            document_summaries: Some(document_summaries),
            rank_profiles: Some(rank_profiles),
            constants: Some(constants),
            onnx_models: Some(onnx_models),
        },
    );
    builder.finish(definition, None);
    builder.finished_data().to_vec()
}

pub(in crate::index::api_tests) fn described_canonical_fixture() -> (
    searchkernel_ir::IndexDefinitionIr,
    searchkernel_ir::IndexDefinitionIr,
) {
    let canonical = canonical_fixture_ir();
    let index = index_from_canonical_ir(canonical.clone());
    let described = index.describe().expect("describe should succeed");
    let described = definition_to_ir(&described).expect("described schema should resolve");
    (canonical, described)
}

pub(in crate::index::api_tests) fn public_described_canonical_fixture(
) -> (searchkernel_ir::IndexDefinitionIr, IndexDefinition) {
    let canonical = canonical_fixture_ir();
    let index = index_from_canonical_ir(canonical.clone());
    let described = index.describe().expect("describe should succeed");
    (canonical, described)
}
