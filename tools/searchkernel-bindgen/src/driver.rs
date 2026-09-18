use flatbuffers::root;
use flatbuffers_reflection::reflection::Schema;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

mod normalized;
mod python_emitter;

const PYTHON_TEMPLATE: &str = include_str!("../templates/searchkernel_python_models.py");
const JSON_TEMPLATE: &str = include_str!("../templates/searchkernel_json_generated.rs");
const GENERATED_PATHS: [&str; 3] = [
    "clients/python/searchkernel/generated/models.py",
    "reference/json-conformance/generated/schema-model.json",
    "core/crates/searchkernel-json/src/generated.rs",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Command {
    Generate,
    Check,
}

#[derive(Debug)]
struct Arguments {
    command: Command,
    bfbs_root: PathBuf,
    destination_root: PathBuf,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("searchkernel-bindgen: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let arguments = parse_arguments()?;
    let request_path = arguments.bfbs_root.join("searchkernel_request.bfbs");
    let response_path = arguments.bfbs_root.join("searchkernel_response.bfbs");
    let request_bytes = read_bytes(&request_path)?;
    let response_bytes = read_bytes(&response_path)?;
    let request = read_schema(&request_bytes, &request_path)?;
    let response = read_schema(&response_bytes, &response_path)?;
    let model = normalized::normalize(&request, &response)?;
    let generated = [
        python_emitter::generate(&model, PYTHON_TEMPLATE)?.into_bytes(),
        serde_json::to_vec_pretty(&normalized::schema_model(&model))
            .map_err(|error| error.to_string())?,
        format!("{}\n", JSON_TEMPLATE.trim_end()).into_bytes(),
    ];

    for (relative, mut contents) in GENERATED_PATHS.into_iter().zip(generated) {
        if relative.ends_with(".json") {
            contents.push(b'\n');
        }
        let path = arguments.destination_root.join(relative);
        match arguments.command {
            Command::Generate => write_file(&path, &contents)?,
            Command::Check => check_file(&path, &contents)?,
        }
    }
    Ok(())
}

fn parse_arguments() -> Result<Arguments, String> {
    let mut args = env::args().skip(1);
    let command = match args.next().as_deref() {
        Some("generate") => Command::Generate,
        Some("check") => Command::Check,
        _ => return Err(usage()),
    };
    let mut bfbs_root = None;
    let mut destination_root = None;
    while let Some(flag) = args.next() {
        let value = args
            .next()
            .ok_or_else(|| format!("missing value for {flag}\n{}", usage()))?;
        match (command, flag.as_str()) {
            (_, "--bfbs-root") => bfbs_root = Some(PathBuf::from(value)),
            (Command::Generate, "--output-root") => destination_root = Some(PathBuf::from(value)),
            (Command::Check, "--repository-root") => {
                destination_root = Some(PathBuf::from(value));
            }
            _ => return Err(format!("unknown argument {flag}\n{}", usage())),
        }
    }
    Ok(Arguments {
        command,
        bfbs_root: bfbs_root.ok_or_else(|| format!("missing --bfbs-root\n{}", usage()))?,
        destination_root: destination_root.ok_or_else(|| {
            let flag = match command {
                Command::Generate => "--output-root",
                Command::Check => "--repository-root",
            };
            format!("missing {flag}\n{}", usage())
        })?,
    })
}

fn usage() -> String {
    "usage: searchkernel-bindgen generate --bfbs-root <dir> --output-root <dir>\n       searchkernel-bindgen check --bfbs-root <dir> --repository-root <dir>".to_owned()
}

fn read_bytes(path: &Path) -> Result<Vec<u8>, String> {
    fs::read(path).map_err(|error| format!("{}: {error}", path.display()))
}

fn read_schema<'a>(bytes: &'a [u8], path: &Path) -> Result<Schema<'a>, String> {
    root::<Schema>(bytes).map_err(|error| format!("invalid BFBS {}: {error}", path.display()))
}

fn write_file(path: &Path, contents: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("{}: {error}", parent.display()))?;
    }
    fs::write(path, contents).map_err(|error| format!("{}: {error}", path.display()))
}

fn check_file(path: &Path, expected: &[u8]) -> Result<(), String> {
    let actual = read_bytes(path)?;
    if actual == expected {
        Ok(())
    } else {
        Err(format!(
            "generated artifact is stale: {}; run searchkernel-bindgen generate",
            path.display()
        ))
    }
}
