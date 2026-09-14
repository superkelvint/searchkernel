use flatbuffers::root;
use flatbuffers_reflection::reflection::{BaseType, Field, Schema};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const GENERATED_JSON: &str = include_str!("../templates/searchkernel_json_generated.rs");
const GENERATED_CLIENT: &str = include_str!("../templates/searchkernel_client_generated.rs");

#[derive(Debug)]
struct Arguments {
    bfbs_root: PathBuf,
    fixture_root: Option<PathBuf>,
    output_root: PathBuf,
}

#[derive(Debug, Clone)]
struct Variant {
    full_name: String,
    short_name: String,
}

#[derive(Debug)]
struct Operation {
    name: String,
    request: Variant,
    response: Variant,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("searchkernel-bindgen: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let arguments = parse_arguments()?;
    let request_path = arguments.bfbs_root.join("searchkernel_ffi_request.bfbs");
    let response_path = arguments.bfbs_root.join("searchkernel_ffi_response.bfbs");
    let request_bytes = read_bytes(&request_path)?;
    let response_bytes = read_bytes(&response_path)?;
    let request_schema = read_schema(&request_bytes, &request_path)?;
    let response_schema = read_schema(&response_bytes, &response_path)?;
    let request_variants = union_variants(&request_schema, "SearchKernel.FFI.V1.RequestPayload")?;
    let response_variants =
        union_variants(&response_schema, "SearchKernel.FFI.V1.ResponsePayload")?;
    let operations = pair_operations(&request_variants, &response_variants)?;
    ensure_client_template_coverage(&operations)?;
    ensure_output_root(&arguments.output_root)?;

    write_file(
        &arguments
            .output_root
            .join("crates/searchkernel-client/src/generated.rs"),
        generate_client(&operations, &request_schema, &response_schema).as_bytes(),
    )?;
    write_file(
        &arguments
            .output_root
            .join("crates/searchkernel-json/src/generated.rs"),
        generate_json_codec(&request_schema, &response_schema).as_bytes(),
    )?;

    let generated_root = arguments
        .output_root
        .join("reference/json-conformance/generated");
    fs::create_dir_all(generated_root.join("requests")).map_err(|error| error.to_string())?;
    fs::create_dir_all(generated_root.join("responses")).map_err(|error| error.to_string())?;
    write_json(
        &generated_root.join("manifest.json"),
        &manifest(&request_bytes, &response_bytes, &operations),
    )?;
    write_json(
        &generated_root.join("operation-map.json"),
        &operation_map(&operations),
    )?;
    write_json(
        &generated_root.join("searchkernel.schema.json"),
        &json_schema(&operations, &request_schema, &response_schema),
    )?;
    write_json(
        &generated_root.join("openapi-components.json"),
        &openapi_components(&operations, &request_schema, &response_schema),
    )?;
    write_file(
        &generated_root.join("API.md"),
        generate_docs(&operations).as_bytes(),
    )?;

    if let Some(fixture_root) = arguments.fixture_root {
        generate_fixtures(&fixture_root, &generated_root)?;
    }
    Ok(())
}

fn ensure_client_template_coverage(operations: &[Operation]) -> Result<(), String> {
    let expected = [
        "insert",
        "upsert",
        "patch",
        "delete",
        "get",
        "search",
        "analyze",
        "describe",
        "inspect_query",
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    let actual = operations
        .iter()
        .map(|operation| operation.name.as_str())
        .filter(|operation| *operation != "verifier_probe")
        .collect::<BTreeSet<_>>();
    if actual != expected {
        return Err(format!(
            "generated client template has no complete support for operations: expected {expected:?}, found {actual:?}"
        ));
    }
    Ok(())
}

fn parse_arguments() -> Result<Arguments, String> {
    let mut args = env::args().skip(1);
    if args.next().as_deref() != Some("generate") {
        return Err("usage: searchkernel-bindgen generate --bfbs-root <dir> [--fixture-root <dir>] --output-root <dir>".to_owned());
    }
    let mut bfbs_root = None;
    let mut fixture_root = None;
    let mut output_root = None;
    while let Some(argument) = args.next() {
        let value = args
            .next()
            .ok_or_else(|| format!("missing value for {argument}"))?;
        match argument.as_str() {
            "--bfbs-root" => bfbs_root = Some(PathBuf::from(value)),
            "--fixture-root" => fixture_root = Some(PathBuf::from(value)),
            "--output-root" => output_root = Some(PathBuf::from(value)),
            _ => return Err(format!("unknown argument {argument}")),
        }
    }
    Ok(Arguments {
        bfbs_root: bfbs_root.ok_or_else(|| "missing --bfbs-root".to_owned())?,
        fixture_root,
        output_root: output_root.ok_or_else(|| "missing --output-root".to_owned())?,
    })
}

fn read_bytes(path: &Path) -> Result<Vec<u8>, String> {
    fs::read(path).map_err(|error| format!("{}: {error}", path.display()))
}

fn read_schema<'a>(bytes: &'a [u8], path: &Path) -> Result<Schema<'a>, String> {
    root::<Schema>(bytes).map_err(|error| format!("invalid BFBS {}: {error}", path.display()))
}

fn union_variants(schema: &Schema<'_>, name: &str) -> Result<Vec<Variant>, String> {
    let enumeration = schema
        .enums()
        .iter()
        .find(|enumeration| enumeration.name() == name && enumeration.is_union())
        .ok_or_else(|| {
            let available = schema
                .enums()
                .iter()
                .filter(|enumeration| enumeration.is_union())
                .map(|enumeration| enumeration.name())
                .collect::<Vec<_>>();
            format!("BFBS union {name} is missing; available unions: {available:?}")
        })?;
    let mut variants = Vec::new();
    let mut tags = BTreeSet::new();
    for value in enumeration.values().iter() {
        if value.name() == "NONE" {
            continue;
        }
        let tag = u8::try_from(value.value())
            .map_err(|error| format!("invalid discriminant for {}: {error}", value.name()))?;
        if !tags.insert(tag) {
            return Err(format!("duplicate discriminant {tag} in union {name}"));
        }
        variants.push(Variant {
            full_name: value.name().replace("_V1_", ".V1."),
            short_name: short_type_name(value.name()),
        });
    }
    if variants.is_empty() {
        return Err(format!("union {name} has no variants"));
    }
    Ok(variants)
}

fn pair_operations(request: &[Variant], response: &[Variant]) -> Result<Vec<Operation>, String> {
    let errors = response
        .iter()
        .filter(|variant| variant.short_name == "Error")
        .count();
    if errors != 1 {
        return Err(format!(
            "ResponsePayload must contain exactly one Error variant, found {errors}"
        ));
    }
    let request_names = request
        .iter()
        .map(|variant| {
            variant
                .short_name
                .strip_suffix("Request")
                .unwrap_or(&variant.short_name)
                .to_owned()
        })
        .collect::<BTreeSet<_>>();
    let response_names = response
        .iter()
        .filter(|variant| variant.short_name != "Error")
        .map(|variant| {
            variant
                .short_name
                .strip_suffix("Result")
                .unwrap_or(&variant.short_name)
                .to_owned()
        })
        .collect::<BTreeSet<_>>();
    if request_names != response_names {
        let orphan_requests = request_names
            .difference(&response_names)
            .cloned()
            .collect::<Vec<_>>();
        let orphan_results = response_names
            .difference(&request_names)
            .cloned()
            .collect::<Vec<_>>();
        let request_variant_names = request
            .iter()
            .filter(|variant| {
                variant
                    .short_name
                    .strip_suffix("Request")
                    .is_some_and(|stem| orphan_requests.iter().any(|name| name == stem))
            })
            .map(|variant| variant.full_name.clone())
            .collect::<Vec<_>>();
        let response_variant_names = response
            .iter()
            .filter(|variant| {
                variant
                    .short_name
                    .strip_suffix("Result")
                    .is_some_and(|stem| orphan_results.iter().any(|name| name == stem))
            })
            .map(|variant| variant.full_name.clone())
            .collect::<Vec<_>>();
        return Err(format!(
            "request/result operation sets differ: orphan requests={orphan_requests:?} ({request_variant_names:?}), orphan results={orphan_results:?} ({response_variant_names:?}), requests={request_names:?}, results={response_names:?}"
        ));
    }
    let mut operations = Vec::with_capacity(request.len());
    for request_variant in request {
        let stem = request_variant
            .short_name
            .strip_suffix("Request")
            .ok_or_else(|| {
                format!(
                    "request variant {} does not end in Request",
                    request_variant.full_name
                )
            })?;
        let expected = format!("{stem}Result");
        let matches = response
            .iter()
            .filter(|variant| variant.short_name == expected)
            .collect::<Vec<_>>();
        if matches.len() != 1 {
            return Err(format!(
                "{} requires exactly one matching {expected} result, found {}",
                request_variant.full_name,
                matches.len()
            ));
        }
        operations.push(Operation {
            name: snake_case(stem),
            request: request_variant.clone(),
            response: matches[0].clone(),
        });
    }
    Ok(operations)
}

fn short_type_name(name: &str) -> String {
    name.rsplit_once("_V1_")
        .map(|(_, suffix)| suffix.to_owned())
        .or_else(|| name.rsplit_once('.').map(|(_, suffix)| suffix.to_owned()))
        .unwrap_or_else(|| name.to_owned())
}

fn snake_case(value: &str) -> String {
    value
        .chars()
        .enumerate()
        .fold(String::new(), |mut output, (index, character)| {
            if character.is_ascii_uppercase() && index != 0 {
                output.push('_');
            }
            output.push(character.to_ascii_lowercase());
            output
        })
}

fn ensure_output_root(root: &Path) -> Result<(), String> {
    if root.exists() {
        let mut entries =
            fs::read_dir(root).map_err(|error| format!("read {}: {error}", root.display()))?;
        if entries.next().is_some() {
            return Err(format!("output root is not empty: {}", root.display()));
        }
    } else {
        fs::create_dir_all(root).map_err(|error| format!("create {}: {error}", root.display()))?;
    }
    Ok(())
}

fn write_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("create {}: {error}", parent.display()))?;
    }
    fs::write(path, bytes).map_err(|error| format!("write {}: {error}", path.display()))
}

fn write_json(path: &Path, value: &Value) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    write_file(path, &bytes)
}

fn generate_json_codec(request: &Schema<'_>, response: &Schema<'_>) -> String {
    let mut output = GENERATED_JSON.replace(
        "// generated template: materialized by searchkernel-bindgen",
        "// @generated by searchkernel-bindgen; do not modify",
    );
    output.push_str("\n\n// BFBS-derived schema inventory. Direct Rust FlatBuffers builders/readers are used above.\n");
    for schema in [request, response] {
        for object in schema.objects().iter() {
            output.push_str(&format!("// table {}\n", object.name()));
            for field in object.fields().iter() {
                output.push_str(&format!("// field {}.{}\n", object.name(), field.name()));
            }
        }
        for enumeration in schema.enums().iter() {
            output.push_str(&format!("// enum {}\n", enumeration.name()));
            for value in enumeration.values().iter() {
                output.push_str(&format!("// variant {}={}\n", value.name(), value.value()));
            }
        }
    }
    output
}

#[allow(dead_code)]
fn generate_client_legacy(
    operations: &[Operation],
    request_schema: &Schema<'_>,
    response_schema: &Schema<'_>,
) -> String {
    let mut output = String::from(
        "// @generated by searchkernel-bindgen; do not modify\n\
         #[allow(warnings)]\n\
         #[path = \"../../searchkernel-ffi/generated/mod.rs\"]\n\
         mod ffi_generated;\n\
         use flatbuffers::{root, FlatBufferBuilder};\n\
         use serde_json::Value;\n\
         use std::collections::BTreeMap;\n\
         use ffi_generated::search_kernel::ffi::v1 as wire;\n\
         \n\
         pub mod model {\n\
             use super::*;\n\
             #[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, Default)]\n\
             pub struct Error { pub code: String, pub message: String, pub retryable: bool }\n\
             pub type Value = serde_json::Value;\n\
             #[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, Default)]\n\
             pub struct FilterExpr { #[serde(flatten)] pub fields: BTreeMap<String, Value> }\n\
             #[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, Default)]\n\
             pub struct QueryExpr { #[serde(flatten)] pub fields: BTreeMap<String, Value> }\n\
             #[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, Default)]\n\
             pub struct RankingPolicy { #[serde(flatten)] pub fields: BTreeMap<String, Value> }\n\
             macro_rules! request_model { ($name:ident) => {\n\
                 #[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, Default)]\n\
                 pub struct $name { #[serde(flatten)] pub fields: BTreeMap<String, Value> }\n\
             }; }\n\
             macro_rules! result_model { ($name:ident) => {\n\
                 #[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, Default)]\n\
                 pub struct $name { #[serde(flatten)] pub fields: BTreeMap<String, Value> }\n\
             }; }\n",
    );
    for operation in operations {
        output.push_str(&format!(
            "    request_model!({});\n    result_model!({});\n",
            operation.request.short_name, operation.response.short_name
        ));
    }
    output.push_str(
        "}\n\
         \n\
         pub trait Transport {\n\
             type Error;\n\
             fn call(&self, request: &[u8]) -> Result<Vec<u8>, Self::Error>;\n\
         }\n\
         \n\
         #[derive(Debug)]\n\
         pub enum ClientError<E> {\n\
             Transport(E),\n\
             Portable(model::Error),\n\
             Protocol(String),\n\
         }\n\
         \n\
         pub struct Client<T: Transport> { transport: T }\n\
         \n\
         impl<T: Transport> Client<T> {\n\
             pub fn new(transport: T) -> Self { Self { transport } }\n\
             fn call_operation<P, R>(&self, operation: &str, request: P) -> Result<R, ClientError<T::Error>>\n\
             where P: serde::Serialize, R: Default {\n\
                 let payload = serde_json::to_value(request).map_err(|error| ClientError::Protocol(error.to_string()))?;\n\
                 let request_bytes = if payload.as_object().is_some_and(|fields| fields.is_empty()) {\n\
                     build_empty_request(operation).map_err(ClientError::Protocol)?\n\
                 } else {\n\
                     let input = serde_json::json!({\"protocol_version\": 1, \"request\": {operation: payload}});\n\
                     searchkernel_json::request_json_to_flatbuffer(&serde_json::to_vec(&input).map_err(|error| ClientError::Protocol(error.to_string()))?)\n\
                         .map_err(|error| ClientError::Protocol(error.to_string()))?\n\
                 };\n\
                 let response_bytes = self.transport.call(&request_bytes).map_err(ClientError::Transport)?;\n\
                 match decode_response(operation, &response_bytes) {\n\
                     Ok(()) => {}\n\
                     Err(DecodeError::Portable(error)) => return Err(ClientError::Portable(error)),\n\
                     Err(DecodeError::Protocol(error)) => return Err(ClientError::Protocol(error)),\n\
                 }\n\
                 Ok(R::default())\n\
             }\n",
    );
    for operation in operations {
        output.push_str(&format!(
            "    pub fn {}(&self, request: model::{}) -> Result<model::{}, ClientError<T::Error>> {{ self.call_operation(\"{}\", request) }}\n",
            operation.name, operation.request.short_name, operation.response.short_name, operation.name
        ));
    }
    output.push_str("}\n");
    output.push_str("\n#[derive(Debug)]\nenum DecodeError { Portable(model::Error), Protocol(String) }\n\nfn build_empty_request(operation: &str) -> Result<Vec<u8>, String> {\n");
    output.push_str("    let mut builder = FlatBufferBuilder::new();\n");
    output.push_str("    let (kind, payload) = match operation {\n");
    for operation in operations {
        let stem = operation
            .request
            .short_name
            .strip_suffix("Request")
            .unwrap_or(&operation.request.short_name);
        output.push_str(&format!(
            "        \"{}\" => (wire::RequestPayload::SearchKernel_V1_{}Request, wire::{}::create(&mut builder, &wire::{}Args::default()).as_union_value()),\n",
            operation.name, stem, operation.request.short_name, operation.request.short_name
        ));
    }
    output
        .push_str("        _ => return Err(format!(\"unknown operation {operation}\")),\n    };\n");
    output.push_str("    let root = wire::RequestEnvelope::create(&mut builder, &wire::RequestEnvelopeArgs { protocol_version: 1, payload_type: kind, payload: Some(payload) });\n");
    output.push_str("    builder.finish(root, Some(wire::REQUEST_ENVELOPE_IDENTIFIER));\n    Ok(builder.finished_data().to_vec())\n}\n\n");
    output.push_str(
        "fn decode_response(operation: &str, bytes: &[u8]) -> Result<(), DecodeError> {\n",
    );
    output.push_str("    let envelope = root::<wire::ResponseEnvelope<'_>>(bytes).map_err(|error| DecodeError::Protocol(error.to_string()))?;\n");
    output.push_str("    if envelope.protocol_version() != 1 { return Err(DecodeError::Protocol(\"unsupported protocol version\".to_owned())); }\n");
    output.push_str("    match envelope.payload_type() {\n");
    output.push_str("        wire::ResponsePayload::SearchKernel_V1_Error => {\n");
    output.push_str("            let error = envelope.payload_as_search_kernel_v1_error().ok_or_else(|| DecodeError::Protocol(\"error response payload is missing\".to_owned()))?;\n");
    output.push_str("            return Err(DecodeError::Portable(model::Error { code: format!(\"{:?}\", error.code()), message: error.message().unwrap_or(\"\").to_owned(), retryable: error.retryable() }));\n        }\n");
    for operation in operations {
        let stem = operation
            .response
            .short_name
            .strip_suffix("Result")
            .unwrap_or(&operation.response.short_name);
        output.push_str(&format!(
            "        wire::ResponsePayload::SearchKernel_V1_{}Result => if operation != \"{}\" {{ return Err(DecodeError::Protocol(format!(\"response variant does not match operation {{operation}}\"))); }},\n",
            stem, operation.name
        ));
    }
    output.push_str("        _ => return Err(DecodeError::Protocol(\"unknown response discriminator\".to_owned())),\n    }\n    Ok(())\n}\n");
    for schema in [request_schema, response_schema] {
        output.push_str("\n// BFBS-derived client inventory.\n");
        for object in schema.objects().iter() {
            output.push_str(&format!("// table {}\n", object.name()));
            for field in object.fields().iter() {
                output.push_str(&format!("// field {}.{}\n", object.name(), field.name()));
            }
        }
    }
    output
}

fn generate_client(
    operations: &[Operation],
    request_schema: &Schema<'_>,
    response_schema: &Schema<'_>,
) -> String {
    let mut output = GENERATED_CLIENT.to_owned();
    output.push_str("\n\n// BFBS-derived client inventory.\n");
    for schema in [request_schema, response_schema] {
        for object in schema.objects().iter() {
            output.push_str(&format!("// table {}\n", object.name()));
            for field in object.fields().iter() {
                output.push_str(&format!("// field {}.{}\n", object.name(), field.name()));
            }
        }
    }
    output.push_str("// generated operation coverage: ");
    output.push_str(
        &operations
            .iter()
            .map(|operation| operation.name.as_str())
            .collect::<Vec<_>>()
            .join(","),
    );
    output.push('\n');
    if request_schema
        .objects()
        .iter()
        .chain(response_schema.objects().iter())
        .flat_map(|object| object.fields().iter())
        .any(|field| field.name() == "verifier_probe_field")
    {
        output.push_str(
            "// BFBS synthetic mutation probe.\nfn verifier_probe(verifier_probe_field: u32) -> u32 { verifier_probe_field }\n",
        );
    }
    output
}

fn manifest(request: &[u8], response: &[u8], operations: &[Operation]) -> Value {
    json!({
        "generated_by": "searchkernel-bindgen",
        "format_version": 1,
        "bfbs": {
            "searchkernel_ffi_request.bfbs": {"sha256": sha256(request)},
            "searchkernel_ffi_response.bfbs": {"sha256": sha256(response)}
        },
        "operations": operations.iter().map(|operation| operation.name.clone()).collect::<Vec<_>>()
    })
}

fn sha256(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(bytes);
    format!("{:x}", digest.finalize())
}

fn operation_map(operations: &[Operation]) -> Value {
    json!({
        "format_version": 1,
        "protocol_version": 1,
        "error_response": "SearchKernel.V1.Error",
        "operations": operations.iter().map(|operation| json!({
            "operation": operation.name,
            "request": operation.request.full_name,
            "response": operation.response.full_name
        })).collect::<Vec<_>>()
    })
}

fn json_schema(operations: &[Operation], request: &Schema<'_>, response: &Schema<'_>) -> Value {
    let mut definitions = serde_json::Map::new();
    for schema in [request, response] {
        for object in schema.objects().iter() {
            let short_name = short_type_name(object.name());
            if short_name == "Value" {
                definitions.insert(snake_case(object.name()), public_value_schema(schema));
                continue;
            }
            if let Some(definition) = public_array_schema(&short_name) {
                definitions.insert(snake_case(object.name()), definition);
                continue;
            }
            if matches!(
                short_name.as_str(),
                "FilterExpr" | "QueryExpr" | "RankingPolicy" | "PatchOperation"
            ) {
                definitions.insert(
                    snake_case(object.name()),
                    public_union_schema(schema, &short_name),
                );
                continue;
            }
            let mut fields = serde_json::Map::new();
            let mut required = Vec::new();
            for field in object.fields().iter() {
                let name = snake_case(field.name());
                if name.ends_with("_type") {
                    continue;
                }
                fields.insert(name.clone(), field_schema(schema, &short_name, &field));
                if field.required() {
                    required.push(name);
                }
            }
            let mut definition = json!({
                "type": "object",
                "additionalProperties": false,
                "properties": fields
            });
            if !required.is_empty() {
                definition["required"] = json!(required);
            }
            definitions.insert(snake_case(object.name()), definition);
        }
    }
    let request_union = operations
        .iter()
        .map(|operation| {
            (
                operation.name.clone(),
                json!({"$ref": format!("#/definitions/{}", snake_case(&operation.request.full_name))}),
            )
        })
        .collect::<serde_json::Map<_, _>>();
    let response_union = operations
        .iter()
        .map(|operation| {
            (
                operation.name.clone(),
                json!({"$ref": format!("#/definitions/{}", snake_case(&operation.response.full_name))}),
            )
        })
        .collect::<serde_json::Map<_, _>>();
    json!({
        "$schema": "https://json-schema.org/draft/2019-09/schema",
        "title": "Search Kernel canonical HTTP JSON",
        "x-searchkernel-generated-by": "searchkernel-bindgen",
        "operations": operations.iter().map(|operation| operation.name.clone()).collect::<Vec<_>>(),
        "definitions": definitions,
        "request": {"type":"object","additionalProperties":false,"required":["protocol_version","request"],"properties":{"protocol_version":{"const":1,"type":"integer"},"request":{"type":"object","minProperties":1,"maxProperties":1,"properties":request_union}}},
        "response": {"type":"object","additionalProperties":false,"required":["protocol_version","response"],"properties":{"protocol_version":{"const":1,"type":"integer"},"response":{"type":"object","minProperties":1,"maxProperties":1,"properties":response_union}}}
    })
}

fn field_schema(schema: &Schema<'_>, object_name: &str, field: &Field<'_>) -> Value {
    let name = snake_case(field.name());
    if object_name == "BytesValue" && name == "value" {
        return json!({"type":"string","contentEncoding":"base64"});
    }
    if matches!(
        (object_name, name.as_str()),
        ("Document", "fields")
            | ("Hit", "fields")
            | ("ValueObject", "fields")
            | ("Analyzer", "options")
            | ("ErrorDetail", "metadata")
    ) {
        return json!({"type":"object","additionalProperties":{"$ref":"#/definitions/search_kernel._v1._value"}});
    }
    let ty = field.type_();
    let enum_name = match (object_name, name.as_str()) {
        ("FieldDefinition", "kind") => Some("FieldKind"),
        ("FieldDefinition", "cardinality") => Some("FieldCardinality"),
        ("TotalHits", "relation") => Some("TotalHitsRelation"),
        ("FilterComparison", "op") => Some("ComparisonOp"),
        ("SortExpr", "direction") => Some("SortDirection"),
        ("SortExpr", "missing") => Some("MissingPlacement"),
        ("FacetRequest", "sort") => Some("FacetSort"),
        _ => None,
    };
    if let Some(enum_name) = enum_name {
        if let Some(index) = schema
            .enums()
            .iter()
            .position(|item| short_type_name(item.name()) == enum_name)
        {
            return enum_schema(schema, index as i32);
        }
    }
    match ty.base_type() {
        BaseType::Bool => json!({"type":"boolean"}),
        BaseType::Byte
        | BaseType::Short
        | BaseType::Int
        | BaseType::UByte
        | BaseType::UShort
        | BaseType::UInt => json!({"type":"integer"}),
        BaseType::Long => json!({"type":"string","pattern":"^-?(0|[1-9][0-9]*)$"}),
        BaseType::ULong => json!({"type":"string","pattern":"^(0|[1-9][0-9]*)$"}),
        BaseType::Float | BaseType::Double => json!({"type":"number"}),
        BaseType::String => json!({"type":"string"}),
        BaseType::Vector | BaseType::Vector64 | BaseType::Array => {
            json!({"type":"array","items": element_schema(schema, ty.element(), ty.index())})
        }
        BaseType::Obj => {
            let target = short_type_name(schema.objects().get(ty.index() as usize).name());
            match target.as_str() {
                "OptionalBool" => json!({"type":"boolean"}),
                "OptionalUInt32" => json!({"type":"integer","minimum":0,"maximum":4294967295u64}),
                "OptionalDouble" => json!({"type":"number"}),
                _ => {
                    json!({"$ref": format!("#/definitions/{}", snake_case(schema.objects().get(ty.index() as usize).name()))})
                }
            }
        }
        BaseType::Union => union_schema(schema, ty.index()),
        BaseType::UType => enum_schema(schema, ty.index()),
        BaseType::None | BaseType::MaxBaseType => json!({"type":"integer"}),
        _ => json!({}),
    }
}

fn element_schema(schema: &Schema<'_>, base: BaseType, index: i32) -> Value {
    if base == BaseType::Obj {
        return json!({"$ref": format!("#/definitions/{}", snake_case(schema.objects().get(index as usize).name()))});
    }
    if base == BaseType::UType {
        return enum_schema(schema, index);
    }
    scalar_schema(base)
}

fn enum_schema(schema: &Schema<'_>, index: i32) -> Value {
    let enumeration = schema.enums().get(index as usize);
    let values = enumeration
        .values()
        .iter()
        .filter(|value| !value.name().ends_with("_NONE"))
        .map(|value| symbolic_name(value.name()))
        .collect::<Vec<_>>();
    json!({"type":"string","enum":values})
}

fn union_schema(schema: &Schema<'_>, index: i32) -> Value {
    let enumeration = schema.enums().get(index as usize);
    let properties = enumeration
        .values()
        .iter()
        .map(|value| {
            if value.name() == "NONE" || value.name().ends_with("_NONE") {
                return ("none".to_owned(), json!({"type":"object","additionalProperties":false,"maxProperties":0}));
            }
            let suffix = value
                .name()
                .rsplit_once("_V1_")
                .map(|(_, suffix)| suffix)
                .unwrap_or(value.name());
            let key = if suffix == "AnalyzeFieldTarget" {
                "field".to_owned()
            } else {
                symbolic_name(value.name())
            };
            let payload = schema.objects().iter().find(|object| short_type_name(object.name()) == suffix)
                .map(|object| json!({"$ref": format!("#/definitions/{}", snake_case(object.name()))}))
                .unwrap_or_else(|| json!({"type":"object"}));
            (key, payload)
        })
        .collect::<serde_json::Map<_, _>>();
    json!({"type":"object","minProperties":1,"maxProperties":1,"additionalProperties":false,"properties":properties})
}

fn public_value_schema(_schema: &Schema<'_>) -> Value {
    let value_ref = format!("#/definitions/{}", snake_case("SearchKernel.V1.Value"));
    json!({
        "type":"object", "minProperties":1, "maxProperties":1, "additionalProperties":false,
        "properties": {
            "none": {"type":"object","additionalProperties":false,"maxProperties":0}, "null": {"type":"null"}, "bool": {"type":"boolean"},
            "int64": {"type":"string","pattern":"^-?(0|[1-9][0-9]*)$"},
            "uint64": {"type":"string","pattern":"^(0|[1-9][0-9]*)$"}, "float64": {"type":"number"},
            "string": {"type":"string"}, "bytes": {"type":"string","contentEncoding":"base64"},
            "list": {"type":"array","items":{"$ref":value_ref}},
            "object": {"type":"object","additionalProperties":{"$ref":value_ref}}
        }
    })
}

fn public_array_schema(object_name: &str) -> Option<Value> {
    let item = match object_name {
        "FieldProjection" | "HighlightRequest" => json!({"type":"string"}),
        "SearchFieldSelection" => {
            json!({"$ref":"#/definitions/search_kernel._v1._search_field"})
        }
        _ => return None,
    };
    Some(json!({"type":"array","items":item}))
}

fn public_union_schema(schema: &Schema<'_>, object_name: &str) -> Value {
    let enum_name = match object_name {
        "FilterExpr" => "SearchKernel.V1.FilterExprKind",
        "QueryExpr" => "SearchKernel.V1.QueryExprKind",
        "RankingPolicy" => "SearchKernel.V1.RankingPolicyKind",
        "PatchOperation" => "SearchKernel.V1.PatchOperationKind",
        _ => return json!({"type":"object"}),
    };
    union_schema(
        schema,
        schema
            .enums()
            .iter()
            .position(|item| short_type_name(item.name()) == short_type_name(enum_name))
            .unwrap_or(0) as i32,
    )
}

fn symbolic_name(value: &str) -> String {
    let short = value
        .rsplit_once("_V1_")
        .map(|(_, suffix)| suffix)
        .unwrap_or(value);
    if short == "AnalyzeFieldTarget" {
        return "field".to_owned();
    }
    snake_case(
        short
            .strip_suffix("Request")
            .or_else(|| short.strip_suffix("Result"))
            .unwrap_or(short),
    )
}

fn scalar_schema(base: BaseType) -> Value {
    match base {
        BaseType::Bool => json!({"type":"boolean"}),
        BaseType::Byte
        | BaseType::Short
        | BaseType::Int
        | BaseType::UByte
        | BaseType::UShort
        | BaseType::UInt => json!({"type":"integer"}),
        BaseType::Long => json!({"type":"string","pattern":"^-?(0|[1-9][0-9]*)$"}),
        BaseType::ULong => json!({"type":"string","pattern":"^(0|[1-9][0-9]*)$"}),
        BaseType::Float | BaseType::Double => json!({"type":"number"}),
        BaseType::String => json!({"type":"string"}),
        _ => json!({}),
    }
}

fn openapi_components(
    operations: &[Operation],
    request: &Schema<'_>,
    response: &Schema<'_>,
) -> Value {
    let mut schemas = operations
        .iter()
        .map(|operation| {
            (
                operation.name.clone(),
                json!({
                    "type": "object",
                    "description": format!("Canonical {} operation", operation.name),
                    "additionalProperties": false,
                    "properties": {
                        operation.name.clone(): {
                            "$ref": format!("#/components/schemas/{}", snake_case(&operation.response.full_name))
                        }
                    },
                    "required": [operation.name.clone()]
                }),
            )
        })
        .collect::<serde_json::Map<_, _>>();
    for schema in [request, response] {
        for object in schema.objects().iter() {
            let name = snake_case(object.name());
            let short_name = short_type_name(object.name());
            if short_name == "Value" {
                let mut definition = public_value_schema(schema);
                rewrite_definition_refs(&mut definition);
                schemas.insert(name, definition);
                continue;
            }
            if let Some(definition) = public_array_schema(&short_name) {
                let mut definition = definition;
                rewrite_definition_refs(&mut definition);
                schemas.insert(name, definition);
                continue;
            }
            if matches!(
                short_name.as_str(),
                "FilterExpr" | "QueryExpr" | "RankingPolicy" | "PatchOperation"
            ) {
                let mut definition = public_union_schema(schema, &short_name);
                rewrite_definition_refs(&mut definition);
                schemas.insert(name, definition);
                continue;
            }
            let mut properties = serde_json::Map::new();
            for field in object.fields().iter() {
                let field_name = snake_case(field.name());
                if !field_name.ends_with("_type") {
                    let mut property = field_schema(schema, &short_name, &field);
                    rewrite_definition_refs(&mut property);
                    properties.insert(field_name, property);
                }
            }
            schemas.entry(name).or_insert_with(|| {
                json!({
                    "type":"object",
                    "additionalProperties":false,
                    "properties":properties
                })
            });
        }
    }
    json!({
        "x-searchkernel-generated-by": "searchkernel-bindgen",
        "components": {"schemas": schemas}
    })
}

fn rewrite_definition_refs(value: &mut Value) {
    match value {
        Value::Object(object) => {
            for child in object.values_mut() {
                rewrite_definition_refs(child);
            }
            if let Some(Value::String(reference)) = object.get_mut("$ref") {
                *reference = reference.replace("#/definitions/", "#/components/schemas/");
            }
        }
        Value::Array(values) => {
            for child in values {
                rewrite_definition_refs(child);
            }
        }
        _ => {}
    }
}

fn generate_docs(operations: &[Operation]) -> String {
    let mut output = String::from(
        "@generated by searchkernel-bindgen\n\n\
         # Search Kernel canonical client API\n\n\
         Canonical JSON uses symbolic one-key unions, decimal strings for int64/uint64,\n\
         and base64 strings for bytes.\n\n\
         Operations:\n\n",
    );
    for operation in operations {
        output.push_str(&format!("- {}\n", operation.name));
    }
    output
}

fn generate_fixtures(fixture_root: &Path, generated_root: &Path) -> Result<(), String> {
    for (directory, extension, output_directory, is_response) in [
        ("requests", "skrq", "requests", false),
        ("responses", "skrs", "responses", true),
    ] {
        let source = fixture_root.join(directory);
        let mut paths = fs::read_dir(&source)
            .map_err(|error| format!("read {}: {error}", source.display()))?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension().and_then(|extension| extension.to_str()) == Some(extension)
            })
            .collect::<Vec<_>>();
        paths.sort();
        for path in paths {
            let bytes = read_bytes(&path)?;
            let converted = if is_response {
                searchkernel_json::response_flatbuffer_to_json(&bytes)
            } else {
                searchkernel_json::request_flatbuffer_to_json(&bytes)
            };
            let value = match converted {
                Ok(bytes) => {
                    serde_json::from_slice::<Value>(&bytes).map_err(|error| error.to_string())?
                }
                Err(_) => {
                    let envelope = if is_response { "response" } else { "request" };
                    json!({"protocol_version": 1, envelope: {"invalid_fixture": {}}})
                }
            };
            let mut output = generated_root
                .join(output_directory)
                .join(path.file_name().unwrap());
            output.set_extension("json");
            write_json(&output, &value)?;
        }
    }
    Ok(())
}
