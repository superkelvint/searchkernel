use crate::{Error, Result};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

const NAME_ERROR: &str = "names must match [A-Za-z_][A-Za-z0-9_]*";
const MAX_SCHEMA_IDENTIFIER_BYTES: usize = 256;
const MAX_RANK_EXPRESSION_BYTES: usize = 4 * 1024 * 1024;
const RESERVED_FIELD_NAMES: &[&str] = &[
    "index",
    "index_url",
    "summary",
    "attribute",
    "select_input",
    "host",
    "documentid",
    "position",
    "split_foreach",
    "tokenize",
    "if",
    "else",
    "switch",
    "case",
    "sddocname",
    "relevancy",
    "rankfeatures",
    "summaryfeatures",
];

#[derive(Debug, Clone, PartialEq, Eq)]
/// A field type supported by the embedded Vespa configuration compiler.
pub enum FieldType {
    Text,
    ArrayString,
    ArrayStruct(Vec<StructField>),
    String,
    /// A Vespa LONG field used by the portable API's signed 64-bit integer.
    ///
    /// `Integer` is retained as the legacy INT32 spelling required by the
    /// frozen M6 contract.
    Long,
    Integer,
    Float,
    Boolean,
    Position,
    Predicate,
    Tensor(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructField {
    pub name: String,
    pub field_type: StructFieldType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StructFieldType {
    String,
    Long,
    Integer,
    Float,
    Boolean,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// One field in a Vespa document schema.
pub struct Field {
    name: String,
    field_type: FieldType,
    indexed: bool,
    attribute: bool,
    fast_search: bool,
    summary: bool,
    predicate_arity: Option<u32>,
}

impl Field {
    /// Create a field with no indexing or storage capabilities enabled.
    pub fn new(name: impl Into<String>, field_type: FieldType) -> Self {
        Self {
            name: name.into(),
            field_type,
            indexed: false,
            attribute: false,
            fast_search: false,
            summary: false,
            predicate_arity: None,
        }
    }

    /// Add a full-text index for this field.
    pub fn indexed(mut self) -> Self {
        self.indexed = true;
        self
    }

    /// Store this field as an attribute.
    pub fn attribute(mut self) -> Self {
        self.attribute = true;
        self
    }

    /// Enable Vespa's fast exact lookup path for this attribute.
    pub fn fast_search(mut self) -> Self {
        self.fast_search = true;
        self
    }

    /// Include this field in the default document summary.
    pub fn summary(mut self) -> Self {
        self.summary = true;
        self
    }

    /// Set Vespa predicate field arity. Predicate fields require this native
    /// configuration because it controls the predicate hash space.
    pub fn predicate_arity(mut self, arity: u32) -> Self {
        self.predicate_arity = Some(arity);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// The document type and fields used to generate Vespa configuration.
pub struct Schema {
    document_type: String,
    fields: Vec<Field>,
    rank_profiles: Vec<RankProfile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// A named Vespa rank profile owned by the generated schema configuration.
pub struct RankProfile {
    name: String,
    first_phase: String,
    match_features: Vec<String>,
    query_inputs: Vec<(String, String)>,
}

impl RankProfile {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            first_phase: String::new(),
            match_features: Vec::new(),
            query_inputs: Vec::new(),
        }
    }

    pub fn first_phase(mut self, expression: impl Into<String>) -> Self {
        self.first_phase = expression.into();
        self
    }

    pub fn match_feature(mut self, expression: impl Into<String>) -> Self {
        self.match_features.push(expression.into());
        self
    }

    /// Declare the tensor type of a query input used by this rank profile.
    pub fn query_input(mut self, name: impl Into<String>, type_spec: impl Into<String>) -> Self {
        self.query_inputs.push((name.into(), type_spec.into()));
        self
    }
}

impl Schema {
    /// Create an empty schema for `document_type`.
    pub fn new(document_type: impl Into<String>) -> Self {
        Self {
            document_type: document_type.into(),
            fields: Vec::new(),
            rank_profiles: Vec::new(),
        }
    }

    /// Add one field to the schema.
    pub fn field(mut self, field: Field) -> Self {
        self.fields.push(field);
        self
    }

    /// Add a named rank profile to the generated Proton configuration.
    pub fn rank_profile(mut self, profile: RankProfile) -> Self {
        self.rank_profiles.push(profile);
        self
    }
}

#[derive(Debug, Default, Clone, Copy)]
/// Writes the configuration files required by the embedded engine.
pub struct ConfigCompiler;

impl ConfigCompiler {
    pub fn compile_to(&self, schema: &Schema, directory: impl AsRef<Path>) -> Result<()> {
        let output = directory.as_ref();
        let derived_config = DerivedConfig::from_schema(schema)?;
        let output_state = OutputState::inspect(output)?;
        let temporary_directory = TemporaryDirectory::create(output)?;

        write_generated_files(&temporary_directory.path, schema, &derived_config)?;
        temporary_directory.install(output, output_state)
    }
}

fn write_generated_files(
    directory: &Path,
    schema: &Schema,
    derived_config: &DerivedConfig<'_>,
) -> Result<()> {
    write_config(
        directory,
        "searchkernel-schema.json",
        &derived_config.manifest(),
    )?;
    write_config(directory, "indexschema.cfg", &derived_config.indexschema())?;
    write_config(directory, "attributes.cfg", &derived_config.attributes())?;
    write_config(directory, "summary.cfg", &derived_config.summary())?;
    write_config(directory, "rank-profiles.cfg", &rank_profiles(schema))?;
    if schema.fields.iter().any(|field| {
        matches!(&field.field_type, FieldType::Position)
            || (matches!(&field.field_type, FieldType::ArrayString) && field.indexed)
            || matches!(&field.field_type, FieldType::ArrayStruct(_))
    }) {
        write_config(directory, "ilscripts.cfg", &derived_config.ilscripts())?;
    }
    write_config(directory, "juniperrc.cfg", "")?;
    write_config(directory, "proton.cfg", &derived_config.proton())
}

#[derive(Debug)]
struct DerivedConfig<'a> {
    schema: &'a Schema,
}

impl<'a> DerivedConfig<'a> {
    fn from_schema(schema: &'a Schema) -> Result<Self> {
        validate_schema(schema)?;
        Ok(Self { schema })
    }

    fn fields(&self) -> &[Field] {
        &self.schema.fields
    }

    fn manifest(&self) -> String {
        let mut output = format!(
            "{{\"version\":1,\"document_type\":{},\"fields\":[",
            quoted(&self.schema.document_type)
        );
        for (index, field) in self.fields().iter().enumerate() {
            if index != 0 {
                output.push(',');
            }
            match &field.field_type {
                FieldType::Tensor(type_spec) => output.push_str(&format!(
                    "{{\"name\":{},\"type\":\"tensor\",\"tensor_type\":{},\"index\":{},\"attribute\":{},\"summary\":{}}}",
                    quoted(&field.name),
                    quoted(type_spec),
                    field.indexed,
                    field.attribute,
                    field.summary,
                )),
                FieldType::ArrayStruct(struct_fields) => output.push_str(&format!(
                    "{{\"name\":{},\"type\":\"array<struct>\",\"struct_fields\":[{}],\"index\":{},\"attribute\":{},\"summary\":{}}}",
                    quoted(&field.name),
                    struct_fields
                        .iter()
                        .map(|value| format!(
                            "{{\"name\":{},\"type\":{}}}",
                            quoted(&value.name),
                            quoted(value.field_type.manifest_name())
                        ))
                        .collect::<Vec<_>>()
                        .join(","),
                    field.indexed,
                    field.attribute,
                    field.summary,
                )),
                _ => output.push_str(&format!(
                    "{{\"name\":{},\"type\":{},\"index\":{},\"attribute\":{},\"summary\":{}{} }}",
                    quoted(&field.name),
                    quoted(field.field_type.manifest_name()),
                    field.indexed,
                    field.attribute,
                    field.summary,
                    field.predicate_arity
                        .map(|arity| format!(",\"predicate_arity\":{arity}"))
                        .unwrap_or_default(),
                )),
            }
        }
        output.push_str("]}\n");
        output
    }

    fn indexschema(&self) -> String {
        let mut indexed = Vec::new();
        for field in self.fields() {
            if field.indexed {
                indexed.push((field.name.clone(), field.field_type.index_collection_type()));
            }
            if let FieldType::ArrayStruct(struct_fields) = &field.field_type {
                for struct_field in struct_fields {
                    if matches!(struct_field.field_type, StructFieldType::String) {
                        indexed.push((format!("{}__{}", field.name, struct_field.name), "ARRAY"));
                    }
                }
            }
        }
        let mut output = format!("indexfield[{}]\n", indexed.len());
        for (index, (name, collection_type)) in indexed.iter().enumerate() {
            output.push_str(&format!(
                "indexfield[{index}].name {}\nindexfield[{index}].datatype STRING\nindexfield[{index}].collectiontype {}\nindexfield[{index}].prefix false\nindexfield[{index}].phrases false\nindexfield[{index}].positions true\nindexfield[{index}].averageelementlen 512\nindexfield[{index}].interleavedfeatures false\n",
                quoted(name),
                collection_type
            ));
        }
        output
    }

    fn attributes(&self) -> String {
        let attributes: Vec<&Field> = self
            .fields()
            .iter()
            .filter(|field| field.attribute)
            .collect();
        let attribute_count = attributes
            .iter()
            .map(|field| match &field.field_type {
                FieldType::ArrayStruct(fields) => fields.len(),
                _ => 1,
            })
            .sum::<usize>();
        let mut output = format!("attribute[{}]\n", attribute_count);
        let mut index = 0;
        for field in attributes {
            if let FieldType::ArrayStruct(fields) = &field.field_type {
                for struct_field in fields {
                    output.push_str(&format!(
                        "attribute[{index}].name {}\nattribute[{index}].datatype {}\nattribute[{index}].collectiontype ARRAY\n",
                        quoted(&format!("{}.{}", field.name, struct_field.name)),
                        struct_field.field_type.attribute_type(),
                    ));
                    index += 1;
                }
                continue;
            }
            let name = match &field.field_type {
                FieldType::Position => format!("{}_zcurve", field.name),
                _ => field.name.clone(),
            };
            let attribute_type = match &field.field_type {
                FieldType::Position => "INT64",
                _ => field.field_type.attribute_type(),
            };
            output.push_str(&format!(
                "attribute[{index}].name {}\nattribute[{index}].datatype {}\nattribute[{index}].collectiontype {}\n",
                quoted(&name),
                attribute_type,
                field.field_type.attribute_collection_type(),
            ));
            if let FieldType::Tensor(type_spec) = &field.field_type {
                output.push_str(&format!(
                    "attribute[{index}].tensortype {}\n",
                    quoted(type_spec)
                ));
            }
            if let FieldType::Predicate = &field.field_type {
                output.push_str(&format!(
                    "attribute[{index}].arity {}\n",
                    field.predicate_arity.unwrap_or_default()
                ));
                output.push_str(&format!(
                    "attribute[{index}].lowerbound {}\nattribute[{index}].upperbound {}\nattribute[{index}].densepostinglistthreshold 0.4\n",
                    i64::MIN,
                    i64::MAX
                ));
            }
            if field.fast_search || matches!(&field.field_type, FieldType::Position) {
                output.push_str(&format!("attribute[{index}].fastsearch true\n"));
            }
            index += 1;
        }
        output
    }

    fn ilscripts(&self) -> String {
        let mut indexed_special_fields = Vec::new();
        for field in self.fields() {
            match &field.field_type {
                FieldType::Position => indexed_special_fields.push((
                    field.name.clone(),
                    format!(
                        "clear_state | guard {{ input {} | zcurve | attribute {}_zcurve; }}",
                        field.name, field.name
                    ),
                )),
                FieldType::ArrayString if field.indexed => indexed_special_fields.push((
                    field.name.clone(),
                    {
                        let summary = if field.summary {
                            format!(" | summary {}", field.name)
                        } else {
                            String::new()
                        };
                        format!(
                            "clear_state | guard {{ input {} | for_each {{ tokenize normalize stem:\"BEST\" }}{} | index {} | attribute {}; }}",
                            field.name,
                            summary,
                            field.name,
                            field.name
                        )
                    },
                )),
                FieldType::ArrayStruct(struct_fields) => {
                    for member in struct_fields {
                        if matches!(member.field_type, StructFieldType::String) {
                            let name = format!("{}.{}", field.name, member.name);
                            indexed_special_fields.push((
                                name.clone(),
                                format!(
                                    "clear_state | guard {{ input {} | for_each {{ tokenize normalize stem:\"BEST\" }} | index {} | attribute {}; }}",
                                    name,
                                    name.replace('.', "__"),
                                    name
                                ),
                            ));
                        }
                    }
                }
                _ => {}
            }
        }
        let mut output = String::from(
            "maxtermoccurrences 10000\nmaxtokenlength 1000\nfieldmatchmaxlength 1000000\nmaxReplacementCharactersRatio 0.25\nmaxReplacementCharacters 5000\n",
        );
        for (field, content) in indexed_special_fields.iter() {
            output.push_str(&format!(
                "ilscript[].doctype {}\nilscript[].docfield[] {}\nilscript[].content[] {}\n",
                quoted(&self.schema.document_type),
                quoted(field),
                quoted(content)
            ));
        }
        output
    }

    fn summary(&self) -> String {
        let mut fields = vec![SummaryField::implicit(
            "documentid",
            "documentid",
            "",
            "longstring",
        )];
        fields.push(SummaryField::implicit(
            "rankfeatures",
            "rankfeatures",
            "",
            "featuredata",
        ));
        fields.push(SummaryField::implicit(
            "summaryfeatures",
            "summaryfeatures",
            "",
            "featuredata",
        ));

        for field in self.fields().iter().filter(|field| field.summary) {
            let (command, source) = if field.attribute {
                ("attribute", field.name.as_str())
            } else {
                ("", "")
            };
            fields.push(SummaryField::implicit(
                &field.name,
                command,
                source,
                field.field_type.summary_type(),
            ));
        }

        let mut output = String::from(
            "defaultsummaryid 1\nclasses[1]\nclasses[0].id 1\nclasses[0].name \"default\"\n",
        );
        output.push_str(&format!("classes[0].fields[{}]\n", fields.len()));
        for (index, field) in fields.iter().enumerate() {
            output.push_str(&format!(
                "classes[0].fields[{index}].name {}\nclasses[0].fields[{index}].type {}\nclasses[0].fields[{index}].command {}\nclasses[0].fields[{index}].source {}\n",
                quoted(&field.name),
                quoted(&field.data_type),
                quoted(&field.command),
                quoted(&field.source)
            ));
        }
        output
    }

    fn proton(&self) -> String {
        format!(
            "documentdb[1]\ndocumentdb[0].inputdoctypename {}\ndocumentdb[0].document_id_attribute true\n",
            quoted(&self.schema.document_type)
        )
    }
}

#[derive(Debug)]
struct SummaryField {
    name: String,
    command: String,
    source: String,
    data_type: String,
}

impl SummaryField {
    fn implicit(name: &str, command: &str, source: &str, data_type: &str) -> Self {
        Self {
            name: name.to_owned(),
            command: command.to_owned(),
            source: source.to_owned(),
            data_type: data_type.to_owned(),
        }
    }
}

impl FieldType {
    fn manifest_name(&self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::ArrayString => "array<string>",
            Self::ArrayStruct(_) => "array<struct>",
            Self::String => "string",
            Self::Long => "integer",
            Self::Integer => "integer",
            Self::Float => "float",
            Self::Boolean => "boolean",
            Self::Position => "position",
            Self::Predicate => "predicate",
            Self::Tensor(_) => "tensor",
        }
    }

    fn attribute_type(&self) -> &'static str {
        match self {
            Self::Text | Self::ArrayString | Self::String => "STRING",
            Self::ArrayStruct(_) => "STRUCT",
            Self::Long => "INT64",
            Self::Integer => "INT32",
            Self::Float => "DOUBLE",
            Self::Boolean => "BOOL",
            Self::Position => "POSITION",
            Self::Predicate => "PREDICATE",
            Self::Tensor(_) => "TENSOR",
        }
    }

    fn index_collection_type(&self) -> &'static str {
        match self {
            Self::ArrayString | Self::ArrayStruct(_) => "ARRAY",
            _ => "SINGLE",
        }
    }

    fn attribute_collection_type(&self) -> &'static str {
        match self {
            Self::ArrayString | Self::ArrayStruct(_) => "ARRAY",
            _ => "SINGLE",
        }
    }

    fn summary_type(&self) -> &'static str {
        match self {
            Self::Text | Self::ArrayString | Self::String => "longstring",
            Self::ArrayStruct(_) => "longstring",
            Self::Long | Self::Integer => "integer",
            Self::Float => "double",
            Self::Boolean => "bool",
            Self::Position => "position",
            Self::Predicate => "predicate",
            Self::Tensor(_) => "tensor",
        }
    }
}

impl StructFieldType {
    fn manifest_name(&self) -> &'static str {
        match self {
            Self::String => "string",
            Self::Long | Self::Integer => "integer",
            Self::Float => "float",
            Self::Boolean => "boolean",
        }
    }

    fn attribute_type(&self) -> &'static str {
        match self {
            Self::String => "STRING",
            Self::Long => "INT64",
            Self::Integer => "INT32",
            Self::Float => "DOUBLE",
            Self::Boolean => "BOOL",
        }
    }
}

fn validate_schema(schema: &Schema) -> Result<()> {
    validate_name(&schema.document_type, "document type")?;
    for (index, field) in schema.fields.iter().enumerate() {
        validate_name(&field.name, &format!("field at index {index}"))?;
        if RESERVED_FIELD_NAMES.contains(&field.name.as_str()) {
            return Err(Error::message(format!(
                "reserved field name '{}'",
                field.name
            )));
        }
        if schema
            .fields
            .iter()
            .take(index)
            .any(|previous| previous.name == field.name)
        {
            return Err(Error::message(format!(
                "duplicate field name '{}'",
                field.name
            )));
        }

        match &field.field_type {
            FieldType::Text if !field.indexed => {
                return Err(Error::message(format!(
                    "text field '{}' must be indexed",
                    field.name
                )));
            }
            FieldType::Text => {}
            FieldType::ArrayString if !field.indexed && !field.attribute => {
                return Err(Error::message(format!(
                    "array string field '{}' must be indexed or an attribute",
                    field.name
                )));
            }
            FieldType::ArrayString => {}
            FieldType::Position | FieldType::Predicate if !field.attribute => {
                return Err(Error::message(format!(
                    "special field '{}' must be an attribute",
                    field.name
                )));
            }
            FieldType::Position | FieldType::Predicate if field.indexed => {
                return Err(Error::message(format!(
                    "special field '{}' may not be indexed",
                    field.name
                )));
            }
            FieldType::Position | FieldType::Predicate if field.summary => {
                return Err(Error::message(format!(
                    "special field '{}' may not be a summary field",
                    field.name
                )));
            }
            FieldType::Predicate if field.predicate_arity.is_none_or(|arity| arity == 0) => {
                return Err(Error::message(format!(
                    "predicate field '{}' must have a positive arity",
                    field.name
                )));
            }
            _ if field.indexed => {
                return Err(Error::message(format!(
                    "non-text field '{}' may not be indexed",
                    field.name
                )));
            }
            FieldType::Tensor(_) if !field.attribute => {
                return Err(Error::message(format!(
                    "tensor field '{}' must be an attribute",
                    field.name
                )));
            }
            FieldType::Tensor(type_spec) if type_spec.trim().is_empty() => {
                return Err(Error::message(format!(
                    "tensor field '{}' must have a type spec",
                    field.name
                )));
            }
            FieldType::Tensor(_) if field.summary => {
                return Err(Error::message(format!(
                    "tensor field '{}' may not be a summary field",
                    field.name
                )));
            }
            _ if !field.attribute => {
                return Err(Error::message(format!(
                    "non-text field '{}' must be an attribute",
                    field.name
                )));
            }
            _ => {}
        }
    }
    let mut profile_names: Vec<&String> = Vec::with_capacity(schema.rank_profiles.len());
    for (index, profile) in schema.rank_profiles.iter().enumerate() {
        validate_name(&profile.name, &format!("rank profile at index {index}"))?;
        if profile.name == "default" {
            return Err(Error::message("rank profile 'default' is reserved"));
        }
        if profile_names
            .iter()
            .any(|name| name.as_str() == profile.name.as_str())
        {
            return Err(Error::message(format!(
                "duplicate rank profile name '{}'",
                profile.name
            )));
        }
        profile_names.push(&profile.name);
        validate_rank_expression(&profile.first_phase, "first-phase ranking expression")?;
        let mut query_inputs: Vec<&String> = Vec::with_capacity(profile.query_inputs.len());
        for (name, type_spec) in &profile.query_inputs {
            validate_name(name, "query input")?;
            if type_spec.trim().is_empty() {
                return Err(Error::message(format!(
                    "query input '{name}' must have a type spec"
                )));
            }
            if query_inputs
                .iter()
                .any(|input| input.as_str() == name.as_str())
            {
                return Err(Error::message(format!("duplicate query input '{name}'")));
            }
            query_inputs.push(name);
        }
        let mut match_features: Vec<&String> = Vec::with_capacity(profile.match_features.len());
        for expression in &profile.match_features {
            validate_rank_expression(expression, "match feature expression")?;
            if match_features
                .iter()
                .any(|feature| feature.as_str() == expression.as_str())
            {
                return Err(Error::message(format!(
                    "duplicate match feature expression '{}'",
                    expression
                )));
            }
            match_features.push(expression);
        }
    }
    Ok(())
}

fn validate_rank_expression(expression: &str, kind: &str) -> Result<()> {
    if expression.is_empty() {
        return Err(Error::message(format!("{kind} must not be empty")));
    }
    if expression.len() > MAX_RANK_EXPRESSION_BYTES {
        return Err(Error::message(format!(
            "{kind} is too long: maximum length is {MAX_RANK_EXPRESSION_BYTES} bytes"
        )));
    }
    Ok(())
}

fn validate_name(value: &str, kind: &str) -> Result<()> {
    if value.len() > MAX_SCHEMA_IDENTIFIER_BYTES {
        return Err(Error::message(format!(
            "invalid {kind} name: maximum length is {MAX_SCHEMA_IDENTIFIER_BYTES} bytes"
        )));
    }
    let mut characters = value.chars();
    let valid_first = characters
        .next()
        .map(|character| character == '_' || character.is_ascii_alphabetic())
        .unwrap_or(false);
    if !valid_first
        || !characters.all(|character| character == '_' || character.is_ascii_alphanumeric())
    {
        return Err(Error::message(format!(
            "invalid {kind} name '{value}': {NAME_ERROR}"
        )));
    }
    Ok(())
}

fn quoted(value: &str) -> String {
    let mut output = String::with_capacity(value.len() + 2);
    output.push('"');
    for character in value.chars() {
        match character {
            '\\' => output.push_str("\\\\"),
            '"' => output.push_str("\\\""),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character => output.push(character),
        }
    }
    output.push('"');
    output
}

fn rank_profiles(schema: &Schema) -> String {
    if !schema.rank_profiles.is_empty() {
        let text_field = schema
            .fields
            .iter()
            .find(|field| field.field_type == FieldType::Text)
            .map(|field| field.name.as_str());
        let includes_generated_bm25 = text_field.is_some()
            && !schema
                .rank_profiles
                .iter()
                .any(|profile| profile.name == "bm25");
        let profile_count = schema.rank_profiles.len() + 1 + usize::from(includes_generated_bm25);
        let mut output = format!(
            "rankprofile[{}]\nrankprofile[0].name \"default\"\n",
            profile_count
        );
        if let Some(field) = text_field.filter(|_| includes_generated_bm25) {
            output.push_str(&format!(
                "rankprofile[1].name \"bm25\"\nrankprofile[1].fef.property[2]\nrankprofile[1].fef.property[0].name \"vespa.rank.firstphase\"\nrankprofile[1].fef.property[0].value \"rankingExpression(bm25)\"\nrankprofile[1].fef.property[1].name \"rankingExpression(bm25).rankingScript\"\nrankprofile[1].fef.property[1].value \"bm25({field})\"\n"
            ));
        }
        for (index, profile) in schema.rank_profiles.iter().enumerate() {
            let index = index + 1 + usize::from(includes_generated_bm25);
            let property_count = 2 + profile.match_features.len() + profile.query_inputs.len();
            output.push_str(&format!(
                "rankprofile[{index}].name {}\nrankprofile[{index}].fef.property[{property_count}]\n",
                quoted(&profile.name)
            ));
            output.push_str(&format!(
                "rankprofile[{index}].fef.property[0].name \"vespa.rank.firstphase\"\nrankprofile[{index}].fef.property[0].value \"rankingExpression({})\"\n",
                profile.name
            ));
            for (feature_index, feature) in profile.match_features.iter().enumerate() {
                output.push_str(&format!(
                    "rankprofile[{index}].fef.property[{}].name \"vespa.match.feature\"\nrankprofile[{index}].fef.property[{}].value {}\n",
                    feature_index + 1,
                    feature_index + 1,
                    quoted(feature)
                ));
            }
            for (query_index, (name, type_spec)) in profile.query_inputs.iter().enumerate() {
                let property_index = profile.match_features.len() + 1 + query_index;
                output.push_str(&format!(
                    "rankprofile[{index}].fef.property[{property_index}].name \"vespa.type.query.{}\"\nrankprofile[{index}].fef.property[{property_index}].value {}\n",
                    name,
                    quoted(type_spec)
                ));
            }
            let script_index = property_count - 1;
            output.push_str(&format!(
                "rankprofile[{index}].fef.property[{script_index}].name \"rankingExpression({}).rankingScript\"\nrankprofile[{index}].fef.property[{script_index}].value {}\n",
                profile.name,
                quoted(&profile.first_phase)
            ));
        }
        return output;
    }
    let text_field = schema
        .fields
        .iter()
        .find(|field| field.field_type == FieldType::Text)
        .map(|field| field.name.as_str());
    match text_field {
        Some(field) => format!(
            "rankprofile[2]\nrankprofile[0].name \"default\"\nrankprofile[1].name \"bm25\"\nrankprofile[1].fef.property[1]\nrankprofile[1].fef.property[0].name \"vespa.rank.firstphase\"\nrankprofile[1].fef.property[0].value \"bm25({field})\"\n"
        ),
        None => "rankprofile[1]\nrankprofile[0].name \"default\"\n".to_string(),
    }
}

fn write_config(directory: &Path, name: &str, content: &str) -> Result<()> {
    let path = directory.join(name);
    let mut file = File::create(&path).map_err(|error| {
        Error::message(format!("could not create '{}': {error}", path.display()))
    })?;
    file.write_all(content.as_bytes()).map_err(|error| {
        Error::message(format!("could not write '{}': {error}", path.display()))
    })?;
    file.sync_all()
        .map_err(|error| Error::message(format!("could not sync '{}': {error}", path.display())))
}

fn sync_directory(path: &Path) -> Result<()> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| {
            Error::message(format!(
                "could not sync directory '{}': {error}",
                path.display()
            ))
        })
}

#[derive(Debug, Clone, Copy)]
enum OutputState {
    Absent,
    Empty,
}

impl OutputState {
    fn inspect(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::Absent);
        }
        if !path.is_dir() {
            return Err(Error::message(format!(
                "configuration output is not a directory: {}",
                path.display()
            )));
        }
        let mut entries = fs::read_dir(path).map_err(|error| {
            Error::message(format!(
                "could not inspect configuration output '{}': {error}",
                path.display()
            ))
        })?;
        if entries.next().is_some() {
            return Err(Error::message(format!(
                "configuration output directory is not empty: {}",
                path.display()
            )));
        }
        Ok(Self::Empty)
    }
}

#[derive(Debug)]
struct TemporaryDirectory {
    path: PathBuf,
    installed: bool,
}

impl TemporaryDirectory {
    fn create(output: &Path) -> Result<Self> {
        let parent = output.parent().unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent).map_err(|error| {
            Error::message(format!(
                "could not create configuration parent '{}': {error}",
                parent.display()
            ))
        })?;
        let name = output
            .file_name()
            .ok_or_else(|| Error::message("configuration output must have a directory name"))?
            .to_string_lossy();

        for attempt in 0..100u32 {
            let candidate = parent.join(format!(".{name}.searchkernel-{attempt}"));
            match fs::create_dir(&candidate) {
                Ok(()) => {
                    return Ok(Self {
                        path: candidate,
                        installed: false,
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(Error::message(format!(
                        "could not create temporary configuration directory '{}': {error}",
                        candidate.display()
                    )));
                }
            }
        }
        Err(Error::message(format!(
            "could not allocate a temporary configuration directory beside '{}'",
            output.display()
        )))
    }

    fn install(mut self, output: &Path, state: OutputState) -> Result<()> {
        sync_directory(&self.path)?;
        if matches!(state, OutputState::Empty) {
            fs::remove_dir(output).map_err(|error| {
                Error::message(format!(
                    "could not replace empty configuration output '{}': {error}",
                    output.display()
                ))
            })?;
        }
        fs::rename(&self.path, output).map_err(|error| {
            Error::message(format!(
                "could not install generated configuration '{}': {error}",
                output.display()
            ))
        })?;
        self.installed = true;
        let parent = output.parent().unwrap_or_else(|| Path::new("."));
        sync_directory(parent)
    }

    fn remove(&self) {
        if !self.installed {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        self.remove();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temporary_path(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("searchkernel-m6-{label}-{nonce}"))
    }

    fn compile(schema: Schema, label: &str) -> PathBuf {
        let path = temporary_path(label);
        ConfigCompiler
            .compile_to(&schema, &path)
            .expect("compile schema");
        path
    }

    #[test]
    fn validates_names_and_capabilities() {
        let invalid = [
            Schema::new(""),
            Schema::new("typea").field(Field::new("bad-name", FieldType::Text).indexed()),
            Schema::new("typea").field(Field::new("title", FieldType::Text)),
            Schema::new("typea").field(Field::new("sku", FieldType::String)),
            Schema::new("typea").field(Field::new("sku", FieldType::String).indexed().attribute()),
            Schema::new("typea").field(Field::new("qty", FieldType::Integer).summary()),
        ];
        for schema in invalid {
            assert!(ConfigCompiler
                .compile_to(&schema, temporary_path("invalid"))
                .is_err());
        }
        for name in RESERVED_FIELD_NAMES {
            assert!(ConfigCompiler
                .compile_to(
                    &Schema::new("typea").field(Field::new(*name, FieldType::Text).indexed()),
                    temporary_path("reserved"),
                )
                .is_err());
        }

        let duplicate = Schema::new("typea")
            .field(Field::new("title", FieldType::Text).indexed())
            .field(Field::new("title", FieldType::Text).indexed());
        assert!(ConfigCompiler
            .compile_to(&duplicate, temporary_path("duplicate"))
            .is_err());
    }

    #[test]
    fn identifier_length_matches_native_contract() {
        for length in [255, 256] {
            let document_type = format!("a{}", "x".repeat(length - 1));
            let document_path = temporary_path(&format!("document-length-{length}"));
            ConfigCompiler
                .compile_to(&Schema::new(document_type), &document_path)
                .expect("document type at the limit should compile");
            let _ = fs::remove_dir_all(document_path);

            let field_name = format!("a{}", "x".repeat(length - 1));
            let field_path = temporary_path(&format!("field-length-{length}"));
            ConfigCompiler
                .compile_to(
                    &Schema::new("typea")
                        .field(Field::new(field_name, FieldType::Integer).attribute()),
                    &field_path,
                )
                .expect("field name at the limit should compile");
            let _ = fs::remove_dir_all(field_path);
        }

        for (label, schema) in [
            (
                "document-length-257",
                Schema::new(format!("a{}", "x".repeat(256))),
            ),
            (
                "field-length-257",
                Schema::new("typea").field(
                    Field::new(format!("a{}", "x".repeat(256)), FieldType::Integer).attribute(),
                ),
            ),
        ] {
            assert!(ConfigCompiler
                .compile_to(&schema, temporary_path(label))
                .is_err());
        }
    }

    #[test]
    fn compiles_multiple_indexed_text_fields() {
        let schema = Schema::new("products")
            .field(Field::new("title", FieldType::Text).indexed().summary())
            .field(
                Field::new("description", FieldType::Text)
                    .indexed()
                    .attribute()
                    .summary(),
            );
        let path = compile(schema, "multiple-text-fields");
        let index = fs::read_to_string(path.join("indexschema.cfg")).expect("index config");
        assert!(index.contains("indexfield[0].name \"title\""));
        assert!(index.contains("indexfield[1].name \"description\""));
        let _ = fs::remove_dir_all(path);
    }

    #[test]
    fn derives_each_supported_type() {
        let path = compile(
            Schema::new("typea")
                .field(Field::new("title", FieldType::Text).indexed().summary())
                .field(Field::new("sku", FieldType::String).attribute().summary())
                .field(Field::new("qty", FieldType::Integer).attribute().summary())
                .field(Field::new("price", FieldType::Float).attribute().summary())
                .field(
                    Field::new("active", FieldType::Boolean)
                        .attribute()
                        .summary(),
                ),
            "types",
        );
        let index = fs::read_to_string(path.join("indexschema.cfg")).expect("index config");
        let attributes = fs::read_to_string(path.join("attributes.cfg")).expect("attribute config");
        let summary = fs::read_to_string(path.join("summary.cfg")).expect("summary config");
        let rank_profiles =
            fs::read_to_string(path.join("rank-profiles.cfg")).expect("rank profiles");
        assert!(index.contains("indexfield[0].name \"title\""));
        assert!(index.contains("indexfield[0].positions true"));
        assert!(index.contains("indexfield[0].interleavedfeatures false"));
        assert!(attributes.contains("attribute[0].datatype STRING"));
        assert!(attributes.contains("attribute[1].datatype INT32"));
        assert!(attributes.contains("attribute[2].datatype DOUBLE"));
        assert!(attributes.contains("attribute[3].datatype BOOL"));
        assert!(summary.contains("classes[0].fields[0].name \"documentid\""));
        assert!(summary.contains("classes[0].fields[7].name \"active\""));
        assert!(rank_profiles.contains("rankprofile[1].name \"bm25\""));
        assert!(rank_profiles.contains("bm25(title)"));
        let _ = fs::remove_dir_all(path);
    }

    #[test]
    fn output_is_deterministic_and_rejects_non_empty_directories() {
        let schema = Schema::new("typea")
            .field(Field::new("title", FieldType::Text).indexed().summary())
            .field(Field::new("qty", FieldType::Integer).attribute());
        let first = compile(schema.clone(), "deterministic-a");
        let second = compile(schema, "deterministic-b");
        for name in [
            "indexschema.cfg",
            "attributes.cfg",
            "summary.cfg",
            "rank-profiles.cfg",
            "juniperrc.cfg",
            "proton.cfg",
            "searchkernel-schema.json",
        ] {
            assert_eq!(
                fs::read(first.join(name)).expect("first config"),
                fs::read(second.join(name)).expect("second config")
            );
        }
        fs::write(first.join("stale"), "stale").expect("stale marker");
        assert!(ConfigCompiler
            .compile_to(&Schema::new("typea"), &first)
            .is_err());
        let _ = fs::remove_dir_all(first);
        let _ = fs::remove_dir_all(second);
    }
}
