use super::{definition_from_ir, definition_to_ir};
#[cfg(feature = "native")]
use crate::error::{Error, Result};
#[cfg(feature = "native")]
use crate::schema::{Analyzer, Field, IndexDefinition, SearchField};
#[cfg(feature = "native")]
use searchkernel_ir::wire::v1::schema as api;
#[cfg(feature = "native")]
use searchkernel_ir::IndexContext;
#[cfg(feature = "native")]
use std::io::Write;

#[cfg(feature = "native")]
pub(crate) const MANIFEST_FORMAT_VERSION: u32 = 4;
#[cfg(feature = "native")]
pub(crate) const PREVIOUS_MANIFEST_FORMAT_VERSION: u32 = 3;
#[cfg(feature = "native")]
pub(crate) const LEGACY_MANIFEST_FORMAT_VERSION: u32 = 2;
#[cfg(feature = "native")]
pub(crate) const STORAGE_COMPATIBILITY_GENERATION: u32 = 1;
#[cfg(feature = "native")]
pub(crate) const RESOLVED_SCHEMA_SNAPSHOT_VERSION: u32 = 1;
#[cfg(feature = "native")]
pub(crate) const VESPA_REVISION: &str = "2300ef0412bdb76f227ae3b497c8ca5e73c5da4e";

#[cfg(feature = "native")]
#[derive(Debug, Clone)]
pub(crate) struct IndexManifest {
    pub(crate) value: serde_json::Value,
    pub(crate) portable_definition_bytes: Vec<u8>,
    pub(crate) resolved_definition_bytes: Vec<u8>,
}

#[cfg(feature = "native")]
fn atomic_write(path: &std::path::Path, bytes: &[u8]) -> Result<()> {
    let directory = path
        .parent()
        .ok_or_else(|| Error::infrastructure("manifest path has no parent directory"))?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| Error::infrastructure("manifest path has an invalid file name"))?;
    let temporary = directory.join(format!(".{file_name}.tmp"));

    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&temporary)
        .map_err(|error| Error::Infrastructure(error.to_string()))?;
    file.write_all(bytes)
        .map_err(|error| Error::Infrastructure(error.to_string()))?;
    file.sync_all()
        .map_err(|error| Error::Infrastructure(error.to_string()))?;
    drop(file);

    std::fs::rename(&temporary, path).map_err(|error| Error::Infrastructure(error.to_string()))?;
    std::fs::File::open(directory)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| Error::Infrastructure(error.to_string()))
}

#[cfg(feature = "native")]
impl IndexManifest {
    pub(crate) fn for_definitions(
        portable_definition_bytes: &[u8],
        resolved_context: &IndexContext,
        config: &std::path::Path,
    ) -> Result<Self> {
        Ok(Self {
            value: serde_json::json!({
                "manifest_format_version": MANIFEST_FORMAT_VERSION,
                "storage_compatibility_generation": STORAGE_COMPATIBILITY_GENERATION,
                "resolved_schema_snapshot_version": RESOLVED_SCHEMA_SNAPSHOT_VERSION,
                "resolved_schema_snapshot": searchkernel_ir::snapshot::index_definition(
                    resolved_context.definition(),
                ),
                // Keep caller-visible document_type lossless in the canonical
                // FlatBuffer, but durably record the concrete backend identity
                // that generated the immutable engine configuration.
                "resolved_document_type": resolved_context.definition().resolved_document_type(),
                "provenance": {
                    "created_by_searchkernel_version": env!("CARGO_PKG_VERSION"),
                    "created_by_native_abi": searchkernel_vespa_bindings::SK_NATIVE_ABI_VERSION,
                    "created_by_vespa_revision": VESPA_REVISION,
                },
                "portable_index_definition": "portable-index-definition.bfbs",
                "canonical_index_definition": "index-definition.bfbs",
                "resolved_engine_config_identity": config_identity(config)?,
            }),
            portable_definition_bytes: portable_definition_bytes.to_vec(),
            resolved_definition_bytes: searchkernel_ir::wire::v1::index_definition_ir_to_api(
                resolved_context.definition(),
            ),
        })
    }

    pub(crate) fn write(&self, root: &std::path::Path) -> Result<()> {
        let directory = root.join("manifest");
        std::fs::create_dir_all(&directory)
            .map_err(|error| Error::Infrastructure(error.to_string()))?;
        let bytes = serde_json::to_vec_pretty(&self.value)
            .map_err(|error| Error::Infrastructure(error.to_string()))?;
        atomic_write(
            &directory.join("index-definition.bfbs"),
            &self.resolved_definition_bytes,
        )?;
        atomic_write(
            &directory.join("portable-index-definition.bfbs"),
            &self.portable_definition_bytes,
        )?;
        atomic_write(&directory.join("index.json"), &bytes)
    }

    pub(crate) fn read(root: &std::path::Path) -> Result<Self> {
        let path = root.join("manifest/index.json");
        let bytes = std::fs::read(&path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                Error::corrupt_index(path.display().to_string())
            } else {
                Error::Infrastructure(error.to_string())
            }
        })?;
        let value: serde_json::Value = serde_json::from_slice(&bytes)
            .map_err(|error| Error::corrupt_index(error.to_string()))?;
        let version = value
            .get("manifest_format_version")
            .or_else(|| value.get("format_version"))
            .and_then(serde_json::Value::as_u64);
        let (resolved_definition_bytes, portable_definition_bytes) = if matches!(
            version,
            Some(value)
                if value == u64::from(MANIFEST_FORMAT_VERSION)
                    || value == u64::from(PREVIOUS_MANIFEST_FORMAT_VERSION)
        ) {
            let read_definition = |name: &str| {
                let path = root.join("manifest").join(name);
                std::fs::read(&path).map_err(|error| {
                    if error.kind() == std::io::ErrorKind::NotFound {
                        Error::corrupt_index(path.display().to_string())
                    } else {
                        Error::Infrastructure(error.to_string())
                    }
                })
            };
            (
                read_definition("index-definition.bfbs")?,
                read_definition("portable-index-definition.bfbs")?,
            )
        } else {
            (Vec::new(), Vec::new())
        };
        let manifest = Self {
            value,
            portable_definition_bytes,
            resolved_definition_bytes,
        };
        manifest.validate_identity()?;
        if matches!(
            version,
            Some(value)
                if value == u64::from(MANIFEST_FORMAT_VERSION)
                    || value == u64::from(PREVIOUS_MANIFEST_FORMAT_VERSION)
        ) {
            // The portable definition is provenance. Its raw FlatBuffers
            // envelope must remain readable, but its semantics were resolved
            // under the creator's defaults and must not be reinterpreted on
            // reopen.
            let _portable_root = searchkernel_ir::__flatbuffers::root::<api::IndexDefinition>(
                &manifest.portable_definition_bytes,
            )
            .map_err(|error| {
                Error::corrupt_index(format!("invalid portable index definition: {error}"))
            })?;
            if version == Some(u64::from(MANIFEST_FORMAT_VERSION)) {
                manifest.resolved_definition_ir()?;
            } else {
                manifest.decode_resolved_definition_ir()?;
            }
        }
        manifest.resolved_context()?;
        Ok(manifest)
    }

    pub(crate) fn validate_identity(&self) -> Result<()> {
        let object = self
            .value
            .as_object()
            .ok_or_else(|| Error::corrupt_index("manifest root is not an object"))?;
        let version = object
            .get("manifest_format_version")
            .or_else(|| object.get("format_version"))
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| Error::corrupt_index("manifest format version is missing"))?;
        match version {
            value if value == u64::from(MANIFEST_FORMAT_VERSION) => {
                Self::validate_binary_identity(object, true)?;
            }
            value if value == u64::from(PREVIOUS_MANIFEST_FORMAT_VERSION) => {
                Self::validate_binary_identity(object, false)?;
            }
            value if value == u64::from(LEGACY_MANIFEST_FORMAT_VERSION) => {
                for key in [
                    "searchkernel_version",
                    "native_abi_version",
                    "vespa_revision",
                ] {
                    if !object.contains_key(key) {
                        return Err(Error::corrupt_index(format!(
                            "legacy manifest {key} is missing"
                        )));
                    }
                }
                if !object
                    .get("portable_index_definition")
                    .is_some_and(serde_json::Value::is_object)
                    || !object
                        .get("resolved_index_definition")
                        .is_some_and(serde_json::Value::is_object)
                {
                    return Err(Error::corrupt_index(
                        "legacy manifest index definition is invalid",
                    ));
                }
            }
            _ => {
                return Err(Error::incompatible_index(format!(
                    "manifest format version {version}"
                )));
            }
        }
        if !object
            .get("resolved_engine_config_identity")
            .is_some_and(serde_json::Value::is_object)
        {
            return Err(Error::corrupt_index(
                "manifest resolved_engine_config_identity is invalid",
            ));
        }
        self.persisted_resolved_document_type()?;
        Ok(())
    }

    pub(crate) fn validate_config(&self, config: &std::path::Path) -> Result<()> {
        for name in [
            "searchkernel-schema.json",
            "indexschema.cfg",
            "attributes.cfg",
            "summary.cfg",
            "rank-profiles.cfg",
            "juniperrc.cfg",
            "proton.cfg",
        ] {
            if !config.join(name).is_file() {
                return Err(Error::corrupt_index(format!(
                    "resolved engine configuration is missing {name}"
                )));
            }
        }
        let expected = self
            .value
            .get("resolved_engine_config_identity")
            .ok_or_else(|| Error::corrupt_index("manifest config identity is missing"))?;
        let actual = config_identity(config)?;
        if expected != &actual {
            return Err(Error::incompatible_index(
                "resolved engine configuration differs",
            ));
        }
        Ok(())
    }

    fn persisted_resolved_document_type(&self) -> Result<Option<&str>> {
        match self.value.get("resolved_document_type") {
            None => Ok(None),
            Some(value) => value
                .as_str()
                .map(Some)
                .ok_or_else(|| Error::corrupt_index("manifest resolved_document_type is invalid")),
        }
    }

    fn compiled_document_type(config: &std::path::Path) -> Result<String> {
        let schema_path = config.join("searchkernel-schema.json");
        let schema_bytes = std::fs::read(&schema_path)
            .map_err(|error| Error::Infrastructure(error.to_string()))?;
        let schema: serde_json::Value = serde_json::from_slice(&schema_bytes).map_err(|error| {
            Error::corrupt_index(format!(
                "resolved engine schema manifest is invalid: {error}"
            ))
        })?;
        schema
            .get("document_type")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| {
                Error::corrupt_index("resolved engine schema document_type is missing or invalid")
            })
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn definition(&self) -> Result<IndexDefinition> {
        let version = self
            .value
            .get("manifest_format_version")
            .or_else(|| self.value.get("format_version"))
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| Error::corrupt_index("manifest format version is missing"))?;
        if version == u64::from(MANIFEST_FORMAT_VERSION)
            || version == u64::from(PREVIOUS_MANIFEST_FORMAT_VERSION)
        {
            let context = self.resolved_context()?;
            return Ok(definition_from_ir(context.definition()));
        }
        definition_from_json(
            self.value
                .get("resolved_index_definition")
                .ok_or_else(|| Error::corrupt_index("resolved definition is missing"))?,
        )
    }

    pub(crate) fn resolved_context(&self) -> Result<IndexContext> {
        let mut definition = self.decoded_definition_ir()?;
        if let Some(document_type) = self.persisted_resolved_document_type()? {
            definition.set_resolved_document_type(document_type);
        }
        Ok(IndexContext::from_definition_ir(definition))
    }

    /// Recover the exact backend document identity used by the persisted
    /// engine configuration before opening Vespa.
    ///
    /// Manifests created after identity persistence carry the value directly.
    /// Historical v2/v3/v4 manifests do not, so the immutable compiled schema
    /// is their compatibility authority. Never replay today's fallback over an
    /// older index.
    pub(crate) fn resolved_context_with_config(
        &self,
        config: &std::path::Path,
    ) -> Result<IndexContext> {
        self.validate_config(config)?;
        let mut definition = self.decoded_definition_ir()?;
        let compiled_document_type = Self::compiled_document_type(config)?;
        let resolved_document_type = match self.persisted_resolved_document_type()? {
            Some(persisted) if persisted != compiled_document_type => {
                return Err(Error::incompatible_index(
                    "resolved document identity differs from compiled engine configuration",
                ));
            }
            Some(persisted) => persisted.to_owned(),
            None => compiled_document_type,
        };
        definition.set_resolved_document_type(resolved_document_type);
        Ok(IndexContext::from_definition_ir(definition))
    }

    fn decoded_definition_ir(&self) -> Result<searchkernel_ir::IndexDefinitionIr> {
        let version = self
            .value
            .get("manifest_format_version")
            .or_else(|| self.value.get("format_version"))
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| Error::corrupt_index("manifest format version is missing"))?;
        if version == u64::from(MANIFEST_FORMAT_VERSION) {
            self.resolved_definition_ir()
        } else if version == u64::from(PREVIOUS_MANIFEST_FORMAT_VERSION) {
            self.decode_resolved_definition_ir()
        } else {
            let definition = definition_from_json(
                self.value
                    .get("resolved_index_definition")
                    .ok_or_else(|| Error::corrupt_index("resolved definition is missing"))?,
            )?;
            definition_to_ir(&definition)
        }
    }

    fn validate_binary_identity(
        object: &serde_json::Map<String, serde_json::Value>,
        require_snapshot: bool,
    ) -> Result<()> {
        let generation = object
            .get("storage_compatibility_generation")
            .and_then(serde_json::Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
            .ok_or_else(|| {
                Error::corrupt_index("manifest storage compatibility generation is missing")
            })?;
        if generation != STORAGE_COMPATIBILITY_GENERATION {
            return Err(Error::incompatible_index(format!(
                "storage compatibility generation {generation}"
            ))
            .with_detail(
                "unsupported_storage_generation",
                Some("storage_compatibility_generation"),
                format!(
                    "storage generation {generation} is not supported; this release supports generation {STORAGE_COMPATIBILITY_GENERATION}"
                ),
            ));
        }
        if require_snapshot {
            let snapshot_version = object
                .get("resolved_schema_snapshot_version")
                .and_then(serde_json::Value::as_u64)
                .and_then(|value| u32::try_from(value).ok())
                .ok_or_else(|| {
                    Error::corrupt_index("resolved schema snapshot version is missing or invalid")
                })?;
            if snapshot_version != RESOLVED_SCHEMA_SNAPSHOT_VERSION {
                return Err(Error::incompatible_index(format!(
                    "resolved schema snapshot version {snapshot_version}"
                )));
            }
            if !object
                .get("resolved_schema_snapshot")
                .is_some_and(serde_json::Value::is_object)
            {
                return Err(Error::corrupt_index(
                    "resolved schema snapshot is missing or invalid",
                ));
            }
        }
        let provenance = object
            .get("provenance")
            .and_then(serde_json::Value::as_object)
            .ok_or_else(|| Error::corrupt_index("manifest provenance is invalid"))?;
        if !provenance
            .get("created_by_searchkernel_version")
            .is_some_and(serde_json::Value::is_string)
            || !provenance
                .get("created_by_native_abi")
                .and_then(serde_json::Value::as_u64)
                .and_then(|value| u32::try_from(value).ok())
                .is_some()
            || !provenance
                .get("created_by_vespa_revision")
                .is_some_and(serde_json::Value::is_string)
        {
            return Err(Error::corrupt_index(
                "manifest provenance has invalid fields",
            ));
        }
        if object
            .get("portable_index_definition")
            .and_then(serde_json::Value::as_str)
            != Some("portable-index-definition.bfbs")
            || object
                .get("canonical_index_definition")
                .and_then(serde_json::Value::as_str)
                != Some("index-definition.bfbs")
        {
            return Err(Error::corrupt_index(
                "manifest index definition references are invalid",
            ));
        }
        Ok(())
    }

    fn decode_resolved_definition_ir(&self) -> Result<searchkernel_ir::IndexDefinitionIr> {
        let root = searchkernel_ir::__flatbuffers::root::<api::IndexDefinition>(
            &self.resolved_definition_bytes,
        )
        .map_err(|error| {
            Error::corrupt_index(format!("invalid resolved index definition: {error}"))
        })?;
        searchkernel_ir::wire::v1::index_definition_to_ir(root)
            .map_err(|_| Error::incompatible_index("resolved schema snapshot is not compatible"))
    }

    fn resolved_definition_ir(&self) -> Result<searchkernel_ir::IndexDefinitionIr> {
        // The FlatBuffers decoder also serves unresolved caller intent and may
        // apply defaults.  The v4 persisted IR snapshot makes that replay
        // explicit: any changed default or normalization is rejected before
        // the decoded definition can become the reopened context. v3 did not
        // persist this sidecar, so its compatibility path decodes only the
        // already-resolved canonical definition and never portable intent.
        let resolved = self.decode_resolved_definition_ir()?;
        let expected = self
            .value
            .get("resolved_schema_snapshot")
            .ok_or_else(|| Error::corrupt_index("resolved schema snapshot is missing"))?;
        if searchkernel_ir::snapshot::index_definition(&resolved) != *expected {
            return Err(Error::incompatible_index(
                "resolved schema snapshot is not compatible",
            ));
        }
        Ok(resolved)
    }
}

#[cfg(feature = "native")]
pub(crate) fn definition_from_json(value: &serde_json::Value) -> Result<IndexDefinition> {
    let object = value
        .as_object()
        .ok_or_else(|| Error::corrupt_index("portable definition is not an object"))?;
    let mut definition = IndexDefinition::new();
    if let Some(name) = object.get("name").and_then(serde_json::Value::as_str) {
        definition = definition.name(name);
    }
    let fields = object
        .get("fields")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| Error::corrupt_index("portable definition fields are missing"))?;
    for field in fields {
        let metadata = field
            .as_object()
            .ok_or_else(|| Error::corrupt_index("manifest field is not an object"))?;
        let name = metadata
            .get("name")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| Error::corrupt_index("manifest field name is missing"))?;
        let kind = match metadata.get("kind").and_then(serde_json::Value::as_str) {
            Some("text" | "Text") => Field::text(name),
            Some("keyword" | "Keyword") => Field::keyword(name),
            Some("int64" | "Int64") => Field::int64(name),
            Some("float64" | "Float64") => Field::float64(name),
            Some("bool" | "Bool") => Field::bool(name),
            _ => return Err(Error::corrupt_index("manifest field kind is invalid")),
        };
        let mut field = kind;
        if let Some(analyzer) = metadata.get("analyzer") {
            if !analyzer.is_null() {
                if let Some(analyzer) = analyzer.as_str() {
                    field = match analyzer {
                        "standard-v1" | "StandardV1" => field.analyzer(Analyzer::standard_v1()),
                        _ => {
                            return Err(Error::corrupt_index("manifest field analyzer is invalid"))
                        }
                    };
                } else {
                    let analyzer = analyzer.as_object().ok_or_else(|| {
                        Error::corrupt_index("manifest field analyzer is invalid")
                    })?;
                    let name = analyzer
                        .get("name")
                        .and_then(serde_json::Value::as_str)
                        .ok_or_else(|| {
                            Error::corrupt_index("manifest field analyzer name is missing")
                        })?;
                    let version = analyzer
                        .get("version")
                        .and_then(serde_json::Value::as_u64)
                        .and_then(|value| u32::try_from(value).ok())
                        .ok_or_else(|| {
                            Error::corrupt_index("manifest field analyzer version is invalid")
                        })?;
                    let options = analyzer
                        .get("options")
                        .map(|options| {
                            options
                                .as_object()
                                .ok_or_else(|| {
                                    Error::corrupt_index(
                                        "manifest field analyzer options are invalid",
                                    )
                                })
                                .map(|options| {
                                    options
                                        .iter()
                                        .map(|(key, value)| (key.clone(), value.clone()))
                                        .collect()
                                })
                        })
                        .transpose()?
                        .unwrap_or_default();
                    field = field.analyzer(Analyzer {
                        name: name.to_owned(),
                        version,
                        options,
                    });
                }
            }
        }
        if metadata
            .get("multi")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
        {
            field = field.multi();
        }
        for (key, setter) in [
            ("stored", 0_u8),
            ("searchable", 1),
            ("filterable", 2),
            ("sortable", 3),
            ("facetable", 4),
        ] {
            if let Some(value) = metadata.get(key).and_then(serde_json::Value::as_bool) {
                field = match setter {
                    0 => field.stored(value),
                    1 => field.searchable(value),
                    2 => field.filterable(value),
                    3 => field.sortable(value),
                    _ => field.facetable(value),
                };
            }
        }
        definition = definition.field(field);
    }
    if let Some(search_fields) = object
        .get("default_search_fields")
        .and_then(serde_json::Value::as_array)
    {
        let mut fields = Vec::with_capacity(search_fields.len());
        for search_field in search_fields {
            let search_field = search_field
                .as_object()
                .ok_or_else(|| Error::corrupt_index("manifest search field is not an object"))?;
            let name = search_field
                .get("field")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| Error::corrupt_index("manifest search field name is missing"))?;
            let mut field = SearchField::new(name);
            if let Some(weight) = search_field
                .get("weight")
                .and_then(serde_json::Value::as_f64)
            {
                field = field.weight(weight);
            }
            fields.push(field);
        }
        definition = definition.default_search_fields(fields);
    }
    Ok(definition)
}

#[cfg(feature = "native")]
pub(crate) fn config_identity(config: &std::path::Path) -> Result<serde_json::Value> {
    let mut files = serde_json::Map::new();
    for name in [
        "searchkernel-schema.json",
        "indexschema.cfg",
        "attributes.cfg",
        "summary.cfg",
        "rank-profiles.cfg",
        "juniperrc.cfg",
        "proton.cfg",
    ] {
        let path = config.join(name);
        let contents = std::fs::read_to_string(&path)
            .map_err(|error| Error::Infrastructure(error.to_string()))?;
        files.insert(name.into(), serde_json::Value::String(contents));
    }
    Ok(serde_json::Value::Object(files))
}
