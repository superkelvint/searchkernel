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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// A field type supported by the embedded Vespa configuration compiler.
pub enum FieldType {
    Text,
    String,
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
    summary: bool,
}

impl Field {
    /// Create a field with no indexing or storage capabilities enabled.
    pub fn new(name: impl Into<String>, field_type: FieldType) -> Self {
        Self {
            name: name.into(),
            field_type,
            indexed: false,
            attribute: false,
            summary: false,
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

    /// Include this field in the default document summary.
    pub fn summary(mut self) -> Self {
        self.summary = true;
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
}

impl RankProfile {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            first_phase: String::new(),
            match_features: Vec::new(),
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
            output.push_str(&format!(
                "{{\"name\":{},\"type\":{},\"index\":{},\"attribute\":{},\"summary\":{}}}",
                quoted(&field.name),
                quoted(field.field_type.manifest_name()),
                field.indexed,
                field.attribute,
                field.summary,
            ));
        }
        output.push_str("]}\n");
        output
    }

    fn indexschema(&self) -> String {
        let indexed: Vec<&Field> = self.fields().iter().filter(|field| field.indexed).collect();
        let mut output = format!("indexfield[{}]\n", indexed.len());
        for (index, field) in indexed.iter().enumerate() {
            output.push_str(&format!(
                "indexfield[{index}].name {}\nindexfield[{index}].datatype STRING\nindexfield[{index}].collectiontype SINGLE\nindexfield[{index}].prefix false\nindexfield[{index}].phrases false\nindexfield[{index}].positions true\nindexfield[{index}].averageelementlen 512\nindexfield[{index}].interleavedfeatures false\n",
                quoted(&field.name)
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
        let mut output = format!("attribute[{}]\n", attributes.len());
        for (index, field) in attributes.iter().enumerate() {
            output.push_str(&format!(
                "attribute[{index}].name {}\nattribute[{index}].datatype {}\nattribute[{index}].collectiontype SINGLE\n",
                quoted(&field.name),
                field.field_type.attribute_type()
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
    fn manifest_name(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::String => "string",
            Self::Integer => "integer",
            Self::Float => "float",
            Self::Boolean => "boolean",
        }
    }

    fn attribute_type(self) -> &'static str {
        match self {
            Self::Text | Self::String => "STRING",
            Self::Integer => "INT64",
            Self::Float => "DOUBLE",
            Self::Boolean => "BOOL",
        }
    }

    fn summary_type(self) -> &'static str {
        match self {
            Self::Text | Self::String => "longstring",
            Self::Integer => "integer",
            Self::Float => "double",
            Self::Boolean => "bool",
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

        match field.field_type {
            FieldType::Text if !field.indexed => {
                return Err(Error::message(format!(
                    "text field '{}' must be indexed",
                    field.name
                )));
            }
            FieldType::Text => {}
            _ if field.indexed => {
                return Err(Error::message(format!(
                    "non-text field '{}' may not be indexed",
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
        let mut output = format!(
            "rankprofile[{}]\nrankprofile[0].name \"default\"\n",
            schema.rank_profiles.len() + 1
        );
        for (index, profile) in schema.rank_profiles.iter().enumerate() {
            let index = index + 1;
            let property_count = 2 + profile.match_features.len();
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
    file.write_all(content.as_bytes())
        .map_err(|error| Error::message(format!("could not write '{}': {error}", path.display())))
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
        Ok(())
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
        assert!(attributes.contains("attribute[1].datatype INT64"));
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
