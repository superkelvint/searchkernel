use std::collections::BTreeMap;
use std::fmt;

use crate::error::{Error, Result};

/// Portable schema values owned by the product-facing Rust API.
#[derive(Clone, Debug, PartialEq)]
pub enum SchemaValue {
    Null,
    Bool(bool),
    Int64(i64),
    UInt64(u64),
    Float64(f64),
    NonFiniteFloat(SchemaNonFiniteFloat),
    String(String),
    Bytes(Vec<u8>),
    List(Vec<SchemaValue>),
    Object(BTreeMap<String, SchemaValue>),
    Tensor(SchemaTensor),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SchemaNonFiniteFloat {
    NaN,
    PositiveInfinity,
    NegativeInfinity,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SchemaTensor {
    pub type_spec: String,
    pub payload: SchemaTensorPayload,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SchemaTensorPayload {
    Dense(Vec<f64>),
    Sparse(Vec<SchemaTensorCell>),
    Mixed(Vec<SchemaTensorBlock>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct SchemaTensorCell {
    pub address: BTreeMap<String, String>,
    pub value: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SchemaTensorBlock {
    pub address: BTreeMap<String, String>,
    pub values: Vec<f64>,
}

pub type SchemaProperties = BTreeMap<String, SchemaValue>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PrimitiveDataType {
    String,
    Bool,
    Byte,
    Int32,
    Int64,
    Float32,
    Float64,
    Raw,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DataType {
    Primitive(PrimitiveDataType),
    Tensor {
        type_spec: String,
    },
    Array(Box<DataType>),
    Map {
        key_type: Box<DataType>,
        value_type: Box<DataType>,
    },
    WeightedSet(Box<DataType>),
    Struct {
        fields: Vec<SchemaStructField>,
    },
    StructReference {
        name: String,
    },
    Predicate,
    Position,
    Reference {
        document_type: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SchemaStructField {
    pub name: String,
    pub data_type: DataType,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatchingMode {
    Unspecified,
    Text,
    Word,
    Exact,
    Gram,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AnalyzerConfig {
    pub name: String,
    pub version: u32,
    pub options: SchemaProperties,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MatchingConfig {
    pub mode: MatchingMode,
    pub options: SchemaProperties,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IndexConfig {
    pub enabled: Option<bool>,
    pub hnsw: Option<HnswConfig>,
    pub options: SchemaProperties,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HnswConfig {
    pub max_links_per_node: Option<u32>,
    pub neighbors_to_explore_at_insert: Option<u32>,
    pub options: SchemaProperties,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AttributeConfig {
    pub enabled: Option<bool>,
    pub fast_search: Option<bool>,
    pub fast_access: Option<bool>,
    pub distance_metric: DistanceMetric,
    pub options: SchemaProperties,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SummaryConfig {
    pub enabled: Option<bool>,
    pub options: SchemaProperties,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StructFieldConfig {
    pub path: String,
    pub indexing_script: Option<String>,
    pub matching: Option<MatchingConfig>,
    pub index: Option<IndexConfig>,
    pub attribute: Option<AttributeConfig>,
    pub summary: Option<SummaryConfig>,
    pub rank: Option<String>,
    pub query_commands: Vec<String>,
    pub properties: SchemaProperties,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RankPhase {
    pub expression: Option<String>,
    pub rerank_count: Option<u32>,
    pub rank_score_drop_limit: Option<f64>,
    pub properties: SchemaProperties,
    pub total_rerank_count: Option<u32>,
    pub keep_rank_count: Option<u32>,
    pub total_keep_rank_count: Option<u32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RankProfileInput {
    pub name: String,
    pub data_type: DataType,
    pub default_value: SchemaValue,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RankFunction {
    pub name: String,
    pub arguments: Vec<String>,
    pub expression: String,
    pub inline: Option<bool>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OnnxModel {
    pub name: String,
    pub file: String,
    pub inputs: BTreeMap<String, String>,
    pub outputs: BTreeMap<String, String>,
    pub gpu_device: Option<u32>,
    pub intraop_threads: Option<u32>,
    pub interop_threads: Option<u32>,
    pub properties: SchemaProperties,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NamedStruct {
    pub name: String,
    pub fields: Vec<SchemaStructField>,
    pub properties: SchemaProperties,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FieldSet {
    pub name: String,
    pub fields: Vec<String>,
    pub matching: Option<MatchingConfig>,
    pub query_commands: Vec<String>,
    pub properties: SchemaProperties,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ImportedField {
    pub reference_field: String,
    pub target_field: String,
    pub alias: String,
    pub properties: SchemaProperties,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DocumentSummaryField {
    pub name: String,
    pub source: String,
    pub data_type: DataType,
    pub struct_fields: Vec<String>,
    pub properties: SchemaProperties,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DocumentSummary {
    pub name: String,
    pub inherits: Vec<String>,
    pub fields: Vec<DocumentSummaryField>,
    pub from_disk: Option<bool>,
    pub omit_summary_features: Option<bool>,
    pub properties: SchemaProperties,
}

impl SchemaValue {
    pub(crate) fn from_ir(value: &searchkernel_ir::RequestValueIr) -> Self {
        use searchkernel_ir::{RequestNonFiniteFloatIr, RequestValueIr};
        match value {
            RequestValueIr::Null => Self::Null,
            RequestValueIr::Bool(value) => Self::Bool(*value),
            RequestValueIr::Int64(value) => Self::Int64(*value),
            RequestValueIr::UInt64(value) => Self::UInt64(*value),
            RequestValueIr::Float64(value) => Self::Float64(*value),
            RequestValueIr::NonFiniteFloat(value) => Self::NonFiniteFloat(match value {
                RequestNonFiniteFloatIr::NaN => SchemaNonFiniteFloat::NaN,
                RequestNonFiniteFloatIr::PositiveInfinity => SchemaNonFiniteFloat::PositiveInfinity,
                RequestNonFiniteFloatIr::NegativeInfinity => SchemaNonFiniteFloat::NegativeInfinity,
            }),
            RequestValueIr::String(value) => Self::String(value.clone()),
            RequestValueIr::Bytes(value) => Self::Bytes(value.clone()),
            RequestValueIr::List(values) => Self::List(values.iter().map(Self::from_ir).collect()),
            RequestValueIr::Object(values) => Self::Object(
                values
                    .iter()
                    .map(|(key, value)| (key.clone(), Self::from_ir(value)))
                    .collect(),
            ),
            RequestValueIr::Tensor(value) => Self::Tensor(SchemaTensor::from_ir(value)),
        }
    }

    pub(crate) fn to_ir(&self) -> searchkernel_ir::RequestValueIr {
        use searchkernel_ir::{RequestNonFiniteFloatIr, RequestValueIr};
        match self {
            Self::Null => RequestValueIr::Null,
            Self::Bool(value) => RequestValueIr::Bool(*value),
            Self::Int64(value) => RequestValueIr::Int64(*value),
            Self::UInt64(value) => RequestValueIr::UInt64(*value),
            Self::Float64(value) => RequestValueIr::Float64(*value),
            Self::NonFiniteFloat(value) => RequestValueIr::NonFiniteFloat(match value {
                SchemaNonFiniteFloat::NaN => RequestNonFiniteFloatIr::NaN,
                SchemaNonFiniteFloat::PositiveInfinity => RequestNonFiniteFloatIr::PositiveInfinity,
                SchemaNonFiniteFloat::NegativeInfinity => RequestNonFiniteFloatIr::NegativeInfinity,
            }),
            Self::String(value) => RequestValueIr::String(value.clone()),
            Self::Bytes(value) => RequestValueIr::Bytes(value.clone()),
            Self::List(values) => RequestValueIr::List(values.iter().map(Self::to_ir).collect()),
            Self::Object(values) => RequestValueIr::Object(
                values
                    .iter()
                    .map(|(key, value)| (key.clone(), value.to_ir()))
                    .collect(),
            ),
            Self::Tensor(value) => RequestValueIr::Tensor(value.to_ir()),
        }
    }
}

impl SchemaTensor {
    pub(crate) fn from_ir(value: &searchkernel_ir::RequestTensorIr) -> Self {
        let payload = match &value.payload {
            searchkernel_ir::RequestTensorPayloadIr::Dense(values) => {
                SchemaTensorPayload::Dense(values.clone())
            }
            searchkernel_ir::RequestTensorPayloadIr::Sparse(values) => SchemaTensorPayload::Sparse(
                values
                    .iter()
                    .map(|value| SchemaTensorCell {
                        address: value.address.clone(),
                        value: value.value,
                    })
                    .collect(),
            ),
            searchkernel_ir::RequestTensorPayloadIr::Mixed(values) => SchemaTensorPayload::Mixed(
                values
                    .iter()
                    .map(|value| SchemaTensorBlock {
                        address: value.address.clone(),
                        values: value.values.clone(),
                    })
                    .collect(),
            ),
        };
        Self {
            type_spec: value.type_spec.clone(),
            payload,
        }
    }

    pub(crate) fn to_ir(&self) -> searchkernel_ir::RequestTensorIr {
        let payload = match &self.payload {
            SchemaTensorPayload::Dense(values) => {
                searchkernel_ir::RequestTensorPayloadIr::Dense(values.clone())
            }
            SchemaTensorPayload::Sparse(values) => searchkernel_ir::RequestTensorPayloadIr::Sparse(
                values
                    .iter()
                    .map(|value| searchkernel_ir::RequestTensorCellIr {
                        address: value.address.clone(),
                        value: value.value,
                    })
                    .collect(),
            ),
            SchemaTensorPayload::Mixed(values) => searchkernel_ir::RequestTensorPayloadIr::Mixed(
                values
                    .iter()
                    .map(|value| searchkernel_ir::RequestTensorBlockIr {
                        address: value.address.clone(),
                        values: value.values.clone(),
                    })
                    .collect(),
            ),
        };
        searchkernel_ir::RequestTensorIr {
            type_spec: self.type_spec.clone(),
            payload,
        }
    }
}

impl DataType {
    pub(crate) fn from_ir(value: &searchkernel_ir::DataTypeIr) -> Self {
        use searchkernel_ir::{DataTypeIr, PrimitiveDataTypeIr};
        match value {
            DataTypeIr::Primitive(value) => Self::Primitive(match value {
                PrimitiveDataTypeIr::String => PrimitiveDataType::String,
                PrimitiveDataTypeIr::Bool => PrimitiveDataType::Bool,
                PrimitiveDataTypeIr::Byte => PrimitiveDataType::Byte,
                PrimitiveDataTypeIr::Int32 => PrimitiveDataType::Int32,
                PrimitiveDataTypeIr::Int64 => PrimitiveDataType::Int64,
                PrimitiveDataTypeIr::Float32 => PrimitiveDataType::Float32,
                PrimitiveDataTypeIr::Float64 => PrimitiveDataType::Float64,
                PrimitiveDataTypeIr::Raw => PrimitiveDataType::Raw,
            }),
            DataTypeIr::Tensor { type_spec } => Self::Tensor {
                type_spec: type_spec.clone(),
            },
            DataTypeIr::Array(value) => Self::Array(Box::new(Self::from_ir(value))),
            DataTypeIr::Map {
                key_type,
                value_type,
            } => Self::Map {
                key_type: Box::new(Self::from_ir(key_type)),
                value_type: Box::new(Self::from_ir(value_type)),
            },
            DataTypeIr::WeightedSet(value) => Self::WeightedSet(Box::new(Self::from_ir(value))),
            DataTypeIr::Struct { fields } => Self::Struct {
                fields: fields
                    .iter()
                    .map(|field| SchemaStructField {
                        name: field.name.clone(),
                        data_type: Self::from_ir(&field.data_type),
                    })
                    .collect(),
            },
            DataTypeIr::StructReference { name } => Self::StructReference { name: name.clone() },
            DataTypeIr::Predicate => Self::Predicate,
            DataTypeIr::Position => Self::Position,
            DataTypeIr::Reference { document_type } => Self::Reference {
                document_type: document_type.clone(),
            },
        }
    }

    pub(crate) fn to_ir(&self) -> searchkernel_ir::DataTypeIr {
        use searchkernel_ir::{DataTypeIr, PrimitiveDataTypeIr};
        match self {
            Self::Primitive(value) => DataTypeIr::Primitive(match value {
                PrimitiveDataType::String => PrimitiveDataTypeIr::String,
                PrimitiveDataType::Bool => PrimitiveDataTypeIr::Bool,
                PrimitiveDataType::Byte => PrimitiveDataTypeIr::Byte,
                PrimitiveDataType::Int32 => PrimitiveDataTypeIr::Int32,
                PrimitiveDataType::Int64 => PrimitiveDataTypeIr::Int64,
                PrimitiveDataType::Float32 => PrimitiveDataTypeIr::Float32,
                PrimitiveDataType::Float64 => PrimitiveDataTypeIr::Float64,
                PrimitiveDataType::Raw => PrimitiveDataTypeIr::Raw,
            }),
            Self::Tensor { type_spec } => DataTypeIr::Tensor {
                type_spec: type_spec.clone(),
            },
            Self::Array(value) => DataTypeIr::Array(Box::new(value.to_ir())),
            Self::Map {
                key_type,
                value_type,
            } => DataTypeIr::Map {
                key_type: Box::new(key_type.to_ir()),
                value_type: Box::new(value_type.to_ir()),
            },
            Self::WeightedSet(value) => DataTypeIr::WeightedSet(Box::new(value.to_ir())),
            Self::Struct { fields } => DataTypeIr::Struct {
                fields: fields
                    .iter()
                    .map(|field| searchkernel_ir::StructFieldIr {
                        name: field.name.clone(),
                        data_type: field.data_type.to_ir(),
                    })
                    .collect(),
            },
            Self::StructReference { name } => DataTypeIr::StructReference { name: name.clone() },
            Self::Predicate => DataTypeIr::Predicate,
            Self::Position => DataTypeIr::Position,
            Self::Reference { document_type } => DataTypeIr::Reference {
                document_type: document_type.clone(),
            },
        }
    }
}

fn properties_from_ir(value: &searchkernel_ir::SchemaPropertiesIr) -> SchemaProperties {
    value
        .iter()
        .map(|(key, value)| (key.clone(), SchemaValue::from_ir(value)))
        .collect()
}

pub(crate) fn properties_to_ir(value: &SchemaProperties) -> searchkernel_ir::SchemaPropertiesIr {
    value
        .iter()
        .map(|(key, value)| (key.clone(), value.to_ir()))
        .collect()
}

impl AnalyzerConfig {
    pub(crate) fn from_ir(value: &searchkernel_ir::AnalyzerIr) -> Self {
        Self {
            name: value.name.clone(),
            version: value.version,
            options: properties_from_ir(&value.options),
        }
    }
}

impl MatchingConfig {
    pub(crate) fn from_ir(value: &searchkernel_ir::MatchingConfigIr) -> Self {
        Self {
            mode: match value.mode {
                searchkernel_ir::MatchingModeIr::Unspecified => MatchingMode::Unspecified,
                searchkernel_ir::MatchingModeIr::Text => MatchingMode::Text,
                searchkernel_ir::MatchingModeIr::Word => MatchingMode::Word,
                searchkernel_ir::MatchingModeIr::Exact => MatchingMode::Exact,
                searchkernel_ir::MatchingModeIr::Gram => MatchingMode::Gram,
            },
            options: properties_from_ir(&value.options),
        }
    }
    pub(crate) fn to_ir(&self) -> searchkernel_ir::MatchingConfigIr {
        searchkernel_ir::MatchingConfigIr {
            mode: match self.mode {
                MatchingMode::Unspecified => searchkernel_ir::MatchingModeIr::Unspecified,
                MatchingMode::Text => searchkernel_ir::MatchingModeIr::Text,
                MatchingMode::Word => searchkernel_ir::MatchingModeIr::Word,
                MatchingMode::Exact => searchkernel_ir::MatchingModeIr::Exact,
                MatchingMode::Gram => searchkernel_ir::MatchingModeIr::Gram,
            },
            options: properties_to_ir(&self.options),
        }
    }
}

impl IndexConfig {
    pub(crate) fn from_ir(value: &searchkernel_ir::IndexConfigIr) -> Self {
        Self {
            enabled: value.enabled,
            hnsw: value.hnsw.as_ref().map(HnswConfig::from_ir),
            options: properties_from_ir(&value.options),
        }
    }
    pub(crate) fn to_ir(&self) -> searchkernel_ir::IndexConfigIr {
        searchkernel_ir::IndexConfigIr {
            enabled: self.enabled,
            hnsw: self.hnsw.as_ref().map(HnswConfig::to_ir),
            options: properties_to_ir(&self.options),
        }
    }
}

impl HnswConfig {
    pub(crate) fn from_ir(value: &searchkernel_ir::HnswIndexConfigIr) -> Self {
        Self {
            max_links_per_node: value.max_links_per_node,
            neighbors_to_explore_at_insert: value.neighbors_to_explore_at_insert,
            options: properties_from_ir(&value.options),
        }
    }
    pub(crate) fn to_ir(&self) -> searchkernel_ir::HnswIndexConfigIr {
        searchkernel_ir::HnswIndexConfigIr {
            max_links_per_node: self.max_links_per_node,
            neighbors_to_explore_at_insert: self.neighbors_to_explore_at_insert,
            options: properties_to_ir(&self.options),
        }
    }
}

impl AttributeConfig {
    pub(crate) fn from_ir(value: &searchkernel_ir::AttributeConfigIr) -> Self {
        Self {
            enabled: value.enabled,
            fast_search: value.fast_search,
            fast_access: value.fast_access,
            distance_metric: match value.distance_metric {
                searchkernel_ir::DistanceMetricIr::Unspecified => DistanceMetric::Unspecified,
                searchkernel_ir::DistanceMetricIr::Euclidean => DistanceMetric::Euclidean,
                searchkernel_ir::DistanceMetricIr::Angular => DistanceMetric::Angular,
                searchkernel_ir::DistanceMetricIr::DotProduct => DistanceMetric::DotProduct,
                searchkernel_ir::DistanceMetricIr::PrenormalizedAngular => {
                    DistanceMetric::PrenormalizedAngular
                }
                searchkernel_ir::DistanceMetricIr::GeoDegrees => DistanceMetric::GeoDegrees,
                searchkernel_ir::DistanceMetricIr::Hamming => DistanceMetric::Hamming,
            },
            options: properties_from_ir(&value.options),
        }
    }
    pub(crate) fn to_ir(&self) -> searchkernel_ir::AttributeConfigIr {
        searchkernel_ir::AttributeConfigIr {
            enabled: self.enabled,
            fast_search: self.fast_search,
            fast_access: self.fast_access,
            distance_metric: distance_metric_to_ir(self.distance_metric),
            options: properties_to_ir(&self.options),
        }
    }
}

impl SummaryConfig {
    pub(crate) fn from_ir(value: &searchkernel_ir::SummaryConfigIr) -> Self {
        Self {
            enabled: value.enabled,
            options: properties_from_ir(&value.options),
        }
    }
    pub(crate) fn to_ir(&self) -> searchkernel_ir::SummaryConfigIr {
        searchkernel_ir::SummaryConfigIr {
            enabled: self.enabled,
            options: properties_to_ir(&self.options),
        }
    }
}

impl StructFieldConfig {
    pub(crate) fn from_ir(value: &searchkernel_ir::StructFieldConfigIr) -> Self {
        Self {
            path: value.path.clone(),
            indexing_script: value.indexing_script.clone(),
            matching: value.matching.as_ref().map(MatchingConfig::from_ir),
            index: value.index.as_ref().map(IndexConfig::from_ir),
            attribute: value.attribute.as_ref().map(AttributeConfig::from_ir),
            summary: value.summary.as_ref().map(SummaryConfig::from_ir),
            rank: value.rank.clone(),
            query_commands: value.query_commands.clone(),
            properties: properties_from_ir(&value.properties),
        }
    }
    pub(crate) fn to_ir(&self) -> searchkernel_ir::StructFieldConfigIr {
        searchkernel_ir::StructFieldConfigIr {
            path: self.path.clone(),
            indexing_script: self.indexing_script.clone(),
            matching: self.matching.as_ref().map(MatchingConfig::to_ir),
            index: self.index.as_ref().map(IndexConfig::to_ir),
            attribute: self.attribute.as_ref().map(AttributeConfig::to_ir),
            summary: self.summary.as_ref().map(SummaryConfig::to_ir),
            rank: self.rank.clone(),
            query_commands: self.query_commands.clone(),
            properties: properties_to_ir(&self.properties),
        }
    }
}

impl RankPhase {
    pub(crate) fn from_ir(value: &searchkernel_ir::RankPhaseDefinitionIr) -> Self {
        Self {
            expression: value.expression.clone(),
            rerank_count: value.rerank_count,
            rank_score_drop_limit: value.rank_score_drop_limit,
            properties: properties_from_ir(&value.properties),
            total_rerank_count: value.total_rerank_count,
            keep_rank_count: value.keep_rank_count,
            total_keep_rank_count: value.total_keep_rank_count,
        }
    }
    pub(crate) fn to_ir(&self) -> searchkernel_ir::RankPhaseDefinitionIr {
        searchkernel_ir::RankPhaseDefinitionIr {
            expression: self.expression.clone(),
            rerank_count: self.rerank_count,
            rank_score_drop_limit: self.rank_score_drop_limit,
            properties: properties_to_ir(&self.properties),
            total_rerank_count: self.total_rerank_count,
            keep_rank_count: self.keep_rank_count,
            total_keep_rank_count: self.total_keep_rank_count,
        }
    }
}

impl RankProfileInput {
    pub(crate) fn from_ir(value: &searchkernel_ir::RankProfileInputIr) -> Self {
        Self {
            name: value.name.clone(),
            data_type: DataType::from_ir(&value.data_type),
            default_value: SchemaValue::from_ir(&value.default_value),
        }
    }
    pub(crate) fn to_ir(&self) -> searchkernel_ir::RankProfileInputIr {
        searchkernel_ir::RankProfileInputIr {
            name: self.name.clone(),
            data_type: self.data_type.to_ir(),
            default_value: self.default_value.to_ir(),
        }
    }
}

impl RankFunction {
    pub(crate) fn from_ir(value: &searchkernel_ir::RankFunctionDefinitionIr) -> Self {
        Self {
            name: value.name.clone(),
            arguments: value.arguments.clone(),
            expression: value.expression.clone(),
            inline: value.inline,
        }
    }
    pub(crate) fn to_ir(&self) -> searchkernel_ir::RankFunctionDefinitionIr {
        searchkernel_ir::RankFunctionDefinitionIr {
            name: self.name.clone(),
            arguments: self.arguments.clone(),
            expression: self.expression.clone(),
            inline: self.inline,
        }
    }
}

impl OnnxModel {
    pub(crate) fn from_ir(value: &searchkernel_ir::OnnxModelDefinitionIr) -> Self {
        Self {
            name: value.name.clone(),
            file: value.file.clone(),
            inputs: value.inputs.clone(),
            outputs: value.outputs.clone(),
            gpu_device: value.gpu_device,
            intraop_threads: value.intraop_threads,
            interop_threads: value.interop_threads,
            properties: properties_from_ir(&value.properties),
        }
    }
    pub(crate) fn to_ir(&self) -> searchkernel_ir::OnnxModelDefinitionIr {
        searchkernel_ir::OnnxModelDefinitionIr {
            name: self.name.clone(),
            file: self.file.clone(),
            inputs: self.inputs.clone(),
            outputs: self.outputs.clone(),
            gpu_device: self.gpu_device,
            intraop_threads: self.intraop_threads,
            interop_threads: self.interop_threads,
            properties: properties_to_ir(&self.properties),
        }
    }
}

impl NamedStruct {
    pub(crate) fn from_ir(value: &searchkernel_ir::NamedStructDefinitionIr) -> Self {
        Self {
            name: value.name.clone(),
            fields: value
                .fields
                .iter()
                .map(|field| SchemaStructField {
                    name: field.name.clone(),
                    data_type: DataType::from_ir(&field.data_type),
                })
                .collect(),
            properties: properties_from_ir(&value.properties),
        }
    }
    pub(crate) fn to_ir(&self) -> searchkernel_ir::NamedStructDefinitionIr {
        searchkernel_ir::NamedStructDefinitionIr {
            name: self.name.clone(),
            fields: self
                .fields
                .iter()
                .map(|field| searchkernel_ir::StructFieldIr {
                    name: field.name.clone(),
                    data_type: field.data_type.to_ir(),
                })
                .collect(),
            properties: properties_to_ir(&self.properties),
        }
    }
}

impl FieldSet {
    pub(crate) fn from_ir(value: &searchkernel_ir::FieldSetDefinitionIr) -> Self {
        Self {
            name: value.name.clone(),
            fields: value
                .fields
                .iter()
                .map(|field| field.as_str().to_owned())
                .collect(),
            matching: value.matching.as_ref().map(MatchingConfig::from_ir),
            query_commands: value.query_commands.clone(),
            properties: properties_from_ir(&value.properties),
        }
    }
    pub(crate) fn to_ir(&self) -> Result<searchkernel_ir::FieldSetDefinitionIr> {
        Ok(searchkernel_ir::FieldSetDefinitionIr {
            name: self.name.clone(),
            fields: self
                .fields
                .iter()
                .map(|field| {
                    searchkernel_ir::FieldName::parse(field.clone()).map_err(Error::from_ir)
                })
                .collect::<Result<Vec<_>>>()?,
            matching: self.matching.as_ref().map(MatchingConfig::to_ir),
            query_commands: self.query_commands.clone(),
            properties: properties_to_ir(&self.properties),
        })
    }
}

impl ImportedField {
    pub(crate) fn from_ir(value: &searchkernel_ir::ImportedFieldDefinitionIr) -> Self {
        Self {
            reference_field: value.reference_field.as_str().to_owned(),
            target_field: value.target_field.clone(),
            alias: value.alias.as_str().to_owned(),
            properties: properties_from_ir(&value.properties),
        }
    }
    pub(crate) fn to_ir(&self) -> Result<searchkernel_ir::ImportedFieldDefinitionIr> {
        Ok(searchkernel_ir::ImportedFieldDefinitionIr {
            reference_field: searchkernel_ir::FieldName::parse(self.reference_field.clone())
                .map_err(Error::from_ir)?,
            target_field: self.target_field.clone(),
            alias: searchkernel_ir::FieldName::parse(self.alias.clone()).map_err(Error::from_ir)?,
            properties: properties_to_ir(&self.properties),
        })
    }
}

impl DocumentSummaryField {
    pub(crate) fn from_ir(value: &searchkernel_ir::DocumentSummaryFieldDefinitionIr) -> Self {
        Self {
            name: value.name.clone(),
            source: value.source.clone(),
            data_type: DataType::from_ir(&value.data_type),
            struct_fields: value.struct_fields.clone(),
            properties: properties_from_ir(&value.properties),
        }
    }
    pub(crate) fn to_ir(&self) -> searchkernel_ir::DocumentSummaryFieldDefinitionIr {
        searchkernel_ir::DocumentSummaryFieldDefinitionIr {
            name: self.name.clone(),
            source: self.source.clone(),
            data_type: self.data_type.to_ir(),
            struct_fields: self.struct_fields.clone(),
            properties: properties_to_ir(&self.properties),
        }
    }
}

impl DocumentSummary {
    pub(crate) fn from_ir(value: &searchkernel_ir::DocumentSummaryDefinitionIr) -> Self {
        Self {
            name: value.name.clone(),
            inherits: value.inherits.clone(),
            fields: value
                .fields
                .iter()
                .map(DocumentSummaryField::from_ir)
                .collect(),
            from_disk: value.from_disk,
            omit_summary_features: value.omit_summary_features,
            properties: properties_from_ir(&value.properties),
        }
    }
    pub(crate) fn to_ir(&self) -> searchkernel_ir::DocumentSummaryDefinitionIr {
        searchkernel_ir::DocumentSummaryDefinitionIr {
            name: self.name.clone(),
            inherits: self.inherits.clone(),
            fields: self
                .fields
                .iter()
                .map(DocumentSummaryField::to_ir)
                .collect(),
            from_disk: self.from_disk,
            omit_summary_features: self.omit_summary_features,
            properties: properties_to_ir(&self.properties),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Distance metric used by a tensor attribute.
pub enum DistanceMetric {
    Unspecified,
    Euclidean,
    Angular,
    DotProduct,
    PrenormalizedAngular,
    GeoDegrees,
    Hamming,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// HNSW graph-construction settings for a tensor field.
pub struct HnswIndexConfig {
    pub(crate) max_links_per_node: Option<u32>,
    pub(crate) neighbors_to_explore_at_insert: Option<u32>,
}

impl HnswIndexConfig {
    pub fn new() -> Self {
        Self {
            max_links_per_node: None,
            neighbors_to_explore_at_insert: None,
        }
    }

    pub fn max_links_per_node(mut self, value: u32) -> Self {
        self.max_links_per_node = Some(value);
        self
    }

    pub fn neighbors_to_explore_at_insert(mut self, value: u32) -> Self {
        self.neighbors_to_explore_at_insert = Some(value);
        self
    }
}

impl Default for HnswIndexConfig {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, PartialEq)]
/// An analyzer configuration supported by Search Kernel.
pub struct Analyzer {
    pub(crate) name: String,
    pub(crate) version: u32,
    pub(crate) options: BTreeMap<String, serde_json::Value>,
}

impl Analyzer {
    pub fn standard_v1() -> Self {
        Self {
            name: "standard".to_owned(),
            version: 1,
            options: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum FieldKind {
    Text,
    Keyword,
    Int64,
    Float64,
    Bool,
    Position,
    Tensor(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Resolved capabilities of a field returned by `Index::describe()`.
///
/// Ergonomic field constructors leave this unresolved so canonical defaults
/// continue to be owned by `searchkernel-ir`. Described fields populate all
/// five flags exactly from canonical IR.
pub struct FieldCapabilities {
    stored: bool,
    searchable: bool,
    filterable: bool,
    sortable: bool,
    groupable: bool,
}

impl FieldCapabilities {
    pub(crate) fn from_ir(value: &searchkernel_ir::FieldCapabilities) -> Self {
        Self {
            stored: value.stored(),
            searchable: value.searchable(),
            filterable: value.filterable(),
            sortable: value.sortable(),
            groupable: value.groupable(),
        }
    }

    pub fn stored(&self) -> bool {
        self.stored
    }

    pub fn searchable(&self) -> bool {
        self.searchable
    }

    pub fn filterable(&self) -> bool {
        self.filterable
    }

    pub fn sortable(&self) -> bool {
        self.sortable
    }

    pub fn groupable(&self) -> bool {
        self.groupable
    }

    pub fn facetable(&self) -> bool {
        self.groupable
    }
}

#[derive(Clone, Debug, PartialEq)]
/// The type and capabilities of one indexed field.
pub struct Field {
    pub(crate) name: String,
    pub(crate) kind: FieldKind,
    pub data_type: DataType,
    pub(crate) multi: bool,
    pub(crate) stored: Option<bool>,
    pub(crate) searchable: Option<bool>,
    pub(crate) filterable: Option<bool>,
    pub(crate) sortable: Option<bool>,
    pub(crate) facetable: Option<bool>,
    pub(crate) capabilities: Option<FieldCapabilities>,
    pub(crate) analyzer: Option<Analyzer>,
    pub(crate) analyzer_config: Option<searchkernel_ir::AnalyzerIr>,
    pub(crate) matching_config: Option<searchkernel_ir::MatchingConfigIr>,
    pub(crate) distance_metric: Option<DistanceMetric>,
    pub(crate) hnsw: Option<HnswIndexConfig>,
    pub(crate) struct_fields: Vec<StructFieldConfig>,
    pub(crate) summary_config: Option<searchkernel_ir::SummaryConfigIr>,
    pub indexing_script: Option<String>,
    pub(crate) properties: searchkernel_ir::SchemaPropertiesIr,
    /// Complete native index configuration returned by `Index::describe`.
    pub(crate) index_config: Option<searchkernel_ir::IndexConfigIr>,
    /// Complete native attribute configuration returned by `Index::describe`.
    pub(crate) attribute_config: Option<searchkernel_ir::AttributeConfigIr>,
}

#[derive(Clone, Debug, PartialEq)]
/// A Vespa rank profile exposed by the native schema compiler.
pub struct RankProfile {
    pub(crate) name: String,
    /// The complete first-phase configuration, when one is declared.
    pub first_phase: Option<RankPhase>,
    /// The complete second-phase configuration, when one is declared.
    pub second_phase: Option<RankPhase>,
    /// The complete global-phase configuration, when one is declared.
    pub global_phase: Option<RankPhase>,
    pub(crate) rerank_count: Option<u32>,
    /// Rank features exposed by this profile, in Vespa's declared order.
    pub rank_features: Vec<String>,
    /// Match features exposed by this profile, in Vespa's declared order.
    pub match_features: Vec<String>,
    /// Summary features exposed by this profile, in Vespa's declared order.
    pub summary_features: Vec<String>,
    /// Sort features exposed by this profile, in Vespa's declared order.
    pub sort_features: Vec<String>,
    /// Rank properties exposed by this profile.
    pub rank_properties: BTreeMap<String, String>,
    /// Whether strict rank-profile validation is enabled.
    pub strict: Option<bool>,
    /// Generic rank-profile properties not covered by dedicated fields.
    pub properties: SchemaProperties,
    /// Canonical rank-profile inputs exposed by Index::describe().
    pub inputs: Vec<RankProfileInput>,
    /// Canonical rank-profile scoped constants.
    pub constants: Vec<ConstantDefinition>,
    /// Canonical rank-profile scoped ONNX models.
    pub onnx_models: Vec<OnnxModel>,
    pub(crate) query_inputs: Vec<(String, String)>,
    /// Canonical profile inheritance exposed by Index::describe().
    pub inherits: Vec<String>,
    /// Canonical Vespa rank functions exposed by Index::describe().
    pub functions: Vec<RankFunction>,
}

impl RankProfile {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            first_phase: None,
            second_phase: None,
            global_phase: None,
            rerank_count: None,
            rank_features: Vec::new(),
            match_features: Vec::new(),
            summary_features: Vec::new(),
            sort_features: Vec::new(),
            rank_properties: BTreeMap::new(),
            strict: None,
            properties: BTreeMap::new(),
            inputs: Vec::new(),
            constants: Vec::new(),
            onnx_models: Vec::new(),
            query_inputs: Vec::new(),
            inherits: Vec::new(),
            functions: Vec::new(),
        }
    }

    pub fn first_phase(mut self, expression: impl Into<String>) -> Self {
        let expression = expression.into();
        self.first_phase
            .get_or_insert_with(empty_rank_phase)
            .expression = Some(expression.clone());
        self
    }

    /// Set the Vespa second-phase ranking expression.
    pub fn second_phase(mut self, expression: impl Into<String>) -> Self {
        let expression = expression.into();
        self.second_phase
            .get_or_insert_with(empty_rank_phase)
            .expression = Some(expression.clone());
        self
    }

    /// Set the number of first-phase candidates Vespa reranks in second phase.
    pub fn rerank_count(mut self, count: u32) -> Self {
        self.rerank_count = Some(count);
        self.second_phase
            .get_or_insert_with(empty_rank_phase)
            .rerank_count = Some(count);
        self
    }

    pub fn match_feature(mut self, expression: impl Into<String>) -> Self {
        let expression = expression.into();
        self.match_features.push(expression.clone());
        self
    }

    /// Declare a tensor query input used by this rank profile.
    pub fn query_input(mut self, name: impl Into<String>, type_spec: impl Into<String>) -> Self {
        let name = name.into();
        let type_spec = type_spec.into();
        self.query_inputs.push((name.clone(), type_spec.clone()));
        let input = RankProfileInput {
            name,
            data_type: DataType::Tensor {
                type_spec: type_spec.clone(),
            },
            default_value: SchemaValue::Tensor(SchemaTensor {
                type_spec,
                payload: SchemaTensorPayload::Dense(Vec::new()),
            }),
        };
        self.inputs.push(input.clone());
        self
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

impl Field {
    fn new(name: impl Into<String>, kind: FieldKind) -> Self {
        let data_type = data_type_from_kind(&kind);
        Self {
            name: name.into(),
            kind,
            data_type,
            multi: false,
            stored: None,
            searchable: None,
            filterable: None,
            sortable: None,
            facetable: None,
            capabilities: None,
            analyzer: None,
            analyzer_config: None,
            matching_config: None,
            distance_metric: None,
            hnsw: None,
            struct_fields: Vec::new(),
            summary_config: None,
            indexing_script: None,
            properties: BTreeMap::new(),
            index_config: None,
            attribute_config: None,
        }
    }

    /// Create a text field.
    pub fn text(name: impl Into<String>) -> Self {
        Self::new(name, FieldKind::Text)
    }
    /// Create a keyword field.
    pub fn keyword(name: impl Into<String>) -> Self {
        Self::new(name, FieldKind::Keyword)
    }
    /// Create a signed 64-bit integer field.
    ///
    /// The native Vespa attribute representation reserves `i64::MIN` as its
    /// undefined sentinel. The native index therefore rejects that value for
    /// writes and attribute queries; all other signed 64-bit values are
    /// supported.
    pub fn int64(name: impl Into<String>) -> Self {
        Self::new(name, FieldKind::Int64)
    }
    /// Create a 64-bit floating-point field.
    pub fn float64(name: impl Into<String>) -> Self {
        Self::new(name, FieldKind::Float64)
    }
    /// Create a boolean field.
    pub fn bool(name: impl Into<String>) -> Self {
        Self::new(name, FieldKind::Bool)
    }

    /// Create a geographic position field with latitude/longitude values.
    pub fn position(name: impl Into<String>) -> Self {
        Self::new(name, FieldKind::Position)
    }

    /// Create a dense tensor field with the supplied Vespa type spec.
    pub fn tensor(name: impl Into<String>, type_spec: impl Into<String>) -> Self {
        let type_spec = type_spec.into();
        Self {
            name: name.into(),
            kind: FieldKind::Tensor(type_spec.clone()),
            data_type: DataType::Tensor { type_spec },
            multi: false,
            stored: None,
            searchable: None,
            filterable: None,
            sortable: None,
            facetable: None,
            capabilities: None,
            analyzer: None,
            analyzer_config: None,
            matching_config: None,
            distance_metric: None,
            hnsw: None,
            struct_fields: Vec::new(),
            summary_config: None,
            indexing_script: None,
            properties: BTreeMap::new(),
            index_config: None,
            attribute_config: None,
        }
    }

    /// Make this field contain a collection of values.
    pub fn multi(mut self) -> Self {
        self.multi = true;
        if !matches!(
            &self.data_type,
            DataType::Array(_) | DataType::WeightedSet(_)
        ) {
            self.data_type = DataType::Array(Box::new(self.data_type));
        }
        self
    }
    /// Explicitly set whether values are stored in search results.
    pub fn stored(mut self, value: bool) -> Self {
        self.stored = Some(value);
        if let Some(capabilities) = self.capabilities.as_mut() {
            capabilities.stored = value;
        }
        if let Some(summary) = self.summary_config.as_mut() {
            summary.enabled = Some(value);
        }
        self
    }
    /// Explicitly set whether this field can be searched with text queries.
    pub fn searchable(mut self, value: bool) -> Self {
        self.searchable = Some(value);
        if let Some(capabilities) = self.capabilities.as_mut() {
            capabilities.searchable = value;
        }
        self
    }
    /// Explicitly set whether this field can be used in filters.
    pub fn filterable(mut self, value: bool) -> Self {
        self.filterable = Some(value);
        if let Some(capabilities) = self.capabilities.as_mut() {
            capabilities.filterable = value;
        }
        self
    }
    /// Explicitly set whether this field can be used for sorting.
    pub fn sortable(mut self, value: bool) -> Self {
        self.sortable = Some(value);
        if let Some(capabilities) = self.capabilities.as_mut() {
            capabilities.sortable = value;
        }
        self
    }
    /// Explicitly set whether this field can be used for facets.
    pub fn facetable(mut self, value: bool) -> Self {
        self.facetable = Some(value);
        if let Some(capabilities) = self.capabilities.as_mut() {
            capabilities.groupable = value;
        }
        self
    }
    /// Select the analyzer used for this field.
    pub fn analyzer(mut self, value: Analyzer) -> Self {
        let canonical = analyzer_to_ir(&value).ok();
        self.analyzer_config = canonical.clone();
        self.analyzer = Some(value);
        self
    }

    /// Set the canonical matching configuration for this field.
    pub fn matching_config(mut self, value: MatchingConfig) -> Self {
        self.matching_config = Some(value.to_ir());
        self
    }

    /// Configure the tensor attribute distance metric.
    pub fn distance_metric(mut self, value: DistanceMetric) -> Self {
        self.distance_metric = Some(value);
        let attribute =
            self.attribute_config
                .get_or_insert_with(|| searchkernel_ir::AttributeConfigIr {
                    enabled: Some(true),
                    fast_search: None,
                    fast_access: None,
                    distance_metric: searchkernel_ir::DistanceMetricIr::Unspecified,
                    options: BTreeMap::new(),
                });
        attribute.distance_metric = distance_metric_to_ir(value);
        self
    }

    /// Configure HNSW indexing for this tensor field.
    pub fn hnsw(mut self, value: HnswIndexConfig) -> Self {
        let hnsw = hnsw_to_ir(&value);
        self.hnsw = Some(value);
        let index = self
            .index_config
            .get_or_insert_with(|| searchkernel_ir::IndexConfigIr {
                enabled: Some(true),
                hnsw: None,
                options: BTreeMap::new(),
            });
        index.enabled = Some(true);
        index.hnsw = Some(hnsw.clone());
        self
    }

    /// Return this field's name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Return the complete canonical data type of this field.
    pub fn data_type(&self) -> &DataType {
        &self.data_type
    }

    /// Return whether this field stores multiple values.
    pub fn is_multi(&self) -> bool {
        matches!(
            &self.data_type,
            DataType::Array(_) | DataType::WeightedSet(_)
        )
    }

    /// Return the fully resolved canonical capabilities when this field
    /// originated from `Index::describe()`.
    pub fn capabilities(&self) -> Option<&FieldCapabilities> {
        self.capabilities.as_ref()
    }

    /// Return the explicitly configured storage capability, if any.
    pub fn configured_stored(&self) -> Option<bool> {
        self.stored
    }

    /// Return the explicitly configured search capability, if any.
    pub fn configured_searchable(&self) -> Option<bool> {
        self.searchable
    }

    /// Return the explicitly configured filter capability, if any.
    pub fn configured_filterable(&self) -> Option<bool> {
        self.filterable
    }

    /// Return the explicitly configured sort capability, if any.
    pub fn configured_sortable(&self) -> Option<bool> {
        self.sortable
    }

    /// Return the explicitly configured facet capability, if any.
    pub fn configured_facetable(&self) -> Option<bool> {
        self.facetable
    }

    /// Return the configured analyzer, if any.
    pub fn configured_analyzer(&self) -> Option<Analyzer> {
        self.analyzer.clone()
    }

    /// Return the complete analyzer configuration, including arbitrary options.
    pub fn analyzer_config(&self) -> Option<AnalyzerConfig> {
        self.analyzer_config.as_ref().map(AnalyzerConfig::from_ir)
    }

    /// Return the complete matching configuration, including arbitrary options.
    pub fn matching(&self) -> Option<MatchingConfig> {
        self.matching_config.as_ref().map(MatchingConfig::from_ir)
    }

    pub fn struct_fields(&self) -> Vec<StructFieldConfig> {
        self.struct_fields.clone()
    }
}

fn data_type_from_kind(kind: &FieldKind) -> DataType {
    match kind {
        FieldKind::Text | FieldKind::Keyword => DataType::Primitive(PrimitiveDataType::String),
        FieldKind::Int64 => DataType::Primitive(PrimitiveDataType::Int64),
        FieldKind::Float64 => DataType::Primitive(PrimitiveDataType::Float64),
        FieldKind::Bool => DataType::Primitive(PrimitiveDataType::Bool),
        FieldKind::Position => DataType::Position,
        FieldKind::Tensor(type_spec) => DataType::Tensor {
            type_spec: type_spec.clone(),
        },
    }
}

#[derive(Clone, Debug, PartialEq)]
/// A field selected for text search, optionally with a weight.
pub struct SearchField {
    pub(crate) field: String,
    pub(crate) weight: Option<f64>,
}

impl SearchField {
    /// Select a field for text search.
    pub fn new(field: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            weight: None,
        }
    }
    /// Set the relative weight of this field during ranking.
    pub fn weight(mut self, value: f64) -> Self {
        self.weight = Some(value);
        self
    }

    /// Return the selected field name.
    pub fn field(&self) -> &str {
        &self.field
    }

    /// Return the relative weight, if one was explicitly configured.
    pub fn weight_value(&self) -> Option<f64> {
        self.weight
    }
}

impl From<&str> for SearchField {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for SearchField {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

/// The source of a schema constant.
#[derive(Clone, Debug, PartialEq)]
pub enum ConstantSource {
    Literal(SchemaValue),
    File { path: String },
}

/// A schema constant exposed by `Index::describe()`.
///
/// Constants use owned schema value and data-type domains so Rust-to-IR
/// conversion consumes the same state that callers inspect, rather than
/// recovering it from a hidden parent definition.
#[derive(Clone, Debug, PartialEq)]
pub struct ConstantDefinition {
    name: String,
    data_type: DataType,
    source: ConstantSource,
    properties: SchemaProperties,
}

impl ConstantDefinition {
    /// Construct a schema constant from its complete canonical members.
    pub fn new(name: impl Into<String>, data_type: DataType, source: ConstantSource) -> Self {
        Self {
            name: name.into(),
            data_type,
            source,
            properties: BTreeMap::new(),
        }
    }

    pub(crate) fn from_ir(value: &searchkernel_ir::ConstantDefinitionIr) -> Self {
        let source = match &value.source {
            searchkernel_ir::ConstantSourceIr::Literal(value) => {
                ConstantSource::Literal(SchemaValue::from_ir(value))
            }
            searchkernel_ir::ConstantSourceIr::File { path } => {
                ConstantSource::File { path: path.clone() }
            }
        };
        Self {
            name: value.name.clone(),
            data_type: DataType::from_ir(&value.data_type),
            source,
            properties: properties_from_ir(&value.properties),
        }
    }

    pub(crate) fn to_ir(&self) -> searchkernel_ir::ConstantDefinitionIr {
        let source = match &self.source {
            ConstantSource::Literal(value) => {
                searchkernel_ir::ConstantSourceIr::Literal(value.to_ir())
            }
            ConstantSource::File { path } => {
                searchkernel_ir::ConstantSourceIr::File { path: path.clone() }
            }
        };
        searchkernel_ir::ConstantDefinitionIr {
            name: self.name.clone(),
            data_type: self.data_type.to_ir(),
            source,
            properties: properties_to_ir(&self.properties),
        }
    }

    /// Return the canonical constant name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Return the canonical constant data type.
    pub fn data_type(&self) -> &DataType {
        &self.data_type
    }

    /// Return the canonical constant source.
    pub fn source(&self) -> &ConstantSource {
        &self.source
    }

    /// Return the canonical constant properties.
    pub fn properties(&self) -> &SchemaProperties {
        &self.properties
    }
}

#[derive(Clone, Default, PartialEq)]
/// The schema and default search settings for an index.
pub struct IndexDefinition {
    pub(crate) name: Option<String>,
    pub(crate) fields: Vec<Field>,
    pub(crate) default_search_fields: Option<Vec<SearchField>>,
    pub document_type: Option<String>,
    pub schema_inherits: Vec<String>,
    pub document_inherits: Vec<String>,
    pub(crate) document_summaries: Vec<DocumentSummary>,
    pub(crate) rank_profiles: Vec<RankProfile>,
    pub(crate) constants: Vec<ConstantDefinition>,
    pub(crate) fieldsets: Vec<FieldSet>,
    pub(crate) onnx_models: Vec<OnnxModel>,
    pub(crate) imported_fields: Vec<ImportedField>,
    pub(crate) structs: Vec<NamedStruct>,
    pub(crate) schema_fields: Vec<Field>,
    pub(crate) properties: SchemaProperties,
}

impl fmt::Debug for IndexDefinition {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IndexDefinition")
            .field("name", &self.name)
            .field("fields", &self.fields)
            .field("default_search_fields", &self.default_search_fields)
            .field("document_summaries", &self.document_summaries)
            .field("rank_profiles", &self.rank_profiles)
            .field("constants", &self.constants)
            .field("fieldsets", &self.fieldsets)
            .field("onnx_models", &self.onnx_models)
            .field("imported_fields", &self.imported_fields)
            .field("structs", &self.structs)
            .field("schema_fields", &self.schema_fields)
            .field("properties", &self.properties)
            .finish()
    }
}

impl IndexDefinition {
    /// Start an empty index definition.
    pub fn new() -> Self {
        Self::default()
    }
    /// Set the optional index name.
    pub fn name(mut self, value: impl Into<String>) -> Self {
        let value = value.into();
        self.name = Some(value);
        self
    }
    /// Add one field to the definition.
    pub fn field(mut self, value: Field) -> Self {
        self.fields.push(value);
        self
    }

    /// Add a schema-only field derived by an indexing expression.
    pub fn schema_field(mut self, value: Field) -> Self {
        self.schema_fields.push(value);
        self
    }
    /// Set the fields used when a search does not specify its own fields.
    pub fn default_search_fields<I>(mut self, fields: I) -> Self
    where
        I: IntoIterator,
        I::Item: Into<SearchField>,
    {
        let fields = fields.into_iter().map(Into::into).collect::<Vec<_>>();
        self.default_search_fields = Some(fields);
        self
    }

    /// Add a native first-phase rank profile to the generated schema.
    pub fn rank_profile(mut self, profile: RankProfile) -> Self {
        self.rank_profiles.push(profile);
        self
    }

    /// Return the optional index name.
    pub fn index_name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Return the fields in this definition.
    pub fn fields(&self) -> &[Field] {
        &self.fields
    }

    /// Return the schema-only fields derived by indexing expressions.
    pub fn schema_fields(&self) -> &[Field] {
        &self.schema_fields
    }

    /// Return the explicitly configured default search fields.
    pub fn configured_default_search_fields(&self) -> Option<&[SearchField]> {
        self.default_search_fields.as_deref()
    }

    /// Return the complete document-summary definitions represented by this public schema value.
    pub fn document_summaries(&self) -> &[DocumentSummary] {
        &self.document_summaries
    }

    pub fn rank_profiles(&self) -> &[RankProfile] {
        &self.rank_profiles
    }

    /// Return schema constants represented by this definition.
    pub fn constants(&self) -> &[ConstantDefinition] {
        &self.constants
    }

    /// Return the top-level fieldset definitions declared by the schema.
    pub fn fieldsets(&self) -> &[FieldSet] {
        &self.fieldsets
    }

    /// Return the top-level ONNX model definitions declared by the schema.
    pub fn onnx_models(&self) -> &[OnnxModel] {
        &self.onnx_models
    }

    /// Return the imported-field definitions declared by the schema.
    pub fn imported_fields(&self) -> &[ImportedField] {
        &self.imported_fields
    }

    /// Return the top-level named struct definitions declared by the schema.
    pub fn structs(&self) -> &[NamedStruct] {
        &self.structs
    }

    /// Return the top-level properties declared by the schema.
    pub fn properties(&self) -> &SchemaProperties {
        &self.properties
    }

    /// Add a schema constant to this definition.
    pub fn constant(mut self, value: ConstantDefinition) -> Self {
        self.constants.push(value);
        self
    }
}

fn empty_rank_phase() -> RankPhase {
    RankPhase {
        expression: None,
        rerank_count: None,
        rank_score_drop_limit: None,
        properties: BTreeMap::new(),
        total_rerank_count: None,
        keep_rank_count: None,
        total_keep_rank_count: None,
    }
}

pub(crate) fn analyzer_to_ir(value: &Analyzer) -> Result<searchkernel_ir::AnalyzerIr> {
    let options = value
        .options
        .iter()
        .map(|(key, option)| {
            searchkernel_ir::request_value_from_json(option, format!("analyzer.options.{key}"))
                .map(|value| (key.clone(), value))
                .map_err(Error::from_ir)
        })
        .collect::<Result<searchkernel_ir::SchemaPropertiesIr>>()?;
    Ok(searchkernel_ir::AnalyzerIr {
        name: value.name.clone(),
        version: value.version,
        options,
    })
}

fn hnsw_to_ir(value: &HnswIndexConfig) -> searchkernel_ir::HnswIndexConfigIr {
    searchkernel_ir::HnswIndexConfigIr {
        max_links_per_node: value.max_links_per_node,
        neighbors_to_explore_at_insert: value.neighbors_to_explore_at_insert,
        options: BTreeMap::new(),
    }
}

pub(crate) fn field_to_unresolved_input(
    field: &Field,
) -> Result<searchkernel_ir::UnresolvedFieldInput> {
    let name = searchkernel_ir::FieldName::parse(field.name.clone()).map_err(|_| {
        Error::from_ir(searchkernel_ir::PortableError::invalid(
            "invalid_field_name",
            Some(format!("fields.{}.name", field.name)),
            "field name must match [A-Za-z][A-Za-z0-9_]*",
        ))
    })?;
    let analyzer = match &field.analyzer_config {
        Some(analyzer) => Some(analyzer.clone()),
        None => field.analyzer.as_ref().map(analyzer_to_ir).transpose()?,
    };
    let matching = field.matching_config.clone().or_else(|| {
        if field.capabilities.is_some() {
            return None;
        }
        match field.kind {
            FieldKind::Text => Some(searchkernel_ir::MatchingConfigIr {
                mode: searchkernel_ir::MatchingModeIr::Text,
                options: BTreeMap::new(),
            }),
            FieldKind::Keyword => Some(searchkernel_ir::MatchingConfigIr {
                mode: searchkernel_ir::MatchingModeIr::Exact,
                options: BTreeMap::new(),
            }),
            _ => None,
        }
    });
    let index = field.index_config.clone().or_else(|| {
        if field.capabilities.is_some() {
            return None;
        }
        field
            .hnsw
            .as_ref()
            .map(|value| searchkernel_ir::IndexConfigIr {
                enabled: Some(true),
                hnsw: Some(hnsw_to_ir(value)),
                options: BTreeMap::new(),
            })
    });
    let attribute = field.attribute_config.clone().or_else(|| {
        if field.capabilities.is_some() {
            return None;
        }
        field
            .distance_metric
            .map(|value| searchkernel_ir::AttributeConfigIr {
                enabled: Some(true),
                fast_search: None,
                fast_access: None,
                distance_metric: distance_metric_to_ir(value),
                options: BTreeMap::new(),
            })
    });
    let indexing_script = field.indexing_script.is_some();
    let stored = field
        .stored
        .filter(|value| !indexing_script || !*value)
        .filter(|value| {
            field
                .summary_config
                .as_ref()
                .and_then(|summary| summary.enabled)
                != Some(*value)
        });
    let searchable = field
        .searchable
        .filter(|value| !indexing_script || *value)
        .filter(|value| {
            field.index_config.as_ref().and_then(|index| index.enabled) != Some(*value)
        });
    let filterable = field
        .filterable
        .filter(|value| !indexing_script || *value)
        .filter(|value| {
            field
                .attribute_config
                .as_ref()
                .and_then(|attribute| attribute.fast_search)
                != Some(true)
                || !*value
        });
    let sortable = field.sortable.filter(|value| !indexing_script || *value);
    let groupable = field.facetable.filter(|value| !indexing_script || *value);
    Ok(searchkernel_ir::UnresolvedFieldInput {
        name,
        data_type: field.data_type.to_ir(),
        stored,
        searchable,
        filterable,
        sortable,
        groupable,
        analyzer,
        matching,
        index,
        attribute,
        summary: field.summary_config.clone(),
        indexing_script: field.indexing_script.clone(),
        properties: field.properties.clone(),
        struct_fields: field
            .struct_fields
            .iter()
            .map(StructFieldConfig::to_ir)
            .collect(),
    })
}

pub(crate) fn index_definition_to_input(
    definition: &IndexDefinition,
) -> Result<searchkernel_ir::IndexDefinitionInput> {
    Ok(searchkernel_ir::IndexDefinitionInput {
        name: definition.name.clone(),
        fields: definition
            .fields
            .iter()
            .map(field_to_unresolved_input)
            .collect::<Result<Vec<_>>>()?,
        schema_fields: definition
            .schema_fields
            .iter()
            .map(field_to_unresolved_input)
            .collect::<Result<Vec<_>>>()?,
        default_search_fields: definition.default_search_fields.as_ref().map(|fields| {
            fields
                .iter()
                .map(|field| searchkernel_ir::RustSearchFieldInput {
                    field: field.field.clone(),
                    weight: field.weight,
                })
                .collect()
        }),
        properties: properties_to_ir(&definition.properties),
        document_type: definition.document_type.clone(),
        schema_inherits: definition.schema_inherits.clone(),
        document_inherits: definition.document_inherits.clone(),
        structs: definition.structs.iter().map(NamedStruct::to_ir).collect(),
        fieldsets: definition
            .fieldsets
            .iter()
            .map(FieldSet::to_ir)
            .collect::<Result<Vec<_>>>()?,
        imported_fields: definition
            .imported_fields
            .iter()
            .map(ImportedField::to_ir)
            .collect::<Result<Vec<_>>>()?,
        document_summaries: definition
            .document_summaries
            .iter()
            .map(DocumentSummary::to_ir)
            .collect(),
        rank_profiles: definition
            .rank_profiles
            .iter()
            .map(rank_profile_to_unresolved_input)
            .collect(),
        constants: definition
            .constants
            .iter()
            .map(ConstantDefinition::to_ir)
            .collect(),
        onnx_models: definition
            .onnx_models
            .iter()
            .map(OnnxModel::to_ir)
            .collect(),
    })
}

fn rank_profile_to_unresolved_input(
    profile: &RankProfile,
) -> searchkernel_ir::UnresolvedRankProfileInput {
    let inputs = if profile.inputs.is_empty() {
        profile
            .query_inputs
            .iter()
            .map(|(name, type_spec)| searchkernel_ir::RankProfileInputIr {
                name: name.clone(),
                data_type: searchkernel_ir::DataTypeIr::Tensor {
                    type_spec: type_spec.clone(),
                },
                default_value: searchkernel_ir::RequestValueIr::Tensor(
                    searchkernel_ir::RequestTensorIr {
                        type_spec: type_spec.clone(),
                        payload: searchkernel_ir::RequestTensorPayloadIr::Dense(Vec::new()),
                    },
                ),
            })
            .collect()
    } else {
        profile.inputs.iter().map(RankProfileInput::to_ir).collect()
    };
    searchkernel_ir::UnresolvedRankProfileInput {
        name: profile.name.clone(),
        inherits: profile.inherits.clone(),
        inputs,
        functions: profile.functions.iter().map(RankFunction::to_ir).collect(),
        first_phase: profile.first_phase.as_ref().map(RankPhase::to_ir),
        second_phase: profile
            .second_phase
            .as_ref()
            .map(RankPhase::to_ir)
            .or_else(|| {
                profile
                    .rerank_count
                    .map(|rerank_count| searchkernel_ir::RankPhaseDefinitionIr {
                        expression: None,
                        rerank_count: Some(rerank_count),
                        rank_score_drop_limit: None,
                        properties: BTreeMap::new(),
                        total_rerank_count: None,
                        keep_rank_count: None,
                        total_keep_rank_count: None,
                    })
            }),
        global_phase: profile.global_phase.as_ref().map(RankPhase::to_ir),
        rank_features: profile.rank_features.clone(),
        match_features: profile.match_features.clone(),
        summary_features: profile.summary_features.clone(),
        sort_features: profile.sort_features.clone(),
        constants: profile
            .constants
            .iter()
            .map(ConstantDefinition::to_ir)
            .collect(),
        onnx_models: profile.onnx_models.iter().map(OnnxModel::to_ir).collect(),
        rank_properties: profile.rank_properties.clone(),
        strict: profile.strict,
        properties: properties_to_ir(&profile.properties),
    }
}

fn distance_metric_to_ir(value: DistanceMetric) -> searchkernel_ir::DistanceMetricIr {
    match value {
        DistanceMetric::Unspecified => searchkernel_ir::DistanceMetricIr::Unspecified,
        DistanceMetric::Euclidean => searchkernel_ir::DistanceMetricIr::Euclidean,
        DistanceMetric::Angular => searchkernel_ir::DistanceMetricIr::Angular,
        DistanceMetric::DotProduct => searchkernel_ir::DistanceMetricIr::DotProduct,
        DistanceMetric::PrenormalizedAngular => {
            searchkernel_ir::DistanceMetricIr::PrenormalizedAngular
        }
        DistanceMetric::GeoDegrees => searchkernel_ir::DistanceMetricIr::GeoDegrees,
        DistanceMetric::Hamming => searchkernel_ir::DistanceMetricIr::Hamming,
    }
}

#[cfg(test)]
mod tests {
    use super::{IndexDefinition, RankProfile, SearchField};

    #[test]
    fn search_field_names_convert_to_unweighted_fields() {
        let borrowed: SearchField = "title".into();
        let owned: SearchField = String::from("description").into();

        assert_eq!(borrowed.field, "title");
        assert_eq!(borrowed.weight, None);
        assert_eq!(owned.field, "description");
        assert_eq!(owned.weight, None);
    }

    #[test]
    fn rank_profile_query_input_populates_public_canonical_state() {
        let profile =
            RankProfile::new("semantic").query_input("query_embedding", "tensor<float>(x[3])");

        assert_eq!(profile.inputs.len(), 1);
        assert_eq!(profile.inputs[0].name, "query_embedding");
        assert!(profile.constants.is_empty());
        assert!(profile.onnx_models.is_empty());
    }

    #[test]
    fn default_search_fields_accept_field_names() {
        let definition = IndexDefinition::new().default_search_fields(["title", "description"]);

        let fields = definition.default_search_fields.unwrap();
        assert_eq!(fields[0].field, "title");
        assert_eq!(fields[1].field, "description");
        assert_eq!(
            fields.iter().map(|field| field.weight).collect::<Vec<_>>(),
            [None, None]
        );
    }
}
