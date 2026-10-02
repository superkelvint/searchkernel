use super::*;
use crate::model::*;
use searchkernel_protocol::wire::v1 as api;
use std::collections::{BTreeMap, BTreeSet};

/// The deliberately small schema intent accepted by the ergonomic Rust
/// facade. It is not a second representation of the portable schema.
#[doc(hidden)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RustFieldKind {
    Text,
    Keyword,
    Int64,
    Float64,
    Bool,
    Position,
    Tensor(String),
}

#[doc(hidden)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RustRankProfileInput {
    pub name: String,
    pub first_phase: Option<String>,
    pub second_phase: Option<String>,
    pub rerank_count: Option<u32>,
    pub match_features: Vec<String>,
    pub query_inputs: Vec<(String, String)>,
}

#[doc(hidden)]
#[derive(Clone, Debug, PartialEq)]
// Both schema ingresses populate this shape before the shared resolver applies
// phase normalization and rank-profile validation.
pub struct UnresolvedRankProfileInput {
    pub name: String,
    pub inherits: Vec<String>,
    pub inputs: Vec<RankProfileInputIr>,
    pub functions: Vec<RankFunctionDefinitionIr>,
    pub first_phase: Option<RankPhaseDefinitionIr>,
    pub second_phase: Option<RankPhaseDefinitionIr>,
    pub global_phase: Option<RankPhaseDefinitionIr>,
    pub rank_features: Vec<String>,
    pub match_features: Vec<String>,
    pub summary_features: Vec<String>,
    pub sort_features: Vec<String>,
    pub constants: Vec<ConstantDefinitionIr>,
    pub onnx_models: Vec<OnnxModelDefinitionIr>,
    pub rank_properties: BTreeMap<String, String>,
    pub strict: Option<bool>,
    pub properties: SchemaPropertiesIr,
}

impl UnresolvedRankProfileInput {
    fn into_ir(self) -> RankProfileDefinitionIr {
        RankProfileDefinitionIr {
            name: self.name,
            inherits: self.inherits,
            inputs: self.inputs,
            functions: self.functions,
            first_phase: self.first_phase,
            second_phase: self.second_phase,
            global_phase: self.global_phase,
            rank_features: self.rank_features,
            match_features: self.match_features,
            summary_features: self.summary_features,
            sort_features: self.sort_features,
            constants: self.constants,
            onnx_models: self.onnx_models,
            rank_properties: self.rank_properties,
            strict: self.strict,
            properties: self.properties,
        }
    }
}

#[doc(hidden)]
#[derive(Clone, Debug, PartialEq)]
pub struct RustAnalyzerInput {
    pub name: String,
    pub version: u32,
    pub options: SchemaPropertiesIr,
}

#[doc(hidden)]
#[derive(Clone, Debug, PartialEq)]
pub struct RustFieldInput {
    pub name: String,
    pub kind: RustFieldKind,
    pub multi: bool,
    pub stored: Option<bool>,
    pub searchable: Option<bool>,
    pub filterable: Option<bool>,
    pub sortable: Option<bool>,
    pub facetable: Option<bool>,
    pub analyzer: Option<RustAnalyzerInput>,
    pub distance_metric: Option<DistanceMetricIr>,
    pub hnsw: Option<HnswIndexConfigIr>,
}

/// Field intent shared by the direct Rust and portable schema ingress paths.
/// The resolver below owns defaults, capability legality, and backend-neutral
/// field semantics; ingress code only decodes each input representation into
/// this shape.
#[doc(hidden)]
#[derive(Clone, Debug, PartialEq)]
pub struct UnresolvedFieldInput {
    pub name: FieldName,
    pub data_type: DataTypeIr,
    pub stored: Option<bool>,
    pub searchable: Option<bool>,
    pub filterable: Option<bool>,
    pub sortable: Option<bool>,
    pub groupable: Option<bool>,
    pub analyzer: Option<AnalyzerIr>,
    pub matching: Option<MatchingConfigIr>,
    pub index: Option<IndexConfigIr>,
    pub attribute: Option<AttributeConfigIr>,
    pub summary: Option<SummaryConfigIr>,
    pub indexing_script: Option<String>,
    pub properties: SchemaPropertiesIr,
    pub struct_fields: Vec<StructFieldConfigIr>,
}

#[doc(hidden)]
#[derive(Clone, Debug, PartialEq)]
pub struct RustSearchFieldInput {
    pub field: String,
    pub weight: Option<f64>,
}

/// Complete unresolved schema intent shared by the direct Rust and portable
/// ingress paths. Both paths decode into this shape before the resolver owns
/// field defaults, duplicate detection, and derived default search fields.
#[doc(hidden)]
#[derive(Clone, Debug, PartialEq)]
pub struct IndexDefinitionInput {
    pub name: Option<String>,
    pub fields: Vec<UnresolvedFieldInput>,
    pub schema_fields: Vec<UnresolvedFieldInput>,
    pub default_search_fields: Option<Vec<RustSearchFieldInput>>,
    pub properties: SchemaPropertiesIr,
    pub document_type: Option<String>,
    pub schema_inherits: Vec<String>,
    pub document_inherits: Vec<String>,
    pub structs: Vec<NamedStructDefinitionIr>,
    pub fieldsets: Vec<FieldSetDefinitionIr>,
    pub imported_fields: Vec<ImportedFieldDefinitionIr>,
    pub document_summaries: Vec<DocumentSummaryDefinitionIr>,
    pub rank_profiles: Vec<UnresolvedRankProfileInput>,
    pub constants: Vec<ConstantDefinitionIr>,
    pub onnx_models: Vec<OnnxModelDefinitionIr>,
}

struct ResolvedIndexFields {
    name: Option<IndexName>,
    fields: Vec<FieldIr>,
    schema_fields: Vec<FieldIr>,
    default_search_fields: Vec<SearchFieldIr>,
}

struct ResolvedIndexDefinition {
    name: Option<IndexName>,
    fields: Vec<FieldIr>,
    schema_fields: Vec<FieldIr>,
    default_search_fields: Vec<SearchFieldIr>,
    properties: SchemaPropertiesIr,
    document_type: Option<String>,
    schema_inherits: Vec<String>,
    document_inherits: Vec<String>,
    structs: Vec<NamedStructDefinitionIr>,
    fieldsets: Vec<FieldSetDefinitionIr>,
    imported_fields: Vec<ImportedFieldDefinitionIr>,
    document_summaries: Vec<DocumentSummaryDefinitionIr>,
    rank_profiles: Vec<UnresolvedRankProfileInput>,
    constants: Vec<ConstantDefinitionIr>,
    onnx_models: Vec<OnnxModelDefinitionIr>,
}

#[doc(hidden)]
#[derive(Clone, Debug, PartialEq)]
pub struct RustIndexDefinitionInput {
    pub name: Option<String>,
    pub fields: Vec<RustFieldInput>,
    pub default_search_fields: Option<Vec<RustSearchFieldInput>>,
}

/// Resolve the Rust facade's concrete schema into the same owned definition
/// used by the portable FlatBuffers path.
#[doc(hidden)]
pub fn resolve_rust_index_definition(input: RustIndexDefinitionInput) -> Result<IndexDefinitionIr> {
    resolve_rust_index_definition_with_profiles(input, Vec::new())
}

#[doc(hidden)]
pub fn resolve_rust_index_definition_with_profiles(
    input: RustIndexDefinitionInput,
    rank_profiles: Vec<RustRankProfileInput>,
) -> Result<IndexDefinitionIr> {
    resolve_rust_index_definition_with_schema(input, Vec::new(), rank_profiles)
}

/// Resolve direct Rust schema intent, including schema-only fields, through
/// the same top-level resolver used by FlatBuffers schema decoding.
#[doc(hidden)]
pub fn resolve_rust_index_definition_with_schema(
    input: RustIndexDefinitionInput,
    schema_fields: Vec<RustFieldInput>,
    rank_profiles: Vec<RustRankProfileInput>,
) -> Result<IndexDefinitionIr> {
    let RustIndexDefinitionInput {
        name,
        fields,
        default_search_fields,
    } = input;
    let fields = rust_fields_to_input(fields, "fields")?;
    let schema_fields = rust_fields_to_input(schema_fields, "schema_fields")?;
    let rank_profiles = rank_profiles
        .into_iter()
        .map(rust_rank_profile_to_input)
        .collect::<Vec<_>>();
    resolve_index_definition(IndexDefinitionInput {
        name,
        fields,
        schema_fields,
        default_search_fields,
        properties: SchemaPropertiesIr::new(),
        document_type: None,
        schema_inherits: Vec::new(),
        document_inherits: Vec::new(),
        structs: Vec::new(),
        fieldsets: Vec::new(),
        imported_fields: Vec::new(),
        document_summaries: Vec::new(),
        rank_profiles,
        constants: Vec::new(),
        onnx_models: Vec::new(),
    })
}

/// Resolve one shared schema input for both direct Rust and portable ingress.
#[doc(hidden)]
pub fn resolve_index_definition(input: IndexDefinitionInput) -> Result<IndexDefinitionIr> {
    let ResolvedIndexFields {
        name,
        fields,
        schema_fields,
        default_search_fields,
    } = resolve_index_definition_fields(
        input.name,
        input.fields,
        input.schema_fields,
        input.default_search_fields,
        "default_search_fields",
    )?;
    assemble_index_definition(ResolvedIndexDefinition {
        name,
        fields,
        schema_fields,
        default_search_fields,
        properties: input.properties,
        document_type: input.document_type,
        schema_inherits: input.schema_inherits,
        document_inherits: input.document_inherits,
        structs: input.structs,
        fieldsets: input.fieldsets,
        imported_fields: input.imported_fields,
        document_summaries: input.document_summaries,
        rank_profiles: input.rank_profiles,
        constants: input.constants,
        onnx_models: input.onnx_models,
    })
}

fn assemble_index_definition(input: ResolvedIndexDefinition) -> Result<IndexDefinitionIr> {
    validate_struct_references(&input.fields, &input.structs)?;
    validate_struct_references(&input.schema_fields, &input.structs)?;
    let rank_profiles = resolve_rank_profiles(input.rank_profiles, "rank_profiles")?;
    let resolved_document_type = input
        .document_type
        .as_deref()
        .or_else(|| input.name.as_ref().map(IndexName::as_str))
        .unwrap_or("searchkernel")
        .to_owned();
    Ok(IndexDefinitionIr {
        name: input.name,
        fields: input.fields,
        default_search_fields: input.default_search_fields,
        properties: input.properties,
        document_type: input.document_type,
        resolved_document_type,
        schema_inherits: input.schema_inherits,
        document_inherits: input.document_inherits,
        structs: input.structs,
        fieldsets: input.fieldsets,
        schema_fields: input.schema_fields,
        imported_fields: input.imported_fields,
        document_summaries: input.document_summaries,
        rank_profiles,
        constants: input.constants,
        onnx_models: input.onnx_models,
    })
}

fn rust_fields_to_input(
    fields: Vec<RustFieldInput>,
    collection: &str,
) -> Result<Vec<UnresolvedFieldInput>> {
    let mut output = Vec::with_capacity(fields.len());
    for (index, field) in fields.into_iter().enumerate() {
        let path = format!("{collection}[{index}]");
        let field_name = FieldName::parse(field.name.clone()).map_err(|_| {
            invalid(
                "invalid_field_name",
                Some(format!("{path}.name")),
                "field name must match [A-Za-z][A-Za-z0-9_]*",
            )
        })?;
        output.push(rust_field_to_input(field, field_name));
    }
    Ok(output)
}

fn rust_rank_profile_to_input(profile: RustRankProfileInput) -> UnresolvedRankProfileInput {
    UnresolvedRankProfileInput {
        name: profile.name,
        inherits: Vec::new(),
        inputs: profile
            .query_inputs
            .into_iter()
            .map(|(name, type_spec)| RankProfileInputIr {
                name,
                data_type: DataTypeIr::Tensor {
                    type_spec: type_spec.clone(),
                },
                default_value: RequestValueIr::Tensor(RequestTensorIr {
                    type_spec,
                    payload: RequestTensorPayloadIr::Dense(Vec::new()),
                }),
            })
            .collect(),
        functions: Vec::new(),
        first_phase: profile.first_phase.map(|expression| RankPhaseDefinitionIr {
            expression: Some(expression),
            rerank_count: None,
            rank_score_drop_limit: None,
            properties: SchemaPropertiesIr::new(),
            total_rerank_count: None,
            keep_rank_count: None,
            total_keep_rank_count: None,
        }),
        second_phase: (profile.second_phase.is_some() || profile.rerank_count.is_some()).then(
            || RankPhaseDefinitionIr {
                expression: profile.second_phase,
                rerank_count: profile.rerank_count,
                rank_score_drop_limit: None,
                properties: SchemaPropertiesIr::new(),
                total_rerank_count: None,
                keep_rank_count: None,
                total_keep_rank_count: None,
            },
        ),
        global_phase: None,
        rank_features: Vec::new(),
        match_features: profile.match_features,
        summary_features: Vec::new(),
        sort_features: Vec::new(),
        constants: Vec::new(),
        onnx_models: Vec::new(),
        rank_properties: BTreeMap::new(),
        strict: None,
        properties: SchemaPropertiesIr::new(),
    }
}

fn resolve_index_definition_fields(
    name: Option<String>,
    fields: Vec<UnresolvedFieldInput>,
    schema_fields: Vec<UnresolvedFieldInput>,
    default_search_fields: Option<Vec<RustSearchFieldInput>>,
    default_search_fields_path: &str,
) -> Result<ResolvedIndexFields> {
    if fields.is_empty() {
        return Err(invalid(
            "invalid_schema",
            Some("fields"),
            "an index must define at least one field",
        ));
    }
    let name = name
        .map(|value| {
            IndexName::parse(value).map_err(|_| {
                invalid(
                    "invalid_index_name",
                    Some("name"),
                    "index name must not be empty",
                )
            })
        })
        .transpose()?;
    let mut names = BTreeSet::new();
    let fields = fields
        .into_iter()
        .enumerate()
        .map(|(index, field)| {
            let path = format!("fields[{index}]");
            if !names.insert(field.name.clone()) {
                return Err(invalid(
                    "duplicate_field",
                    Some(format!("{path}.name")),
                    "field names must be unique",
                ));
            }
            Ok(field)
        })
        .collect::<Result<Vec<_>>>()?;
    let schema_fields = schema_fields
        .into_iter()
        .enumerate()
        .map(|(index, field)| {
            let path = format!("schema_fields[{index}]");
            if !names.insert(field.name.clone()) {
                return Err(invalid(
                    "duplicate_field",
                    Some(format!("{path}.name")),
                    "field names must be unique across document and schema fields",
                ));
            }
            Ok(field)
        })
        .collect::<Result<Vec<_>>>()?;
    let fields = fields
        .into_iter()
        .enumerate()
        .map(|(index, field)| resolve_field_input(field, &format!("fields[{index}]")))
        .collect::<Result<Vec<_>>>()?;
    let schema_fields = schema_fields
        .into_iter()
        .enumerate()
        .map(|(index, field)| resolve_field_input(field, &format!("schema_fields[{index}]")))
        .collect::<Result<Vec<_>>>()?;
    let all_fields = fields
        .iter()
        .chain(schema_fields.iter())
        .collect::<Vec<_>>();
    let default_search_fields = match default_search_fields {
        Some(selection) => {
            resolve_default_search_fields(selection, &all_fields, default_search_fields_path)?
        }
        None => all_fields
            .iter()
            .filter(|field| field.capabilities.searchable)
            .map(|field| SearchFieldIr {
                field: field.name.clone(),
                weight: SearchFieldWeight::new(1.0).expect("constant is valid"),
            })
            .collect(),
    };
    Ok(ResolvedIndexFields {
        name,
        fields,
        schema_fields,
        default_search_fields,
    })
}

fn resolve_default_search_fields(
    selection: Vec<RustSearchFieldInput>,
    fields: &[&FieldIr],
    path_prefix: &str,
) -> Result<Vec<SearchFieldIr>> {
    let mut output = Vec::with_capacity(selection.len());
    let mut seen = BTreeSet::new();
    for (index, input) in selection.into_iter().enumerate() {
        let path = format!("{path_prefix}[{index}]");
        let field_name = FieldName::parse(input.field.clone()).map_err(|_| {
            invalid(
                "invalid_field_name",
                Some(format!("{path}.field")),
                "field name must match [A-Za-z][A-Za-z0-9_]*",
            )
        })?;
        let field = fields
            .iter()
            .find(|field| field.name == field_name)
            .copied()
            .ok_or_else(|| {
                invalid(
                    "unknown_field",
                    Some(format!("{path}.field")),
                    "field is not defined",
                )
            })?;
        if !field.capabilities.searchable {
            return Err(invalid(
                "field_not_searchable",
                Some(format!("{path}.field")),
                "default search fields must be searchable",
            ));
        }
        if !seen.insert(field_name.clone()) {
            return Err(invalid(
                "duplicate_search_field",
                Some(format!("{path}.field")),
                "search fields must be unique",
            ));
        }
        let weight = SearchFieldWeight::new(input.weight.unwrap_or(1.0)).map_err(|_| {
            invalid(
                "invalid_search_weight",
                Some(format!("{path}.weight")),
                "search-field weight must be finite and non-negative",
            )
        })?;
        output.push(SearchFieldIr {
            field: field_name,
            weight,
        });
    }
    Ok(output)
}

fn rust_field_to_input(field: RustFieldInput, name: FieldName) -> UnresolvedFieldInput {
    let RustFieldInput {
        kind,
        multi,
        stored,
        searchable,
        filterable,
        sortable,
        facetable,
        analyzer,
        distance_metric,
        hnsw,
        ..
    } = field;
    let is_keyword = matches!(&kind, RustFieldKind::Keyword);
    let data_type = match kind {
        RustFieldKind::Text | RustFieldKind::Keyword => {
            DataTypeIr::Primitive(PrimitiveDataTypeIr::String)
        }
        RustFieldKind::Int64 => DataTypeIr::Primitive(PrimitiveDataTypeIr::Int64),
        RustFieldKind::Float64 => DataTypeIr::Primitive(PrimitiveDataTypeIr::Float64),
        RustFieldKind::Bool => DataTypeIr::Primitive(PrimitiveDataTypeIr::Bool),
        RustFieldKind::Position => DataTypeIr::Position,
        RustFieldKind::Tensor(type_spec) => DataTypeIr::Tensor { type_spec },
    };
    let data_type = if multi {
        DataTypeIr::Array(Box::new(data_type))
    } else {
        data_type
    };
    let is_string = matches!(
        data_type.scalar_primitive(),
        Some(PrimitiveDataTypeIr::String)
    );
    let matching = is_string.then(|| MatchingConfigIr {
        mode: if is_keyword {
            MatchingModeIr::Exact
        } else {
            MatchingModeIr::Text
        },
        options: SchemaPropertiesIr::new(),
    });
    let analyzer = analyzer.map(|value| AnalyzerIr {
        name: value.name,
        version: value.version,
        options: value.options,
    });
    let index = hnsw.map(|hnsw| IndexConfigIr {
        enabled: Some(true),
        hnsw: Some(hnsw),
        options: SchemaPropertiesIr::new(),
    });
    let attribute = distance_metric.map(|distance_metric| AttributeConfigIr {
        enabled: Some(true),
        fast_search: None,
        fast_access: None,
        distance_metric,
        options: SchemaPropertiesIr::new(),
    });
    UnresolvedFieldInput {
        name,
        data_type,
        stored,
        searchable,
        filterable,
        sortable,
        groupable: facetable,
        analyzer,
        matching,
        index,
        attribute,
        summary: None,
        indexing_script: None,
        properties: SchemaPropertiesIr::new(),
        struct_fields: Vec::new(),
    }
}

fn resolve_field_input(input: UnresolvedFieldInput, path: &str) -> Result<FieldIr> {
    let UnresolvedFieldInput {
        name,
        data_type,
        stored,
        searchable,
        filterable,
        sortable,
        groupable,
        analyzer,
        matching,
        index,
        attribute,
        summary,
        indexing_script,
        properties,
        struct_fields,
    } = input;
    if index.as_ref().is_some_and(|config| config.hnsw.is_some())
        && !matches!(data_type, DataTypeIr::Tensor { .. })
    {
        return Err(invalid(
            "invalid_type_combination",
            Some(format!("{path}.index.hnsw")),
            "HNSW indexing requires a tensor field",
        ));
    }
    if attribute
        .as_ref()
        .is_some_and(|config| config.distance_metric != DistanceMetricIr::Unspecified)
        && !matches!(data_type, DataTypeIr::Tensor { .. })
    {
        return Err(invalid(
            "invalid_type_combination",
            Some(format!("{path}.attribute.distance_metric")),
            "a distance metric requires a tensor field",
        ));
    }
    let attribute_fast_search = attribute.as_ref().and_then(|value| value.fast_search);
    check_bool_agreement(
        stored,
        summary.as_ref().and_then(|value| value.enabled),
        path,
        "stored",
        "summary.enabled",
    )?;
    check_bool_agreement(
        searchable,
        index.as_ref().and_then(|value| value.enabled),
        path,
        "searchable",
        "index.enabled",
    )?;
    if attribute_fast_search
        .zip(filterable)
        .is_some_and(|(fast_search, filterable)| fast_search && !filterable)
    {
        return Err(invalid(
            "invalid_capability",
            Some(format!("{path}.attribute.fast_search")),
            "attribute.fast_search requires a filterable field",
        ));
    }
    let string_type = data_type.scalar_primitive() == Some(PrimitiveDataTypeIr::String);
    let explicit_analyzer = analyzer.is_some();
    let explicit_matching = matching.is_some();
    if indexing_script.is_some()
        && (stored.is_some()
            || searchable.is_some()
            || filterable.is_some()
            || sortable.is_some()
            || groupable.is_some()
            || explicit_analyzer
            || explicit_matching
            || index.is_some()
            || attribute.is_some()
            || summary.is_some())
    {
        return Err(invalid(
            "ambiguous_indexing_script",
            Some(format!("{path}.indexing_script")),
            "an indexing script cannot be combined with typed field behavior that C2 cannot prove equivalent",
        ));
    }
    let keyword = string_type
        && matches!(
            matching.as_ref().map(|value| value.mode),
            Some(MatchingModeIr::Exact | MatchingModeIr::Word)
        );
    if keyword && searchable == Some(true) {
        return Err(invalid(
            "invalid_capability",
            Some(format!("{path}.searchable")),
            "keyword fields cannot opt into full-text search",
        ));
    }
    if matches!(
        data_type,
        DataTypeIr::Primitive(PrimitiveDataTypeIr::String)
    ) && !keyword
        && filterable == Some(true)
    {
        return Err(invalid(
            "invalid_capability",
            Some(format!("{path}.filterable")),
            "text fields cannot opt into filtering",
        ));
    }
    if analyzer.is_some() && (!string_type || keyword) {
        return Err(invalid(
            "invalid_analyzer",
            Some(format!("{path}.analyzer")),
            "only text fields may specify an analyzer",
        ));
    }
    if analyzer.as_ref().is_some_and(|value| value.name.is_empty()) {
        return Err(invalid(
            "invalid_analyzer",
            Some(format!("{path}.analyzer")),
            "analyzer name is required",
        ));
    }
    let analyzer = if string_type && !keyword && indexing_script.is_none() {
        let analyzer = analyzer.unwrap_or(AnalyzerIr {
            name: "standard".to_owned(),
            version: 1,
            options: SchemaPropertiesIr::new(),
        });
        // Analyzer support is a backend executability concern. The portable
        // schema contract must preserve rich analyzer configuration losslessly;
        // the Vespa adapter rejects analyzer configurations it cannot execute.
        Some(analyzer)
    } else {
        analyzer
    };
    let matching = if string_type && indexing_script.is_none() {
        // Preserve whether matching was explicitly represented by the ingress.
        // Rust Text/Keyword structural decoding supplies Text/Exact above;
        // portable canonical IR may intentionally carry no matching config.
        // Inventing one here breaks lossless IR -> FlatBuffers -> IR round trips.
        matching
    } else {
        if matching.is_some() {
            return Err(invalid(
                "invalid_type_combination",
                Some(format!("{path}.matching")),
                "matching is valid only for string fields",
            ));
        }
        None
    };
    let default_searchable = string_type && !keyword && indexing_script.is_none();
    let default_filterable = if indexing_script.is_some() {
        false
    } else if string_type {
        keyword
    } else {
        true
    };
    let default_sortable = if indexing_script.is_some() {
        false
    } else {
        !data_type.is_multi_valued() && (!string_type || keyword)
    };
    let default_groupable = if indexing_script.is_some() {
        false
    } else if string_type {
        keyword
    } else {
        data_type.scalar_primitive() == Some(PrimitiveDataTypeIr::Bool)
    };
    let capabilities = FieldCapabilities {
        stored: stored
            .or_else(|| summary.as_ref().and_then(|value| value.enabled))
            .unwrap_or(true),
        searchable: searchable
            .or_else(|| index.as_ref().and_then(|value| value.enabled))
            .or_else(|| {
                index
                    .as_ref()
                    .and_then(|value| value.hnsw.as_ref())
                    .map(|_| true)
            })
            .unwrap_or(default_searchable),
        filterable: filterable
            .or(attribute_fast_search.filter(|value| *value))
            .unwrap_or(default_filterable),
        sortable: sortable.unwrap_or(default_sortable),
        groupable: groupable.unwrap_or(default_groupable),
    };
    if attribute
        .as_ref()
        .and_then(|value| value.enabled)
        .is_some_and(|enabled| !enabled)
        && (capabilities.filterable
            || capabilities.sortable
            || capabilities.groupable
            || !matches!(
                &data_type,
                DataTypeIr::Primitive(PrimitiveDataTypeIr::String)
            ))
    {
        return Err(invalid(
            "invalid_capability",
            Some(format!("{path}.attribute.enabled")),
            "attribute.enabled=false conflicts with the resolved attribute-backed capabilities",
        ));
    }
    if data_type.is_multi_valued() && capabilities.sortable {
        return Err(invalid(
            "invalid_capability",
            Some(format!("{path}.sortable")),
            "multi-valued fields cannot be sortable",
        ));
    }
    Ok(FieldIr {
        name,
        data_type,
        analyzer,
        capabilities,
        matching,
        index,
        attribute,
        summary,
        indexing_script,
        properties,
        struct_fields,
    })
}

pub fn index_definition_to_ir(definition: api::IndexDefinition<'_>) -> Result<IndexDefinitionIr> {
    let field_views = definition.fields();
    let field_count = field_views.map_or(0, |v| v.len());
    if field_count == 0 {
        return Err(invalid(
            "invalid_schema",
            Some("fields"),
            "an index must define at least one field",
        ));
    }
    let mut fields = Vec::with_capacity(field_count);
    let mut names = BTreeSet::new();
    if let Some(views) = field_views {
        for (index, field) in views.iter().enumerate() {
            let path = format!("fields[{index}]");
            let name = parse_field_name(field.name(), format!("{path}.name"))?;
            if !names.insert(name.clone()) {
                return Err(invalid(
                    "duplicate_field",
                    Some(format!("{path}.name")),
                    "field names must be unique",
                ));
            }
            fields.push(field_to_input_at(field, name, &path)?);
        }
    }
    let name = definition.name().map(ToOwned::to_owned);
    let properties = schema_properties_to_ir(definition.properties(), "properties")?;
    let document_type = optional_nonempty(definition.document_type(), "document_type")?;
    let schema_inherits = strings_to_ir(definition.schema_inherits(), "schema_inherits")?;
    let document_inherits = strings_to_ir(definition.document_inherits(), "document_inherits")?;
    let structs = named_structs_to_ir(definition.structs())?;
    let schema_fields = schema_fields_to_input(definition.schema_fields(), &names)?;
    let default_search_fields = search_fields_to_input(definition.default_search_fields())?;
    let ResolvedIndexFields {
        name,
        fields,
        schema_fields,
        default_search_fields,
    } = resolve_index_definition_fields(
        name,
        fields,
        schema_fields,
        default_search_fields,
        "default_search_fields.fields",
    )?;
    let all_fields = fields
        .iter()
        .chain(schema_fields.iter())
        .collect::<Vec<_>>();
    let document_field_refs = fields.iter().collect::<Vec<_>>();
    let fieldsets = fieldsets_to_ir(definition.fieldsets(), &all_fields)?;
    // Imported fields are fed from document/reference fields.  Schema-only
    // fields are query-visible, but are not document values and cannot be the
    // reference anchor for an imported alias.
    let imported_fields =
        imported_fields_to_ir(definition.imported_fields(), &document_field_refs)?;
    let document_summaries =
        document_summaries_to_ir(definition.document_summaries(), &all_fields)?;
    let constants = constants_to_ir(definition.constants(), "constants")?;
    let onnx_models = onnx_models_to_ir(definition.onnx_models(), "onnx_models")?;
    let rank_profiles = rank_profiles_to_input(definition.rank_profiles())?;
    assemble_index_definition(ResolvedIndexDefinition {
        name,
        fields,
        schema_fields,
        default_search_fields,
        properties,
        document_type,
        schema_inherits,
        document_inherits,
        structs,
        fieldsets,
        imported_fields,
        document_summaries,
        rank_profiles,
        constants,
        onnx_models,
    })
}

pub fn field_to_ir(field: api::FieldDefinition<'_>) -> Result<FieldIr> {
    let name = parse_field_name(field.name(), "name".to_owned())?;
    field_to_ir_at(field, name, "field")
}

pub fn analyzer_to_ir(
    analyzer: Option<api::Analyzer<'_>>,
    kind: FieldKind,
) -> Result<Option<AnalyzerIdentity>> {
    analyzer_to_ir_at(analyzer, kind, "analyzer")
}

impl IndexContext {
    pub(crate) fn from_api(definition: api::IndexDefinition<'_>) -> Result<Self> {
        let definition = index_definition_to_ir(definition)?;
        Ok(Self::from_definition_ir(definition))
    }

    /// Construct a context from a definition that has already gone through
    /// semantic resolution.  The Rust facade uses this path so it does not
    /// encode a definition to FlatBuffers merely to reconstruct the context.
    #[doc(hidden)]
    pub fn from_definition_ir(definition: IndexDefinitionIr) -> Self {
        let document_fields = definition
            .fields
            .iter()
            .enumerate()
            .map(|(i, f)| (f.name.as_str().to_owned(), i))
            .collect();
        let mut query_fields = definition
            .fields
            .iter()
            .enumerate()
            .map(|(i, f)| (f.name.as_str().to_owned(), QueryFieldRef::Document(i)))
            .collect::<BTreeMap<_, _>>();
        query_fields.extend(
            definition
                .schema_fields
                .iter()
                .enumerate()
                .map(|(i, f)| (f.name.as_str().to_owned(), QueryFieldRef::Schema(i))),
        );
        Self {
            definition,
            document_fields,
            query_fields,
        }
    }

    pub fn definition(&self) -> &IndexDefinitionIr {
        &self.definition
    }
    pub fn describe(&self) -> IndexDefinitionIr {
        self.definition.clone()
    }
    pub(crate) fn field(&self, name: &str, path: String) -> Result<&FieldIr> {
        self.document_fields
            .get(name)
            .map(|&i| &self.definition.fields[i])
            .ok_or_else(|| invalid("unknown_field", Some(path), "field is not defined"))
    }

    pub(crate) fn field_for_resolver(&self, name: &str, path: String) -> Result<&FieldIr> {
        self.field(name, path)
    }

    pub(crate) fn query_field(&self, name: &str, path: String) -> Result<&FieldIr> {
        match self.query_fields.get(name).copied() {
            Some(QueryFieldRef::Document(index)) => Ok(&self.definition.fields[index]),
            Some(QueryFieldRef::Schema(index)) => Ok(&self.definition.schema_fields[index]),
            None if self.definition.imported_alias(name) => Err(PortableError::unsupported(
                "imported_field_query_not_supported",
                Some(path),
                "imported field aliases are preserved in the schema but have no local type for query binding",
            )),
            None if name.contains('.') => {
                let root = name.split('.').next().unwrap_or_default();
                let field = self.query_field(root, path.clone())?;
                let mut data_type = field.data_type().clone();
                for part in name.split('.').skip(1) {
                    if let crate::model::DataTypeIr::Array(element) = &data_type {
                        data_type = (**element).clone();
                    }
                    if let crate::model::DataTypeIr::StructReference { name } = &data_type {
                        let Some(structure) = self
                            .definition
                            .structs()
                            .iter()
                            .find(|value| value.name == *name)
                        else {
                            return Err(invalid(
                                "unknown_struct",
                                Some(path),
                                "struct field path references an unknown struct",
                            ));
                        };
                        data_type = crate::model::DataTypeIr::Struct {
                            fields: structure.fields.clone(),
                        };
                    }
                    let crate::model::DataTypeIr::Struct { fields } = &data_type else {
                        return Err(invalid(
                            "unknown_field",
                            Some(path),
                            "struct field path is not defined",
                        ));
                    };
                    let Some(struct_field) = fields.iter().find(|value| value.name == part) else {
                        return Err(invalid(
                            "unknown_field",
                            Some(path),
                            "struct field path is not defined",
                        ));
                    };
                    data_type = struct_field.data_type.clone();
                }
                Ok(field)
            }
            None => Err(invalid("unknown_field", Some(path), "field is not defined")),
        }
    }

    pub(crate) fn query_field_for_resolver(&self, name: &str, path: String) -> Result<&FieldIr> {
        self.query_field(name, path)
    }
}

pub(crate) fn field_to_ir_at(
    definition: api::FieldDefinition<'_>,
    name: FieldName,
    path: &str,
) -> Result<FieldIr> {
    resolve_field_input(field_to_input_at(definition, name, path)?, path)
}

pub(crate) fn field_to_input_at(
    definition: api::FieldDefinition<'_>,
    name: FieldName,
    path: &str,
) -> Result<UnresolvedFieldInput> {
    let data_type = definition.data_type().ok_or_else(|| {
        invalid(
            "invalid_data_type",
            Some(format!("{path}.data_type")),
            "field data type is required",
        )
    })?;
    let data_type = data_type_to_ir(data_type, &format!("{path}.data_type"))?;
    let indexing_script = definition
        .indexing_script()
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    Ok(UnresolvedFieldInput {
        name,
        data_type,
        stored: definition.stored().map(|value| value.value()),
        searchable: definition.searchable().map(|value| value.value()),
        filterable: definition.filterable().map(|value| value.value()),
        sortable: definition.sortable().map(|value| value.value()),
        groupable: definition.facetable().map(|value| value.value()),
        analyzer: analyzer_config_to_ir(definition.analyzer(), &format!("{path}.analyzer"))?,
        matching: matching_to_ir(definition.matching(), &format!("{path}.matching"))?,
        index: index_config_to_ir(definition.index(), &format!("{path}.index"))?,
        attribute: attribute_config_to_ir(definition.attribute(), &format!("{path}.attribute"))?,
        summary: summary_config_to_ir(definition.summary(), &format!("{path}.summary"))?,
        indexing_script,
        properties: schema_properties_to_ir(
            definition.properties(),
            &format!("{path}.properties"),
        )?,
        struct_fields: struct_field_configs_to_ir(
            definition.struct_fields(),
            &format!("{path}.struct_fields"),
        )?,
    })
}

pub(crate) fn analyzer_to_ir_at(
    analyzer: Option<api::Analyzer<'_>>,
    kind: FieldKind,
    path: &str,
) -> Result<Option<AnalyzerIdentity>> {
    if kind != FieldKind::Text {
        if analyzer.is_some() {
            return Err(invalid(
                "invalid_analyzer",
                Some(path),
                "only text fields may specify an analyzer",
            ));
        }
        return Ok(None);
    }
    let Some(analyzer) = analyzer else {
        return Ok(Some(AnalyzerIdentity::StandardV1));
    };
    analyzer_identity(
        &AnalyzerIr {
            name: required_string(analyzer.name(), path, "analyzer name")?,
            version: analyzer.version(),
            options: schema_properties_to_ir(analyzer.options(), &format!("{path}.options"))?,
        },
        path,
    )
    .map(Some)
}

pub(crate) fn analyzer_config_to_ir(
    analyzer: Option<api::Analyzer<'_>>,
    path: &str,
) -> Result<Option<AnalyzerIr>> {
    analyzer
        .map(|analyzer| {
            let name = analyzer
                .name()
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    invalid(
                        "invalid_analyzer",
                        Some(path.to_owned()),
                        "analyzer name is required",
                    )
                })?;
            let analyzer = AnalyzerIr {
                name: name.to_owned(),
                version: analyzer.version(),
                options: schema_properties_to_ir(analyzer.options(), &format!("{path}.options"))?,
            };
            Ok(analyzer)
        })
        .transpose()
}

pub(crate) fn analyzer_identity(analyzer: &AnalyzerIr, path: &str) -> Result<AnalyzerIdentity> {
    if analyzer.name == "standard" && analyzer.version == 1 && analyzer.options.is_empty() {
        Ok(AnalyzerIdentity::StandardV1)
    } else {
        Err(PortableError::unsupported("unsupported_analyzer", Some(path), "analysis execution currently supports only standard analyzer version 1 without options"))
    }
}

pub(crate) fn data_type_to_ir(value: api::DataType<'_>, path: &str) -> Result<DataTypeIr> {
    match value.kind_type() {
        api::DataTypeKind::PrimitiveDataType => {
            let primitive = value.kind_as_primitive_data_type().ok_or_else(|| {
                invalid(
                    "invalid_data_type",
                    Some(path),
                    "primitive data type is missing",
                )
            })?;
            let kind = match primitive.kind() {
                api::PrimitiveDataTypeKind::String => PrimitiveDataTypeIr::String,
                api::PrimitiveDataTypeKind::Bool => PrimitiveDataTypeIr::Bool,
                api::PrimitiveDataTypeKind::Byte => PrimitiveDataTypeIr::Byte,
                api::PrimitiveDataTypeKind::Int32 => PrimitiveDataTypeIr::Int32,
                api::PrimitiveDataTypeKind::Int64 => PrimitiveDataTypeIr::Int64,
                api::PrimitiveDataTypeKind::Float32 => PrimitiveDataTypeIr::Float32,
                api::PrimitiveDataTypeKind::Float64 => PrimitiveDataTypeIr::Float64,
                api::PrimitiveDataTypeKind::Raw => PrimitiveDataTypeIr::Raw,
                _ => {
                    return Err(invalid(
                        "invalid_data_type",
                        Some(path),
                        "primitive data type kind is required",
                    ))
                }
            };
            Ok(DataTypeIr::Primitive(kind))
        }
        api::DataTypeKind::TensorDataType => Ok(DataTypeIr::Tensor {
            type_spec: required_string(
                value.kind_as_tensor_data_type().and_then(|v| v.type_spec()),
                path,
                "tensor type spec",
            )?,
        }),
        api::DataTypeKind::ArrayDataType => Ok(DataTypeIr::Array(Box::new(data_type_to_ir(
            value
                .kind_as_array_data_type()
                .and_then(|v| v.element_type())
                .ok_or_else(|| {
                    invalid(
                        "invalid_data_type",
                        Some(path),
                        "array element type is required",
                    )
                })?,
            path,
        )?))),
        api::DataTypeKind::MapDataType => {
            let map = value.kind_as_map_data_type().ok_or_else(|| {
                invalid("invalid_data_type", Some(path), "map data type is missing")
            })?;
            Ok(DataTypeIr::Map {
                key_type: Box::new(data_type_to_ir(
                    map.key_type().ok_or_else(|| {
                        invalid("invalid_data_type", Some(path), "map key type is required")
                    })?,
                    path,
                )?),
                value_type: Box::new(data_type_to_ir(
                    map.value_type().ok_or_else(|| {
                        invalid(
                            "invalid_data_type",
                            Some(path),
                            "map value type is required",
                        )
                    })?,
                    path,
                )?),
            })
        }
        api::DataTypeKind::WeightedSetDataType => {
            Ok(DataTypeIr::WeightedSet(Box::new(data_type_to_ir(
                value
                    .kind_as_weighted_set_data_type()
                    .and_then(|v| v.element_type())
                    .ok_or_else(|| {
                        invalid(
                            "invalid_data_type",
                            Some(path),
                            "weighted-set element type is required",
                        )
                    })?,
                path,
            )?)))
        }
        api::DataTypeKind::StructDataType => Ok(DataTypeIr::Struct {
            fields: struct_fields_to_ir(
                value.kind_as_struct_data_type().and_then(|v| v.fields()),
                &format!("{path}.fields"),
            )?,
        }),
        api::DataTypeKind::StructReferenceDataType => Ok(DataTypeIr::StructReference {
            name: required_string(
                value
                    .kind_as_struct_reference_data_type()
                    .and_then(|v| v.name()),
                path,
                "struct reference name",
            )?,
        }),
        api::DataTypeKind::PredicateDataType => Ok(DataTypeIr::Predicate),
        api::DataTypeKind::PositionDataType => Ok(DataTypeIr::Position),
        api::DataTypeKind::ReferenceDataType => Ok(DataTypeIr::Reference {
            document_type: required_string(
                value
                    .kind_as_reference_data_type()
                    .and_then(|v| v.document_type()),
                path,
                "reference document type",
            )?,
        }),
        _ => Err(invalid(
            "invalid_data_type",
            Some(path),
            "data type must select a kind",
        )),
    }
}

pub(crate) fn struct_fields_to_ir(
    values: Option<
        flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<api::StructFieldDefinition<'_>>>,
    >,
    path: &str,
) -> Result<Vec<StructFieldIr>> {
    let mut output = Vec::new();
    let mut names = BTreeSet::new();
    if let Some(values) = values {
        for (index, value) in values.iter().enumerate() {
            let item_path = format!("{path}[{index}]");
            let name = required_string(
                Some(value.name()),
                &format!("{item_path}.name"),
                "struct field name",
            )?;
            if !names.insert(name.clone()) {
                return Err(invalid(
                    "duplicate_field",
                    Some(format!("{item_path}.name")),
                    "struct field names must be unique",
                ));
            }
            output.push(StructFieldIr {
                name,
                data_type: data_type_to_ir(
                    value.data_type().ok_or_else(|| {
                        invalid(
                            "invalid_data_type",
                            Some(format!("{item_path}.data_type")),
                            "struct field data type is required",
                        )
                    })?,
                    &format!("{item_path}.data_type"),
                )?,
            });
        }
    }
    Ok(output)
}

pub(crate) fn matching_to_ir(
    value: Option<api::MatchingConfig<'_>>,
    path: &str,
) -> Result<Option<MatchingConfigIr>> {
    value
        .map(|value| {
            let mode = match value.mode() {
                api::MatchingMode::Unspecified => MatchingModeIr::Unspecified,
                api::MatchingMode::Text => MatchingModeIr::Text,
                api::MatchingMode::Word => MatchingModeIr::Word,
                api::MatchingMode::Exact => MatchingModeIr::Exact,
                api::MatchingMode::Gram => MatchingModeIr::Gram,
                _ => {
                    return Err(invalid(
                        "invalid_matching",
                        Some(path),
                        "unknown matching mode",
                    ))
                }
            };
            Ok(MatchingConfigIr {
                mode,
                options: schema_properties_to_ir(value.options(), &format!("{path}.options"))?,
            })
        })
        .transpose()
}

pub(crate) fn index_config_to_ir(
    value: Option<api::IndexConfig<'_>>,
    path: &str,
) -> Result<Option<IndexConfigIr>> {
    value
        .map(|value| {
            Ok(IndexConfigIr {
                enabled: value.enabled().map(|value| value.value()),
                hnsw: value
                    .hnsw()
                    .map(|hnsw| {
                        Ok(HnswIndexConfigIr {
                            max_links_per_node: hnsw
                                .max_links_per_node()
                                .map(|value| value.value()),
                            neighbors_to_explore_at_insert: hnsw
                                .neighbors_to_explore_at_insert()
                                .map(|value| value.value()),
                            options: schema_properties_to_ir(
                                hnsw.options(),
                                &format!("{path}.hnsw.options"),
                            )?,
                        })
                    })
                    .transpose()?,
                options: schema_properties_to_ir(value.options(), &format!("{path}.options"))?,
            })
        })
        .transpose()
}

pub(crate) fn attribute_config_to_ir(
    value: Option<api::AttributeConfig<'_>>,
    path: &str,
) -> Result<Option<AttributeConfigIr>> {
    value
        .map(|value| {
            let distance_metric = match value.distance_metric() {
                api::DistanceMetric::Unspecified => DistanceMetricIr::Unspecified,
                api::DistanceMetric::Euclidean => DistanceMetricIr::Euclidean,
                api::DistanceMetric::Angular => DistanceMetricIr::Angular,
                api::DistanceMetric::DotProduct => DistanceMetricIr::DotProduct,
                api::DistanceMetric::PrenormalizedAngular => DistanceMetricIr::PrenormalizedAngular,
                api::DistanceMetric::GeoDegrees => DistanceMetricIr::GeoDegrees,
                api::DistanceMetric::Hamming => DistanceMetricIr::Hamming,
                _ => {
                    return Err(invalid(
                        "invalid_distance_metric",
                        Some(path),
                        "unknown distance metric",
                    ))
                }
            };
            Ok(AttributeConfigIr {
                enabled: value.enabled().map(|value| value.value()),
                fast_search: value.fast_search().map(|value| value.value()),
                fast_access: value.fast_access().map(|value| value.value()),
                distance_metric,
                options: schema_properties_to_ir(value.options(), &format!("{path}.options"))?,
            })
        })
        .transpose()
}

pub(crate) fn summary_config_to_ir(
    value: Option<api::SummaryConfig<'_>>,
    path: &str,
) -> Result<Option<SummaryConfigIr>> {
    value
        .map(|value| {
            Ok(SummaryConfigIr {
                enabled: value.enabled().map(|value| value.value()),
                options: schema_properties_to_ir(value.options(), &format!("{path}.options"))?,
            })
        })
        .transpose()
}

pub(crate) fn schema_properties_to_ir(
    values: Option<flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<api::ValueEntry<'_>>>>,
    path: &str,
) -> Result<SchemaPropertiesIr> {
    request_values_to_map(values, path)
}

pub(crate) fn required_string(
    value: Option<&str>,
    path: &str,
    description: &str,
) -> Result<String> {
    value
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| {
            invalid(
                "invalid_schema",
                Some(path),
                format!("{description} is required"),
            )
        })
}

pub(crate) fn optional_nonempty(value: Option<&str>, path: &str) -> Result<Option<String>> {
    value
        .map(|value| required_string(Some(value), path, "schema identifier"))
        .transpose()
}

pub(crate) fn strings_to_ir(
    values: Option<flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<&str>>>,
    path: &str,
) -> Result<Vec<String>> {
    values
        .map(|values| {
            values
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    required_string(Some(value), &format!("{path}[{index}]"), "identifier")
                })
                .collect()
        })
        .unwrap_or_else(|| Ok(Vec::new()))
}

pub(crate) fn check_bool_agreement(
    left: Option<bool>,
    right: Option<bool>,
    path: &str,
    left_name: &str,
    right_name: &str,
) -> Result<()> {
    if left.zip(right).is_some_and(|(left, right)| left != right) {
        return Err(invalid(
            "invalid_capability",
            Some(format!("{path}.{left_name}")),
            format!("{left_name} conflicts with {right_name}"),
        ));
    }
    Ok(())
}

pub(crate) fn struct_field_configs_to_ir(
    values: Option<
        flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<api::StructFieldConfig<'_>>>,
    >,
    path: &str,
) -> Result<Vec<StructFieldConfigIr>> {
    let mut output = Vec::new();
    let mut names = BTreeSet::new();
    if let Some(values) = values {
        for (index, value) in values.iter().enumerate() {
            let item = format!("{path}[{index}]");
            let field_path = required_string(
                Some(value.path()),
                &format!("{item}.path"),
                "struct field path",
            )?;
            if !names.insert(field_path.clone()) {
                return Err(invalid(
                    "duplicate_struct_field_config",
                    Some(format!("{item}.path")),
                    "struct field paths must be unique",
                ));
            }
            output.push(StructFieldConfigIr {
                path: field_path,
                indexing_script: value
                    .indexing_script()
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned),
                matching: matching_to_ir(value.matching(), &format!("{item}.matching"))?,
                index: index_config_to_ir(value.index(), &format!("{item}.index"))?,
                attribute: attribute_config_to_ir(value.attribute(), &format!("{item}.attribute"))?,
                summary: summary_config_to_ir(value.summary(), &format!("{item}.summary"))?,
                rank: value
                    .rank()
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned),
                query_commands: strings_to_ir(
                    value.query_commands(),
                    &format!("{item}.query_commands"),
                )?,
                properties: schema_properties_to_ir(
                    value.properties(),
                    &format!("{item}.properties"),
                )?,
            });
        }
    }
    Ok(output)
}

pub(crate) fn named_structs_to_ir(
    values: Option<
        flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<api::NamedStructDefinition<'_>>>,
    >,
) -> Result<Vec<NamedStructDefinitionIr>> {
    let mut output = Vec::new();
    let mut names = BTreeSet::new();
    if let Some(values) = values {
        for (index, value) in values.iter().enumerate() {
            let path = format!("structs[{index}]");
            let name = required_string(Some(value.name()), &format!("{path}.name"), "struct name")?;
            if !names.insert(name.clone()) {
                return Err(invalid(
                    "duplicate_struct",
                    Some(format!("{path}.name")),
                    "struct names must be unique",
                ));
            }
            output.push(NamedStructDefinitionIr {
                name,
                fields: struct_fields_to_ir(value.fields(), &format!("{path}.fields"))?,
                properties: schema_properties_to_ir(
                    value.properties(),
                    &format!("{path}.properties"),
                )?,
            });
        }
    }
    Ok(output)
}

pub(crate) fn validate_struct_references(
    fields: &[FieldIr],
    structs: &[NamedStructDefinitionIr],
) -> Result<()> {
    let names = structs
        .iter()
        .map(|value| value.name.as_str())
        .collect::<BTreeSet<_>>();
    fn visit(value: &DataTypeIr, names: &BTreeSet<&str>) -> bool {
        match value {
            DataTypeIr::StructReference { name } => names.contains(name.as_str()),
            DataTypeIr::Array(value) | DataTypeIr::WeightedSet(value) => visit(value, names),
            DataTypeIr::Map {
                key_type,
                value_type,
            } => visit(key_type, names) && visit(value_type, names),
            DataTypeIr::Struct { fields } => {
                fields.iter().all(|field| visit(&field.data_type, names))
            }
            _ => true,
        }
    }
    for field in fields {
        if !visit(&field.data_type, &names) {
            return Err(invalid(
                "unknown_struct",
                Some("fields"),
                "a struct reference is not declared by this schema",
            ));
        }
    }
    for value in structs {
        for field in &value.fields {
            if !visit(&field.data_type, &names) {
                return Err(invalid(
                    "unknown_struct",
                    Some("structs"),
                    "a struct reference is not declared by this schema",
                ));
            }
        }
    }
    Ok(())
}

pub(crate) fn schema_fields_to_input(
    values: Option<
        flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<api::SearchOnlyFieldDefinition<'_>>>,
    >,
    names: &BTreeSet<FieldName>,
) -> Result<Vec<UnresolvedFieldInput>> {
    let mut output = Vec::new();
    let mut all_names = names.clone();
    if let Some(values) = values {
        for (index, value) in values.iter().enumerate() {
            let path = format!("schema_fields[{index}].field");
            let field = value.field().ok_or_else(|| {
                invalid(
                    "invalid_schema",
                    Some(path.clone()),
                    "schema field is required",
                )
            })?;
            let name = parse_field_name(field.name(), format!("{path}.name"))?;
            if !all_names.insert(name.clone()) {
                return Err(invalid(
                    "duplicate_field",
                    Some(format!("{path}.name")),
                    "field names must be unique across document and schema fields",
                ));
            }
            output.push(field_to_input_at(field, name, &path)?);
        }
    }
    Ok(output)
}

fn search_fields_to_input(
    selection: Option<api::SearchFieldSelection<'_>>,
) -> Result<Option<Vec<RustSearchFieldInput>>> {
    let Some(selection) = selection else {
        return Ok(None);
    };
    let fields = selection
        .fields()
        .map(|fields| {
            fields
                .iter()
                .map(|field| RustSearchFieldInput {
                    field: field.field().unwrap_or_default().to_owned(),
                    weight: field.weight().map(|value| value.value()),
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(Some(fields))
}

pub(crate) fn fieldsets_to_ir(
    values: Option<
        flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<api::FieldSetDefinition<'_>>>,
    >,
    fields: &[&FieldIr],
) -> Result<Vec<FieldSetDefinitionIr>> {
    let known = fields
        .iter()
        .map(|field| field.name.as_str())
        .collect::<BTreeSet<_>>();
    let mut output = Vec::new();
    let mut names = BTreeSet::new();
    if let Some(values) = values {
        for (index, value) in values.iter().enumerate() {
            let path = format!("fieldsets[{index}]");
            let name =
                required_string(Some(value.name()), &format!("{path}.name"), "fieldset name")?;
            if !names.insert(name.clone()) {
                return Err(invalid(
                    "duplicate_fieldset",
                    Some(format!("{path}.name")),
                    "fieldset names must be unique",
                ));
            }
            let fields = strings_to_ir(value.fields(), &format!("{path}.fields"))?
                .into_iter()
                .enumerate()
                .map(|(field_index, field)| {
                    if !known.contains(field.as_str()) {
                        return Err(invalid(
                            "unknown_field",
                            Some(format!("{path}.fields[{field_index}]")),
                            "fieldset field is not defined",
                        ));
                    }
                    parse_field_name(&field, format!("{path}.fields[{field_index}]"))
                })
                .collect::<Result<Vec<_>>>()?;
            output.push(FieldSetDefinitionIr {
                name,
                fields,
                matching: matching_to_ir(value.matching(), &format!("{path}.matching"))?,
                query_commands: strings_to_ir(
                    value.query_commands(),
                    &format!("{path}.query_commands"),
                )?,
                properties: schema_properties_to_ir(
                    value.properties(),
                    &format!("{path}.properties"),
                )?,
            });
        }
    }
    Ok(output)
}

pub(crate) fn imported_fields_to_ir(
    values: Option<
        flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<api::ImportedFieldDefinition<'_>>>,
    >,
    fields: &[&FieldIr],
) -> Result<Vec<ImportedFieldDefinitionIr>> {
    let known = fields
        .iter()
        .map(|field| field.name.as_str())
        .collect::<BTreeSet<_>>();
    let mut output = Vec::new();
    let mut aliases = BTreeSet::new();
    if let Some(values) = values {
        for (index, value) in values.iter().enumerate() {
            let path = format!("imported_fields[{index}]");
            let reference_name = required_string(
                value.reference_field(),
                &format!("{path}.reference_field"),
                "reference field",
            )?;
            let reference = parse_field_name(&reference_name, format!("{path}.reference_field"))?;
            let reference_definition = fields
                .iter()
                .find(|field| field.name == reference)
                .ok_or_else(|| {
                    invalid(
                        "unknown_field",
                        Some(format!("{path}.reference_field")),
                        "imported field reference is not defined",
                    )
                })?;
            if !matches!(reference_definition.data_type, DataTypeIr::Reference { .. }) {
                return Err(invalid(
                    "invalid_imported_field",
                    Some(format!("{path}.reference_field")),
                    "an imported field must reference a document-reference field",
                ));
            }
            let target_field = required_string(
                value.target_field(),
                &format!("{path}.target_field"),
                "imported target field",
            )?;
            let alias = parse_field_name(value.alias(), format!("{path}.alias"))?;
            if !aliases.insert(alias.clone()) || known.contains(alias.as_str()) {
                return Err(invalid(
                    "duplicate_imported_field",
                    Some(format!("{path}.alias")),
                    "imported aliases must not collide with schema fields",
                ));
            }
            output.push(ImportedFieldDefinitionIr {
                reference_field: reference,
                target_field,
                alias,
                properties: schema_properties_to_ir(
                    value.properties(),
                    &format!("{path}.properties"),
                )?,
            });
        }
    }
    Ok(output)
}

pub(crate) fn document_summaries_to_ir(
    values: Option<
        flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<api::DocumentSummaryDefinition<'_>>>,
    >,
    fields: &[&FieldIr],
) -> Result<Vec<DocumentSummaryDefinitionIr>> {
    let known = fields
        .iter()
        .map(|field| field.name.as_str())
        .collect::<BTreeSet<_>>();
    let mut output = Vec::new();
    let mut names = BTreeSet::new();
    if let Some(values) = values {
        for (index, value) in values.iter().enumerate() {
            let path = format!("document_summaries[{index}]");
            let name = required_string(
                Some(value.name()),
                &format!("{path}.name"),
                "document summary name",
            )?;
            if !names.insert(name.clone()) {
                return Err(invalid(
                    "duplicate_document_summary",
                    Some(format!("{path}.name")),
                    "document summary names must be unique",
                ));
            }
            let mut summary_fields = Vec::new();
            let mut field_names = BTreeSet::new();
            if let Some(items) = value.fields() {
                for (field_index, item) in items.iter().enumerate() {
                    let item_path = format!("{path}.fields[{field_index}]");
                    let field_name = required_string(
                        Some(item.name()),
                        &format!("{item_path}.name"),
                        "summary field name",
                    )?;
                    if !field_names.insert(field_name.clone()) {
                        return Err(invalid(
                            "duplicate_summary_field",
                            Some(format!("{item_path}.name")),
                            "summary field names must be unique",
                        ));
                    }
                    let source = required_string(
                        item.source(),
                        &format!("{item_path}.source"),
                        "summary source",
                    )?;
                    if !known.contains(source.as_str()) {
                        return Err(invalid(
                            "unknown_field",
                            Some(format!("{item_path}.source")),
                            "summary source field is not defined",
                        ));
                    }
                    summary_fields.push(DocumentSummaryFieldDefinitionIr {
                        name: field_name,
                        source,
                        data_type: data_type_to_ir(
                            item.data_type().ok_or_else(|| {
                                invalid(
                                    "invalid_data_type",
                                    Some(format!("{item_path}.data_type")),
                                    "summary field data type is required",
                                )
                            })?,
                            &format!("{item_path}.data_type"),
                        )?,
                        struct_fields: strings_to_ir(
                            item.struct_fields(),
                            &format!("{item_path}.struct_fields"),
                        )?,
                        properties: schema_properties_to_ir(
                            item.properties(),
                            &format!("{item_path}.properties"),
                        )?,
                    });
                }
            }
            output.push(DocumentSummaryDefinitionIr {
                name,
                inherits: strings_to_ir(value.inherits(), &format!("{path}.inherits"))?,
                fields: summary_fields,
                from_disk: value.from_disk().map(|value| value.value()),
                omit_summary_features: value.omit_summary_features().map(|value| value.value()),
                properties: schema_properties_to_ir(
                    value.properties(),
                    &format!("{path}.properties"),
                )?,
            });
        }
    }
    Ok(output)
}

pub(crate) fn constants_to_ir(
    values: Option<
        flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<api::ConstantDefinition<'_>>>,
    >,
    path: &str,
) -> Result<Vec<ConstantDefinitionIr>> {
    let mut output = Vec::new();
    let mut names = BTreeSet::new();
    if let Some(values) = values {
        for (index, value) in values.iter().enumerate() {
            let item = format!("{path}[{index}]");
            let name =
                required_string(Some(value.name()), &format!("{item}.name"), "constant name")?;
            if !names.insert(name.clone()) {
                return Err(invalid(
                    "duplicate_constant",
                    Some(format!("{item}.name")),
                    "constant names must be unique",
                ));
            }
            let source = match value.source_type() {
                api::ConstantSourceKind::LiteralConstantSource => {
                    ConstantSourceIr::Literal(RequestValueIr::from_api(
                        value
                            .source_as_literal_constant_source()
                            .and_then(|value| value.value())
                            .ok_or_else(|| {
                                invalid(
                                    "invalid_constant",
                                    Some(format!("{item}.source")),
                                    "literal constant value is required",
                                )
                            })?,
                        format!("{item}.source.value"),
                    )?)
                }
                api::ConstantSourceKind::FileConstantSource => ConstantSourceIr::File {
                    path: required_string(
                        value
                            .source_as_file_constant_source()
                            .and_then(|value| value.path()),
                        &format!("{item}.source.path"),
                        "constant file path",
                    )?,
                },
                _ => {
                    return Err(invalid(
                        "invalid_constant",
                        Some(format!("{item}.source")),
                        "constant source is required",
                    ))
                }
            };
            output.push(ConstantDefinitionIr {
                name,
                data_type: data_type_to_ir(
                    value.data_type().ok_or_else(|| {
                        invalid(
                            "invalid_data_type",
                            Some(format!("{item}.data_type")),
                            "constant data type is required",
                        )
                    })?,
                    &format!("{item}.data_type"),
                )?,
                source,
                properties: schema_properties_to_ir(
                    value.properties(),
                    &format!("{item}.properties"),
                )?,
            });
        }
    }
    Ok(output)
}

pub(crate) fn string_map_to_ir(
    values: Option<flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<api::StringEntry<'_>>>>,
    path: &str,
) -> Result<BTreeMap<String, String>> {
    let mut output = BTreeMap::new();
    if let Some(values) = values {
        for (index, value) in values.iter().enumerate() {
            let key = required_string(
                Some(value.key()),
                &format!("{path}[{index}].key"),
                "map key",
            )?;
            let entry = required_string(
                value.value(),
                &format!("{path}[{index}].value"),
                "map value",
            )?;
            if output.insert(key.clone(), entry).is_some() {
                return Err(invalid(
                    "duplicate_property",
                    Some(format!("{path}[{index}].key")),
                    format!("duplicate key `{key}`"),
                ));
            }
        }
    }
    Ok(output)
}

pub(crate) fn onnx_models_to_ir(
    values: Option<
        flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<api::OnnxModelDefinition<'_>>>,
    >,
    path: &str,
) -> Result<Vec<OnnxModelDefinitionIr>> {
    let mut output = Vec::new();
    let mut names = BTreeSet::new();
    if let Some(values) = values {
        for (index, value) in values.iter().enumerate() {
            let item = format!("{path}[{index}]");
            let name = required_string(
                Some(value.name()),
                &format!("{item}.name"),
                "ONNX model name",
            )?;
            if !names.insert(name.clone()) {
                return Err(invalid(
                    "duplicate_onnx_model",
                    Some(format!("{item}.name")),
                    "ONNX model names must be unique",
                ));
            }
            output.push(OnnxModelDefinitionIr {
                name,
                file: required_string(value.file(), &format!("{item}.file"), "ONNX model file")?,
                inputs: string_map_to_ir(value.inputs(), &format!("{item}.inputs"))?,
                outputs: string_map_to_ir(value.outputs(), &format!("{item}.outputs"))?,
                gpu_device: value.gpu_device().map(|value| value.value()),
                intraop_threads: value.intraop_threads().map(|value| value.value()),
                interop_threads: value.interop_threads().map(|value| value.value()),
                properties: schema_properties_to_ir(
                    value.properties(),
                    &format!("{item}.properties"),
                )?,
            });
        }
    }
    Ok(output)
}

pub(crate) fn rank_phase_to_ir(
    value: Option<api::RankPhaseDefinition<'_>>,
    path: &str,
) -> Result<Option<RankPhaseDefinitionIr>> {
    value
        .map(|value| {
            Ok(RankPhaseDefinitionIr {
                expression: value.expression().map(ToOwned::to_owned),
                rerank_count: value.rerank_count().map(|value| value.value()),
                rank_score_drop_limit: value.rank_score_drop_limit().map(|value| value.value()),
                properties: schema_properties_to_ir(
                    value.properties(),
                    &format!("{path}.properties"),
                )?,
                total_rerank_count: value.total_rerank_count().map(|value| value.value()),
                keep_rank_count: value.keep_rank_count().map(|value| value.value()),
                total_keep_rank_count: value.total_keep_rank_count().map(|value| value.value()),
            })
        })
        .transpose()
}

#[derive(Clone, Copy)]
enum RankPhaseKind {
    First,
    Second,
    Global,
}

fn validate_rank_phase(
    value: Option<&RankPhaseDefinitionIr>,
    kind: RankPhaseKind,
    path: &str,
) -> Result<()> {
    let Some(value) = value else {
        return Ok(());
    };
    if value
        .rank_score_drop_limit
        .is_some_and(|limit| !limit.is_finite())
    {
        return Err(invalid(
            "invalid_rank_phase",
            Some(format!("{path}.rank_score_drop_limit")),
            "rank-score-drop-limit must be finite",
        ));
    }
    if value.rerank_count.is_some() && value.total_rerank_count.is_some() {
        return Err(invalid(
            "invalid_rank_phase",
            Some(path),
            "rerank_count and total_rerank_count are mutually exclusive",
        ));
    }
    if value.keep_rank_count.is_some() && value.total_keep_rank_count.is_some() {
        return Err(invalid(
            "invalid_rank_phase",
            Some(path),
            "keep_rank_count and total_keep_rank_count are mutually exclusive",
        ));
    }
    let invalid_for_phase = match kind {
        RankPhaseKind::First => value.rerank_count.is_some() || value.total_rerank_count.is_some(),
        RankPhaseKind::Second => {
            value.keep_rank_count.is_some() || value.total_keep_rank_count.is_some()
        }
        RankPhaseKind::Global => {
            value.keep_rank_count.is_some()
                || value.total_keep_rank_count.is_some()
                || value.total_rerank_count.is_some()
        }
    };
    if invalid_for_phase {
        return Err(invalid(
            "invalid_rank_phase",
            Some(path),
            "rank-phase controls are not applicable to this phase",
        ));
    }
    Ok(())
}

fn validate_rank_profiles(profiles: &[RankProfileDefinitionIr], path: &str) -> Result<()> {
    let mut profile_names = BTreeSet::new();
    for (index, profile) in profiles.iter().enumerate() {
        let profile_path = format!("{path}[{index}]");
        if !profile_names.insert(profile.name.clone()) {
            return Err(invalid(
                "duplicate_rank_profile",
                Some(format!("{profile_path}.name")),
                "rank profile names must be unique",
            ));
        }
        if profile.name.is_empty() {
            return Err(invalid(
                "invalid_schema",
                Some(format!("{profile_path}.name")),
                "rank profile name is required",
            ));
        }

        let mut input_names = BTreeSet::new();
        for (input_index, input) in profile.inputs.iter().enumerate() {
            let input_path = format!("{profile_path}.inputs[{input_index}].name");
            if input.name.is_empty() {
                return Err(invalid(
                    "invalid_schema",
                    Some(input_path),
                    "rank input name is required",
                ));
            }
            if !input_names.insert(input.name.clone()) {
                return Err(invalid(
                    "duplicate_rank_input",
                    Some(input_path),
                    "rank input names must be unique",
                ));
            }
        }

        let mut function_names = BTreeSet::new();
        for (function_index, function) in profile.functions.iter().enumerate() {
            if !function_names.insert(function.name.clone()) {
                return Err(invalid(
                    "duplicate_rank_function",
                    Some(format!("{profile_path}.functions[{function_index}].name")),
                    "rank function names must be unique",
                ));
            }
        }

        validate_rank_phase(
            profile.first_phase.as_ref(),
            RankPhaseKind::First,
            &format!("{profile_path}.first_phase"),
        )?;
        validate_rank_phase(
            profile.second_phase.as_ref(),
            RankPhaseKind::Second,
            &format!("{profile_path}.second_phase"),
        )?;
        validate_rank_phase(
            profile.global_phase.as_ref(),
            RankPhaseKind::Global,
            &format!("{profile_path}.global_phase"),
        )?;
    }
    Ok(())
}

fn resolve_rank_profiles(
    profiles: Vec<UnresolvedRankProfileInput>,
    path: &str,
) -> Result<Vec<RankProfileDefinitionIr>> {
    let profiles = profiles
        .into_iter()
        .map(|mut profile| {
            for phase in [
                profile.first_phase.as_mut(),
                profile.second_phase.as_mut(),
                profile.global_phase.as_mut(),
            ]
            .into_iter()
            .flatten()
            {
                phase.expression = normalized_optional_string(phase.expression.as_deref());
            }
            profile.into_ir()
        })
        .collect::<Vec<_>>();
    validate_rank_profiles(&profiles, path)?;
    Ok(profiles)
}

pub(crate) fn rank_profiles_to_input(
    values: Option<
        flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<api::RankProfileDefinition<'_>>>,
    >,
) -> Result<Vec<UnresolvedRankProfileInput>> {
    let mut output = Vec::new();
    if let Some(values) = values {
        for (index, value) in values.iter().enumerate() {
            let path = format!("rank_profiles[{index}]");
            let name = required_string(
                Some(value.name()),
                &format!("{path}.name"),
                "rank profile name",
            )?;
            let mut inputs = Vec::new();
            if let Some(items) = value.inputs() {
                for (input_index, input) in items.iter().enumerate() {
                    let item = format!("{path}.inputs[{input_index}]");
                    let input_name = required_string(
                        Some(input.name()),
                        &format!("{item}.name"),
                        "rank input name",
                    )?;
                    inputs.push(RankProfileInputIr {
                        name: input_name,
                        data_type: data_type_to_ir(
                            input.data_type().ok_or_else(|| {
                                invalid(
                                    "invalid_data_type",
                                    Some(format!("{item}.data_type")),
                                    "rank input data type is required",
                                )
                            })?,
                            &format!("{item}.data_type"),
                        )?,
                        default_value: RequestValueIr::from_api(
                            input.default_value().ok_or_else(|| {
                                invalid(
                                    "missing_value",
                                    Some(format!("{item}.default_value")),
                                    "rank input default value is required",
                                )
                            })?,
                            format!("{item}.default_value"),
                        )?,
                    });
                }
            }
            let mut functions = Vec::new();
            if let Some(items) = value.functions() {
                for (function_index, function) in items.iter().enumerate() {
                    let item = format!("{path}.functions[{function_index}]");
                    let function_name = required_string(
                        Some(function.name()),
                        &format!("{item}.name"),
                        "rank function name",
                    )?;
                    functions.push(RankFunctionDefinitionIr {
                        name: function_name,
                        arguments: strings_to_ir(
                            function.arguments(),
                            &format!("{item}.arguments"),
                        )?,
                        expression: required_string(
                            function.expression(),
                            &format!("{item}.expression"),
                            "rank function expression",
                        )?,
                        inline: function.inline().map(|value| value.value()),
                    });
                }
            }
            let first_phase =
                rank_phase_to_ir(value.first_phase(), &format!("{path}.first_phase"))?;
            let second_phase =
                rank_phase_to_ir(value.second_phase(), &format!("{path}.second_phase"))?;
            let global_phase =
                rank_phase_to_ir(value.global_phase(), &format!("{path}.global_phase"))?;
            output.push(UnresolvedRankProfileInput {
                name,
                inherits: strings_to_ir(value.inherits(), &format!("{path}.inherits"))?,
                inputs,
                functions,
                first_phase,
                second_phase,
                global_phase,
                rank_features: strings_to_ir(
                    value.rank_features(),
                    &format!("{path}.rank_features"),
                )?,
                match_features: strings_to_ir(
                    value.match_features(),
                    &format!("{path}.match_features"),
                )?,
                summary_features: strings_to_ir(
                    value.summary_features(),
                    &format!("{path}.summary_features"),
                )?,
                sort_features: strings_to_ir(
                    value.sort_features(),
                    &format!("{path}.sort_features"),
                )?,
                constants: constants_to_ir(value.constants(), &format!("{path}.constants"))?,
                onnx_models: onnx_models_to_ir(
                    value.onnx_models(),
                    &format!("{path}.onnx_models"),
                )?,
                rank_properties: string_map_to_ir(
                    value.rank_properties(),
                    &format!("{path}.rank_properties"),
                )?,
                strict: value.strict().map(|value| value.value()),
                properties: schema_properties_to_ir(
                    value.properties(),
                    &format!("{path}.properties"),
                )?,
            });
        }
    }
    Ok(output)
}
pub(crate) fn parse_field_name(v: &str, path: String) -> Result<FieldName> {
    FieldName::parse(v.to_owned()).map_err(|_| {
        invalid(
            "invalid_field_name",
            Some(path),
            "field name must match [A-Za-z][A-Za-z0-9_]*",
        )
    })
}
