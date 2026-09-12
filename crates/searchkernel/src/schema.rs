#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

    pub fn text(name: impl Into<String>) -> Self {
        Self::new(name, FieldKind::Text)
    }
    pub fn keyword(name: impl Into<String>) -> Self {
        Self::new(name, FieldKind::Keyword)
    }
    pub fn int64(name: impl Into<String>) -> Self {
        Self::new(name, FieldKind::Int64)
    }
    pub fn float64(name: impl Into<String>) -> Self {
        Self::new(name, FieldKind::Float64)
    }
    pub fn bool(name: impl Into<String>) -> Self {
        Self::new(name, FieldKind::Bool)
    }

    pub fn multi(mut self) -> Self {
        self.multi = true;
        self
    }
    pub fn stored(mut self, value: bool) -> Self {
        self.stored = Some(value);
        self
    }
    pub fn searchable(mut self, value: bool) -> Self {
        self.searchable = Some(value);
        self
    }
    pub fn filterable(mut self, value: bool) -> Self {
        self.filterable = Some(value);
        self
    }
    pub fn sortable(mut self, value: bool) -> Self {
        self.sortable = Some(value);
        self
    }
    pub fn facetable(mut self, value: bool) -> Self {
        self.facetable = Some(value);
        self
    }
    pub fn analyzer(mut self, value: Analyzer) -> Self {
        self.analyzer = Some(value);
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SearchField {
    pub(crate) field: String,
    pub(crate) weight: Option<f64>,
}

impl SearchField {
    pub fn new(field: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            weight: None,
        }
    }
    pub fn weight(mut self, value: f64) -> Self {
        self.weight = Some(value);
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct IndexDefinition {
    pub(crate) name: Option<String>,
    pub(crate) fields: Vec<Field>,
    pub(crate) default_search_fields: Option<Vec<SearchField>>,
}

impl IndexDefinition {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn name(mut self, value: impl Into<String>) -> Self {
        self.name = Some(value.into());
        self
    }
    pub fn field(mut self, value: Field) -> Self {
        self.fields.push(value);
        self
    }
    pub fn default_search_fields<I>(mut self, fields: I) -> Self
    where
        I: IntoIterator<Item = SearchField>,
    {
        self.default_search_fields = Some(fields.into_iter().collect());
        self
    }
}
