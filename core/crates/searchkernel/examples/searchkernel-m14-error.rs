#[path = "support/m14.rs"]
mod m14;

use crate::m14::load;
use searchkernel_ir::{wire::v1::index_definition_to_ir, ErrorCode, PortableError};
use searchkernel_vespa_engine::{
    ConfigCompiler, DistanceMetric, Field, FieldType, HnswIndexConfig, Schema,
};
use std::{
    fs,
    path::{Path, PathBuf},
};

fn main() {
    if let Err(error) = run() {
        eprintln!("M14 error driver: {error}");
        std::process::exit(1)
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let errors = PathBuf::from(args.next().ok_or("errors")?);
    let id = args.next().ok_or("case")?.to_string_lossy().into_owned();
    let definition = args.next().map(PathBuf::from);
    let config = PathBuf::from(args.next().ok_or("config")?);
    if args.next().is_some() {
        return Err("unexpected arg".into());
    }

    let root = load(&errors);
    let case = root["cases"]
        .as_array()
        .and_then(|values| values.iter().find(|candidate| candidate["id"] == id))
        .ok_or("unknown case")?;
    let expected = case["expected"].as_str().ok_or("expected")?;

    match case["path"].as_str() {
        Some("low_level") => run_low(case, &config)?,
        Some("portable_ir") => run_portable_ir(definition.as_ref().ok_or("definition")?, &id)?,
        Some("adapter") => run_adapter(definition.as_ref().ok_or("definition")?, &config, &id)?,
        _ => return Err("unknown path".into()),
    }

    println!("{expected}");
    Ok(())
}

fn run_low(case: &serde_json::Value, config: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mutation = &case["mutation"];
    let mut field = if mutation["field_type"] == "string" {
        Field::new("embedding", FieldType::String)
    } else {
        Field::new("embedding", FieldType::Tensor("tensor<float>(x[3])".into()))
    };

    if mutation
        .get("attribute")
        .and_then(|value| value.as_bool())
        .unwrap_or(true)
    {
        field = field.attribute();
    }
    if mutation.get("distance_metric").is_some() || mutation["field_type"] != "string" {
        field = field.distance_metric(DistanceMetric::Euclidean);
    }
    if mutation
        .get("hnsw")
        .and_then(|value| value.as_bool())
        .unwrap_or(
            mutation.get("max_links_per_node").is_some()
                || mutation.get("neighbors_to_explore_at_insert").is_some(),
        )
    {
        let mut hnsw = HnswIndexConfig::new();
        if let Some(value) = mutation
            .get("max_links_per_node")
            .and_then(|value| value.as_u64())
        {
            hnsw = hnsw.max_links_per_node(value as u32);
        } else {
            hnsw = hnsw.max_links_per_node(4);
        }
        if let Some(value) = mutation
            .get("neighbors_to_explore_at_insert")
            .and_then(|value| value.as_u64())
        {
            hnsw = hnsw.neighbors_to_explore_at_insert(value as u32);
        } else {
            hnsw = hnsw.neighbors_to_explore_at_insert(16);
        }
        field = field.hnsw(hnsw);
    }

    let result = ConfigCompiler.compile_to(&Schema::new("typea").field(field), config);
    if result.is_ok() {
        return Err("negative low-level schema unexpectedly succeeded".into());
    }
    Ok(())
}

fn parse(path: &Path) -> Result<searchkernel_ir::IndexDefinitionIr, PortableError> {
    let bytes = fs::read(path).map_err(|error| {
        PortableError::new(
            ErrorCode::Internal,
            format!("failed to read portable definition: {error}"),
        )
    })?;
    let root = searchkernel_ir::__flatbuffers::root::<
        searchkernel_ir::wire::v1::schema::IndexDefinition,
    >(&bytes)
    .map_err(|error| {
        PortableError::invalid(
            "malformed_flatbuffer",
            Some("definition"),
            error.to_string(),
        )
    })?;
    index_definition_to_ir(root)
}

fn assert_portable_error(
    error: &PortableError,
    detail_code: &str,
    path_suffix: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let detail = error
        .details
        .first()
        .ok_or("portable error had no details")?;
    if detail.code != detail_code {
        return Err(format!(
            "wrong portable detail code: expected {detail_code}, got {}",
            detail.code
        )
        .into());
    }
    let path = detail.path.as_deref().unwrap_or_default();
    if !path.ends_with(path_suffix) {
        return Err(format!(
            "wrong portable error path: expected suffix {path_suffix}, got {path:?}"
        )
        .into());
    }
    Ok(())
}

fn run_portable_ir(path: &Path, id: &str) -> Result<(), Box<dyn std::error::Error>> {
    let error = match parse(path) {
        Ok(_) => return Err("negative portable IR schema unexpectedly succeeded".into()),
        Err(error) => error,
    };
    match id {
        "portable-hnsw-non-tensor" => {
            assert_portable_error(&error, "invalid_type_combination", ".index.hnsw")
        }
        "portable-metric-non-tensor" => assert_portable_error(
            &error,
            "invalid_type_combination",
            ".attribute.distance_metric",
        ),
        _ => Err("unexpected portable IR negative case".into()),
    }
}

fn run_adapter(path: &Path, config: &Path, id: &str) -> Result<(), Box<dyn std::error::Error>> {
    let ir = parse(path)?;
    let error = match searchkernel_ir_adapter::compile_index_definition(&ir, config) {
        Ok(_) => return Err("negative adapter schema unexpectedly succeeded".into()),
        Err(error) => error,
    };
    if error.code != ErrorCode::Unsupported {
        return Err(format!("adapter returned wrong error code: {:?}", error.code).into());
    }
    let detail = error
        .details
        .first()
        .ok_or("adapter error had no details")?;
    if detail.code != "schema_feature_not_supported" {
        return Err(format!("adapter returned wrong detail code: {}", detail.code).into());
    }
    match id {
        "portable-hnsw-options" | "portable-attribute-options" => Ok(()),
        _ => Err("unexpected adapter negative case".into()),
    }
}
