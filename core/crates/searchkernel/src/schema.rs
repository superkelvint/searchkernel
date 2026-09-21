use std::collections::BTreeMap;
use std::fmt;

use crate::error::{Error, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Distance metric used by a tensor attribute.
pub enum DistanceMetric {
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
/// Complete capability state exposed by a described field.
pub struct FieldCapabilities {
    pub stored: bool,
    pub searchable: bool,
    pub filterable: bool,
    pub sortable: bool,
    pub groupable: bool,
}

#[derive(Clone, Debug, PartialEq)]
/// The type and capabilities of one indexed field.
pub struct Field {
    pub name: String,
    pub(crate) kind: FieldKind,
    pub(crate) multi: bool,
    pub(crate) stored: Option<bool>,
    pub(crate) searchable: Option<bool>,
    pub(crate) filterable: Option<bool>,
    pub(crate) sortable: Option<bool>,
    pub(crate) facetable: Option<bool>,
    pub(crate) analyzer: Option<Analyzer>,
    pub(crate) distance_metric: Option<DistanceMetric>,
    pub(crate) hnsw: Option<HnswIndexConfig>,
    pub data_type: searchkernel_ir::DataTypeIr,
    pub analyzer_config: Option<searchkernel_ir::AnalyzerIr>,
    pub capabilities: FieldCapabilities,
    pub matching_config: Option<searchkernel_ir::MatchingConfigIr>,
    pub index_config: Option<searchkernel_ir::IndexConfigIr>,
    pub attribute_config: Option<searchkernel_ir::AttributeConfigIr>,
    pub summary_config: Option<searchkernel_ir::SummaryConfigIr>,
    pub indexing_script: Option<String>,
    pub properties: searchkernel_ir::SchemaPropertiesIr,
    pub struct_fields: Vec<searchkernel_ir::StructFieldConfigIr>,
    // Described values carry the complete canonical members above. This bit
    // records provenance only, so ordinary builder values still receive the
    // resolver's defaults without acting as a second schema representation.
    pub(crate) materialized: bool,
}

#[derive(Clone, Debug, PartialEq)]
/// A Vespa rank profile exposed by the native schema compiler.
pub struct RankProfile {
    pub name: String,
    pub first_phase: Option<searchkernel_ir::RankPhaseDefinitionIr>,
    pub second_phase: Option<searchkernel_ir::RankPhaseDefinitionIr>,
    pub global_phase: Option<searchkernel_ir::RankPhaseDefinitionIr>,
    pub inherits: Vec<String>,
    pub inputs: Vec<searchkernel_ir::RankProfileInputIr>,
    pub functions: Vec<searchkernel_ir::RankFunctionDefinitionIr>,
    pub rank_features: Vec<String>,
    pub match_features: Vec<String>,
    pub summary_features: Vec<String>,
    pub sort_features: Vec<String>,
    pub constants: Vec<searchkernel_ir::ConstantDefinitionIr>,
    pub onnx_models: Vec<searchkernel_ir::OnnxModelDefinitionIr>,
    pub rank_properties: BTreeMap<String, String>,
    pub strict: Option<bool>,
    pub properties: searchkernel_ir::SchemaPropertiesIr,
    pub(crate) first_phase_expression: Option<String>,
    pub(crate) second_phase_expression: Option<String>,
    pub(crate) rerank_count: Option<u32>,
    pub(crate) query_input_specs: Vec<(String, String)>,
    pub(crate) materialized: bool,
}

impl RankProfile {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            first_phase: None,
            second_phase: None,
            global_phase: None,
            inherits: Vec::new(),
            inputs: Vec::new(),
            functions: Vec::new(),
            rank_features: Vec::new(),
            match_features: Vec::new(),
            summary_features: Vec::new(),
            sort_features: Vec::new(),
            constants: Vec::new(),
            onnx_models: Vec::new(),
            rank_properties: BTreeMap::new(),
            strict: None,
            properties: BTreeMap::new(),
            first_phase_expression: None,
            second_phase_expression: None,
            rerank_count: None,
            query_input_specs: Vec::new(),
            materialized: false,
        }
    }

    pub fn first_phase(mut self, expression: impl Into<String>) -> Self {
        let expression = expression.into();
        self.first_phase_expression = Some(expression.clone());
        self.first_phase
            .get_or_insert_with(empty_rank_phase)
            .expression = Some(expression);
        self
    }

    /// Set the Vespa second-phase ranking expression.
    pub fn second_phase(mut self, expression: impl Into<String>) -> Self {
        let expression = expression.into();
        self.second_phase_expression = Some(expression.clone());
        self.second_phase
            .get_or_insert_with(empty_rank_phase)
            .expression = Some(expression);
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
        self.query_input_specs
            .push((name.clone(), type_spec.clone()));
        self.inputs.push(searchkernel_ir::RankProfileInputIr {
            name,
            data_type: searchkernel_ir::DataTypeIr::Tensor {
                type_spec: type_spec.clone(),
            },
            default_value: searchkernel_ir::RequestValueIr::Tensor(
                searchkernel_ir::RequestTensorIr {
                    type_spec,
                    payload: searchkernel_ir::RequestTensorPayloadIr::Dense(Vec::new()),
                },
            ),
        });
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
            multi: false,
            stored: None,
            searchable: None,
            filterable: None,
            sortable: None,
            facetable: None,
            analyzer: None,
            distance_metric: None,
            hnsw: None,
            data_type,
            analyzer_config: None,
            capabilities: FieldCapabilities::default(),
            matching_config: None,
            index_config: None,
            attribute_config: None,
            summary_config: None,
            indexing_script: None,
            properties: BTreeMap::new(),
            struct_fields: Vec::new(),
            materialized: false,
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
            multi: false,
            stored: None,
            searchable: None,
            filterable: None,
            sortable: None,
            facetable: None,
            analyzer: None,
            distance_metric: None,
            hnsw: None,
            data_type: searchkernel_ir::DataTypeIr::Tensor { type_spec },
            analyzer_config: None,
            capabilities: FieldCapabilities::default(),
            matching_config: None,
            index_config: None,
            attribute_config: None,
            summary_config: None,
            indexing_script: None,
            properties: BTreeMap::new(),
            struct_fields: Vec::new(),
            materialized: false,
        }
    }

    /// Make this field contain a collection of values.
    pub fn multi(mut self) -> Self {
        self.multi = true;
        if !matches!(
            self.data_type,
            searchkernel_ir::DataTypeIr::Array(_) | searchkernel_ir::DataTypeIr::WeightedSet(_)
        ) {
            self.data_type = searchkernel_ir::DataTypeIr::Array(Box::new(self.data_type));
        }
        self
    }
    /// Explicitly set whether values are stored in search results.
    pub fn stored(mut self, value: bool) -> Self {
        self.stored = Some(value);
        self.capabilities.stored = value;
        self
    }
    /// Explicitly set whether this field can be searched with text queries.
    pub fn searchable(mut self, value: bool) -> Self {
        self.searchable = Some(value);
        self.capabilities.searchable = value;
        self
    }
    /// Explicitly set whether this field can be used in filters.
    pub fn filterable(mut self, value: bool) -> Self {
        self.filterable = Some(value);
        self.capabilities.filterable = value;
        self
    }
    /// Explicitly set whether this field can be used for sorting.
    pub fn sortable(mut self, value: bool) -> Self {
        self.sortable = Some(value);
        self.capabilities.sortable = value;
        self
    }
    /// Explicitly set whether this field can be used for facets.
    pub fn facetable(mut self, value: bool) -> Self {
        self.facetable = Some(value);
        self.capabilities.groupable = value;
        self
    }
    /// Select the analyzer used for this field.
    pub fn analyzer(mut self, value: Analyzer) -> Self {
        self.analyzer_config = analyzer_to_ir(&value).ok();
        self.analyzer = Some(value);
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
        index.hnsw = Some(hnsw);
        self
    }

    /// Return this field's name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Return whether this field stores multiple values.
    pub fn is_multi(&self) -> bool {
        self.multi
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
}

#[derive(Clone, Debug, PartialEq)]
/// A field selected for text search, optionally with a weight.
pub struct SearchField {
    pub field: String,
    pub weight: Option<f64>,
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

#[derive(Clone, PartialEq)]
/// The schema and default search settings for an index.
pub struct IndexDefinition {
    pub name: Option<String>,
    pub fields: Vec<Field>,
    pub default_search_fields: Vec<SearchField>,
    pub default_ranking: searchkernel_ir::RankingPolicyIr,
    pub default_prefix_policy: searchkernel_ir::PrefixPolicy,
    pub default_typo_policy: searchkernel_ir::TypoPolicy,
    pub properties: searchkernel_ir::SchemaPropertiesIr,
    pub document_type: Option<String>,
    pub schema_inherits: Vec<String>,
    pub document_inherits: Vec<String>,
    pub structs: Vec<searchkernel_ir::NamedStructDefinitionIr>,
    pub fieldsets: Vec<searchkernel_ir::FieldSetDefinitionIr>,
    pub schema_fields: Vec<Field>,
    pub imported_fields: Vec<searchkernel_ir::ImportedFieldDefinitionIr>,
    pub document_summaries: Vec<searchkernel_ir::DocumentSummaryDefinitionIr>,
    pub rank_profiles: Vec<RankProfile>,
    pub constants: Vec<searchkernel_ir::ConstantDefinitionIr>,
    pub onnx_models: Vec<searchkernel_ir::OnnxModelDefinitionIr>,
    pub(crate) configured_default_search_fields: Option<Vec<SearchField>>,
    pub(crate) materialized: bool,
}

impl Default for IndexDefinition {
    fn default() -> Self {
        Self {
            name: None,
            fields: Vec::new(),
            default_search_fields: Vec::new(),
            default_ranking: searchkernel_ir::RankingPolicyIr::DefaultRelevanceV1,
            default_prefix_policy: searchkernel_ir::PrefixPolicy::Disabled,
            default_typo_policy: searchkernel_ir::TypoPolicy::Disabled,
            properties: BTreeMap::new(),
            document_type: None,
            schema_inherits: Vec::new(),
            document_inherits: Vec::new(),
            structs: Vec::new(),
            fieldsets: Vec::new(),
            schema_fields: Vec::new(),
            imported_fields: Vec::new(),
            document_summaries: Vec::new(),
            rank_profiles: Vec::new(),
            constants: Vec::new(),
            onnx_models: Vec::new(),
            configured_default_search_fields: None,
            materialized: false,
        }
    }
}

impl fmt::Debug for IndexDefinition {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IndexDefinition")
            .field("name", &self.name)
            .field("fields", &self.fields)
            .field("default_search_fields", &self.default_search_fields)
            .field("rank_profiles", &self.rank_profiles)
            .field("properties", &self.properties)
            .field("document_type", &self.document_type)
            .field("schema_inherits", &self.schema_inherits)
            .field("document_inherits", &self.document_inherits)
            .field("structs", &self.structs)
            .field("fieldsets", &self.fieldsets)
            .field("schema_fields", &self.schema_fields)
            .field("imported_fields", &self.imported_fields)
            .field("document_summaries", &self.document_summaries)
            .field("constants", &self.constants)
            .field("onnx_models", &self.onnx_models)
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
        self.name = Some(value.into());
        self
    }
    /// Add one field to the definition.
    pub fn field(mut self, value: Field) -> Self {
        self.fields.push(value);
        self
    }
    /// Set the fields used when a search does not specify its own fields.
    pub fn default_search_fields<I>(mut self, fields: I) -> Self
    where
        I: IntoIterator,
        I::Item: Into<SearchField>,
    {
        let fields = fields.into_iter().map(Into::into).collect::<Vec<_>>();
        self.default_search_fields = fields.clone();
        self.configured_default_search_fields = Some(fields);
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

    /// Return the explicitly configured default search fields.
    pub fn configured_default_search_fields(&self) -> Option<&[SearchField]> {
        self.configured_default_search_fields.as_deref()
    }

    pub fn rank_profiles(&self) -> &[RankProfile] {
        &self.rank_profiles
    }
}

fn empty_rank_phase() -> searchkernel_ir::RankPhaseDefinitionIr {
    searchkernel_ir::RankPhaseDefinitionIr {
        expression: None,
        rerank_count: None,
        rank_score_drop_limit: None,
        properties: BTreeMap::new(),
        total_rerank_count: None,
        keep_rank_count: None,
        total_keep_rank_count: None,
    }
}

fn analyzer_to_ir(value: &Analyzer) -> Result<searchkernel_ir::AnalyzerIr> {
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

fn data_type_from_kind(kind: &FieldKind) -> searchkernel_ir::DataTypeIr {
    match kind {
        FieldKind::Text | FieldKind::Keyword => {
            searchkernel_ir::DataTypeIr::Primitive(searchkernel_ir::PrimitiveDataTypeIr::String)
        }
        FieldKind::Int64 => {
            searchkernel_ir::DataTypeIr::Primitive(searchkernel_ir::PrimitiveDataTypeIr::Int64)
        }
        FieldKind::Float64 => {
            searchkernel_ir::DataTypeIr::Primitive(searchkernel_ir::PrimitiveDataTypeIr::Float64)
        }
        FieldKind::Bool => {
            searchkernel_ir::DataTypeIr::Primitive(searchkernel_ir::PrimitiveDataTypeIr::Bool)
        }
        FieldKind::Position => searchkernel_ir::DataTypeIr::Position,
        FieldKind::Tensor(type_spec) => searchkernel_ir::DataTypeIr::Tensor {
            type_spec: type_spec.clone(),
        },
    }
}

pub(crate) fn field_from_ir(field: &searchkernel_ir::FieldIr) -> Field {
    let mut value = match field.kind() {
        searchkernel_ir::FieldKind::Text => Field::text(field.name().as_str()),
        searchkernel_ir::FieldKind::Keyword => Field::keyword(field.name().as_str()),
        searchkernel_ir::FieldKind::Int64 => Field::int64(field.name().as_str()),
        searchkernel_ir::FieldKind::Float64 => Field::float64(field.name().as_str()),
        searchkernel_ir::FieldKind::Bool => Field::bool(field.name().as_str()),
    };
    if matches!(field.data_type(), searchkernel_ir::DataTypeIr::Position) {
        value = Field::position(field.name().as_str());
    }
    if let searchkernel_ir::DataTypeIr::Tensor { type_spec } = field.data_type() {
        value = Field::tensor(field.name().as_str(), type_spec);
    }
    value.data_type = field.data_type().clone();
    value.multi = matches!(
        field.data_type(),
        searchkernel_ir::DataTypeIr::Array(_) | searchkernel_ir::DataTypeIr::WeightedSet(_)
    );
    let capabilities = field.capabilities();
    value.capabilities = FieldCapabilities {
        stored: capabilities.stored(),
        searchable: capabilities.searchable(),
        filterable: capabilities.filterable(),
        sortable: capabilities.sortable(),
        groupable: capabilities.groupable(),
    };
    value.stored = Some(capabilities.stored());
    value.searchable = Some(capabilities.searchable());
    value.filterable = Some(capabilities.filterable());
    value.sortable = Some(capabilities.sortable());
    value.facetable = Some(capabilities.groupable());
    value.analyzer_config = field.analyzer().cloned();
    value.matching_config = field.matching().cloned();
    value.index_config = field.index().cloned();
    value.attribute_config = field.attribute().cloned();
    value.summary_config = field.summary().cloned();
    value.indexing_script = field.indexing_script().map(str::to_owned);
    value.properties = field.properties().clone();
    value.struct_fields = field.struct_fields().to_vec();
    value.analyzer = field.analyzer().and_then(|analyzer| {
        (analyzer.name == "standard" && analyzer.version == 1 && analyzer.options.is_empty())
            .then(Analyzer::standard_v1)
    });
    if let Some(attribute) = field.attribute() {
        value.distance_metric = match attribute.distance_metric {
            searchkernel_ir::DistanceMetricIr::Unspecified => None,
            searchkernel_ir::DistanceMetricIr::Euclidean => Some(DistanceMetric::Euclidean),
            searchkernel_ir::DistanceMetricIr::Angular => Some(DistanceMetric::Angular),
            searchkernel_ir::DistanceMetricIr::DotProduct => Some(DistanceMetric::DotProduct),
            searchkernel_ir::DistanceMetricIr::PrenormalizedAngular => {
                Some(DistanceMetric::PrenormalizedAngular)
            }
            searchkernel_ir::DistanceMetricIr::GeoDegrees => Some(DistanceMetric::GeoDegrees),
            searchkernel_ir::DistanceMetricIr::Hamming => Some(DistanceMetric::Hamming),
        };
    }
    if let Some(config) = field.index().and_then(|value| value.hnsw.as_ref()) {
        let mut hnsw = HnswIndexConfig::new();
        if let Some(value) = config.max_links_per_node {
            hnsw = hnsw.max_links_per_node(value);
        }
        if let Some(value) = config.neighbors_to_explore_at_insert {
            hnsw = hnsw.neighbors_to_explore_at_insert(value);
        }
        value.hnsw = Some(hnsw);
    }
    value.materialized = true;
    value
}

pub(crate) fn field_to_ir(field: &Field) -> Result<searchkernel_ir::FieldIr> {
    let name = searchkernel_ir::FieldName::parse(field.name.clone()).map_err(Error::from_ir)?;
    Ok(searchkernel_ir::FieldIr::from_components(
        name,
        field.data_type.clone(),
        field.analyzer_config.clone(),
        searchkernel_ir::FieldCapabilities::from_values(
            field.capabilities.stored,
            field.capabilities.searchable,
            field.capabilities.filterable,
            field.capabilities.sortable,
            field.capabilities.groupable,
        ),
        field.matching_config.clone(),
        field.index_config.clone(),
        field.attribute_config.clone(),
        field.summary_config.clone(),
        field.indexing_script.clone(),
        field.properties.clone(),
        field.struct_fields.clone(),
    ))
}

pub(crate) fn rank_profile_from_ir(
    profile: &searchkernel_ir::RankProfileDefinitionIr,
) -> RankProfile {
    RankProfile {
        name: profile.name.clone(),
        first_phase: profile.first_phase.clone(),
        second_phase: profile.second_phase.clone(),
        global_phase: profile.global_phase.clone(),
        inherits: profile.inherits.clone(),
        inputs: profile.inputs.clone(),
        functions: profile.functions.clone(),
        rank_features: profile.rank_features.clone(),
        match_features: profile.match_features.clone(),
        summary_features: profile.summary_features.clone(),
        sort_features: profile.sort_features.clone(),
        constants: profile.constants.clone(),
        onnx_models: profile.onnx_models.clone(),
        rank_properties: profile.rank_properties.clone(),
        strict: profile.strict,
        properties: profile.properties.clone(),
        first_phase_expression: profile
            .first_phase
            .as_ref()
            .and_then(|phase| phase.expression.clone()),
        second_phase_expression: profile
            .second_phase
            .as_ref()
            .and_then(|phase| phase.expression.clone()),
        rerank_count: profile
            .second_phase
            .as_ref()
            .and_then(|phase| phase.rerank_count),
        query_input_specs: profile
            .inputs
            .iter()
            .filter_map(|input| match &input.data_type {
                searchkernel_ir::DataTypeIr::Tensor { type_spec } => {
                    Some((input.name.clone(), type_spec.clone()))
                }
                _ => None,
            })
            .collect(),
        materialized: true,
    }
}

pub(crate) fn rank_profile_to_ir(
    profile: &RankProfile,
) -> searchkernel_ir::RankProfileDefinitionIr {
    searchkernel_ir::RankProfileDefinitionIr {
        name: profile.name.clone(),
        inherits: profile.inherits.clone(),
        inputs: profile.inputs.clone(),
        functions: profile.functions.clone(),
        first_phase: profile.first_phase.clone(),
        second_phase: profile.second_phase.clone(),
        global_phase: profile.global_phase.clone(),
        rank_features: profile.rank_features.clone(),
        match_features: profile.match_features.clone(),
        summary_features: profile.summary_features.clone(),
        sort_features: profile.sort_features.clone(),
        constants: profile.constants.clone(),
        onnx_models: profile.onnx_models.clone(),
        rank_properties: profile.rank_properties.clone(),
        strict: profile.strict,
        properties: profile.properties.clone(),
    }
}

pub(crate) fn field_to_rust_input(field: &Field) -> Result<searchkernel_ir::RustFieldInput> {
    let analyzer = field
        .analyzer
        .as_ref()
        .map(|analyzer| {
            analyzer
                .options
                .iter()
                .map(|(key, value)| {
                    searchkernel_ir::request_value_from_json(
                        value,
                        format!("fields.{}.analyzer.options.{key}", field.name),
                    )
                    .map(|value| (key.clone(), value))
                    .map_err(Error::from_ir)
                })
                .collect::<Result<searchkernel_ir::SchemaPropertiesIr>>()
                .map(|options| searchkernel_ir::RustAnalyzerInput {
                    name: analyzer.name.clone(),
                    version: analyzer.version,
                    options,
                })
        })
        .transpose()?;
    Ok(searchkernel_ir::RustFieldInput {
        name: field.name.clone(),
        kind: match &field.kind {
            FieldKind::Text => searchkernel_ir::RustFieldKind::Text,
            FieldKind::Keyword => searchkernel_ir::RustFieldKind::Keyword,
            FieldKind::Int64 => searchkernel_ir::RustFieldKind::Int64,
            FieldKind::Float64 => searchkernel_ir::RustFieldKind::Float64,
            FieldKind::Bool => searchkernel_ir::RustFieldKind::Bool,
            FieldKind::Position => searchkernel_ir::RustFieldKind::Position,
            FieldKind::Tensor(type_spec) => {
                searchkernel_ir::RustFieldKind::Tensor(type_spec.clone())
            }
        },
        multi: field.multi,
        stored: field.stored,
        searchable: field.searchable,
        filterable: field.filterable,
        sortable: field.sortable,
        facetable: field.facetable,
        analyzer,
        distance_metric: field.distance_metric.map(distance_metric_to_ir),
        hnsw: field
            .hnsw
            .as_ref()
            .map(|value| searchkernel_ir::HnswIndexConfigIr {
                max_links_per_node: value.max_links_per_node,
                neighbors_to_explore_at_insert: value.neighbors_to_explore_at_insert,
                options: BTreeMap::new(),
            }),
    })
}

pub(crate) fn rank_profile_to_rust_input(
    profile: &RankProfile,
) -> searchkernel_ir::RustRankProfileInput {
    searchkernel_ir::RustRankProfileInput {
        name: profile.name.clone(),
        first_phase: profile.first_phase_expression.clone().unwrap_or_default(),
        second_phase: profile.second_phase_expression.clone(),
        rerank_count: profile.rerank_count,
        match_features: profile.match_features.clone(),
        query_inputs: profile.query_input_specs.clone(),
    }
}

fn distance_metric_to_ir(value: DistanceMetric) -> searchkernel_ir::DistanceMetricIr {
    match value {
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
    use super::{IndexDefinition, SearchField};

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
    fn default_search_fields_accept_field_names() {
        let definition = IndexDefinition::new().default_search_fields(["title", "description"]);

        let fields = definition.configured_default_search_fields.unwrap();
        assert_eq!(fields[0].field, "title");
        assert_eq!(fields[1].field, "description");
        assert_eq!(
            fields.iter().map(|field| field.weight).collect::<Vec<_>>(),
            [None, None]
        );
    }
}
