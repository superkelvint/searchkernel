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

#[derive(Clone, Debug, PartialEq)]
/// The type and capabilities of one indexed field.
pub struct Field {
    pub(crate) name: String,
    pub(crate) kind: FieldKind,
    pub(crate) multi: bool,
    pub(crate) stored: Option<bool>,
    pub(crate) searchable: Option<bool>,
    pub(crate) filterable: Option<bool>,
    pub(crate) sortable: Option<bool>,
    pub(crate) facetable: Option<bool>,
    pub(crate) analyzer: Option<Analyzer>,
    pub(crate) analyzer_config: Option<searchkernel_ir::AnalyzerIr>,
    pub(crate) matching_config: Option<searchkernel_ir::MatchingConfigIr>,
    pub(crate) distance_metric: Option<DistanceMetric>,
    pub(crate) hnsw: Option<HnswIndexConfig>,
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
    pub(crate) query_inputs: Vec<(String, String)>,
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
            query_inputs: Vec::new(),
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
        if let Some(profile) = self.canonical.as_mut() {
            profile.inputs.push(searchkernel_ir::RankProfileInputIr {
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
        }
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
            analyzer_config: None,
            matching_config: None,
            distance_metric: None,
            hnsw: None,
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
        Self {
            name: name.into(),
            kind: FieldKind::Tensor(type_spec.into()),
            multi: false,
            stored: None,
            searchable: None,
            filterable: None,
            sortable: None,
            facetable: None,
            analyzer: None,
            analyzer_config: None,
            matching_config: None,
            distance_metric: None,
            hnsw: None,
            canonical: None,
        }
    }

    /// Make this field contain a collection of values.
    pub fn multi(mut self) -> Self {
        self.multi = true;
        if let Some(field) = self.canonical.as_mut() {
            field.set_multi(true);
        }
        self
    }
    /// Explicitly set whether values are stored in search results.
    pub fn stored(mut self, value: bool) -> Self {
        self.stored = Some(value);
        if let Some(field) = self.canonical.as_mut() {
            let capabilities = field.capabilities();
            field.set_capabilities(
                value,
                capabilities.searchable(),
                capabilities.filterable(),
                capabilities.sortable(),
                capabilities.groupable(),
            );
        }
        self
    }
    /// Explicitly set whether this field can be searched with text queries.
    pub fn searchable(mut self, value: bool) -> Self {
        self.searchable = Some(value);
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
        if let Some(field) = self.canonical.as_mut() {
            field.set_distance_metric(distance_metric_to_ir(value));
        }
        self
    }

    /// Configure HNSW indexing for this tensor field.
    pub fn hnsw(mut self, value: HnswIndexConfig) -> Self {
        let hnsw = hnsw_to_ir(&value);
        self.hnsw = Some(value);
        if let Some(field) = self.canonical.as_mut() {
            field.set_hnsw(hnsw);
        }
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

    /// Return the complete analyzer configuration, including arbitrary options.
    pub fn analyzer_config(&self) -> Option<&searchkernel_ir::AnalyzerIr> {
        self.analyzer_config.as_ref()
    }

    /// Return the complete matching configuration, including arbitrary options.
    pub fn matching(&self) -> Option<&searchkernel_ir::MatchingConfigIr> {
        self.matching_config.as_ref()
    }

    /// Return the complete canonical field state represented by this public
    /// value, when it originated from `Index::describe()`.
    pub fn canonical_definition(&self) -> Option<&searchkernel_ir::FieldIr> {
        self.canonical.as_ref()
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
    pub(crate) rank_profiles: Vec<RankProfile>,
    pub(crate) constants: Vec<ConstantDefinition>,
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
            .field("rank_profiles", &self.rank_profiles)
            .field("constants", &self.constants)
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

    /// Return the explicitly configured default search fields.
    pub fn configured_default_search_fields(&self) -> Option<&[SearchField]> {
        self.default_search_fields.as_deref()
    }

    pub fn rank_profiles(&self) -> &[RankProfile] {
        &self.rank_profiles
    }

    /// Return schema constants represented by this definition.
    pub fn constants(&self) -> &[ConstantDefinition] {
        &self.constants
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

        let fields = definition.default_search_fields.unwrap();
        assert_eq!(fields[0].field, "title");
        assert_eq!(fields[1].field, "description");
        assert_eq!(
            fields.iter().map(|field| field.weight).collect::<Vec<_>>(),
            [None, None]
        );
    }
}
