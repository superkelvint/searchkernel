//! Ranking policies and rank-profile definitions.

use super::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RankingPolicyIr {
    DefaultRelevanceV1,
    Bm25,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrefixPolicy {
    Disabled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TypoPolicy {
    Disabled,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RankingRequestIr {
    pub profile: String,
    pub properties: std::collections::BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RankProfileInputIr {
    pub name: String,
    pub data_type: DataTypeIr,
    pub default_value: RequestValueIr,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RankFunctionDefinitionIr {
    pub name: String,
    pub arguments: Vec<String>,
    pub expression: String,
    pub inline: Option<bool>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RankPhaseDefinitionIr {
    pub expression: Option<String>,
    pub rerank_count: Option<u32>,
    pub rank_score_drop_limit: Option<f64>,
    pub properties: SchemaPropertiesIr,
    pub total_rerank_count: Option<u32>,
    pub keep_rank_count: Option<u32>,
    pub total_keep_rank_count: Option<u32>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ConstantSourceIr {
    Literal(RequestValueIr),
    File { path: String },
}

#[derive(Clone, Debug, PartialEq)]
pub struct ConstantDefinitionIr {
    pub name: String,
    pub data_type: DataTypeIr,
    pub source: ConstantSourceIr,
    pub properties: SchemaPropertiesIr,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OnnxModelDefinitionIr {
    pub name: String,
    pub file: String,
    pub inputs: std::collections::BTreeMap<String, String>,
    pub outputs: std::collections::BTreeMap<String, String>,
    pub gpu_device: Option<u32>,
    pub intraop_threads: Option<u32>,
    pub interop_threads: Option<u32>,
    pub properties: SchemaPropertiesIr,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RankProfileDefinitionIr {
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
    pub rank_properties: std::collections::BTreeMap<String, String>,
    pub strict: Option<bool>,
    pub properties: SchemaPropertiesIr,
}
