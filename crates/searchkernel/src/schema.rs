#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// An analyzer supported by Search Kernel.
pub enum Analyzer {
    StandardV1,
}

impl Analyzer {
    pub fn standard_v1() -> Self {
        Self::StandardV1
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FieldKind {
    Text,
    Keyword,
    Int64,
    Float64,
    Bool,
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
}

#[derive(Clone, Debug, Default, PartialEq)]
/// The schema and default search settings for an index.
pub struct IndexDefinition {
    pub(crate) name: Option<String>,
    pub(crate) fields: Vec<Field>,
    pub(crate) default_search_fields: Option<Vec<SearchField>>,
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
        I: IntoIterator<Item = SearchField>,
    {
        self.default_search_fields = Some(fields.into_iter().collect());
        self
    }
}
