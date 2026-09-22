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
    pub data_type: searchkernel_ir::DataTypeIr,
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
    pub(crate) struct_fields: Vec<searchkernel_ir::StructFieldConfigIr>,
    pub summary_config: Option<searchkernel_ir::SummaryConfigIr>,
    pub indexing_script: Option<String>,
    pub properties: searchkernel_ir::SchemaPropertiesIr,
    /// Complete native index configuration returned by `Index::describe`.
    pub index_config: Option<searchkernel_ir::IndexConfigIr>,
    /// Complete native attribute configuration returned by `Index::describe`.
    pub attribute_config: Option<searchkernel_ir::AttributeConfigIr>,
    pub(crate) canonical: Option<searchkernel_ir::FieldIr>,
}

#[derive(Clone, Debug, PartialEq)]
/// A Vespa rank profile exposed by the native schema compiler.
pub struct RankProfile {
    pub(crate) name: String,
    /// The complete first-phase configuration, when one is declared.
    pub first_phase: Option<searchkernel_ir::RankPhaseDefinitionIr>,
    /// The complete second-phase configuration, when one is declared.
    pub second_phase: Option<searchkernel_ir::RankPhaseDefinitionIr>,
    /// The complete global-phase configuration, when one is declared.
    pub global_phase: Option<searchkernel_ir::RankPhaseDefinitionIr>,
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
    pub properties: searchkernel_ir::SchemaPropertiesIr,
    /// Canonical rank-profile inputs exposed by Index::describe().
    pub inputs: Vec<searchkernel_ir::RankProfileInputIr>,
    /// Canonical rank-profile scoped constants.
    pub constants: Vec<searchkernel_ir::ConstantDefinitionIr>,
    /// Canonical rank-profile scoped ONNX models.
    pub onnx_models: Vec<searchkernel_ir::OnnxModelDefinitionIr>,
    pub(crate) query_inputs: Vec<(String, String)>,
    /// Canonical profile inheritance exposed by Index::describe().
    pub inherits: Vec<String>,
    /// Canonical Vespa rank functions exposed by Index::describe().
    pub functions: Vec<searchkernel_ir::RankFunctionDefinitionIr>,
    pub(crate) canonical: Option<searchkernel_ir::RankProfileDefinitionIr>,
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
            canonical: None,
        }
    }

    pub fn first_phase(mut self, expression: impl Into<String>) -> Self {
        let expression = expression.into();
        self.first_phase
            .get_or_insert_with(empty_rank_phase)
            .expression = Some(expression.clone());
        if let Some(profile) = self.canonical.as_mut() {
            profile
                .first_phase
                .get_or_insert_with(empty_rank_phase)
                .expression = Some(expression);
        }
        self
    }

    /// Set the Vespa second-phase ranking expression.
    pub fn second_phase(mut self, expression: impl Into<String>) -> Self {
        let expression = expression.into();
        self.second_phase
            .get_or_insert_with(empty_rank_phase)
            .expression = Some(expression.clone());
        if let Some(profile) = self.canonical.as_mut() {
            profile
                .second_phase
                .get_or_insert_with(empty_rank_phase)
                .expression = Some(expression);
        }
        self
    }

    /// Set the number of first-phase candidates Vespa reranks in second phase.
    pub fn rerank_count(mut self, count: u32) -> Self {
        self.rerank_count = Some(count);
        self.second_phase
            .get_or_insert_with(empty_rank_phase)
            .rerank_count = Some(count);
        if let Some(profile) = self.canonical.as_mut() {
            profile
                .second_phase
                .get_or_insert_with(empty_rank_phase)
                .rerank_count = Some(count);
        }
        self
    }

    pub fn match_feature(mut self, expression: impl Into<String>) -> Self {
        let expression = expression.into();
        self.match_features.push(expression.clone());
        if let Some(profile) = self.canonical.as_mut() {
            profile.match_features.push(expression);
        }
        self
    }

    /// Declare a tensor query input used by this rank profile.
    pub fn query_input(mut self, name: impl Into<String>, type_spec: impl Into<String>) -> Self {
        let name = name.into();
        let type_spec = type_spec.into();
        self.query_inputs.push((name.clone(), type_spec.clone()));
        let input = searchkernel_ir::RankProfileInputIr {
            name,
            data_type: searchkernel_ir::DataTypeIr::Tensor { type_spec: type_spec.clone() },
            default_value: searchkernel_ir::RequestValueIr::Tensor(searchkernel_ir::RequestTensorIr {
                type_spec,
                payload: searchkernel_ir::RequestTensorPayloadIr::Dense(Vec::new()),
            }),
        };
        self.inputs.push(input.clone());
        if let Some(profile) = self.canonical.as_mut() { profile.inputs.push(input); }
        self
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// Return the complete canonical rank-profile state represented by this
    /// public value, when it originated from `Index::describe()`.
    pub fn canonical_definition(&self) -> Option<&searchkernel_ir::RankProfileDefinitionIr> {
        self.canonical.as_ref()
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
            canonical: None,
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
            data_type: searchkernel_ir::DataTypeIr::Tensor { type_spec },
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
            canonical: None,
        }
    }

    /// Make this field contain a collection of values.
    pub fn multi(mut self) -> Self {
        self.multi = true;
        if !matches!(
            &self.data_type,
            searchkernel_ir::DataTypeIr::Array(_) | searchkernel_ir::DataTypeIr::WeightedSet(_)
        ) {
            self.data_type = searchkernel_ir::DataTypeIr::Array(Box::new(self.data_type));
        }
        if let Some(field) = self.canonical.as_mut() {
            field.set_multi(true);
        }
        self
    }
    /// Explicitly set whether values are stored in search results.
    pub fn stored(mut self, value: bool) -> Self {
        self.stored = Some(value);
        if let Some(summary) = self.summary_config.as_mut() {
            summary.enabled = Some(value);
        }
        if let Some(capabilities) = self.capabilities.as_mut() {
            capabilities.stored = value;
        }
        if let Some(field) = self.canonical.as_mut() {
            let capabilities = field.capabilities();
            field.set_capabilities(
                value,
                capabilities.searchable(),
                capabilities.filterable(),
                capabilities.sortable(),
                capabilities.groupable(),
            );
            if let Some(mut summary) = field.summary().cloned() {
                summary.enabled = Some(value);
                field.set_summary_config(Some(summary));
            }
        }
        self
    }
    /// Explicitly set whether this field can be searched with text queries.
    pub fn searchable(mut self, value: bool) -> Self {
        self.searchable = Some(value);
        if let Some(capabilities) = self.capabilities.as_mut() {
            capabilities.searchable = value;
        }
        if let Some(field) = self.canonical.as_mut() {
            let capabilities = field.capabilities();
            field.set_capabilities(
                capabilities.stored(),
                value,
                capabilities.filterable(),
                capabilities.sortable(),
                capabilities.groupable(),
            );
        }
        self
    }
    /// Explicitly set whether this field can be used in filters.
    pub fn filterable(mut self, value: bool) -> Self {
        self.filterable = Some(value);
        if let Some(capabilities) = self.capabilities.as_mut() {
            capabilities.filterable = value;
        }
        if let Some(field) = self.canonical.as_mut() {
            let capabilities = field.capabilities();
            field.set_capabilities(
                capabilities.stored(),
                capabilities.searchable(),
                value,
                capabilities.sortable(),
                capabilities.groupable(),
            );
        }
        self
    }
    /// Explicitly set whether this field can be used for sorting.
    pub fn sortable(mut self, value: bool) -> Self {
        self.sortable = Some(value);
        if let Some(capabilities) = self.capabilities.as_mut() {
            capabilities.sortable = value;
        }
        if let Some(field) = self.canonical.as_mut() {
            let capabilities = field.capabilities();
            field.set_capabilities(
                capabilities.stored(),
                capabilities.searchable(),
                capabilities.filterable(),
                value,
                capabilities.groupable(),
            );
        }
        self
    }
    /// Explicitly set whether this field can be used for facets.
    pub fn facetable(mut self, value: bool) -> Self {
        self.facetable = Some(value);
        if let Some(capabilities) = self.capabilities.as_mut() {
            capabilities.groupable = value;
        }
        if let Some(field) = self.canonical.as_mut() {
            let capabilities = field.capabilities();
            field.set_capabilities(
                capabilities.stored(),
                capabilities.searchable(),
                capabilities.filterable(),
                capabilities.sortable(),
                value,
            );
        }
        self
    }
    /// Select the analyzer used for this field.
    pub fn analyzer(mut self, value: Analyzer) -> Self {
        let canonical = analyzer_to_ir(&value).ok();
        self.analyzer_config = canonical.clone();
        self.analyzer = Some(value);
        if let Some(field) = self.canonical.as_mut() {
            if let Some(analyzer) = canonical {
                field.set_analyzer(Some(analyzer));
            } else {
                self.canonical = None;
            }
        }
        self
    }

    /// Set the canonical matching configuration for this field.
    pub fn matching_config(mut self, value: searchkernel_ir::MatchingConfigIr) -> Self {
        self.matching_config = Some(value.clone());
        if let Some(field) = self.canonical.as_mut() {
            field.set_matching_config(Some(value));
        }
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
        if let Some(field) = self.canonical.as_mut() {
            field.set_distance_metric(distance_metric_to_ir(value));
        }
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
        if let Some(field) = self.canonical.as_mut() {
            field.set_hnsw(hnsw);
        }
        self
    }

    /// Return this field's name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Return the complete canonical data type of this field.
    pub fn data_type(&self) -> &searchkernel_ir::DataTypeIr {
        &self.data_type
    }

    /// Return whether this field stores multiple values.
    pub fn is_multi(&self) -> bool {
        matches!(
            &self.data_type,
            searchkernel_ir::DataTypeIr::Array(_) | searchkernel_ir::DataTypeIr::WeightedSet(_)
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
    pub fn analyzer_config(&self) -> Option<&searchkernel_ir::AnalyzerIr> {
        self.analyzer_config.as_ref()
    }

    /// Return the complete matching configuration, including arbitrary options.
    pub fn matching(&self) -> Option<&searchkernel_ir::MatchingConfigIr> {
        self.matching_config.as_ref()
    }

    pub fn struct_fields(&self) -> &[searchkernel_ir::StructFieldConfigIr] {
        &self.struct_fields
    }

    /// Return the complete canonical field state represented by this public
    /// value, when it originated from `Index::describe()`.
    pub fn canonical_definition(&self) -> Option<&searchkernel_ir::FieldIr> {
        self.canonical.as_ref()
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
    Literal(searchkernel_ir::RequestValueIr),
    File { path: String },
}

/// A schema constant exposed by `Index::describe()`.
///
/// Constants use the IR value and data-type domains directly because those
/// domains are already the canonical, lossless representation. Keeping these
/// members on the public value lets Rust-to-IR conversion consume the same
/// state that callers inspect, rather than recovering it from a hidden parent
/// definition.
#[derive(Clone, Debug, PartialEq)]
pub struct ConstantDefinition {
    name: String,
    data_type: searchkernel_ir::DataTypeIr,
    source: ConstantSource,
    properties: searchkernel_ir::SchemaPropertiesIr,
}

impl ConstantDefinition {
    /// Construct a schema constant from its complete canonical members.
    pub fn new(
        name: impl Into<String>,
        data_type: searchkernel_ir::DataTypeIr,
        source: ConstantSource,
    ) -> Self {
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
                ConstantSource::Literal(value.clone())
            }
            searchkernel_ir::ConstantSourceIr::File { path } => {
                ConstantSource::File { path: path.clone() }
            }
        };
        Self {
            name: value.name.clone(),
            data_type: value.data_type.clone(),
            source,
            properties: value.properties.clone(),
        }
    }

    pub(crate) fn to_ir(&self) -> searchkernel_ir::ConstantDefinitionIr {
        let source = match &self.source {
            ConstantSource::Literal(value) => {
                searchkernel_ir::ConstantSourceIr::Literal(value.clone())
            }
            ConstantSource::File { path } => {
                searchkernel_ir::ConstantSourceIr::File { path: path.clone() }
            }
        };
        searchkernel_ir::ConstantDefinitionIr {
            name: self.name.clone(),
            data_type: self.data_type.clone(),
            source,
            properties: self.properties.clone(),
        }
    }

    /// Return the canonical constant name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Return the canonical constant data type.
    pub fn data_type(&self) -> &searchkernel_ir::DataTypeIr {
        &self.data_type
    }

    /// Return the canonical constant source.
    pub fn source(&self) -> &ConstantSource {
        &self.source
    }

    /// Return the canonical constant properties.
    pub fn properties(&self) -> &searchkernel_ir::SchemaPropertiesIr {
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
    pub(crate) document_summaries: Vec<searchkernel_ir::DocumentSummaryDefinitionIr>,
    pub(crate) rank_profiles: Vec<RankProfile>,
    pub(crate) constants: Vec<ConstantDefinition>,
    pub(crate) fieldsets: Vec<searchkernel_ir::FieldSetDefinitionIr>,
    pub(crate) onnx_models: Vec<searchkernel_ir::OnnxModelDefinitionIr>,
    pub(crate) imported_fields: Vec<searchkernel_ir::ImportedFieldDefinitionIr>,
    pub(crate) structs: Vec<searchkernel_ir::NamedStructDefinitionIr>,
    pub(crate) schema_fields: Vec<Field>,
    pub(crate) properties: searchkernel_ir::SchemaPropertiesIr,
    // For values returned by Index::describe(), this is the authoritative
    // canonical schema state. The ergonomic members above are projections for
    // the common builder path; canonical_definition() makes the complete
    // state observable instead of silently narrowing it at the Rust boundary.
    pub(crate) canonical: Option<searchkernel_ir::IndexDefinitionIr>,
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
        if self.canonical.is_some() {
            let canonical_name = searchkernel_ir::IndexName::parse(value.clone()).ok();
            apply_canonical_mutation(&mut self.canonical, |canonical| {
                let Some(name) = canonical_name else {
                    return false;
                };
                canonical.set_name(Some(name));
                true
            });
        }
        self.name = Some(value);
        self
    }
    /// Add one field to the definition.
    pub fn field(mut self, value: Field) -> Self {
        if self.canonical.is_some() {
            let canonical_field = value.canonical.clone().or_else(|| {
                field_to_rust_input(&value)
                    .and_then(|field| {
                        searchkernel_ir::resolve_rust_index_definition(
                            searchkernel_ir::RustIndexDefinitionInput {
                                name: None,
                                fields: vec![field],
                                default_search_fields: None,
                            },
                        )
                        .map_err(Error::from_ir)
                    })
                    .ok()
                    .and_then(|definition| definition.fields().first().cloned())
            });
            if let Some(canonical_field) = canonical_field {
                apply_canonical_mutation(&mut self.canonical, |canonical| {
                    canonical.push_field(canonical_field)
                });
            } else {
                self.canonical = None;
            }
        }
        self.fields.push(value);
        self
    }

    /// Add a schema-only field derived by an indexing expression.
    pub fn schema_field(mut self, value: Field) -> Self {
        if self.canonical.is_some() {
            let canonical_field = value.canonical.clone().or_else(|| {
                field_to_rust_input(&value)
                    .and_then(|field| {
                        searchkernel_ir::resolve_rust_index_definition(
                            searchkernel_ir::RustIndexDefinitionInput {
                                name: None,
                                fields: vec![field],
                                default_search_fields: None,
                            },
                        )
                        .map_err(Error::from_ir)
                    })
                    .ok()
                    .and_then(|definition| definition.fields().first().cloned())
            });
            if let Some(canonical_field) = canonical_field {
                apply_canonical_mutation(&mut self.canonical, |canonical| {
                    let duplicate = canonical
                        .fields()
                        .iter()
                        .chain(canonical.schema_fields().iter())
                        .any(|field| field.name() == canonical_field.name());
                    if duplicate {
                        return false;
                    }
                    let mut schema_fields = canonical.schema_fields().to_vec();
                    schema_fields.push(canonical_field);
                    canonical.set_schema_fields(schema_fields);
                    true
                });
            } else {
                self.canonical = None;
            }
        }
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
        if self.canonical.is_some() {
            let canonical_fields = self
                .canonical
                .as_ref()
                .and_then(|canonical| canonical_search_fields(canonical, &fields));
            if let Some(canonical_fields) = canonical_fields {
                apply_canonical_mutation(&mut self.canonical, |canonical| {
                    canonical.set_default_search_fields(canonical_fields);
                    true
                });
            } else {
                self.canonical = None;
            }
        }
        self.default_search_fields = Some(fields);
        self
    }

    /// Add a native first-phase rank profile to the generated schema.
    pub fn rank_profile(mut self, profile: RankProfile) -> Self {
        if self.canonical.is_some() {
            let canonical_profile = profile.canonical.clone().or_else(|| {
                let rust_profile = rank_profile_to_rust_input(&profile);
                let placeholder = searchkernel_ir::RustFieldInput {
                    name: "rank_profile_placeholder".to_owned(),
                    kind: searchkernel_ir::RustFieldKind::Keyword,
                    multi: false,
                    stored: None,
                    searchable: None,
                    filterable: None,
                    sortable: None,
                    facetable: None,
                    analyzer: None,
                    distance_metric: None,
                    hnsw: None,
                };
                let canonical_profile =
                    searchkernel_ir::resolve_rust_index_definition_with_profiles(
                        searchkernel_ir::RustIndexDefinitionInput {
                            name: None,
                            fields: vec![placeholder],
                            default_search_fields: None,
                        },
                        vec![rust_profile],
                    )
                    .map_err(Error::from_ir)
                    .ok()
                    .and_then(|definition| definition.rank_profiles().first().cloned());
                canonical_profile
            });
            if let Some(canonical_profile) = canonical_profile {
                apply_canonical_mutation(&mut self.canonical, |canonical| {
                    canonical.push_rank_profile(canonical_profile)
                });
            } else {
                self.canonical = None;
            }
        }
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
    pub fn document_summaries(&self) -> &[searchkernel_ir::DocumentSummaryDefinitionIr] {
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
    pub fn fieldsets(&self) -> &[searchkernel_ir::FieldSetDefinitionIr] {
        &self.fieldsets
    }

    /// Return the top-level ONNX model definitions declared by the schema.
    pub fn onnx_models(&self) -> &[searchkernel_ir::OnnxModelDefinitionIr] {
        &self.onnx_models
    }

    /// Return the imported-field definitions declared by the schema.
    pub fn imported_fields(&self) -> &[searchkernel_ir::ImportedFieldDefinitionIr] {
        &self.imported_fields
    }

    /// Return the top-level named struct definitions declared by the schema.
    pub fn structs(&self) -> &[searchkernel_ir::NamedStructDefinitionIr] {
        &self.structs
    }

    /// Return the top-level properties declared by the schema.
    pub fn properties(&self) -> &searchkernel_ir::SchemaPropertiesIr {
        &self.properties
    }

    /// Add a schema constant to this definition.
    pub fn constant(mut self, value: ConstantDefinition) -> Self {
        if self.canonical.is_some() {
            let constant = value.to_ir();
            apply_canonical_mutation(&mut self.canonical, |canonical| {
                let mut constants = canonical.constants().to_vec();
                constants.push(constant);
                canonical.set_constants(constants);
                true
            });
        }
        self.constants.push(value);
        self
    }

    /// Return the complete canonical schema state represented by this public
    /// value, when it originated from `Index::describe()`.
    pub fn canonical_definition(&self) -> Option<&searchkernel_ir::IndexDefinitionIr> {
        self.canonical.as_ref()
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

fn apply_canonical_mutation(
    canonical: &mut Option<searchkernel_ir::IndexDefinitionIr>,
    mutation: impl FnOnce(&mut searchkernel_ir::IndexDefinitionIr) -> bool,
) {
    let Some(mut value) = canonical.take() else {
        return;
    };
    if mutation(&mut value) {
        *canonical = Some(value);
    }
}

fn canonical_search_fields(
    canonical: &searchkernel_ir::IndexDefinitionIr,
    fields: &[SearchField],
) -> Option<Vec<searchkernel_ir::SearchFieldIr>> {
    fields
        .iter()
        .map(|field| {
            let field_name = searchkernel_ir::FieldName::parse(field.field.clone()).ok()?;
            let resolved_field = canonical
                .fields()
                .iter()
                .find(|value| value.name() == &field_name)?;
            if !resolved_field.capabilities().searchable() {
                return None;
            }
            let weight =
                searchkernel_ir::SearchFieldWeight::new(field.weight.unwrap_or(1.0)).ok()?;
            Some(searchkernel_ir::SearchFieldIr::new(field_name, weight))
        })
        .collect()
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

pub(crate) fn field_to_unresolved_input(
    field: &Field,
    path: &str,
) -> Result<searchkernel_ir::UnresolvedFieldInput> {
    validate_public_field_data_type(field, path)?;
    let name = searchkernel_ir::FieldName::parse(field.name.clone()).map_err(|_| {
        Error::from_ir(searchkernel_ir::PortableError::invalid(
            "invalid_field_name",
            Some(format!("fields.{}.name", field.name)),
            "field name must match [A-Za-z][A-Za-z0-9_]*",
        ))
    })?;
    // Origin is a facade-local structural concern only. Described values
    // already expose their complete analyzer/matching/index/attribute state,
    // while fresh ergonomic builders need those structural shorthands decoded.
    // Do not carry this provenance into searchkernel-ir semantic resolution.
    let described = field.canonical.is_some() || field.capabilities.is_some();
    let analyzer = match &field.analyzer_config {
        Some(analyzer) => Some(analyzer.clone()),
        None if !described => field.analyzer.as_ref().map(analyzer_to_ir).transpose()?,
        None => None,
    };
    let matching = field.matching_config.clone().or_else(|| {
        if described {
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
        if described {
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
        if described {
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
    // Described indexing-script fields expose resolved capability defaults as
    // public state. Omit only those defaults before shared validation; any
    // caller mutation away from the script semantics remains visible and is
    // rejected by the resolver.
    let indexing_script = field.indexing_script.clone();
    let script_defaults = indexing_script.is_some();
    let stored = omit_script_default(field.stored, script_defaults, true);
    let searchable = omit_script_default(field.searchable, script_defaults, false);
    let filterable = omit_script_default(field.filterable, script_defaults, false);
    let sortable = omit_script_default(field.sortable, script_defaults, false);
    let groupable = omit_script_default(field.facetable, script_defaults, false);
    Ok(searchkernel_ir::UnresolvedFieldInput {
        name,
        data_type: field.data_type.clone(),
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
        indexing_script,
        properties: field.properties.clone(),
        struct_fields: field.struct_fields.clone(),
    })
}

fn omit_script_default(value: Option<bool>, script_defaults: bool, default: bool) -> Option<bool> {
    if script_defaults && value == Some(default) {
        None
    } else {
        value
    }
}

// FieldKind is the declaration retained by the ergonomic Rust facade, while
// data_type is public rich state. The shared IR resolver only receives the
// latter, so validate their consistency before the facade declaration is
// intentionally lowered into the common unresolved input.
fn validate_public_field_data_type(field: &Field, path: &str) -> Result<()> {
    let data_type = match &field.data_type {
        searchkernel_ir::DataTypeIr::Array(value)
        | searchkernel_ir::DataTypeIr::WeightedSet(value) => value.as_ref(),
        value => value,
    };
    let matches_kind = match (&field.kind, data_type) {
        (
            FieldKind::Text | FieldKind::Keyword,
            searchkernel_ir::DataTypeIr::Primitive(searchkernel_ir::PrimitiveDataTypeIr::String),
        )
        | (
            FieldKind::Int64,
            searchkernel_ir::DataTypeIr::Primitive(searchkernel_ir::PrimitiveDataTypeIr::Int64),
        )
        | (
            FieldKind::Float64,
            searchkernel_ir::DataTypeIr::Primitive(searchkernel_ir::PrimitiveDataTypeIr::Float64),
        )
        | (
            FieldKind::Bool,
            searchkernel_ir::DataTypeIr::Primitive(searchkernel_ir::PrimitiveDataTypeIr::Bool),
        )
        | (FieldKind::Position, searchkernel_ir::DataTypeIr::Position) => true,
        (FieldKind::Tensor(expected_type), searchkernel_ir::DataTypeIr::Tensor { type_spec }) => {
            expected_type == type_spec
        }
        // Index::describe() can expose document types that the ergonomic
        // FieldKind enum does not model. Preserve those opaque described
        // types, but never let a supported scalar mismatch through.
        (FieldKind::Keyword, _) if field.capabilities.is_some() => {
            is_opaque_public_data_type(data_type)
        }
        _ => false,
    };
    if !matches_kind {
        return Err(Error::from_ir(searchkernel_ir::PortableError::invalid(
            "invalid_type_combination",
            Some(format!("{path}.data_type")),
            "the public field data type must match its declared field kind",
        )));
    }
    let is_multi = matches!(
        field.data_type,
        searchkernel_ir::DataTypeIr::Array(_) | searchkernel_ir::DataTypeIr::WeightedSet(_)
    );
    if is_multi != field.multi {
        return Err(Error::from_ir(searchkernel_ir::PortableError::invalid(
            "invalid_type_combination",
            Some(format!("{path}.data_type")),
            "the public field collection shape must match its declared cardinality",
        )));
    }
    if let Some(matching) = &field.matching_config {
        let matches_kind = match &field.kind {
            FieldKind::Text => !matches!(
                matching.mode,
                searchkernel_ir::MatchingModeIr::Exact | searchkernel_ir::MatchingModeIr::Word
            ),
            FieldKind::Keyword => matches!(
                matching.mode,
                searchkernel_ir::MatchingModeIr::Exact | searchkernel_ir::MatchingModeIr::Word
            ),
            _ => false,
        };
        if !matches_kind {
            return Err(Error::from_ir(searchkernel_ir::PortableError::invalid(
                "invalid_type_combination",
                Some(format!("{path}.matching")),
                "the public field matching mode must match its declared field kind",
            )));
        }
    }
    Ok(())
}

fn is_opaque_public_data_type(data_type: &searchkernel_ir::DataTypeIr) -> bool {
    !matches!(
        data_type,
        searchkernel_ir::DataTypeIr::Primitive(
            searchkernel_ir::PrimitiveDataTypeIr::String
                | searchkernel_ir::PrimitiveDataTypeIr::Bool
                | searchkernel_ir::PrimitiveDataTypeIr::Int64
                | searchkernel_ir::PrimitiveDataTypeIr::Float64,
        ) | searchkernel_ir::DataTypeIr::Tensor { .. }
            | searchkernel_ir::DataTypeIr::Position
    )
}

pub(crate) fn index_definition_to_input(
    definition: &IndexDefinition,
) -> Result<searchkernel_ir::IndexDefinitionInput> {
    Ok(searchkernel_ir::IndexDefinitionInput {
        name: definition.name.clone(),
        fields: definition
            .fields
            .iter()
            .enumerate()
            .map(|(index, field)| field_to_unresolved_input(field, &format!("fields[{index}]")))
            .collect::<Result<Vec<_>>>()?,
        schema_fields: definition
            .schema_fields
            .iter()
            .enumerate()
            .map(|(index, field)| {
                field_to_unresolved_input(field, &format!("schema_fields[{index}]"))
            })
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
        properties: definition.properties.clone(),
        document_type: definition.document_type.clone(),
        schema_inherits: definition.schema_inherits.clone(),
        document_inherits: definition.document_inherits.clone(),
        structs: definition.structs.clone(),
        fieldsets: definition.fieldsets.clone(),
        imported_fields: definition.imported_fields.clone(),
        document_summaries: definition.document_summaries.clone(),
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
        onnx_models: definition.onnx_models.clone(),
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
        profile.inputs.clone()
    };
    searchkernel_ir::UnresolvedRankProfileInput {
        name: profile.name.clone(),
        inherits: profile.inherits.clone(),
        inputs,
        functions: profile.functions.clone(),
        first_phase: profile.first_phase.clone(),
        second_phase: profile.second_phase.clone().or_else(|| {
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

pub(crate) fn rank_profile_to_rust_input(
    profile: &RankProfile,
) -> searchkernel_ir::RustRankProfileInput {
    searchkernel_ir::RustRankProfileInput {
        name: profile.name.clone(),
        first_phase: profile
            .first_phase
            .as_ref()
            .and_then(|phase| phase.expression.clone()),
        second_phase: profile
            .second_phase
            .as_ref()
            .and_then(|phase| phase.expression.clone()),
        rerank_count: profile.rerank_count.or_else(|| {
            profile
                .second_phase
                .as_ref()
                .and_then(|phase| phase.rerank_count)
        }),
        match_features: profile.match_features.clone(),
        query_inputs: profile.query_inputs.clone(),
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
        let profile = RankProfile::new("semantic")
            .query_input("query_embedding", "tensor<float>(x[3])");

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
