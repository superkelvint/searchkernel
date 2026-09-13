use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const REQUEST_VARIANTS: &[&str] = &[
    "SearchKernel_V1_InsertRequest",
    "SearchKernel_V1_UpsertRequest",
    "SearchKernel_V1_PatchRequest",
    "SearchKernel_V1_DeleteRequest",
    "SearchKernel_V1_GetRequest",
    "SearchKernel_V1_SearchRequest",
    "SearchKernel_V1_AnalyzeRequest",
    "SearchKernel_V1_DescribeRequest",
    "SearchKernel_V1_InspectQueryRequest",
];
const RESPONSE_VARIANTS: &[&str] = &[
    "SearchKernel_V1_InsertResult",
    "SearchKernel_V1_UpsertResult",
    "SearchKernel_V1_PatchResult",
    "SearchKernel_V1_DeleteResult",
    "SearchKernel_V1_GetResult",
    "SearchKernel_V1_SearchResult",
    "SearchKernel_V1_AnalyzeResult",
    "SearchKernel_V1_Error",
    "SearchKernel_V1_DescribeResult",
    "SearchKernel_V1_InspectQueryResult",
];
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
    verify_union(
        &request,
        "SearchKernel_FFI_V1_RequestEnvelope",
        REQUEST_VARIANTS,
    )?;
    verify_union(
        &response,
        "SearchKernel_FFI_V1_ResponseEnvelope",
        RESPONSE_VARIANTS,
    )?;

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
    let manifest = serde_json::json!({
        "flatc": "25.12.19",
        "schema_digest": schema_digest,
        "source": ["api/searchkernel.fbs", "api/searchkernel_ffi_request.fbs", "api/searchkernel_ffi_response.fbs"],
        "request_variants": REQUEST_VARIANTS,
        "response_variants": RESPONSE_VARIANTS,
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

    let template = root.join("crates/searchkernel-json/src/codec.rs");
    let generated_codec = root.join("crates/searchkernel-json/src/generated_codec.rs");
    let mut generated_bytes = b"// @generated by searchkernel-bindgen; do not modify\n".to_vec();
    generated_bytes
        .extend(fs::read(&template).map_err(|e| format!("{}: {e}", template.display()))?);
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
    generate_metadata(root, &request, &response, check)?;
    Ok(())
}

fn generate_metadata(
    root: &Path,
    request: &Value,
    response: &Value,
    check: bool,
) -> Result<(), String> {
    let mut definitions = BTreeMap::<String, Vec<String>>::new();
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
            definitions.entry(name.clone()).or_insert(fields);
        }
    }
    let mut output = String::from("// @generated by searchkernel-bindgen; do not modify\n\n");
    output.push_str("pub(crate) fn fields(name: &str) -> Option<&'static [&'static str]> {\n");
    output.push_str("    match name {\n");
    for (name, fields) in &definitions {
        output.push_str(&format!("        {name:?} => Some(&["));
        for field in fields {
            output.push_str(&format!("{field:?},"));
        }
        output.push_str("]),\n");
    }
    output.push_str("        _ => None,\n    }\n}\n");
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

fn verify_union(schema: &Value, envelope: &str, expected: &[&str]) -> Result<(), String> {
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
    let rendered = serde_json::to_string(variants).map_err(|e| e.to_string())?;
    for variant in expected {
        if !rendered.contains(variant) {
            return Err(format!("{envelope} is missing {variant}"));
        }
    }
    Ok(())
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
