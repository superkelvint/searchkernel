use std::collections::BTreeMap;
use std::fmt;

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
    pub(crate) distance_metric: Option<DistanceMetric>,
    pub(crate) hnsw: Option<HnswIndexConfig>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// A Vespa rank profile exposed by the native schema compiler.
pub struct RankProfile {
    pub(crate) name: String,
    pub(crate) first_phase: String,
    pub(crate) second_phase: Option<String>,
    pub(crate) rerank_count: Option<u32>,
    pub(crate) match_features: Vec<String>,
    pub(crate) query_inputs: Vec<(String, String)>,
}

impl RankProfile {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            first_phase: String::new(),
            second_phase: None,
            rerank_count: None,
            match_features: Vec::new(),
            query_inputs: Vec::new(),
        }
    }

    pub fn first_phase(mut self, expression: impl Into<String>) -> Self {
        self.first_phase = expression.into();
        self
    }

    /// Set the Vespa second-phase ranking expression.
    pub fn second_phase(mut self, expression: impl Into<String>) -> Self {
        self.second_phase = Some(expression.into());
        self
    }

    /// Set the number of first-phase candidates Vespa reranks in second phase.
    pub fn rerank_count(mut self, count: u32) -> Self {
        self.rerank_count = Some(count);
        self
    }

    pub fn match_feature(mut self, expression: impl Into<String>) -> Self {
        self.match_features.push(expression.into());
        self
    }

    /// Declare a tensor query input used by this rank profile.
    pub fn query_input(mut self, name: impl Into<String>, type_spec: impl Into<String>) -> Self {
        self.query_inputs.push((name.into(), type_spec.into()));
        self
    }

    pub fn name(&self) -> &str {
        &self.name
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
            distance_metric: None,
            hnsw: None,
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
            distance_metric: None,
            hnsw: None,
        }
    }

    /// Make this field contain a collection of values.
    pub fn multi(mut self) -> Self {
        self.multi = true;
        self
    }
    /// Explicitly set whether values are stored in search results.
    pub fn stored(mut self, value: bool) -> Self {
        self.stored = Some(value);
        self
    }
    /// Explicitly set whether this field can be searched with text queries.
    pub fn searchable(mut self, value: bool) -> Self {
        self.searchable = Some(value);
        self
    }
    /// Explicitly set whether this field can be used in filters.
    pub fn filterable(mut self, value: bool) -> Self {
        self.filterable = Some(value);
        self
    }
    /// Explicitly set whether this field can be used for sorting.
    pub fn sortable(mut self, value: bool) -> Self {
        self.sortable = Some(value);
        self
    }
    /// Explicitly set whether this field can be used for facets.
    pub fn facetable(mut self, value: bool) -> Self {
        self.facetable = Some(value);
        self
    }
    /// Select the analyzer used for this field.
    pub fn analyzer(mut self, value: Analyzer) -> Self {
        self.analyzer = Some(value);
        self
    }

    /// Configure the tensor attribute distance metric.
    pub fn distance_metric(mut self, value: DistanceMetric) -> Self {
        self.distance_metric = Some(value);
        self
    }

    /// Configure HNSW indexing for this tensor field.
    pub fn hnsw(mut self, value: HnswIndexConfig) -> Self {
        self.hnsw = Some(value);
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

#[derive(Clone, Default, PartialEq)]
/// The schema and default search settings for an index.
pub struct IndexDefinition {
    pub(crate) name: Option<String>,
    pub(crate) fields: Vec<Field>,
    pub(crate) default_search_fields: Option<Vec<SearchField>>,
    pub(crate) rank_profiles: Vec<RankProfile>,
    // A described definition may contain canonical schema state that the
    // ergonomic Rust builders intentionally do not expose yet. Retain that
    // state so the IndexContext can be reconstructed without silently
    // resolving it through the narrower builder representation.
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
        self.canonical = None;
        self.name = Some(value.into());
        self
    }
    /// Add one field to the definition.
    pub fn field(mut self, value: Field) -> Self {
        self.canonical = None;
        self.fields.push(value);
        self
    }
    /// Set the fields used when a search does not specify its own fields.
    pub fn default_search_fields<I>(mut self, fields: I) -> Self
    where
        I: IntoIterator,
        I::Item: Into<SearchField>,
    {
        self.canonical = None;
        self.default_search_fields = Some(fields.into_iter().map(Into::into).collect());
        self
    }

    /// Add a native first-phase rank profile to the generated schema.
    pub fn rank_profile(mut self, profile: RankProfile) -> Self {
        self.canonical = None;
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
