//! Schema and field definitions.

use super::*;
use crate::error::{PortableError, Result};
/// The portable document type.  Matching and index configuration are
/// intentionally modelled separately; `Text` and `Keyword` are not document
/// types in the portable contract.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DataTypeIr {
    Primitive(PrimitiveDataTypeIr),
    Tensor {
        type_spec: String,
    },
    Array(Box<DataTypeIr>),
    Map {
        key_type: Box<DataTypeIr>,
        value_type: Box<DataTypeIr>,
    },
    WeightedSet(Box<DataTypeIr>),
    Struct {
        fields: Vec<StructFieldIr>,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrimitiveDataTypeIr {
    String,
    Bool,
    Byte,
    Int32,
    Int64,
    Float32,
    Float64,
    Raw,
}

/// Derived query type used by the existing request validator.  It is not a
/// schema representation: `FieldIr` stores the complete `DataTypeIr` above.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldKind {
    Text,
    Keyword,
    Int64,
    Float64,
    Bool,
}

/// Derived collection shape used only by existing request validation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldCardinality {
    Single,
    Multi,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnalyzerIdentity {
    StandardV1,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StructFieldIr {
    pub name: String,
    pub data_type: DataTypeIr,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatchingModeIr {
    Unspecified,
    Text,
    Word,
    Exact,
    Gram,
}

pub type SchemaPropertiesIr = std::collections::BTreeMap<String, RequestValueIr>;

#[derive(Clone, Debug, PartialEq)]
pub struct AnalyzerIr {
    pub name: String,
    pub version: u32,
    pub options: SchemaPropertiesIr,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MatchingConfigIr {
    pub mode: MatchingModeIr,
    pub options: SchemaPropertiesIr,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HnswIndexConfigIr {
    pub max_links_per_node: Option<u32>,
    pub neighbors_to_explore_at_insert: Option<u32>,
    pub options: SchemaPropertiesIr,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IndexConfigIr {
    pub enabled: Option<bool>,
    pub hnsw: Option<HnswIndexConfigIr>,
    pub options: SchemaPropertiesIr,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DistanceMetricIr {
    Unspecified,
    Euclidean,
    Angular,
    DotProduct,
    PrenormalizedAngular,
    GeoDegrees,
    Hamming,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AttributeConfigIr {
    pub enabled: Option<bool>,
    pub fast_search: Option<bool>,
    pub fast_access: Option<bool>,
    pub distance_metric: DistanceMetricIr,
    pub options: SchemaPropertiesIr,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SummaryConfigIr {
    pub enabled: Option<bool>,
    pub options: SchemaPropertiesIr,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FieldCapabilities {
    pub(crate) stored: bool,
    pub(crate) searchable: bool,
    pub(crate) filterable: bool,
    pub(crate) sortable: bool,
    /// Canonical grouping/attribute capability resolved from `facetable`.
    pub(crate) groupable: bool,
}

impl FieldCapabilities {
    #[cfg(any(test, feature = "test"))]
    pub fn new(
        stored: bool,
        searchable: bool,
        filterable: bool,
        sortable: bool,
        groupable: bool,
    ) -> Self {
        Self {
            stored,
            searchable,
            filterable,
            sortable,
            groupable,
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

    /// Compatibility accessor for callers that still use the old facade name.
    /// The canonical IR stores this as the resolved grouping capability.
    pub fn facetable(&self) -> bool {
        self.groupable()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FieldIr {
    pub(crate) name: FieldName,
    pub(crate) data_type: DataTypeIr,
    pub(crate) analyzer: Option<AnalyzerIr>,
    pub(crate) capabilities: FieldCapabilities,
    pub(crate) matching: Option<MatchingConfigIr>,
    pub(crate) index: Option<IndexConfigIr>,
    pub(crate) attribute: Option<AttributeConfigIr>,
    pub(crate) summary: Option<SummaryConfigIr>,
    pub(crate) indexing_script: Option<String>,
    pub(crate) properties: SchemaPropertiesIr,
    pub(crate) struct_fields: Vec<StructFieldConfigIr>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StructFieldConfigIr {
    pub path: String,
    pub indexing_script: Option<String>,
    pub matching: Option<MatchingConfigIr>,
    pub index: Option<IndexConfigIr>,
    pub attribute: Option<AttributeConfigIr>,
    pub summary: Option<SummaryConfigIr>,
    pub rank: Option<String>,
    pub query_commands: Vec<String>,
    pub properties: SchemaPropertiesIr,
}

impl FieldIr {
    #[cfg(any(test, feature = "test"))]
    pub fn new(
        name: FieldName,
        data_type: DataTypeIr,
        analyzer: Option<AnalyzerIr>,
        capabilities: FieldCapabilities,
    ) -> Self {
        Self {
            name,
            data_type,
            analyzer,
            capabilities,
            matching: None,
            index: None,
            attribute: None,
            summary: None,
            indexing_script: None,
            properties: SchemaPropertiesIr::new(),
            struct_fields: Vec::new(),
        }
    }

    #[doc(hidden)]
    pub fn set_attribute_config(&mut self, config: Option<AttributeConfigIr>) {
        self.attribute = config;
    }

    #[doc(hidden)]
    pub fn set_index_config(&mut self, config: Option<IndexConfigIr>) {
        self.index = config;
    }

    #[doc(hidden)]
    pub fn set_summary_config(&mut self, config: Option<SummaryConfigIr>) {
        self.summary = config;
    }

    #[doc(hidden)]
    pub fn set_indexing_script(&mut self, script: Option<String>) {
        self.indexing_script = script;
    }
    #[doc(hidden)]
    pub fn set_properties(&mut self, properties: SchemaPropertiesIr) {
        self.properties = properties;
    }

    #[doc(hidden)]
    pub fn set_matching_config(&mut self, config: Option<MatchingConfigIr>) {
        self.matching = config;
    }

    #[doc(hidden)]
    pub fn set_struct_fields(&mut self, struct_fields: Vec<StructFieldConfigIr>) {
        self.struct_fields = struct_fields;
    }

    /// Update the resolved collection shape without changing other field
    /// configuration.
    #[doc(hidden)]
    pub fn set_multi(&mut self, multi: bool) {
        let current = self.data_type.clone();
        self.data_type = if multi {
            if current.is_multi_valued() {
                current
            } else {
                DataTypeIr::Array(Box::new(current))
            }
        } else {
            match current {
                DataTypeIr::Array(value) => *value,
                value => value,
            }
        };
    }

    /// Update the resolved field capabilities without changing other field
    /// configuration.
    #[doc(hidden)]
    pub fn set_capabilities(
        &mut self,
        stored: bool,
        searchable: bool,
        filterable: bool,
        sortable: bool,
        groupable: bool,
    ) {
        self.capabilities = FieldCapabilities {
            stored,
            searchable,
            filterable,
            sortable,
            groupable,
        };
    }

    /// Update the analyzer while retaining every other resolved field member.
    #[doc(hidden)]
    pub fn set_analyzer(&mut self, analyzer: Option<AnalyzerIr>) {
        self.analyzer = analyzer;
    }

    /// Replace the resolved field data type without changing other field
    /// configuration.
    #[doc(hidden)]
    pub fn set_data_type(&mut self, data_type: DataTypeIr) {
        self.data_type = data_type;
    }

    /// Update the resolved tensor attribute distance metric.
    #[doc(hidden)]
    pub fn set_distance_metric(&mut self, distance_metric: DistanceMetricIr) {
        let attribute = self.attribute.get_or_insert_with(|| AttributeConfigIr {
            enabled: Some(true),
            fast_search: None,
            fast_access: None,
            distance_metric: DistanceMetricIr::Unspecified,
            options: SchemaPropertiesIr::new(),
        });
        attribute.distance_metric = distance_metric;
    }

    /// Update the resolved HNSW configuration.
    #[doc(hidden)]
    pub fn set_hnsw(&mut self, hnsw: HnswIndexConfigIr) {
        let index = self.index.get_or_insert_with(|| IndexConfigIr {
            enabled: Some(true),
            hnsw: None,
            options: SchemaPropertiesIr::new(),
        });
        index.enabled = Some(true);
        index.hnsw = Some(hnsw);
    }

    pub fn name(&self) -> &FieldName {
        &self.name
    }

    pub fn data_type(&self) -> &DataTypeIr {
        &self.data_type
    }

    pub fn kind(&self) -> FieldKind {
        match self.query_scalar_type().unwrap_or(QueryScalarType::Keyword) {
            QueryScalarType::Text => FieldKind::Text,
            QueryScalarType::Keyword => FieldKind::Keyword,
            QueryScalarType::Int64 => FieldKind::Int64,
            QueryScalarType::Float64 => FieldKind::Float64,
            QueryScalarType::Bool => FieldKind::Bool,
        }
    }

    pub fn cardinality(&self) -> FieldCardinality {
        if self.is_multi_valued() {
            FieldCardinality::Multi
        } else {
            FieldCardinality::Single
        }
    }

    pub fn analyzer(&self) -> Option<&AnalyzerIr> {
        self.analyzer.as_ref()
    }

    pub fn capabilities(&self) -> &FieldCapabilities {
        &self.capabilities
    }

    pub fn matching(&self) -> Option<&MatchingConfigIr> {
        self.matching.as_ref()
    }
    pub fn index(&self) -> Option<&IndexConfigIr> {
        self.index.as_ref()
    }
    pub fn attribute(&self) -> Option<&AttributeConfigIr> {
        self.attribute.as_ref()
    }
    pub fn summary(&self) -> Option<&SummaryConfigIr> {
        self.summary.as_ref()
    }
    pub fn indexing_script(&self) -> Option<&str> {
        self.indexing_script.as_deref()
    }
    pub fn properties(&self) -> &SchemaPropertiesIr {
        &self.properties
    }
    pub fn struct_fields(&self) -> &[StructFieldConfigIr] {
        &self.struct_fields
    }

    pub(crate) fn query_scalar_type(&self) -> Option<QueryScalarType> {
        let primitive = self.data_type.scalar_primitive()?;
        match primitive {
            PrimitiveDataTypeIr::String => match self.matching.as_ref().map(|m| m.mode) {
                Some(MatchingModeIr::Exact | MatchingModeIr::Word) => {
                    Some(QueryScalarType::Keyword)
                }
                _ => Some(QueryScalarType::Text),
            },
            PrimitiveDataTypeIr::Bool => Some(QueryScalarType::Bool),
            PrimitiveDataTypeIr::Int64 => Some(QueryScalarType::Int64),
            PrimitiveDataTypeIr::Float64 => Some(QueryScalarType::Float64),
            _ => None,
        }
    }

    pub(crate) fn is_multi_valued(&self) -> bool {
        self.data_type.is_multi_valued()
    }
}

impl DataTypeIr {
    pub(crate) fn scalar_primitive(&self) -> Option<PrimitiveDataTypeIr> {
        match self {
            Self::Primitive(kind) => Some(*kind),
            Self::Array(element) | Self::WeightedSet(element) => element.scalar_primitive(),
            _ => None,
        }
    }

    pub(crate) fn is_multi_valued(&self) -> bool {
        matches!(self, Self::Array(_) | Self::WeightedSet(_))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum QueryScalarType {
    Text,
    Keyword,
    Int64,
    Float64,
    Bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SearchFieldWeight(f64);

impl SearchFieldWeight {
    pub fn new(value: f64) -> Result<Self> {
        if !value.is_finite() || value < 0.0 {
            return Err(PortableError::invalid(
                "invalid_search_weight",
                None::<String>,
                "search-field weight must be finite and non-negative",
            ));
        }
        Ok(Self(value))
    }

    pub fn get(self) -> f64 {
        self.0
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SearchFieldIr {
    pub(crate) field: FieldName,
    pub(crate) weight: SearchFieldWeight,
}

impl SearchFieldIr {
    #[cfg(any(test, feature = "test"))]
    pub fn new(field: FieldName, weight: SearchFieldWeight) -> Self {
        Self { field, weight }
    }

    pub fn field(&self) -> &FieldName {
        &self.field
    }

    pub fn weight(&self) -> SearchFieldWeight {
        self.weight
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NamedStructDefinitionIr {
    pub name: String,
    pub fields: Vec<StructFieldIr>,
    pub properties: SchemaPropertiesIr,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FieldSetDefinitionIr {
    pub name: String,
    pub fields: Vec<FieldName>,
    pub matching: Option<MatchingConfigIr>,
    pub query_commands: Vec<String>,
    pub properties: SchemaPropertiesIr,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ImportedFieldDefinitionIr {
    pub reference_field: FieldName,
    pub target_field: String,
    pub alias: FieldName,
    pub properties: SchemaPropertiesIr,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DocumentSummaryFieldDefinitionIr {
    pub name: String,
    pub source: String,
    pub data_type: DataTypeIr,
    pub struct_fields: Vec<String>,
    pub properties: SchemaPropertiesIr,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DocumentSummaryDefinitionIr {
    pub name: String,
    pub inherits: Vec<String>,
    pub fields: Vec<DocumentSummaryFieldDefinitionIr>,
    pub from_disk: Option<bool>,
    pub omit_summary_features: Option<bool>,
    pub properties: SchemaPropertiesIr,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IndexDefinitionIr {
    pub(crate) name: Option<IndexName>,
    pub(crate) fields: Vec<FieldIr>,
    pub(crate) default_search_fields: Vec<SearchFieldIr>,
    pub(crate) properties: SchemaPropertiesIr,
    pub(crate) document_type: Option<String>,
    // Backend document identity is semantic resolved state, but is deliberately
    // not part of the caller-visible IndexDefinition contract.
    pub(crate) resolved_document_type: String,
    pub(crate) schema_inherits: Vec<String>,
    pub(crate) document_inherits: Vec<String>,
    pub(crate) structs: Vec<NamedStructDefinitionIr>,
    pub(crate) fieldsets: Vec<FieldSetDefinitionIr>,
    pub(crate) schema_fields: Vec<FieldIr>,
    pub(crate) imported_fields: Vec<ImportedFieldDefinitionIr>,
    pub(crate) document_summaries: Vec<DocumentSummaryDefinitionIr>,
    pub(crate) rank_profiles: Vec<RankProfileDefinitionIr>,
    pub(crate) constants: Vec<ConstantDefinitionIr>,
    pub(crate) onnx_models: Vec<OnnxModelDefinitionIr>,
}

impl IndexDefinitionIr {
    #[cfg(any(test, feature = "test"))]
    pub fn new(
        name: Option<IndexName>,
        fields: Vec<FieldIr>,
        default_search_fields: Vec<SearchFieldIr>,
    ) -> Result<Self> {
        if fields.is_empty() {
            return Err(PortableError::invalid(
                "invalid_schema",
                Some("fields"),
                "an index must define at least one field",
            ));
        }
        let resolved_document_type = name
            .as_ref()
            .map(IndexName::as_str)
            .unwrap_or("searchkernel")
            .to_owned();
        Ok(Self {
            name,
            fields,
            default_search_fields,
            properties: SchemaPropertiesIr::new(),
            document_type: None,
            resolved_document_type,
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
        })
    }

    #[doc(hidden)]
    pub fn set_fields(&mut self, fields: Vec<FieldIr>) {
        self.fields = fields;
    }

    #[cfg(any(test, feature = "test"))]
    pub fn set_document_type(&mut self, document_type: impl Into<String>) {
        self.document_type = Some(document_type.into());
        self.refresh_resolved_document_type();
    }

    #[doc(hidden)]
    pub fn set_schema_fields(&mut self, fields: Vec<FieldIr>) {
        self.schema_fields = fields;
    }

    /// Replace top-level fieldset definitions while retaining every other
    /// canonical schema member.
    #[doc(hidden)]
    pub fn set_fieldsets(&mut self, fieldsets: Vec<FieldSetDefinitionIr>) {
        self.fieldsets = fieldsets;
    }

    /// Replace top-level ONNX model definitions while retaining every other
    /// canonical schema member.
    #[doc(hidden)]
    pub fn set_onnx_models(&mut self, models: Vec<OnnxModelDefinitionIr>) {
        self.onnx_models = models;
    }

    /// Replace top-level struct definitions while retaining every other
    /// canonical schema member.
    #[doc(hidden)]
    pub fn set_structs(&mut self, structs: Vec<NamedStructDefinitionIr>) {
        self.structs = structs;
    }

    #[doc(hidden)]
    pub fn set_rank_profiles(&mut self, profiles: Vec<RankProfileDefinitionIr>) {
        self.rank_profiles = profiles;
    }

    /// Replace schema constants while retaining every other canonical member.
    #[doc(hidden)]
    pub fn set_constants(&mut self, constants: Vec<ConstantDefinitionIr>) {
        self.constants = constants;
    }

    #[doc(hidden)]
    pub fn set_imported_fields(&mut self, fields: Vec<ImportedFieldDefinitionIr>) {
        self.imported_fields = fields;
    }

    /// Replace document-summary definitions while retaining the rest of the canonical schema state.
    #[doc(hidden)]
    pub fn set_document_summaries(&mut self, summaries: Vec<DocumentSummaryDefinitionIr>) {
        self.document_summaries = summaries;
    }

    /// Replace the name while retaining every other canonical schema member.
    #[doc(hidden)]
    pub fn set_name(&mut self, name: Option<IndexName>) {
        self.name = name;
        self.refresh_resolved_document_type();
    }

    /// Replace default search fields while retaining every other canonical
    /// schema member.
    #[doc(hidden)]
    pub fn set_default_search_fields(&mut self, fields: Vec<SearchFieldIr>) {
        self.default_search_fields = fields;
    }

    /// Replace document metadata while retaining every other schema member.
    #[doc(hidden)]
    pub fn set_document_metadata(
        &mut self,
        document_type: Option<String>,
        schema_inherits: Vec<String>,
        document_inherits: Vec<String>,
    ) {
        self.document_type = document_type;
        self.refresh_resolved_document_type();
        self.schema_inherits = schema_inherits;
        self.document_inherits = document_inherits;
    }

    /// Replace top-level schema properties while retaining every other
    /// canonical schema member.
    #[doc(hidden)]
    pub fn set_properties(&mut self, properties: SchemaPropertiesIr) {
        self.properties = properties;
    }

    /// Append a document field if its name is not already present.
    #[doc(hidden)]
    pub fn push_field(&mut self, field: FieldIr) -> bool {
        if self
            .fields
            .iter()
            .any(|existing| existing.name.as_str() == field.name.as_str())
        {
            return false;
        }
        self.fields.push(field);
        true
    }

    /// Append a rank profile if its name is not already present.
    #[doc(hidden)]
    pub fn push_rank_profile(&mut self, profile: RankProfileDefinitionIr) -> bool {
        if self
            .rank_profiles
            .iter()
            .any(|existing| existing.name == profile.name)
        {
            return false;
        }
        self.rank_profiles.push(profile);
        true
    }

    pub fn name(&self) -> Option<&IndexName> {
        self.name.as_ref()
    }

    pub fn fields(&self) -> &[FieldIr] {
        &self.fields
    }

    /// Fields that may be addressed by query-side operations.  Document
    /// fields are feedable and queryable; schema-only fields are queryable but
    /// are derived by indexing expressions and must not be accepted in a
    /// document write.
    pub(crate) fn query_fields(&self) -> impl Iterator<Item = &FieldIr> {
        self.fields.iter().chain(self.schema_fields.iter())
    }

    pub(crate) fn imported_alias(&self, name: &str) -> bool {
        self.imported_fields
            .iter()
            .any(|field| field.alias.as_str() == name)
    }

    pub fn default_search_fields(&self) -> &[SearchFieldIr] {
        &self.default_search_fields
    }

    pub fn properties(&self) -> &SchemaPropertiesIr {
        &self.properties
    }
    pub fn document_type(&self) -> Option<&str> {
        self.document_type.as_deref()
    }

    /// Return the canonical document identity used by backend adapters.
    ///
    /// document_type() preserves the caller-visible optional field for
    /// lossless Describe/API round trips. Native execution resolves that
    /// intent here, inside the semantic IR boundary, so adapters never invent
    /// their own fallback.
    pub fn resolved_document_type(&self) -> &str {
        &self.resolved_document_type
    }

    /// Override the already-resolved backend document identity when reopening
    /// persisted state. This never changes caller-visible document_type.
    #[doc(hidden)]
    pub fn set_resolved_document_type(&mut self, document_type: impl Into<String>) {
        self.resolved_document_type = document_type.into();
    }

    fn refresh_resolved_document_type(&mut self) {
        self.resolved_document_type = self
            .document_type
            .as_deref()
            .or_else(|| self.name.as_ref().map(IndexName::as_str))
            .unwrap_or("searchkernel")
            .to_owned();
    }

    pub fn schema_inherits(&self) -> &[String] {
        &self.schema_inherits
    }
    pub fn document_inherits(&self) -> &[String] {
        &self.document_inherits
    }
    pub fn structs(&self) -> &[NamedStructDefinitionIr] {
        &self.structs
    }
    pub fn fieldsets(&self) -> &[FieldSetDefinitionIr] {
        &self.fieldsets
    }
    pub fn schema_fields(&self) -> &[FieldIr] {
        &self.schema_fields
    }
    pub fn imported_fields(&self) -> &[ImportedFieldDefinitionIr] {
        &self.imported_fields
    }
    pub fn document_summaries(&self) -> &[DocumentSummaryDefinitionIr] {
        &self.document_summaries
    }
    pub fn rank_profiles(&self) -> &[RankProfileDefinitionIr] {
        &self.rank_profiles
    }
    pub fn constants(&self) -> &[ConstantDefinitionIr] {
        &self.constants
    }
    pub fn onnx_models(&self) -> &[OnnxModelDefinitionIr] {
        &self.onnx_models
    }
}
