use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

// A schema edit must be accompanied by regenerated codec support. Until the
// generator knows how to emit that support, fail explicitly instead of
// allowing an old per-message mapping to remain silently incomplete.
const SUPPORTED_SCHEMA_DIGEST: &str =
    "010284fc0b2c92134e959a144b579ac72a5f05ef5fba74c837123172453878de";

fn main() {
    let command = env::args().nth(1).unwrap_or_else(|| "generate".to_owned());
    let check = command == "check" || command == "--check" || env::args().any(|a| a == "--check");
    if command != "generate" && command != "check" && command != "--check" {
        eprintln!("usage: searchkernel-bindgen [generate|check] [--check]");
        std::process::exit(2);
    }

    let root = repo_root();
    let flatc = env::var_os("FLATC_PATH")
        .map(PathBuf::from)
        .or_else(|| discover_flatc(&root))
        .unwrap_or_else(|| PathBuf::from("flatc"));
    let version = command_output(&flatc, &["--version"]).unwrap_or_else(|error| fail(&error));
    if version.trim() != "flatc version 25.12.19" {
        fail(&format!("expected flatc version 25.12.19, got {version}"));
    }

    let temp = env::temp_dir().join(format!("searchkernel-bindgen-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp);
    fs::create_dir_all(&temp).unwrap_or_else(|e| fail(&format!("create {}: {e}", temp.display())));
    let result = run(&root, &flatc, &temp, check);
    let _ = fs::remove_dir_all(&temp);
    if let Err(error) = result {
        fail(&error);
    }
}

fn run(root: &Path, flatc: &Path, temp: &Path, check: bool) -> Result<(), String> {
    for schema in [
        "searchkernel_ffi_request.fbs",
        "searchkernel_ffi_response.fbs",
    ] {
        run_command(
            flatc,
            &[
                "--binary",
                "--schema",
                "-o",
                temp.to_str().unwrap(),
                &root.join("api").join(schema).to_string_lossy(),
            ],
        )?;
    }

    let mut request = None;
    let mut response = None;
    for file in fs::read_dir(temp).map_err(|e| e.to_string())? {
        let file = file.map_err(|e| e.to_string())?.path();
        if file.extension().and_then(|x| x.to_str()) != Some("bfbs") {
            continue;
        }
        run_command(
            flatc,
            &[
                "--jsonschema",
                "-o",
                temp.to_str().unwrap(),
                file.to_str().unwrap(),
            ],
        )?;
        let schema =
            fs::read_to_string(file.with_extension("schema.json")).map_err(|e| e.to_string())?;
        let value: Value = serde_json::from_str(&schema).map_err(|e| e.to_string())?;
        let text = schema_name(&value);
        if text.contains("RequestEnvelope") {
            request = Some(value);
        } else if text.contains("ResponseEnvelope") {
            response = Some(value);
        }
    }
    let request = request.ok_or_else(|| "BFBS did not contain RequestEnvelope".to_owned())?;
    let response = response.ok_or_else(|| "BFBS did not contain ResponseEnvelope".to_owned())?;
    validate_schema(&request, "request")?;
    validate_schema(&response, "response")?;
    let request_types = verify_union(&request, "SearchKernel_FFI_V1_RequestEnvelope")?;
    let response_types = verify_union(&response, "SearchKernel_FFI_V1_ResponseEnvelope")?;
    let support = fs::read_to_string(root.join("crates/searchkernel-json/src/codec.rs"))
        .map_err(|e| format!("read codec support: {e}"))?;
    for type_name in request_types.iter().chain(response_types.iter()) {
        let functions = codec_support_functions(type_name)?;
        for function in functions {
            if !support.contains(&format!("fn {function}")) {
                return Err(format!(
                    "union member {type_name} has no codec support function {function}; generation aborted"
                ));
            }
        }
    }
    let mut digest = Sha256::new();
    digest.update(
        fs::read(temp.join("searchkernel_ffi_request.schema.json")).map_err(|e| e.to_string())?,
    );
    digest.update(
        fs::read(temp.join("searchkernel_ffi_response.schema.json")).map_err(|e| e.to_string())?,
    );
    let schema_digest = format!("{:x}", digest.finalize());
    if schema_digest != SUPPORTED_SCHEMA_DIGEST {
        return Err(format!(
            "unsupported schema change (digest {schema_digest}); update the generated JSON codec and bindgen support"
        ));
    }
    generate_json_schema(root, &request, &response, check)?;
    generate_json_types(root, &request, &response, check)?;
    let manifest = serde_json::json!({
        "flatc": "25.12.19",
        "schema_digest": schema_digest,
        "source": ["api/searchkernel.fbs", "api/searchkernel_ffi_request.fbs", "api/searchkernel_ffi_response.fbs"],
        "request_variants": request_types,
        "response_variants": response_types,
    });
    let output = root.join("crates/searchkernel-json/src/schema_manifest.json");
    let bytes = serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?;
    if check {
        let current = fs::read(&output).map_err(|e| format!("{}: {e}", output.display()))?;
        if current != bytes {
            return Err(format!("generated file is stale: {}", output.display()));
        }
    } else {
        fs::write(&output, bytes).map_err(|e| format!("{}: {e}", output.display()))?;
    }

    let generated_codec = root.join("crates/searchkernel-json/src/generated_codec.rs");
    // The handwritten module contains only codec-support routines. The
    // generated artifact includes it while keeping the schema-derived
    // dispatch/field metadata in generated/. This avoids copying a second
    // source file into the generated output and makes regeneration stable.
    let generated_bytes = b"// @generated by searchkernel-bindgen; do not modify\ninclude!(\"codec.rs\");\ninclude!(\"generated/codec_dispatch.rs\");\n".to_vec();
    if check {
        let current = fs::read(&generated_codec)
            .map_err(|e| format!("{}: {e}", generated_codec.display()))?;
        if current != generated_bytes {
            return Err(format!(
                "generated file is stale: {}",
                generated_codec.display()
            ));
        }
    } else {
        fs::write(&generated_codec, generated_bytes)
            .map_err(|e| format!("{}: {e}", generated_codec.display()))?;
    }
    generate_metadata(
        root,
        &request,
        &response,
        &request_types,
        &response_types,
        check,
    )?;
    generate_dispatch(root, &request_types, &response_types, check)?;
    Ok(())
}

fn generate_json_schema(
    root: &Path,
    request: &Value,
    response: &Value,
    check: bool,
) -> Result<(), String> {
    let schema = serde_json::json!({
        "request": request,
        "response": response,
    });
    let bytes = serde_json::to_vec_pretty(&schema).map_err(|e| e.to_string())?;
    let path = root.join("reference/json-conformance/generated/schema.json");
    if check {
        let current = fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        if current != bytes {
            return Err(format!("generated file is stale: {}", path.display()));
        }
    } else {
        fs::create_dir_all(path.parent().expect("schema has a parent"))
            .map_err(|e| format!("create generated schema directory: {e}"))?;
        fs::write(&path, bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(())
}

fn generate_json_types(
    root: &Path,
    request: &Value,
    response: &Value,
    check: bool,
) -> Result<(), String> {
    let mut definitions = BTreeMap::<String, Value>::new();
    for schema in [request, response] {
        let items = schema
            .get("definitions")
            .and_then(Value::as_object)
            .ok_or_else(|| "BFBS JSON schema has no definitions".to_owned())?;
        for (name, definition) in items {
            definitions.insert(name.clone(), definition.clone());
        }
    }
    let mut output = String::from("// @generated by searchkernel-bindgen; do not modify\n\n");
    for (name, definition) in definitions {
        let rust_name = json_type_name(&name);
        if let Some(values) = definition.get("enum").and_then(Value::as_array) {
            output.push_str(
                "#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]\n",
            );
            output.push_str(&format!("pub(crate) enum {rust_name} {{\n"));
            for value in values.iter().filter_map(Value::as_str) {
                output.push_str(&format!(
                    "    #[serde(rename = {value:?})]\n    {} ,\n",
                    json_variant_name(value)
                ));
            }
            output.push_str("}\n\n");
            continue;
        }
        if definition.get("type").and_then(Value::as_str) != Some("object") {
            return Err(format!("unsupported DTO definition {name}"));
        }
        output
            .push_str("#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]\n");
        output.push_str(&format!("pub(crate) struct {rust_name} {{\n"));
        if let Some(properties) = definition.get("properties").and_then(Value::as_object) {
            for field in properties.keys() {
                output.push_str(&format!(
                    "    #[serde(rename = {field:?}, default, skip_serializing_if = \"Option::is_none\")]\n    pub {}: Option<serde_json::Value>,\n",
                    json_field_name(field)
                ));
            }
        }
        output.push_str("}\n\n");
    }
    let path = root.join("crates/searchkernel-json/src/generated/json_types.rs");
    let bytes = format_rust(output.into_bytes())?;
    if check {
        let current = fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        if current != bytes {
            return Err(format!("generated file is stale: {}", path.display()));
        }
    } else {
        fs::write(&path, bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(())
}

fn json_type_name(name: &str) -> String {
    let mut output = String::from("Json");
    for part in name.split('_') {
        if !part.is_empty() {
            let mut chars = part.chars();
            if let Some(first) = chars.next() {
                output.push(first.to_ascii_uppercase());
                output.extend(chars.map(|c| c.to_ascii_lowercase()));
            }
        }
    }
    output
}

fn json_field_name(name: &str) -> String {
    match name {
        "type" | "ref" | "self" | "match" | "crate" | "super" | "mod" | "use" => {
            format!("r#{name}")
        }
        _ => name.to_owned(),
    }
}

fn json_variant_name(name: &str) -> String {
    let mut output = String::new();
    for part in name.split(|c: char| !c.is_ascii_alphanumeric()) {
        if !part.is_empty() {
            let mut chars = part.chars();
            if let Some(first) = chars.next() {
                output.push(first.to_ascii_uppercase());
                output.extend(chars.map(|c| c.to_ascii_lowercase()));
            }
        }
    }
    if output.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        format!("Value{output}")
    } else {
        output
    }
}

fn generate_metadata(
    root: &Path,
    request: &Value,
    response: &Value,
    request_types: &[String],
    response_types: &[String],
    check: bool,
) -> Result<(), String> {
    let mut definitions = BTreeMap::<String, (Vec<String>, BTreeMap<String, String>)>::new();
    let mut enums = BTreeMap::<String, Vec<String>>::new();
    for schema in [request, response] {
        let Some(items) = schema.get("definitions").and_then(Value::as_object) else {
            return Err("BFBS JSON schema has no definitions".to_owned());
        };
        for (name, definition) in items {
            let fields = definition
                .get("properties")
                .and_then(Value::as_object)
                .map(|properties| properties.keys().cloned().collect())
                .unwrap_or_default();
            let field_types = definition
                .get("properties")
                .and_then(Value::as_object)
                .map(|properties| {
                    properties
                        .iter()
                        .map(|(field, value)| (field.clone(), property_kind(value)))
                        .collect()
                })
                .unwrap_or_default();
            definitions
                .entry(name.clone())
                .or_insert((fields, field_types));
            if let Some(values) = definition.get("enum").and_then(Value::as_array) {
                enums.insert(
                    name.clone(),
                    values
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_owned)
                        .collect(),
                );
            }
        }
    }
    let mut output = String::from("// @generated by searchkernel-bindgen; do not modify\n\n");
    output.push_str(
        "pub(crate) struct FieldMetadata { pub name: &'static str, pub schema: &'static str }\n\n",
    );
    output.push_str("pub(crate) fn metadata(name: &str) -> Option<&'static [FieldMetadata]> {\n");
    output.push_str("    match name {\n");
    for (name, (_, field_types)) in &definitions {
        output.push_str(&format!("        {name:?} => Some(&["));
        for (field, kind) in field_types {
            output.push_str(&format!(
                "FieldMetadata {{ name: {field:?}, schema: {kind:?} }},"
            ));
        }
        output.push_str("]),\n");
    }
    output.push_str("        _ => None,\n    }\n}\n");
    output
        .push_str("\npub(crate) fn enum_values(name: &str) -> Option<&'static [&'static str]> {\n");
    output.push_str("    match name {\n");
    for (name, values) in &enums {
        output.push_str(&format!("        {name:?} => Some(&["));
        for value in values {
            output.push_str(&format!("{value:?},"));
        }
        output.push_str("]),\n");
    }
    output.push_str("        _ => None,\n    }\n}\n");
    output.push_str("\npub(crate) fn fields(name: &str) -> Option<&'static [&'static str]> {\n");
    output.push_str("    match name {\n");
    for (name, (fields, _)) in &definitions {
        output.push_str(&format!("        {name:?} => Some(&["));
        for field in fields {
            output.push_str(&format!("{field:?},"));
        }
        output.push_str("]),\n");
    }
    output.push_str("        _ => None,\n    }\n}\n");
    output.push_str("\npub(crate) fn request_operation(tag: u8) -> Option<&'static str> {\n");
    output.push_str("    match tag {\n");
    for name in request_types {
        output.push_str(&format!(
            "        value if value == crate::generated::search_kernel::ffi::v1::RequestPayload::{0}.0 => Some({1:?}),\n",
            enum_variant(name),
            operation_name(name)?
        ));
    }
    output.push_str("        _ => None,\n    }\n}\n");
    output.push_str("\npub(crate) fn is_request_operation(name: &str) -> bool {\n");
    output.push_str("    matches!(name, ");
    for (index, variant) in request_types.iter().enumerate() {
        if index != 0 {
            output.push_str(" | ");
        }
        output.push_str(&format!("{:?}", operation_name(variant)?));
    }
    output.push_str(")\n}\n");
    output.push_str("\npub(crate) fn response_operation(tag: u8) -> Option<&'static str> {\n");
    output.push_str("    match tag {\n");
    for name in response_types {
        output.push_str(&format!(
            "        value if value == crate::generated::search_kernel::ffi::v1::ResponsePayload::{0}.0 => Some({1:?}),\n",
            enum_variant(name),
            operation_name(name)?
        ));
    }
    output.push_str("        _ => None,\n    }\n}\n");
    output.push_str("\npub(crate) fn is_response_operation(name: &str) -> bool {\n");
    output.push_str("    matches!(name, ");
    for (index, variant) in response_types.iter().enumerate() {
        if index != 0 {
            output.push_str(" | ");
        }
        output.push_str(&format!("{:?}", operation_name(variant)?));
    }
    output.push_str(")\n}\n");
    let path = root.join("crates/searchkernel-json/src/generated/schema_metadata.rs");
    let bytes = format_rust(output.into_bytes())?;
    if check {
        let current = fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        if current != bytes {
            return Err(format!("generated file is stale: {}", path.display()));
        }
    } else {
        fs::write(&path, bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(())
}

fn generate_dispatch(
    root: &Path,
    request_types: &[String],
    response_types: &[String],
    check: bool,
) -> Result<(), String> {
    let mut output = String::from("// @generated by searchkernel-bindgen; do not modify\n\n");
    output.push_str("pub(crate) fn generated_request_json_to_payload<'a>(\n");
    output.push_str("    builder: &mut FlatBufferBuilder<'a>,\n");
    output.push_str("    operation: &str,\n");
    output.push_str("    payload: &Map<String, Value>,\n");
    output.push_str(
        ") -> Result<(transport::RequestPayload, WIPOffset<UnionWIPOffset>), JsonCodecError> {\n",
    );
    output.push_str("    match operation {\n");
    for type_name in request_types {
        let operation = operation_name(type_name)?;
        let builder = if type_name.ends_with("SearchRequest") {
            "offset(search(builder, payload)?)"
        } else {
            &format!("{}(builder, payload)?", operation)
        };
        output.push_str(&format!(
            "        {operation:?} => Ok((transport::RequestPayload::{variant}, {builder})),\n",
            variant = enum_variant(type_name)
        ));
    }
    output.push_str("        _ => Err(contract(format!(\"unknown operation `{operation}`\"))),\n");
    output.push_str("    }\n}\n\n");

    output.push_str("pub(crate) fn generated_request_payload_to_json<'a>(\n");
    output.push_str("    envelope: transport::RequestEnvelope<'a>,\n");
    output.push_str("    tag: u8,\n");
    output.push_str(") -> Result<Value, JsonCodecError> {\n");
    output.push_str("    match tag {\n");
    for type_name in request_types {
        let operation = operation_name(type_name)?;
        let function = match operation.as_str() {
            "describe" => "generated_empty_request_json".to_owned(),
            "inspect_query" => "request_inspect".to_owned(),
            other => format!("request_{other}"),
        };
        output.push_str(&format!(
            "        value if value == transport::RequestPayload::{variant}.0 => {function}(envelope.payload_as_{accessor}()),\n",
            variant = enum_variant(type_name),
            accessor = accessor_name(type_name),
        ));
    }
    output.push_str("        _ => Err(JsonCodecError::FlatBuffer(format!(\"unknown request discriminator {tag}\"))),\n");
    output.push_str("    }\n}\n\n");

    output.push_str("pub(crate) fn generated_response_json_to_payload<'a>(\n");
    output.push_str("    builder: &mut FlatBufferBuilder<'a>,\n");
    output.push_str("    operation: &str,\n");
    output.push_str("    payload: &Value,\n");
    output.push_str(
        ") -> Result<(transport::ResponsePayload, WIPOffset<UnionWIPOffset>), JsonCodecError> {\n",
    );
    output.push_str("    match operation {\n");
    for type_name in response_types {
        let operation = operation_name(type_name)?;
        let function = if type_name == "SearchKernel_V1_Error" {
            "error_from_json"
        } else if operation == "inspect_query" {
            "response_inspect_from_json"
        } else {
            &format!("response_{operation}_from_json")
        };
        output.push_str(&format!(
            "        {operation:?} => Ok((transport::ResponsePayload::{variant}, {function}(builder, payload)?)),\n",
            variant = enum_variant(type_name),
        ));
    }
    output.push_str(
        "        _ => Err(contract(format!(\"unknown response operation `{operation}`\"))),\n",
    );
    output.push_str("    }\n}\n\n");

    output.push_str("pub(crate) fn generated_response_payload_to_json<'a>(\n");
    output.push_str("    envelope: transport::ResponseEnvelope<'a>,\n");
    output.push_str("    tag: u8,\n");
    output.push_str(") -> Result<Value, JsonCodecError> {\n");
    output.push_str("    match tag {\n");
    for type_name in response_types {
        let operation = operation_name(type_name)?;
        let function = if type_name == "SearchKernel_V1_Error" {
            "response_error"
        } else if operation == "inspect_query" {
            "response_inspect"
        } else {
            &format!("response_{operation}")
        };
        output.push_str(&format!(
            "        value if value == transport::ResponsePayload::{variant}.0 => {function}(envelope.payload_as_{accessor}()),\n",
            variant = enum_variant(type_name),
            accessor = accessor_name(type_name),
        ));
    }
    output.push_str("        _ => Err(JsonCodecError::FlatBuffer(format!(\"unknown response discriminator {tag}\"))),\n");
    output.push_str("    }\n}\n");

    let path = root.join("crates/searchkernel-json/src/generated/codec_dispatch.rs");
    let bytes = format_rust(output.into_bytes())?;
    if check {
        let current = fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        if current != bytes {
            return Err(format!("generated file is stale: {}", path.display()));
        }
    } else {
        fs::write(&path, bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(())
}

fn property_kind(value: &Value) -> String {
    if value.get("anyOf").is_some() {
        return "union".to_owned();
    }
    if let Some(reference) = value.get("$ref").and_then(Value::as_str) {
        return format!("ref:{reference}");
    }
    if value.get("type").and_then(Value::as_str) == Some("array") {
        return value
            .get("items")
            .map(property_kind)
            .map(|item| format!("array:{item}"))
            .unwrap_or_else(|| "array".to_owned());
    }
    value
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_owned()
}

fn codec_support_functions(type_name: &str) -> Result<Vec<String>, String> {
    let operation = operation_name(type_name)?;
    let request = type_name.ends_with("Request");
    if request {
        let encoder = operation.clone();
        let decoder = match operation.as_str() {
            "describe" => "generated_empty_request_json".to_owned(),
            "inspect_query" => "request_inspect".to_owned(),
            other => format!("request_{other}"),
        };
        Ok(vec![encoder, decoder])
    } else {
        let encoder = if operation == "error" {
            "error_from_json".to_owned()
        } else if operation == "inspect_query" {
            "response_inspect_from_json".to_owned()
        } else {
            format!("response_{operation}_from_json")
        };
        let decoder = if operation == "error" {
            "response_error".to_owned()
        } else if operation == "inspect_query" {
            "response_inspect".to_owned()
        } else {
            format!("response_{operation}")
        };
        Ok(vec![encoder, decoder])
    }
}

fn validate_schema(schema: &Value, label: &str) -> Result<(), String> {
    let definitions = schema
        .get("definitions")
        .and_then(Value::as_object)
        .ok_or_else(|| format!("{label}: BFBS schema has no definitions"))?;
    for (name, definition) in definitions {
        let object = definition
            .as_object()
            .ok_or_else(|| format!("{label}: definition {name} is not an object"))?;
        if object.contains_key("oneOf") || object.contains_key("allOf") {
            return Err(format!(
                "{label}: unsupported composition in definition {name}"
            ));
        }
        if object.get("additionalProperties") == Some(&Value::Bool(true)) {
            return Err(format!(
                "{label}: open additionalProperties in definition {name}"
            ));
        }
        if let Some(properties) = object.get("properties").and_then(Value::as_object) {
            for (field, property) in properties {
                validate_property(property)
                    .map_err(|error| format!("{label}: unsupported {name}.{field}: {error}"))?;
            }
        }
    }
    Ok(())
}

fn validate_property(value: &Value) -> Result<(), String> {
    if value.get("anyOf").is_some() {
        let variants = value
            .get("anyOf")
            .and_then(Value::as_array)
            .ok_or_else(|| "anyOf is not an array".to_owned())?;
        if variants.is_empty() {
            return Err("empty union".to_owned());
        }
        for variant in variants {
            if variant.get("$ref").and_then(Value::as_str).is_none() {
                return Err("union member is not a definition reference".to_owned());
            }
        }
        return Ok(());
    }
    if value.get("$ref").and_then(Value::as_str).is_some() {
        return Ok(());
    }
    match value.get("type").and_then(Value::as_str) {
        Some("string" | "integer" | "number" | "boolean") => Ok(()),
        Some("array") => value
            .get("items")
            .ok_or_else(|| "array has no items".to_owned())
            .and_then(validate_property),
        Some("object") => Err("nested anonymous objects are unsupported".to_owned()),
        Some(other) => Err(format!("unknown JSON Schema type {other}")),
        None => Err("property has no type, reference, or union".to_owned()),
    }
}

fn enum_variant(type_name: &str) -> &str {
    type_name
}

fn accessor_name(type_name: &str) -> String {
    let mut output = String::new();
    for character in type_name.chars() {
        if character == '_' {
            output.push('_');
            continue;
        }
        if character.is_ascii_uppercase() && !output.is_empty() && !output.ends_with('_') {
            output.push('_');
        }
        output.push(character.to_ascii_lowercase());
    }
    output
}

fn format_rust(input: Vec<u8>) -> Result<Vec<u8>, String> {
    let mut child = Command::new("rustfmt")
        .args(["--edition", "2021", "--emit", "stdout"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("run rustfmt: {e}"))?;
    child
        .stdin
        .take()
        .ok_or_else(|| "rustfmt stdin unavailable".to_owned())?
        .write_all(&input)
        .map_err(|e| format!("write rustfmt input: {e}"))?;
    let output = child
        .wait_with_output()
        .map_err(|e| format!("wait for rustfmt: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "rustfmt failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(output.stdout)
}

fn schema_name(value: &Value) -> String {
    value
        .get("definitions")
        .and_then(Value::as_object)
        .map(|v| v.keys().cloned().collect::<Vec<_>>().join(" "))
        .unwrap_or_default()
}

fn verify_union(schema: &Value, envelope: &str) -> Result<Vec<String>, String> {
    let definition = schema
        .get("definitions")
        .and_then(Value::as_object)
        .and_then(|x| x.get(envelope))
        .ok_or_else(|| format!("missing BFBS definition {envelope}"))?;
    let variants = definition
        .get("properties")
        .and_then(Value::as_object)
        .and_then(|x| x.get("payload"))
        .and_then(|x| x.get("anyOf"))
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{envelope}.payload is not a union"))?;
    let names = variants
        .iter()
        .map(|variant| {
            variant
                .get("$ref")
                .and_then(Value::as_str)
                .and_then(|reference| reference.rsplit('/').next())
                .map(str::to_owned)
                .ok_or_else(|| format!("{envelope}.payload contains an invalid union member"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(names)
}

fn operation_name(type_name: &str) -> Result<String, String> {
    let type_name = type_name
        .strip_prefix("SearchKernel_V1_")
        .ok_or_else(|| format!("unsupported union type {type_name}"))?;
    if type_name == "Error" {
        return Ok("error".to_owned());
    }
    let type_name = type_name
        .strip_suffix("Request")
        .or_else(|| type_name.strip_suffix("Result"))
        .ok_or_else(|| format!("union type {type_name} has no operation suffix"))?;
    Ok(type_name
        .chars()
        .enumerate()
        .fold(String::new(), |mut output, (index, c)| {
            if c.is_ascii_uppercase() && index != 0 {
                output.push('_');
            }
            output.push(c.to_ascii_lowercase());
            output
        }))
}

fn run_command(program: &Path, args: &[&str]) -> Result<(), String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|e| format!("{}: {e}", program.display()))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).into_owned())
    }
}

fn command_output(program: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|e| format!("{}: {e}", program.display()))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).into_owned())
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn fail(message: &str) -> ! {
    eprintln!("searchkernel-bindgen: {message}");
    std::process::exit(1)
}

fn discover_flatc(root: &Path) -> Option<PathBuf> {
    let build = root.join("target/debug/build");
    let mut candidates = fs::read_dir(build)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("flatbuffers-build-"))
        })
        .map(|path| path.join("out/bin/flatc"))
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    candidates.sort();
    candidates.into_iter().next()
}
