//! M12 negative-driver contract.

#[path = "support/m12.rs"]
mod m12;

use crate::m12::{load, low_level_request, portable_request_json};
use searchkernel_dispatch::Dispatcher;
use searchkernel_ir::{wire::v1::index_definition_to_ir, IndexContext};
use searchkernel_protocol::json::{request_json_to_flatbuffer, response_flatbuffer_to_json};
use searchkernel_vespa_engine::{Engine, Error as EngineError};
use serde_json::{json, Value as Json};
use std::{
    env, fs,
    path::{Path, PathBuf},
    sync::Arc,
};

fn main() {
    if let Err(error) = run() {
        eprintln!("M12 error driver: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args_os().skip(1);
    let errors_path = PathBuf::from(args.next().ok_or("errors.json is required")?);
    let case_id = args
        .next()
        .ok_or("error case id is required")?
        .to_string_lossy()
        .into_owned();
    let config = PathBuf::from(args.next().ok_or("config directory is required")?);
    let data = PathBuf::from(args.next().ok_or("data directory is required")?);
    let definition = PathBuf::from(args.next().ok_or("definition is required")?);
    if args.next().is_some() {
        return Err("unexpected argument".into());
    }

    let errors = load(&errors_path);
    let case = errors["cases"]
        .as_array()
        .and_then(|cases| cases.iter().find(|case| case["id"] == case_id))
        .ok_or_else(|| format!("unknown M12 error case {case_id}"))?;
    let mutation = &case["mutation"];
    let baseline = load(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../reference/vespa/m12/scenarios/01-exact-near-far.json"),
    );
    let step = &baseline["steps"][0];

    let category = match case["path"].as_str() {
        Some("low_level") => run_low_level(&config, &data, step, mutation)?,
        Some("portable") => run_portable(&config, &data, &definition, step, mutation)?,
        Some(path) => return Err(format!("unsupported M12 error path {path}").into()),
        None => return Err("M12 error case has no path".into()),
    };
    println!("{category}");
    Ok(())
}

fn run_low_level(
    config: &Path,
    data: &Path,
    baseline: &Json,
    mutation: &Json,
) -> Result<&'static str, Box<dyn std::error::Error>> {
    let mut step = baseline.clone();
    apply_low_level_mutation(&mut step, mutation)?;
    let engine = Engine::open(data, config)?;
    let result = engine.search(low_level_request(&step));
    let category = match result {
        Ok(_) => return Err("negative low-level request unexpectedly succeeded".into()),
        Err(error) => category_for_low_level_failure(mutation, &error)?,
    };
    engine.close()?;
    Ok(category)
}

fn run_portable(
    config: &Path,
    data: &Path,
    definition: &Path,
    baseline: &Json,
    mutation: &Json,
) -> Result<&'static str, Box<dyn std::error::Error>> {
    let definition_bytes = fs::read(definition)?;
    let root = searchkernel_ir::__flatbuffers::root::<
        searchkernel_ir::wire::v1::schema::IndexDefinition,
    >(&definition_bytes)?;
    let context = Arc::new(IndexContext::from_definition_ir(index_definition_to_ir(
        root,
    )?));
    let backend = Arc::new(
        searchkernel_ir_adapter::VespaBackend::open(data, config)
            .map_err(|error| error.to_string())?,
    );
    let dispatcher = Dispatcher::from_backend(context, backend);
    let request_json = portable_error_request_json(baseline, mutation)?;
    let request = match request_json_to_flatbuffer(&request_json) {
        Ok(request) => request,
        Err(error) => {
            if mutation["portable_non_finite"] != true
                || !error.to_string().to_ascii_lowercase().contains("finite")
            {
                return Err(
                    format!("unexpected portable request conversion error: {error}").into(),
                );
            }
            dispatcher.close().map_err(|error| error.to_string())?;
            return category_for_mutation(mutation);
        }
    };
    let response = dispatcher
        .dispatch(&request)
        .map_err(|error| format!("portable dispatch failed: {error:?}"))?;
    let printed = response_flatbuffer_to_json(&response)?;
    let response: Json = serde_json::from_slice(&printed)?;
    if response["payload_type"] != "SearchKernel_Wire_Error" {
        return Err("negative portable request unexpectedly succeeded".into());
    }
    let detail = response["payload"]["details"][0]["code"]
        .as_str()
        .ok_or("portable error response has no detail code")?;
    validate_portable_detail(mutation, detail)?;
    dispatcher.close().map_err(|error| error.to_string())?;
    category_for_mutation(mutation)
}

fn category_for_low_level_failure(
    mutation: &Json,
    error: &EngineError,
) -> Result<&'static str, Box<dyn std::error::Error>> {
    let category = category_for_mutation(mutation)?;
    let message = error.to_string().to_ascii_lowercase();
    let matches = match category {
        "duplicate_query_tensor" => message.contains("duplicate"),
        "query_input_collision" => message.contains("collision"),
        "query_tensor_field_invalid" => {
            message.contains("field") || message.contains("tensor attribute")
        }
        "query_tensor_type_mismatch" => message.contains("tensor") || message.contains("type"),
        "invalid_query_tensor" => true,
        _ => false,
    };
    if !matches {
        return Err(format!("unexpected native error for {category}: {error}").into());
    }
    Ok(category)
}

fn validate_portable_detail(
    mutation: &Json,
    detail: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let expected = match category_for_mutation(mutation)? {
        "query_tensor_missing" => "missing_query_input",
        "non_finite_tensor_cell" => "non_finite_tensor_cell",
        "expression_query_input_not_supported" => "query_input_expression_not_supported",
        "sparse_query_tensor_not_supported" | "mixed_query_tensor_not_supported" => {
            "query_input_tensor_not_supported"
        }
        category => {
            return Err(format!("unexpected portable mutation category {category}").into());
        }
    };
    if detail != expected {
        return Err(format!("unexpected portable detail {detail}, expected {expected}").into());
    }
    Ok(())
}

fn apply_low_level_mutation(
    step: &mut Json,
    mutation: &Json,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(name) = mutation.get("tensor_name") {
        step["inputs"][0]["name"] = name.clone();
    }
    if let Some(type_spec) = mutation.get("type_spec") {
        step["inputs"][0]["type_spec"] = type_spec.clone();
    }
    if let Some(values) = mutation.get("values") {
        step["inputs"][0]["values"] = values.clone();
    }
    if let Some(field) = mutation.get("document_field") {
        step["document_field"] = field.clone();
    }
    if let Some(non_finite) = mutation.get("non_finite").and_then(Json::as_str) {
        step["inputs"][0]["values"] = match non_finite {
            "nan" => json!(["nan", 0.0, 0.0]),
            "positive_infinity" => json!(["positive_infinity", 0.0, 0.0]),
            value => return Err(format!("unknown non-finite value {value}").into()),
        };
    }
    if mutation["duplicate_tensor_name"] == true {
        let duplicate = step["inputs"][0].clone();
        step["inputs"]
            .as_array_mut()
            .ok_or("inputs must be an array")?
            .push(duplicate);
    }
    if mutation["scalar_tensor_collision"] == true {
        step["scalar_query_features"] = json!([{"name":"q_embedding","value":1.0}]);
    }
    if mutation["rank_property_tensor_collision"] == true {
        step["rank_properties"] = json!([{"name":"q_embedding","value":"3"}]);
    }
    Ok(())
}

fn portable_error_request_json(
    baseline: &Json,
    mutation: &Json,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut request: Json = serde_json::from_slice(&portable_request_json(&baseline["steps"][0]))?;
    let inputs = request["payload"]["inputs"]
        .as_array_mut()
        .ok_or("portable inputs must be an array")?;

    if mutation["omit_referenced_input"] == true {
        inputs.clear();
        return Ok(serde_json::to_vec(&request)?);
    }
    let input = inputs
        .first_mut()
        .ok_or("portable baseline input is missing")?;
    if let Some(expression) = mutation.get("expression_input") {
        *input = json!({
            "name":"q_embedding",
            "input_type":"ExpressionQueryInput",
            "input":{"expression":expression}
        });
    }
    if mutation["sparse_tensor"] == true {
        input["input"]["value"]["kind"]["payload_type"] = "SparseTensorCells".into();
        input["input"]["value"]["kind"]["payload"] = json!({
            "cells":[{"address":[{"dimension":"x","label":"0"}],"value":1.0}]
        });
    }
    if mutation["mixed_tensor"] == true {
        input["input"]["value"]["kind"]["payload_type"] = "MixedTensorBlocks".into();
        input["input"]["value"]["kind"]["payload"] = json!({
            "blocks":[{"address":[],"values":[1.0,0.0,0.0]}]
        });
    }
    let mut encoded = serde_json::to_vec(&request)?;
    if mutation["portable_non_finite"] == true {
        let text = String::from_utf8(encoded)?;
        encoded = text
            .replacen("1.0,0.0,0.0", "1e309,0.0,0.0", 1)
            .into_bytes();
    }
    Ok(encoded)
}

fn category_for_mutation(mutation: &Json) -> Result<&'static str, Box<dyn std::error::Error>> {
    if mutation["duplicate_tensor_name"] == true {
        Ok("duplicate_query_tensor")
    } else if mutation["scalar_tensor_collision"] == true
        || mutation["rank_property_tensor_collision"] == true
    {
        Ok("query_input_collision")
    } else if mutation["omit_referenced_input"] == true {
        Ok("query_tensor_missing")
    } else if mutation["portable_non_finite"] == true {
        Ok("non_finite_tensor_cell")
    } else if mutation.get("expression_input").is_some() {
        Ok("expression_query_input_not_supported")
    } else if mutation["sparse_tensor"] == true {
        Ok("sparse_query_tensor_not_supported")
    } else if mutation["mixed_tensor"] == true {
        Ok("mixed_query_tensor_not_supported")
    } else if mutation.get("document_field").is_some() {
        Ok("query_tensor_field_invalid")
    } else if mutation["type_spec"].as_str() == Some("tensor<float>(x[4])") {
        Ok("query_tensor_type_mismatch")
    } else if mutation.get("tensor_name").is_some()
        || mutation.get("type_spec").is_some()
        || mutation.get("values").is_some()
        || mutation.get("non_finite").is_some()
    {
        Ok("invalid_query_tensor")
    } else {
        Err("unknown M12 error mutation".into())
    }
}
