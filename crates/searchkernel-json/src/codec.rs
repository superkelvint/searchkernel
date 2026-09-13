//! Source template for the generated JSON codec.
//!
//! `searchkernel-bindgen` copies this implementation into
//! `generated_codec.rs` after validating the BFBS schema. The checked-in
//! generated file is the module compiled by this crate.

use base64::Engine;
use flatbuffers::{root, FlatBufferBuilder, UnionWIPOffset, WIPOffset};
use searchkernel_api::v1 as api;
use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Value};
use std::fmt;

use crate::generated::search_kernel::ffi::v1 as transport;

const PROTOCOL_VERSION: u32 = 1;
const BASE64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

#[derive(Debug)]
pub enum JsonCodecError {
    Syntax(String),
    Contract(String),
    FlatBuffer(String),
}

impl fmt::Display for JsonCodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Syntax(s) => write!(f, "JSON syntax error: {s}"),
            Self::Contract(s) => write!(f, "JSON contract error: {s}"),
            Self::FlatBuffer(s) => write!(f, "FlatBuffers error: {s}"),
        }
    }
}

impl std::error::Error for JsonCodecError {}

struct StrictValue(Value);

impl<'de> Deserialize<'de> for StrictValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = StrictValue;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a JSON value")
            }
            fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(StrictValue(Value::Bool(v)))
            }
            fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(StrictValue(Value::Number(v.into())))
            }
            fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(StrictValue(Value::Number(v.into())))
            }
            fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                serde_json::Number::from_f64(v)
                    .map(|n| StrictValue(Value::Number(n)))
                    .ok_or_else(|| E::custom("non-finite JSON number"))
            }
            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(StrictValue(Value::String(v.to_owned())))
            }
            fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(StrictValue(Value::String(v)))
            }
            fn visit_none<E>(self) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(StrictValue(Value::Null))
            }
            fn visit_unit<E>(self) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(StrictValue(Value::Null))
            }
            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut out = Vec::new();
                while let Some(StrictValue(v)) = seq.next_element()? {
                    out.push(v);
                }
                Ok(StrictValue(Value::Array(out)))
            }
            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut out = Map::new();
                while let Some(key) = map.next_key::<String>()? {
                    if out.contains_key(&key) {
                        return Err(de::Error::custom(format!("duplicate object field `{key}`")));
                    }
                    let StrictValue(value) = map.next_value()?;
                    out.insert(key, value);
                }
                Ok(StrictValue(Value::Object(out)))
            }
        }
        deserializer.deserialize_any(V)
    }
}

fn parse(input: &[u8]) -> Result<Value, JsonCodecError> {
    let mut deserializer = serde_json::Deserializer::from_slice(input);
    let StrictValue(value) = StrictValue::deserialize(&mut deserializer)
        .map_err(|e| JsonCodecError::Syntax(e.to_string()))?;
    deserializer
        .end()
        .map_err(|e| JsonCodecError::Syntax(e.to_string()))?;
    Ok(value)
}

fn contract(message: impl Into<String>) -> JsonCodecError {
    JsonCodecError::Contract(message.into())
}

const ABSENT_VECTOR_KEY: &str = "$absent";

fn absent_vector_json() -> Value {
    serde_json::json!({ABSENT_VECTOR_KEY: true})
}

fn is_absent_vector(value: &Value, path: &str) -> Result<bool, JsonCodecError> {
    let Some(object) = value.as_object() else {
        return Ok(false);
    };
    if object.len() == 1 && object.get(ABSENT_VECTOR_KEY) == Some(&Value::Bool(true)) {
        return Ok(true);
    }
    if object.contains_key(ABSENT_VECTOR_KEY) {
        return Err(contract(format!(
            "{path}.{ABSENT_VECTOR_KEY} must be true and must be the only field"
        )));
    }
    Ok(false)
}

fn object<'a>(value: &'a Value, path: &str) -> Result<&'a Map<String, Value>, JsonCodecError> {
    value
        .as_object()
        .ok_or_else(|| contract(format!("{path} must be an object")))
}

fn check_keys(
    object: &Map<String, Value>,
    message: &str,
    path: &str,
) -> Result<(), JsonCodecError> {
    let allowed = crate::generated::schema::fields(message).ok_or_else(|| {
        JsonCodecError::FlatBuffer(format!("missing generated schema metadata for `{message}`"))
    })?;
    if let Some(key) = object.keys().find(|key| !allowed.contains(&key.as_str())) {
        return Err(contract(format!("unknown field `{path}.{key}`")));
    }
    Ok(())
}

fn check_envelope_keys(object: &Map<String, Value>, path: &str) -> Result<(), JsonCodecError> {
    // `operation` and `request`/`response` are the generated codec's natural
    // presentation of the canonical FlatBuffers union. They intentionally do
    // not mirror the raw envelope fields (`payload_type`, `payload`).
    let allowed = ["protocol_version", "operation", "request", "response"];
    if let Some(key) = object.keys().find(|key| !allowed.contains(&key.as_str())) {
        return Err(contract(format!("unknown field `{path}.{key}`")));
    }
    Ok(())
}

fn field<'a>(object: &'a Map<String, Value>, name: &str) -> Option<&'a Value> {
    object.get(name)
}

fn required<'a>(
    object: &'a Map<String, Value>,
    name: &str,
    path: &str,
) -> Result<&'a Value, JsonCodecError> {
    field(object, name).ok_or_else(|| contract(format!("missing field `{path}.{name}`")))
}

fn non_null<'a>(value: &'a Value, path: &str) -> Result<&'a Value, JsonCodecError> {
    if value.is_null() {
        Err(contract(format!("`{path}` does not accept JSON null")))
    } else {
        Ok(value)
    }
}

fn string(value: &Value, path: &str) -> Result<String, JsonCodecError> {
    non_null(value, path)?
        .as_str()
        .map(ToOwned::to_owned)
        .ok_or_else(|| contract(format!("`{path}` must be a string")))
}

fn optional_string(
    object: &Map<String, Value>,
    name: &str,
    path: &str,
) -> Result<Option<String>, JsonCodecError> {
    field(object, name)
        .map(|v| string(v, &format!("{path}.{name}")))
        .transpose()
}

fn bool_value(value: &Value, path: &str) -> Result<bool, JsonCodecError> {
    non_null(value, path)?
        .as_bool()
        .ok_or_else(|| contract(format!("`{path}` must be a boolean")))
}

fn u32_value(value: &Value, path: &str) -> Result<u32, JsonCodecError> {
    let value = non_null(value, path)?;
    value
        .as_u64()
        .and_then(|v| u32::try_from(v).ok())
        .ok_or_else(|| contract(format!("`{path}` must be an unsigned 32-bit integer")))
}

fn f64_value(value: &Value, path: &str) -> Result<f64, JsonCodecError> {
    let value = non_null(value, path)?;
    let number = value
        .as_f64()
        .ok_or_else(|| contract(format!("`{path}` must be a number")))?;
    if number.is_finite() {
        Ok(number)
    } else {
        Err(contract(format!("`{path}` must be finite")))
    }
}

fn decimal<T: std::str::FromStr>(
    value: &Value,
    path: &str,
    type_name: &str,
) -> Result<T, JsonCodecError> {
    let text = string(value, path)?;
    text.parse::<T>()
        .map_err(|_| contract(format!("`{path}` must be a decimal {type_name}")))
}

fn array<'a>(value: &'a Value, path: &str) -> Result<&'a [Value], JsonCodecError> {
    non_null(value, path)?
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| contract(format!("`{path}` must be an array")))
}

fn optional_bool(
    object: &Map<String, Value>,
    name: &str,
    path: &str,
) -> Result<Option<api::OptionalBool>, JsonCodecError> {
    field(object, name)
        .map(|v| bool_value(v, &format!("{path}.{name}")).map(api::OptionalBool::new))
        .transpose()
}

fn optional_u32(
    object: &Map<String, Value>,
    name: &str,
    path: &str,
) -> Result<Option<api::OptionalUInt32>, JsonCodecError> {
    field(object, name)
        .map(|v| u32_value(v, &format!("{path}.{name}")).map(api::OptionalUInt32::new))
        .transpose()
}

fn optional_f64(
    object: &Map<String, Value>,
    name: &str,
    path: &str,
) -> Result<Option<api::OptionalDouble>, JsonCodecError> {
    field(object, name)
        .map(|v| f64_value(v, &format!("{path}.{name}")).map(api::OptionalDouble::new))
        .transpose()
}

fn one_key<'a>(value: &'a Value, path: &str) -> Result<(&'a str, &'a Value), JsonCodecError> {
    let object = object(value, path)?;
    if object.len() != 1 {
        return Err(contract(format!(
            "`{path}` must contain exactly one variant"
        )));
    }
    let (key, value) = object.iter().next().expect("length checked");
    Ok((key, value))
}

pub fn request_json_to_flatbuffer(input: &[u8]) -> Result<Vec<u8>, JsonCodecError> {
    let value = parse(input)?;
    let envelope = object(&value, "request")?;
    check_envelope_keys(envelope, "request")?;
    let version = u32_value(
        required(envelope, "protocol_version", "request")?,
        "request.protocol_version",
    )?;
    if version != PROTOCOL_VERSION {
        return Err(contract(format!("unsupported protocol_version {version}")));
    }
    let operation = string(
        required(envelope, "operation", "request")?,
        "request.operation",
    )?;
    let payload = object(required(envelope, "request", "request")?, "request.request")?;

    let mut builder = FlatBufferBuilder::new();
    let (kind, payload) = match operation.as_str() {
        "insert" => (
            transport::RequestPayload::SearchKernel_V1_InsertRequest,
            insert(&mut builder, payload)?,
        ),
        "upsert" => (
            transport::RequestPayload::SearchKernel_V1_UpsertRequest,
            upsert(&mut builder, payload)?,
        ),
        "patch" => (
            transport::RequestPayload::SearchKernel_V1_PatchRequest,
            patch(&mut builder, payload)?,
        ),
        "delete" => (
            transport::RequestPayload::SearchKernel_V1_DeleteRequest,
            delete(&mut builder, payload)?,
        ),
        "get" => (
            transport::RequestPayload::SearchKernel_V1_GetRequest,
            get(&mut builder, payload)?,
        ),
        "search" => (
            transport::RequestPayload::SearchKernel_V1_SearchRequest,
            offset(search(&mut builder, payload)?),
        ),
        "analyze" => (
            transport::RequestPayload::SearchKernel_V1_AnalyzeRequest,
            analyze(&mut builder, payload)?,
        ),
        "describe" => (
            transport::RequestPayload::SearchKernel_V1_DescribeRequest,
            describe(&mut builder, payload)?,
        ),
        "inspect_query" => (
            transport::RequestPayload::SearchKernel_V1_InspectQueryRequest,
            inspect_query(&mut builder, payload)?,
        ),
        _ => return Err(contract(format!("unknown operation `{operation}`"))),
    };
    let root = transport::RequestEnvelope::create(
        &mut builder,
        &transport::RequestEnvelopeArgs {
            protocol_version: version,
            payload_type: kind,
            payload: Some(payload),
        },
    );
    builder.finish(root, Some(transport::REQUEST_ENVELOPE_IDENTIFIER));
    Ok(builder.finished_data().to_vec())
}

/// Convert a canonical SKRQ envelope to the public JSON representation.
///
/// Keeping this operation beside the JSON-to-FlatBuffers entry point makes
/// request conformance tests exercise the same representation in both
/// directions; it is not a second semantic request path.
pub fn request_flatbuffer_to_json(input: &[u8]) -> Result<Vec<u8>, JsonCodecError> {
    if input.len() < 8 || input[4..8] != *transport::REQUEST_ENVELOPE_IDENTIFIER.as_bytes() {
        return Err(JsonCodecError::FlatBuffer(
            "missing SKRQ file identifier".to_owned(),
        ));
    }
    let envelope = root::<transport::RequestEnvelope<'_>>(input)
        .map_err(|e| JsonCodecError::FlatBuffer(e.to_string()))?;
    if envelope.protocol_version() != PROTOCOL_VERSION {
        return Err(JsonCodecError::Contract(format!(
            "unsupported protocol_version {}",
            envelope.protocol_version()
        )));
    }
    let (operation, payload) = match envelope.payload_type().0 {
        x if x == transport::RequestPayload::SearchKernel_V1_InsertRequest.0 => (
            "insert",
            request_insert(envelope.payload_as_search_kernel_v1_insert_request())?,
        ),
        x if x == transport::RequestPayload::SearchKernel_V1_UpsertRequest.0 => (
            "upsert",
            request_upsert(envelope.payload_as_search_kernel_v1_upsert_request())?,
        ),
        x if x == transport::RequestPayload::SearchKernel_V1_PatchRequest.0 => (
            "patch",
            request_patch(envelope.payload_as_search_kernel_v1_patch_request())?,
        ),
        x if x == transport::RequestPayload::SearchKernel_V1_DeleteRequest.0 => (
            "delete",
            request_delete(envelope.payload_as_search_kernel_v1_delete_request())?,
        ),
        x if x == transport::RequestPayload::SearchKernel_V1_GetRequest.0 => (
            "get",
            request_get(envelope.payload_as_search_kernel_v1_get_request())?,
        ),
        x if x == transport::RequestPayload::SearchKernel_V1_SearchRequest.0 => (
            "search",
            request_search(envelope.payload_as_search_kernel_v1_search_request())?,
        ),
        x if x == transport::RequestPayload::SearchKernel_V1_AnalyzeRequest.0 => (
            "analyze",
            request_analyze(envelope.payload_as_search_kernel_v1_analyze_request())?,
        ),
        x if x == transport::RequestPayload::SearchKernel_V1_DescribeRequest.0 => {
            ("describe", Value::Object(Map::new()))
        }
        x if x == transport::RequestPayload::SearchKernel_V1_InspectQueryRequest.0 => (
            "inspect_query",
            request_inspect(envelope.payload_as_search_kernel_v1_inspect_query_request())?,
        ),
        0 => {
            return Err(JsonCodecError::FlatBuffer(
                "missing request payload".to_owned(),
            ))
        }
        other => {
            return Err(JsonCodecError::FlatBuffer(format!(
                "unknown request discriminator {other}"
            )))
        }
    };
    serde_json::to_vec(&serde_json::json!({
        "protocol_version": PROTOCOL_VERSION,
        "operation": operation,
        "request": payload,
    }))
    .map_err(|e| JsonCodecError::Contract(e.to_string()))
}

fn request_insert(value: Option<api::InsertRequest<'_>>) -> Result<Value, JsonCodecError> {
    let value = required_result(value, "insert")?;
    let mut out = Map::new();
    if let Some(document) = value.document() {
        out.insert("document".to_owned(), document_json(document)?);
    }
    Ok(Value::Object(out))
}

fn request_upsert(value: Option<api::UpsertRequest<'_>>) -> Result<Value, JsonCodecError> {
    let value = required_result(value, "upsert")?;
    let mut out = Map::new();
    if let Some(document) = value.document() {
        out.insert("document".to_owned(), document_json(document)?);
    }
    Ok(Value::Object(out))
}

fn request_patch(value: Option<api::PatchRequest<'_>>) -> Result<Value, JsonCodecError> {
    let value = required_result(value, "patch")?;
    let mut out = Map::new();
    if let Some(id) = value.id() {
        out.insert("id".to_owned(), Value::String(id.to_owned()));
    }
    if let Some(items) = value.operations() {
        let mut operations = Vec::new();
        for item in items.iter() {
            operations.push(patch_operation_json(item)?);
        }
        out.insert("operations".to_owned(), Value::Array(operations));
    }
    Ok(Value::Object(out))
}

fn patch_operation_json(value: api::PatchOperation<'_>) -> Result<Value, JsonCodecError> {
    match value.operation_type() {
        api::PatchOperationKind::SetField => {
            let operation = required_result(value.operation_as_set_field(), "set_field")?;
            Ok(serde_json::json!({"set_field": {
                "field": required_result(operation.field(), "set_field.field")?,
                "value": value_json(required_result(operation.value(), "set_field.value")?)?
            }}))
        }
        api::PatchOperationKind::UnsetField => {
            let operation = required_result(value.operation_as_unset_field(), "unset_field")?;
            Ok(serde_json::json!({"unset_field": {
                "field": required_result(operation.field(), "unset_field.field")?
            }}))
        }
        _ => Err(JsonCodecError::FlatBuffer(
            "unknown patch operation discriminator".to_owned(),
        )),
    }
}

fn request_delete(value: Option<api::DeleteRequest<'_>>) -> Result<Value, JsonCodecError> {
    let value = required_result(value, "delete")?;
    let mut out = Map::new();
    if let Some(id) = value.id() {
        out.insert("id".to_owned(), Value::String(id.to_owned()));
    }
    Ok(Value::Object(out))
}

fn request_get(value: Option<api::GetRequest<'_>>) -> Result<Value, JsonCodecError> {
    let value = required_result(value, "get")?;
    let mut out = Map::new();
    if let Some(id) = value.id() {
        out.insert("id".to_owned(), Value::String(id.to_owned()));
    }
    if let Some(projection) = value.projection() {
        out.insert("projection".to_owned(), field_projection_json(projection)?);
    }
    Ok(Value::Object(out))
}

fn request_search(value: Option<api::SearchRequest<'_>>) -> Result<Value, JsonCodecError> {
    search_json(required_result(value, "search")?)
}

fn request_analyze(value: Option<api::AnalyzeRequest<'_>>) -> Result<Value, JsonCodecError> {
    let value = required_result(value, "analyze")?;
    let mut out = Map::new();
    if value.target().is_some() {
        let target = match value.target_type() {
            api::AnalyzeTarget::Analyzer => {
                serde_json::json!({"analyzer": analyzer_json(required_result(value.target_as_analyzer(), "analyze.target.analyzer")?)?})
            }
            api::AnalyzeTarget::AnalyzeFieldTarget => {
                let target = required_result(
                    value.target_as_analyze_field_target(),
                    "analyze.target.field",
                )?;
                serde_json::json!({"field": {"field": required_result(target.field(), "analyze.target.field.field")?}})
            }
            _ => {
                return Err(JsonCodecError::FlatBuffer(
                    "unknown analyze target discriminator".to_owned(),
                ))
            }
        };
        out.insert("target".to_owned(), target);
    }
    if let Some(text) = value.text() {
        out.insert("text".to_owned(), Value::String(text.to_owned()));
    }
    if let Some(language) = value.language() {
        out.insert("language".to_owned(), Value::String(language.to_owned()));
    }
    Ok(Value::Object(out))
}

fn request_inspect(value: Option<api::InspectQueryRequest<'_>>) -> Result<Value, JsonCodecError> {
    let value = required_result(value, "inspect_query")?;
    Ok(serde_json::json!({
        "request": search_json(required_result(value.request(), "inspect_query.request")?)?
    }))
}

pub fn response_flatbuffer_to_json(input: &[u8]) -> Result<Vec<u8>, JsonCodecError> {
    if input.len() < 8 || input[4..8] != *transport::RESPONSE_ENVELOPE_IDENTIFIER.as_bytes() {
        return Err(JsonCodecError::FlatBuffer(
            "missing SKRS file identifier".to_owned(),
        ));
    }
    let envelope = root::<transport::ResponseEnvelope<'_>>(input)
        .map_err(|e| JsonCodecError::FlatBuffer(e.to_string()))?;
    if envelope.protocol_version() != PROTOCOL_VERSION {
        return Err(JsonCodecError::Contract(format!(
            "unsupported protocol_version {}",
            envelope.protocol_version()
        )));
    }
    response_payload_fallback(envelope)
}

fn response_payload_fallback<'a>(
    envelope: transport::ResponseEnvelope<'a>,
) -> Result<Vec<u8>, JsonCodecError> {
    let (name, payload) = match envelope.payload_type().0 {
        x if x == transport::ResponsePayload::SearchKernel_V1_InsertResult.0 => (
            "insert",
            response_insert(envelope.payload_as_search_kernel_v1_insert_result())?,
        ),
        x if x == transport::ResponsePayload::SearchKernel_V1_UpsertResult.0 => (
            "upsert",
            response_upsert(envelope.payload_as_search_kernel_v1_upsert_result())?,
        ),
        x if x == transport::ResponsePayload::SearchKernel_V1_PatchResult.0 => (
            "patch",
            response_patch(envelope.payload_as_search_kernel_v1_patch_result())?,
        ),
        x if x == transport::ResponsePayload::SearchKernel_V1_DeleteResult.0 => (
            "delete",
            response_delete(envelope.payload_as_search_kernel_v1_delete_result())?,
        ),
        x if x == transport::ResponsePayload::SearchKernel_V1_GetResult.0 => (
            "get",
            response_get(envelope.payload_as_search_kernel_v1_get_result())?,
        ),
        x if x == transport::ResponsePayload::SearchKernel_V1_SearchResult.0 => (
            "search",
            response_search(envelope.payload_as_search_kernel_v1_search_result())?,
        ),
        x if x == transport::ResponsePayload::SearchKernel_V1_AnalyzeResult.0 => (
            "analyze",
            response_analyze(envelope.payload_as_search_kernel_v1_analyze_result())?,
        ),
        x if x == transport::ResponsePayload::SearchKernel_V1_Error.0 => (
            "error",
            response_error(envelope.payload_as_search_kernel_v1_error())?,
        ),
        x if x == transport::ResponsePayload::SearchKernel_V1_DescribeResult.0 => (
            "describe",
            response_describe(envelope.payload_as_search_kernel_v1_describe_result())?,
        ),
        x if x == transport::ResponsePayload::SearchKernel_V1_InspectQueryResult.0 => (
            "inspect_query",
            response_inspect(envelope.payload_as_search_kernel_v1_inspect_query_result())?,
        ),
        0 => {
            return Err(JsonCodecError::FlatBuffer(
                "missing response payload".to_owned(),
            ))
        }
        other => {
            return Err(JsonCodecError::FlatBuffer(format!(
                "unknown response discriminator {other}"
            )))
        }
    };
    let response = serde_json::json!({name: payload});
    serde_json::to_vec(
        &serde_json::json!({"protocol_version": PROTOCOL_VERSION, "response": response}),
    )
    .map_err(|e| JsonCodecError::Contract(e.to_string()))
}

fn required_result<T>(value: Option<T>, name: &str) -> Result<T, JsonCodecError> {
    value.ok_or_else(|| JsonCodecError::FlatBuffer(format!("missing response field `{name}`")))
}

fn response_insert(value: Option<api::InsertResult<'_>>) -> Result<Value, JsonCodecError> {
    let value = required_result(value, "insert")?;
    Ok(serde_json::json!({"id": required_result(value.id(), "id")?}))
}
fn response_upsert(value: Option<api::UpsertResult<'_>>) -> Result<Value, JsonCodecError> {
    let value = required_result(value, "upsert")?;
    Ok(serde_json::json!({"id": required_result(value.id(), "id")?}))
}
fn response_patch(value: Option<api::PatchResult<'_>>) -> Result<Value, JsonCodecError> {
    let value = required_result(value, "patch")?;
    Ok(serde_json::json!({"id": required_result(value.id(), "id")?}))
}
fn response_delete(value: Option<api::DeleteResult<'_>>) -> Result<Value, JsonCodecError> {
    let value = required_result(value, "delete")?;
    Ok(serde_json::json!({"id": required_result(value.id(), "id")?, "deleted": value.deleted()}))
}
fn response_get(value: Option<api::GetResult<'_>>) -> Result<Value, JsonCodecError> {
    let value = required_result(value, "get")?;
    let mut out = Map::new();
    if let Some(document) = value.document() {
        out.insert("document".to_owned(), document_json(document)?);
    }
    Ok(Value::Object(out))
}
fn response_analyze(value: Option<api::AnalyzeResult<'_>>) -> Result<Value, JsonCodecError> {
    let value = required_result(value, "analyze")?;
    let mut out = Map::new();
    let mut tokens = Vec::new();
    if let Some(items) = value.tokens() {
        for token in items.iter() {
            tokens.push(serde_json::json!({
                "surface": required_result(token.surface(), "surface")?,
                "term": required_result(token.term(), "term")?,
                "position": token.position(), "start_byte": token.start_byte(), "end_byte": token.end_byte()
            }));
        }
        out.insert("tokens".to_owned(), Value::Array(tokens));
    }
    Ok(Value::Object(out))
}
fn response_describe(value: Option<api::DescribeResult<'_>>) -> Result<Value, JsonCodecError> {
    let value = required_result(value, "describe")?;
    Ok(serde_json::json!({"index": required_result(value.index(), "index").and_then(index_json)?}))
}
fn response_inspect(value: Option<api::InspectQueryResult<'_>>) -> Result<Value, JsonCodecError> {
    let value = required_result(value, "inspect_query")?;
    Ok(
        serde_json::json!({"resolved_request": required_result(value.resolved_request(), "resolved_request").and_then(search_json)?}),
    )
}

fn response_search(value: Option<api::SearchResult<'_>>) -> Result<Value, JsonCodecError> {
    let value = required_result(value, "search")?;
    let mut out = Map::new();
    let mut hits = Vec::new();
    if let Some(items) = value.hits() {
        for hit in items.iter() {
            hits.push(hit_json(hit)?);
        }
        out.insert("hits".to_owned(), Value::Array(hits));
    }
    let total = required_result(value.total_hits(), "total_hits")?;
    out.insert(
        "total_hits".to_owned(),
        serde_json::json!({"value": total.value().to_string(), "relation": enum_json(total.relation().variant_name(), "total_hits.relation")?}),
    );
    let mut facets = Vec::new();
    if let Some(items) = value.facets() {
        for facet in items.iter() {
            facets.push(facet_json(facet)?);
        }
        out.insert("facets".to_owned(), Value::Array(facets));
    }
    Ok(Value::Object(out))
}

fn hit_json(hit: api::Hit<'_>) -> Result<Value, JsonCodecError> {
    let mut out = Map::new();
    out.insert(
        "id".to_owned(),
        Value::String(required_result(hit.id(), "hit.id")?.to_owned()),
    );
    if let Some(score) = hit.score() {
        out.insert("score".to_owned(), number(score.value(), "hit.score")?);
    }
    if let Some(fields) = hit.fields() {
        out.insert("fields".to_owned(), value_map_json(Some(fields))?);
    }
    let mut highlights = Vec::new();
    if let Some(items) = hit.highlights() {
        for item in items.iter() {
            highlights.push(highlight_json(item)?);
        }
        out.insert("highlights".to_owned(), Value::Array(highlights));
    }
    Ok(Value::Object(out))
}

fn facet_json(facet: api::FacetResult<'_>) -> Result<Value, JsonCodecError> {
    let mut out = Map::new();
    out.insert(
        "field".to_owned(),
        Value::String(required_result(facet.field(), "facet.field")?.to_owned()),
    );
    let mut values = Vec::new();
    if let Some(items) = facet.values() {
        for item in items.iter() {
            values.push(serde_json::json!({"value": value_json(required_result(item.value(), "facet.value")?)?, "count": item.count().to_string()}));
        }
        out.insert("values".to_owned(), Value::Array(values));
    }
    Ok(Value::Object(out))
}

fn number(value: f64, path: &str) -> Result<Value, JsonCodecError> {
    serde_json::Number::from_f64(value)
        .map(Value::Number)
        .ok_or_else(|| JsonCodecError::FlatBuffer(format!("non-finite number at {path}")))
}

fn value_map_json<'a>(
    values: Option<flatbuffers::Vector<'a, flatbuffers::ForwardsUOffset<api::ValueEntry<'a>>>>,
) -> Result<Value, JsonCodecError> {
    if values.is_none() {
        return Ok(absent_vector_json());
    }
    let mut out = Map::new();
    if let Some(values) = values {
        for entry in values.iter() {
            out.insert(
                entry.key().to_owned(),
                value_json(required_result(entry.value(), "value")?)?,
            );
        }
    }
    Ok(Value::Object(out))
}

fn string_map_json<'a>(
    values: Option<flatbuffers::Vector<'a, flatbuffers::ForwardsUOffset<api::StringEntry<'a>>>>,
) -> Result<Value, JsonCodecError> {
    if values.is_none() {
        return Ok(absent_vector_json());
    }
    let mut out = Map::new();
    if let Some(values) = values {
        for entry in values.iter() {
            out.insert(
                entry.key().to_owned(),
                Value::String(required_result(entry.value(), "string value")?.to_owned()),
            );
        }
    }
    Ok(Value::Object(out))
}

fn document_json(document: api::Document<'_>) -> Result<Value, JsonCodecError> {
    let mut out = Map::new();
    if let Some(id) = document.id() {
        out.insert("id".to_owned(), Value::String(id.to_owned()));
    }
    if let Some(fields) = document.fields() {
        out.insert("fields".to_owned(), value_map_json(Some(fields))?);
    }
    Ok(Value::Object(out))
}

fn value_json(value: api::Value<'_>) -> Result<Value, JsonCodecError> {
    let (name, payload) = match value.kind_type() {
        api::ValueKind::NullValueValue => {
            let null = required_result(value.kind_as_null_value_value(), "value.null")?;
            if null.value() != api::NullValue::Null {
                return Err(JsonCodecError::FlatBuffer(
                    "unknown or unspecified null value".to_owned(),
                ));
            }
            ("null", Some(Value::Null))
        }
        api::ValueKind::BoolValue => (
            "bool",
            value.kind_as_bool_value().map(|v| Value::Bool(v.value())),
        ),
        api::ValueKind::Int64Value => (
            "int64",
            value
                .kind_as_int_64_value()
                .map(|v| Value::String(v.value().to_string())),
        ),
        api::ValueKind::UInt64Value => (
            "uint64",
            value
                .kind_as_uint_64_value()
                .map(|v| Value::String(v.value().to_string())),
        ),
        api::ValueKind::Float64Value => (
            "float64",
            value
                .kind_as_float_64_value()
                .map(|v| number(v.value(), "value.float64"))
                .transpose()?,
        ),
        api::ValueKind::StringValue => (
            "string",
            value
                .kind_as_string_value()
                .map(|v| {
                    required_result(v.value(), "value.string").map(|s| Value::String(s.to_owned()))
                })
                .transpose()?,
        ),
        api::ValueKind::BytesValue => (
            "bytes",
            Some(match required_result(value.kind_as_bytes_value(), "value.bytes")?.value() {
                Some(bytes) => Value::String(BASE64.encode(bytes.bytes())),
                None => absent_vector_json(),
            }),
        ),
        api::ValueKind::ValueList => {
            let list = required_result(value.kind_as_value_list(), "value.list")?;
            let mut values = Vec::new();
            if let Some(items) = list.values() {
                for item in items.iter() {
                    values.push(value_json(item)?);
                }
            }
            (
                "list",
                Some(if list.values().is_some() {
                    Value::Array(values)
                } else {
                    absent_vector_json()
                }),
            )
        }
        api::ValueKind::ValueObject => {
            let object = required_result(value.kind_as_value_object(), "value.object")?;
            ("object", Some(value_map_json(object.fields())?))
        }
        _ => {
            return Err(JsonCodecError::FlatBuffer(
                "missing or unknown value variant".to_owned(),
            ))
        }
    };
    Ok(serde_json::json!({name: required_result(payload, "value payload")?}))
}

fn field_projection_json(value: api::FieldProjection<'_>) -> Result<Value, JsonCodecError> {
    if value.fields().is_none() {
        return Ok(absent_vector_json());
    }
    let mut out = Vec::new();
    if let Some(items) = value.fields() {
        for item in items.iter() {
            out.push(Value::String(item.to_owned()));
        }
    }
    Ok(Value::Array(out))
}

fn selection_json(value: api::SearchFieldSelection<'_>) -> Result<Value, JsonCodecError> {
    if value.fields().is_none() {
        return Ok(absent_vector_json());
    }
    let mut out = Vec::new();
    if let Some(items) = value.fields() {
        for item in items.iter() {
            let mut f = Map::new();
            f.insert(
                "field".to_owned(),
                Value::String(required_result(item.field(), "search field")?.to_owned()),
            );
            if let Some(weight) = item.weight() {
                f.insert(
                    "weight".to_owned(),
                    number(weight.value(), "search field weight")?,
                );
            }
            out.push(Value::Object(f));
        }
    }
    Ok(Value::Array(out))
}

fn index_json(value: api::IndexDefinition<'_>) -> Result<Value, JsonCodecError> {
    let mut out = Map::new();
    if let Some(name) = value.name() {
        out.insert("name".to_owned(), Value::String(name.to_owned()));
    }
    let mut fields = Vec::new();
    if let Some(items) = value.fields() {
        for item in items.iter() {
            let mut f = Map::new();
            f.insert(
                "name".to_owned(),
                Value::String(required_result(item.name(), "field.name")?.to_owned()),
            );
            f.insert(
                "kind".to_owned(),
                Value::String(enum_json(item.kind().variant_name(), "field.kind")?),
            );
            f.insert(
                "cardinality".to_owned(),
                Value::String(enum_json(
                    item.cardinality().variant_name(),
                    "field.cardinality",
                )?),
            );
            for (name, v) in [
                ("stored", item.stored().map(|x| x.value())),
                ("searchable", item.searchable().map(|x| x.value())),
                ("filterable", item.filterable().map(|x| x.value())),
                ("sortable", item.sortable().map(|x| x.value())),
                ("facetable", item.facetable().map(|x| x.value())),
            ] {
                if let Some(v) = v {
                    f.insert(name.to_owned(), Value::Bool(v));
                }
            }
            if let Some(a) = item.analyzer() {
                f.insert("analyzer".to_owned(), analyzer_json(a)?);
            }
            fields.push(Value::Object(f));
        }
    }
    if value.fields().is_some() {
        out.insert("fields".to_owned(), Value::Array(fields));
    }
    if let Some(selection) = value.default_search_fields() {
        out.insert(
            "default_search_fields".to_owned(),
            selection_json(selection)?,
        );
    }
    Ok(Value::Object(out))
}

fn analyzer_json(value: api::Analyzer<'_>) -> Result<Value, JsonCodecError> {
    let mut out = Map::new();
    if let Some(name) = value.name() {
        out.insert("name".to_owned(), Value::String(name.to_owned()));
    }
    out.insert("version".to_owned(), Value::Number(value.version().into()));
    if let Some(options) = value.options() {
        out.insert("options".to_owned(), value_map_json(Some(options))?);
    }
    Ok(Value::Object(out))
}

fn search_json(value: api::SearchRequest<'_>) -> Result<Value, JsonCodecError> {
    let mut out = Map::new();
    if let Some(v) = value.query() {
        out.insert("query".to_owned(), Value::String(v.to_owned()));
    }
    if let Some(v) = value.search_fields() {
        out.insert("search_fields".to_owned(), selection_json(v)?);
    }
    if let Some(v) = value.filter() {
        out.insert("filter".to_owned(), filter_json(v)?);
    }
    if let Some(items) = value.sort() {
        let mut a = Vec::new();
        for item in items.iter() {
            let mut o = Map::new();
            o.insert(
                "field".to_owned(),
                Value::String(required_result(item.field(), "sort.field")?.to_owned()),
            );
            o.insert(
                "direction".to_owned(),
                Value::String(enum_json(
                    item.direction().variant_name(),
                    "sort.direction",
                )?),
            );
            if let Some(v) = item.missing() {
                o.insert(
                    "missing".to_owned(),
                    Value::String(enum_json(v.value().variant_name(), "sort.missing")?),
                );
            }
            a.push(Value::Object(o));
        }
        out.insert("sort".to_owned(), Value::Array(a));
    }
    if let Some(items) = value.facets() {
        let mut a = Vec::new();
        for item in items.iter() {
            let mut o = Map::new();
            o.insert(
                "field".to_owned(),
                Value::String(required_result(item.field(), "facet.field")?.to_owned()),
            );
            if let Some(v) = item.limit() {
                o.insert("limit".to_owned(), Value::Number(v.value().into()));
            }
            if let Some(v) = item.sort() {
                o.insert(
                    "sort".to_owned(),
                    Value::String(enum_json(v.value().variant_name(), "facet.sort")?),
                );
            }
            a.push(Value::Object(o));
        }
        out.insert("facets".to_owned(), Value::Array(a));
    }
    if let Some(v) = value.ranking() {
        out.insert("ranking".to_owned(), ranking_json(v)?);
    }
    if let Some(v) = value.limit() {
        out.insert("limit".to_owned(), Value::Number(v.value().into()));
    }
    if let Some(v) = value.offset() {
        out.insert("offset".to_owned(), Value::Number(v.value().into()));
    }
    if let Some(v) = value.projection() {
        out.insert("projection".to_owned(), field_projection_json(v)?);
    }
    if let Some(v) = value.highlighting() {
        let fields = if let Some(items) = v.fields() {
            let mut a = Vec::new();
            for x in items.iter() {
                a.push(Value::String(x.to_owned()));
            }
            Value::Array(a)
        } else {
            absent_vector_json()
        };
        out.insert("highlighting".to_owned(), fields);
    }
    if let Some(v) = value.language() {
        out.insert("language".to_owned(), Value::String(v.to_owned()));
    }
    if let Some(v) = value.query_expr() {
        out.insert("query_expr".to_owned(), query_json(v)?);
    }
    Ok(Value::Object(out))
}

fn ranking_json(value: api::RankingPolicy<'_>) -> Result<Value, JsonCodecError> {
    let key = match value.policy_type() {
        api::RankingPolicyKind::DefaultRelevanceV1 => "default_relevance_v1",
        api::RankingPolicyKind::Bm25Ranking => "bm25_ranking",
        _ => {
            return Err(JsonCodecError::FlatBuffer(
                "unknown ranking discriminator".to_owned(),
            ))
        }
    };
    Ok(serde_json::json!({key: {}}))
}

fn filter_json(value: api::FilterExpr<'_>) -> Result<Value, JsonCodecError> {
    let (key, payload) = match value.expr_type() {
        api::FilterExprKind::FilterAnd => {
            let v = required_result(value.expr_as_filter_and(), "filter.and")?;
            let mut a = Vec::new();
            if let Some(items) = v.clauses() {
                for x in items.iter() {
                    a.push(filter_json(x)?);
                }
            }
            ("and", Value::Array(a))
        }
        api::FilterExprKind::FilterOr => {
            let v = required_result(value.expr_as_filter_or(), "filter.or")?;
            let mut a = Vec::new();
            if let Some(items) = v.clauses() {
                for x in items.iter() {
                    a.push(filter_json(x)?);
                }
            }
            ("or", Value::Array(a))
        }
        api::FilterExprKind::FilterNot => (
            "not",
            filter_json(
                required_result(value.expr_as_filter_not(), "filter.not")?
                    .clause()
                    .ok_or_else(|| {
                        JsonCodecError::FlatBuffer("missing filter.not clause".to_owned())
                    })?,
            )?,
        ),
        api::FilterExprKind::FilterComparison => {
            let v = required_result(value.expr_as_filter_comparison(), "filter.comparison")?;
            (
                "comparison",
                serde_json::json!({"field": required_result(v.field(), "comparison.field")?, "op": enum_json(v.op().variant_name(), "comparison.op")?, "value": value_json(required_result(v.value(), "comparison.value")?)?}),
            )
        }
        api::FilterExprKind::FilterRange => {
            let v = required_result(value.expr_as_filter_range(), "filter.range")?;
            let mut o = Map::new();
            o.insert(
                "field".to_owned(),
                Value::String(required_result(v.field(), "range.field")?.to_owned()),
            );
            if let Some(x) = v.lower() {
                o.insert("lower".to_owned(), value_json(x)?);
            }
            if let Some(x) = v.upper() {
                o.insert("upper".to_owned(), value_json(x)?);
            }
            if let Some(x) = v.include_lower() {
                o.insert("include_lower".to_owned(), Value::Bool(x.value()));
            }
            if let Some(x) = v.include_upper() {
                o.insert("include_upper".to_owned(), Value::Bool(x.value()));
            }
            ("range", Value::Object(o))
        }
        api::FilterExprKind::FilterExists => {
            let v = required_result(value.expr_as_filter_exists(), "filter.exists")?;
            (
                "exists",
                serde_json::json!({"field": required_result(v.field(), "exists.field")?}),
            )
        }
        _ => {
            return Err(JsonCodecError::FlatBuffer(
                "unknown filter discriminator".to_owned(),
            ))
        }
    };
    Ok(serde_json::json!({key: payload}))
}

fn query_json(value: api::QueryExpr<'_>) -> Result<Value, JsonCodecError> {
    let (key, payload) = match value.expr_type() {
        api::QueryExprKind::TermQuery => {
            let v = required_result(value.expr_as_term_query(), "query.term_query")?;
            (
                "term_query",
                serde_json::json!({"field": required_result(v.field(), "term.field")?, "value": value_json(required_result(v.value(), "term.value")?)?}),
            )
        }
        api::QueryExprKind::QueryAnd => {
            let v = required_result(value.expr_as_query_and(), "query.and")?;
            let mut a = Vec::new();
            if let Some(items) = v.clauses() {
                for x in items.iter() {
                    a.push(query_json(x)?);
                }
            }
            ("and", Value::Array(a))
        }
        api::QueryExprKind::QueryOr => {
            let v = required_result(value.expr_as_query_or(), "query.or")?;
            let mut a = Vec::new();
            if let Some(items) = v.clauses() {
                for x in items.iter() {
                    a.push(query_json(x)?);
                }
            }
            ("or", Value::Array(a))
        }
        api::QueryExprKind::QueryNot => (
            "not",
            query_json(
                required_result(value.expr_as_query_not(), "query.not")?
                    .clause()
                    .ok_or_else(|| {
                        JsonCodecError::FlatBuffer("missing query.not clause".to_owned())
                    })?,
            )?,
        ),
        _ => {
            return Err(JsonCodecError::FlatBuffer(
                "unknown query discriminator".to_owned(),
            ))
        }
    };
    Ok(serde_json::json!({key: payload}))
}

fn highlight_json(value: api::HighlightResult<'_>) -> Result<Value, JsonCodecError> {
    let Some(items) = value.fragments() else {
        return Ok(serde_json::json!({
            "field": required_result(value.field(), "highlight.field")?,
            "fragments": absent_vector_json(),
        }));
    };
    let mut fragments = Vec::new();
    for fragment in items.iter() {
        let mut o = Map::new();
        o.insert(
            "text".to_owned(),
            Value::String(required_result(fragment.text(), "fragment.text")?.to_owned()),
        );
        if let Some(items) = fragment.matches() {
            let mut matches = Vec::new();
            for span in items.iter() {
                matches.push(serde_json::json!({"start_byte": span.start_byte(), "end_byte": span.end_byte()}));
            }
            o.insert("matches".to_owned(), Value::Array(matches));
        } else {
            o.insert("matches".to_owned(), absent_vector_json());
        }
        fragments.push(Value::Object(o));
    }
    Ok(
        serde_json::json!({"field": required_result(value.field(), "highlight.field")?, "fragments": fragments}),
    )
}
fn response_error(value: Option<api::Error<'_>>) -> Result<Value, JsonCodecError> {
    let value = required_result(value, "error")?;
    let mut out = Map::new();
    out.insert(
        "code".to_owned(),
        Value::String(enum_json(value.code().variant_name(), "error.code")?),
    );
    out.insert(
        "message".to_owned(),
        Value::String(required_result(value.message(), "message")?.to_owned()),
    );
    out.insert("retryable".to_owned(), Value::Bool(value.retryable()));
    let mut details = Vec::new();
    if let Some(items) = value.details() {
        for detail in items.iter() {
            let mut d = Map::new();
            d.insert(
                "code".to_owned(),
                Value::String(required_result(detail.code(), "detail.code")?.to_owned()),
            );
            if let Some(path) = detail.path() {
                d.insert("path".to_owned(), Value::String(path.to_owned()));
            }
            d.insert(
                "message".to_owned(),
                Value::String(required_result(detail.message(), "detail.message")?.to_owned()),
            );
            d.insert("metadata".to_owned(), string_map_json(detail.metadata())?);
            details.push(Value::Object(d));
        }
    }
    if value.details().is_some() {
        out.insert("details".to_owned(), Value::Array(details));
    }
    Ok(Value::Object(out))
}

fn enum_json(value: Option<&'static str>, path: &str) -> Result<String, JsonCodecError> {
    let value =
        value.ok_or_else(|| JsonCodecError::FlatBuffer(format!("unknown enum at {path}")))?;
    Ok(to_snake(value))
}

fn to_snake(value: &str) -> String {
    value
        .chars()
        .enumerate()
        .fold(String::new(), |mut out, (i, c)| {
            if c.is_ascii_uppercase() && i != 0 {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
            out
        })
}

fn offset<T>(value: WIPOffset<T>) -> WIPOffset<UnionWIPOffset> {
    value.as_union_value()
}

fn insert<'a>(
    b: &mut FlatBufferBuilder<'a>,
    o: &Map<String, Value>,
) -> Result<WIPOffset<UnionWIPOffset>, JsonCodecError> {
    check_keys(o, "SearchKernel_V1_InsertRequest", "insert")?;
    let document = document(b, required(o, "document", "insert")?)?;
    Ok(offset(api::InsertRequest::create(
        b,
        &api::InsertRequestArgs {
            document: Some(document),
        },
    )))
}

fn upsert<'a>(
    b: &mut FlatBufferBuilder<'a>,
    o: &Map<String, Value>,
) -> Result<WIPOffset<UnionWIPOffset>, JsonCodecError> {
    check_keys(o, "SearchKernel_V1_UpsertRequest", "upsert")?;
    let document = document(b, required(o, "document", "upsert")?)?;
    Ok(offset(api::UpsertRequest::create(
        b,
        &api::UpsertRequestArgs {
            document: Some(document),
        },
    )))
}

fn delete<'a>(
    b: &mut FlatBufferBuilder<'a>,
    o: &Map<String, Value>,
) -> Result<WIPOffset<UnionWIPOffset>, JsonCodecError> {
    check_keys(o, "SearchKernel_V1_DeleteRequest", "delete")?;
    let id = b.create_string(&string(required(o, "id", "delete")?, "delete.id")?);
    Ok(offset(api::DeleteRequest::create(
        b,
        &api::DeleteRequestArgs { id: Some(id) },
    )))
}

fn get<'a>(
    b: &mut FlatBufferBuilder<'a>,
    o: &Map<String, Value>,
) -> Result<WIPOffset<UnionWIPOffset>, JsonCodecError> {
    check_keys(o, "SearchKernel_V1_GetRequest", "get")?;
    let id_text = string(required(o, "id", "get")?, "get.id")?;
    let id = b.create_string(&id_text);
    let projection = match field(o, "projection") {
        None => None,
        Some(value) => Some(projection(b, value, "get.projection")?),
    };
    Ok(offset(api::GetRequest::create(
        b,
        &api::GetRequestArgs {
            id: Some(id),
            projection,
        },
    )))
}

fn patch<'a>(
    b: &mut FlatBufferBuilder<'a>,
    o: &Map<String, Value>,
) -> Result<WIPOffset<UnionWIPOffset>, JsonCodecError> {
    check_keys(o, "SearchKernel_V1_PatchRequest", "patch")?;
    let id_text = string(required(o, "id", "patch")?, "patch.id")?;
    let id = b.create_string(&id_text);
    let operations = array(required(o, "operations", "patch")?, "patch.operations")?;
    let mut values = Vec::with_capacity(operations.len());
    for (i, operation) in operations.iter().enumerate() {
        values.push(patch_operation(
            b,
            operation,
            &format!("patch.operations[{i}]"),
        )?);
    }
    let operations = b.create_vector(&values);
    Ok(offset(api::PatchRequest::create(
        b,
        &api::PatchRequestArgs {
            id: Some(id),
            operations: Some(operations),
        },
    )))
}

fn patch_operation<'a>(
    b: &mut FlatBufferBuilder<'a>,
    value: &Value,
    path: &str,
) -> Result<WIPOffset<api::PatchOperation<'a>>, JsonCodecError> {
    let (kind, payload) = one_key(value, path)?;
    let (operation_type, operation) = match kind {
        "set_field" => (
            api::PatchOperationKind::SetField,
            offset(set_field(b, object(payload, path)?)?),
        ),
        "unset_field" => (
            api::PatchOperationKind::UnsetField,
            offset(unset_field(b, object(payload, path)?)?),
        ),
        _ => return Err(contract(format!("unknown patch operation `{path}.{kind}`"))),
    };
    Ok(api::PatchOperation::create(
        b,
        &api::PatchOperationArgs {
            operation_type,
            operation: Some(operation),
        },
    ))
}

fn set_field<'a>(
    b: &mut FlatBufferBuilder<'a>,
    o: &Map<String, Value>,
) -> Result<WIPOffset<api::SetField<'a>>, JsonCodecError> {
    check_keys(o, "SearchKernel_V1_SetField", "set_field")?;
    let field_text = string(required(o, "field", "set_field")?, "set_field.field")?;
    let field = b.create_string(&field_text);
    let value = value(b, required(o, "value", "set_field")?)?;
    Ok(api::SetField::create(
        b,
        &api::SetFieldArgs {
            field: Some(field),
            value: Some(value),
        },
    ))
}

fn unset_field<'a>(
    b: &mut FlatBufferBuilder<'a>,
    o: &Map<String, Value>,
) -> Result<WIPOffset<api::UnsetField<'a>>, JsonCodecError> {
    check_keys(o, "SearchKernel_V1_UnsetField", "unset_field")?;
    let field = b.create_string(&string(
        required(o, "field", "unset_field")?,
        "unset_field.field",
    )?);
    Ok(api::UnsetField::create(
        b,
        &api::UnsetFieldArgs { field: Some(field) },
    ))
}

fn document<'a>(
    b: &mut FlatBufferBuilder<'a>,
    value: &Value,
) -> Result<WIPOffset<api::Document<'a>>, JsonCodecError> {
    let o = object(value, "document")?;
    check_keys(o, "SearchKernel_V1_Document", "document")?;
    let id = optional_string(o, "id", "document")?.map(|v| b.create_string(&v));
    let entries = map_entries(b, o.get("fields"), "document.fields")?;
    let fields = match o.get("fields") {
        None => None,
        Some(value) if is_absent_vector(value, "document.fields")? => None,
        Some(_) => Some(b.create_vector(&entries)),
    };
    Ok(api::Document::create(b, &api::DocumentArgs { id, fields }))
}

fn map_entries<'a>(
    b: &mut FlatBufferBuilder<'a>,
    input: Option<&Value>,
    path: &str,
) -> Result<Vec<WIPOffset<api::ValueEntry<'a>>>, JsonCodecError> {
    let Some(input) = input else {
        return Ok(Vec::new());
    };
    if is_absent_vector(input, path)? {
        return Ok(Vec::new());
    }
    let object = object(input, path)?;
    let mut entries = Vec::with_capacity(object.len());
    for (key, entry_value) in object {
        let key_offset = b.create_string(key);
        let value_offset = value(b, entry_value)?;
        entries.push(api::ValueEntry::create(
            b,
            &api::ValueEntryArgs {
                key: Some(key_offset),
                value: Some(value_offset),
            },
        ));
    }
    Ok(entries)
}

fn value<'a>(
    b: &mut FlatBufferBuilder<'a>,
    input: &Value,
) -> Result<WIPOffset<api::Value<'a>>, JsonCodecError> {
    let (kind, payload) = one_key(input, "value")?;
    let (kind_type, kind_value) = match kind {
        "null" => {
            if !payload.is_null() {
                return Err(contract("value.null must be null"));
            }
            let payload = api::NullValueValue::create(
                b,
                &api::NullValueValueArgs {
                    value: api::NullValue::Null,
                },
            );
            (api::ValueKind::NullValueValue, offset(payload))
        }
        "bool" => {
            let payload = api::BoolValue::create(
                b,
                &api::BoolValueArgs {
                    value: bool_value(payload, "value.bool")?,
                },
            );
            (api::ValueKind::BoolValue, offset(payload))
        }
        "int64" => {
            let payload = api::Int64Value::create(
                b,
                &api::Int64ValueArgs {
                    value: decimal(payload, "value.int64", "signed 64-bit integer")?,
                },
            );
            (api::ValueKind::Int64Value, offset(payload))
        }
        "uint64" => {
            let payload = api::UInt64Value::create(
                b,
                &api::UInt64ValueArgs {
                    value: decimal(payload, "value.uint64", "unsigned 64-bit integer")?,
                },
            );
            (api::ValueKind::UInt64Value, offset(payload))
        }
        "float64" => {
            let payload = api::Float64Value::create(
                b,
                &api::Float64ValueArgs {
                    value: f64_value(payload, "value.float64")?,
                },
            );
            (api::ValueKind::Float64Value, offset(payload))
        }
        "string" => {
            let text = string(payload, "value.string")?;
            let text = b.create_string(&text);
            let payload = api::StringValue::create(b, &api::StringValueArgs { value: Some(text) });
            (api::ValueKind::StringValue, offset(payload))
        }
        "bytes" => {
            if is_absent_vector(payload, "value.bytes")? {
                let payload = api::BytesValue::create(b, &api::BytesValueArgs { value: None });
                return Ok(api::Value::create(
                    b,
                    &api::ValueArgs {
                        kind_type: api::ValueKind::BytesValue,
                        kind: Some(offset(payload)),
                    },
                ));
            }
            let text = string(payload, "value.bytes")?;
            let bytes = BASE64
                .decode(text)
                .map_err(|e| contract(format!("value.bytes is not valid base64: {e}")))?;
            let bytes = b.create_vector(&bytes);
            let payload = api::BytesValue::create(b, &api::BytesValueArgs { value: Some(bytes) });
            (api::ValueKind::BytesValue, offset(payload))
        }
        "list" => {
            let values = if is_absent_vector(payload, "value.list")? {
                None
            } else {
                Some(array(payload, "value.list")?)
            };
            let values = values
                .as_ref()
                .map(|values| {
                    values
                        .iter()
                        .map(|v| value(b, v))
                        .collect::<Result<Vec<_>, _>>()
                })
                .transpose()?;
            let values = values.map(|values| b.create_vector(&values));
            let payload = api::ValueList::create(
                b,
                &api::ValueListArgs {
                    values,
                },
            );
            (api::ValueKind::ValueList, offset(payload))
        }
        "object" => {
            let entries = if is_absent_vector(payload, "value.object")? {
                None
            } else {
                let entries = map_entries(b, Some(payload), "value.object")?;
                Some(b.create_vector(&entries))
            };
            let payload = api::ValueObject::create(
                b,
                &api::ValueObjectArgs {
                    fields: entries,
                },
            );
            (api::ValueKind::ValueObject, offset(payload))
        }
        _ => return Err(contract(format!("unknown value variant `{kind}`"))),
    };
    Ok(api::Value::create(
        b,
        &api::ValueArgs {
            kind_type,
            kind: Some(kind_value),
        },
    ))
}

fn projection<'a>(
    b: &mut FlatBufferBuilder<'a>,
    value: &Value,
    path: &str,
) -> Result<WIPOffset<api::FieldProjection<'a>>, JsonCodecError> {
    let fields = if is_absent_vector(value, path)? {
        None
    } else {
        let fields = array(value, path)?
            .iter()
            .map(|v| string(v, path))
            .collect::<Result<Vec<_>, _>>()?;
        let fields = fields
            .iter()
            .map(|v| b.create_string(v))
            .collect::<Vec<_>>();
        Some(b.create_vector(&fields))
    };
    Ok(api::FieldProjection::create(
        b,
        &api::FieldProjectionArgs {
            fields,
        },
    ))
}

fn selection<'a>(
    b: &mut FlatBufferBuilder<'a>,
    value: &Value,
    path: &str,
) -> Result<WIPOffset<api::SearchFieldSelection<'a>>, JsonCodecError> {
    let fields = if is_absent_vector(value, path)? {
        None
    } else {
        let items = array(value, path)?;
        let fields = items
            .iter()
            .enumerate()
            .map(|(i, v)| search_field(b, v, &format!("{path}[{i}]")))
            .collect::<Result<Vec<_>, _>>()?;
        Some(b.create_vector(&fields))
    };
    Ok(api::SearchFieldSelection::create(
        b,
        &api::SearchFieldSelectionArgs {
            fields,
        },
    ))
}

fn search_field<'a>(
    b: &mut FlatBufferBuilder<'a>,
    value: &Value,
    path: &str,
) -> Result<WIPOffset<api::SearchField<'a>>, JsonCodecError> {
    let o = object(value, path)?;
    check_keys(o, "SearchKernel_V1_SearchField", path)?;
    let field = b.create_string(&string(
        required(o, "field", path)?,
        &format!("{path}.field"),
    )?);
    let weight = optional_f64(o, "weight", path)?;
    Ok(api::SearchField::create(
        b,
        &api::SearchFieldArgs {
            field: Some(field),
            weight: weight.as_ref(),
        },
    ))
}

fn search<'a>(
    b: &mut FlatBufferBuilder<'a>,
    o: &Map<String, Value>,
) -> Result<WIPOffset<api::SearchRequest<'a>>, JsonCodecError> {
    check_keys(o, "SearchKernel_V1_SearchRequest", "search")?;
    let query = optional_string(o, "query", "search")?.map(|v| b.create_string(&v));
    let search_fields = field(o, "search_fields")
        .map(|v| selection(b, v, "search.search_fields"))
        .transpose()?;
    let filter = field(o, "filter")
        .map(|v| filter(b, v, "search.filter"))
        .transpose()?;
    let sort = match field(o, "sort") {
        Some(v) => {
            let items = array(v, "search.sort")?;
            let items = items
                .iter()
                .enumerate()
                .map(|(i, v)| sort(b, v, &format!("search.sort[{i}]")))
                .collect::<Result<Vec<_>, _>>()?;
            Some(b.create_vector(&items))
        }
        None => None,
    };
    let facets = match field(o, "facets") {
        Some(v) => {
            let items = array(v, "search.facets")?;
            let items = items
                .iter()
                .enumerate()
                .map(|(i, v)| facet(b, v, &format!("search.facets[{i}]")))
                .collect::<Result<Vec<_>, _>>()?;
            Some(b.create_vector(&items))
        }
        None => None,
    };
    let ranking = field(o, "ranking")
        .map(|v| ranking(b, v, "search.ranking"))
        .transpose()?;
    let limit = optional_u32(o, "limit", "search")?;
    let offset_value = optional_u32(o, "offset", "search")?;
    let projection = field(o, "projection")
        .map(|v| projection(b, v, "search.projection"))
        .transpose()?;
    let highlighting = field(o, "highlighting")
        .map(|v| highlighting(b, v, "search.highlighting"))
        .transpose()?;
    let language = optional_string(o, "language", "search")?.map(|v| b.create_string(&v));
    let query_expr = field(o, "query_expr")
        .map(|v| query_expr(b, v, "search.query_expr"))
        .transpose()?;
    Ok(api::SearchRequest::create(
        b,
        &api::SearchRequestArgs {
            query,
            search_fields,
            filter,
            sort,
            facets,
            ranking,
            limit: limit.as_ref(),
            offset: offset_value.as_ref(),
            projection,
            highlighting,
            language,
            query_expr,
        },
    ))
}

fn highlighting<'a>(
    b: &mut FlatBufferBuilder<'a>,
    value: &Value,
    path: &str,
) -> Result<WIPOffset<api::HighlightRequest<'a>>, JsonCodecError> {
    let fields = if is_absent_vector(value, path)? {
        None
    } else {
        let fields = array(value, path)?
            .iter()
            .map(|v| string(v, path))
            .collect::<Result<Vec<_>, _>>()?;
        let fields = fields
            .iter()
            .map(|v| b.create_string(v))
            .collect::<Vec<_>>();
        Some(b.create_vector(&fields))
    };
    Ok(api::HighlightRequest::create(
        b,
        &api::HighlightRequestArgs {
            fields,
        },
    ))
}

fn filter<'a>(
    b: &mut FlatBufferBuilder<'a>,
    value: &Value,
    path: &str,
) -> Result<WIPOffset<api::FilterExpr<'a>>, JsonCodecError> {
    let (kind, payload) = one_key(value, path)?;
    let (expr_type, expr) = match kind {
        "and" => {
            let items = array(payload, &format!("{path}.and"))?;
            let items = items
                .iter()
                .enumerate()
                .map(|(i, v)| filter(b, v, &format!("{path}.and[{i}]")))
                .collect::<Result<Vec<_>, _>>()?;
            let items = b.create_vector(&items);
            let value = api::FilterAnd::create(
                b,
                &api::FilterAndArgs {
                    clauses: Some(items),
                },
            );
            (api::FilterExprKind::FilterAnd, offset(value))
        }
        "or" => {
            let items = array(payload, &format!("{path}.or"))?;
            let items = items
                .iter()
                .enumerate()
                .map(|(i, v)| filter(b, v, &format!("{path}.or[{i}]")))
                .collect::<Result<Vec<_>, _>>()?;
            let items = b.create_vector(&items);
            let value = api::FilterOr::create(
                b,
                &api::FilterOrArgs {
                    clauses: Some(items),
                },
            );
            (api::FilterExprKind::FilterOr, offset(value))
        }
        "not" => {
            let clause = filter(b, payload, &format!("{path}.not"))?;
            let value = api::FilterNot::create(
                b,
                &api::FilterNotArgs {
                    clause: Some(clause),
                },
            );
            (api::FilterExprKind::FilterNot, offset(value))
        }
        "comparison" => (
            api::FilterExprKind::FilterComparison,
            offset(comparison(
                b,
                object(payload, path)?,
                &format!("{path}.comparison"),
            )?),
        ),
        "range" => (
            api::FilterExprKind::FilterRange,
            offset(range(b, object(payload, path)?, &format!("{path}.range"))?),
        ),
        "exists" => (
            api::FilterExprKind::FilterExists,
            offset(exists(
                b,
                object(payload, path)?,
                &format!("{path}.exists"),
            )?),
        ),
        _ => return Err(contract(format!("unknown filter variant `{path}.{kind}`"))),
    };
    Ok(api::FilterExpr::create(
        b,
        &api::FilterExprArgs {
            expr_type,
            expr: Some(expr),
        },
    ))
}

fn comparison<'a>(
    b: &mut FlatBufferBuilder<'a>,
    o: &Map<String, Value>,
    path: &str,
) -> Result<WIPOffset<api::FilterComparison<'a>>, JsonCodecError> {
    check_keys(o, "SearchKernel_V1_FilterComparison", path)?;
    let field = b.create_string(&string(
        required(o, "field", path)?,
        &format!("{path}.field"),
    )?);
    let op = parse_comparison(required(o, "op", path)?, &format!("{path}.op"))?;
    let value = value(b, required(o, "value", path)?)?;
    Ok(api::FilterComparison::create(
        b,
        &api::FilterComparisonArgs {
            field: Some(field),
            op,
            value: Some(value),
        },
    ))
}

fn range<'a>(
    b: &mut FlatBufferBuilder<'a>,
    o: &Map<String, Value>,
    path: &str,
) -> Result<WIPOffset<api::FilterRange<'a>>, JsonCodecError> {
    check_keys(o, "SearchKernel_V1_FilterRange", path)?;
    let field_offset = b.create_string(&string(
        required(o, "field", path)?,
        &format!("{path}.field"),
    )?);
    let lower = field(o, "lower").map(|v| value(b, v)).transpose()?;
    let upper = field(o, "upper").map(|v| value(b, v)).transpose()?;
    let include_lower = optional_bool(o, "include_lower", path)?;
    let include_upper = optional_bool(o, "include_upper", path)?;
    Ok(api::FilterRange::create(
        b,
        &api::FilterRangeArgs {
            field: Some(field_offset),
            lower,
            upper,
            include_lower: include_lower.as_ref(),
            include_upper: include_upper.as_ref(),
        },
    ))
}

fn exists<'a>(
    b: &mut FlatBufferBuilder<'a>,
    o: &Map<String, Value>,
    path: &str,
) -> Result<WIPOffset<api::FilterExists<'a>>, JsonCodecError> {
    check_keys(o, "SearchKernel_V1_FilterExists", path)?;
    let field = b.create_string(&string(
        required(o, "field", path)?,
        &format!("{path}.field"),
    )?);
    Ok(api::FilterExists::create(
        b,
        &api::FilterExistsArgs { field: Some(field) },
    ))
}

fn sort<'a>(
    b: &mut FlatBufferBuilder<'a>,
    value: &Value,
    path: &str,
) -> Result<WIPOffset<api::SortExpr<'a>>, JsonCodecError> {
    let o = object(value, path)?;
    check_keys(o, "SearchKernel_V1_SortExpr", path)?;
    let field_offset = b.create_string(&string(
        required(o, "field", path)?,
        &format!("{path}.field"),
    )?);
    let direction = field(o, "direction")
        .map(|v| parse_direction(v, &format!("{path}.direction")))
        .transpose()?
        .unwrap_or(api::SortDirection::Unspecified);
    let missing = field(o, "missing")
        .map(|v| {
            parse_missing(v, &format!("{path}.missing")).map(api::OptionalMissingPlacement::new)
        })
        .transpose()?;
    Ok(api::SortExpr::create(
        b,
        &api::SortExprArgs {
            field: Some(field_offset),
            direction,
            missing: missing.as_ref(),
        },
    ))
}

fn facet<'a>(
    b: &mut FlatBufferBuilder<'a>,
    value: &Value,
    path: &str,
) -> Result<WIPOffset<api::FacetRequest<'a>>, JsonCodecError> {
    let o = object(value, path)?;
    check_keys(o, "SearchKernel_V1_FacetRequest", path)?;
    let field_offset = b.create_string(&string(
        required(o, "field", path)?,
        &format!("{path}.field"),
    )?);
    let limit = optional_u32(o, "limit", path)?;
    let sort = field(o, "sort")
        .map(|v| parse_facet_sort(v, &format!("{path}.sort")).map(api::OptionalFacetSort::new))
        .transpose()?;
    Ok(api::FacetRequest::create(
        b,
        &api::FacetRequestArgs {
            field: Some(field_offset),
            limit: limit.as_ref(),
            sort: sort.as_ref(),
        },
    ))
}

fn ranking<'a>(
    b: &mut FlatBufferBuilder<'a>,
    value: &Value,
    path: &str,
) -> Result<WIPOffset<api::RankingPolicy<'a>>, JsonCodecError> {
    let (kind, payload) = one_key(value, path)?;
    let policy = match kind {
        "default_relevance_v1" => offset(api::DefaultRelevanceV1::create(
            b,
            &api::DefaultRelevanceV1Args {},
        )),
        "bm25_ranking" => offset(api::Bm25Ranking::create(b, &api::Bm25RankingArgs {})),
        _ => return Err(contract(format!("unknown ranking variant `{path}.{kind}`"))),
    };
    if !payload.is_object() {
        return Err(contract(format!("`{path}.{kind}` must be an object")));
    }
    Ok(api::RankingPolicy::create(
        b,
        &api::RankingPolicyArgs {
            policy_type: match kind {
                "default_relevance_v1" => api::RankingPolicyKind::DefaultRelevanceV1,
                _ => api::RankingPolicyKind::Bm25Ranking,
            },
            policy: Some(policy),
        },
    ))
}

fn query_expr<'a>(
    b: &mut FlatBufferBuilder<'a>,
    value: &Value,
    path: &str,
) -> Result<WIPOffset<api::QueryExpr<'a>>, JsonCodecError> {
    let (kind, payload) = one_key(value, path)?;
    let (expr_type, expr) = match kind {
        "term_query" => (
            api::QueryExprKind::TermQuery,
            offset(term_query(
                b,
                object(payload, path)?,
                &format!("{path}.term_query"),
            )?),
        ),
        "and" => {
            let values = array(payload, &format!("{path}.and"))?;
            let values = values
                .iter()
                .enumerate()
                .map(|(i, v)| query_expr(b, v, &format!("{path}.and[{i}]")))
                .collect::<Result<Vec<_>, _>>()?;
            let values = b.create_vector(&values);
            let node = api::QueryAnd::create(
                b,
                &api::QueryAndArgs {
                    clauses: Some(values),
                },
            );
            (api::QueryExprKind::QueryAnd, offset(node))
        }
        "or" => {
            let values = array(payload, &format!("{path}.or"))?;
            let values = values
                .iter()
                .enumerate()
                .map(|(i, v)| query_expr(b, v, &format!("{path}.or[{i}]")))
                .collect::<Result<Vec<_>, _>>()?;
            let values = b.create_vector(&values);
            let node = api::QueryOr::create(
                b,
                &api::QueryOrArgs {
                    clauses: Some(values),
                },
            );
            (api::QueryExprKind::QueryOr, offset(node))
        }
        "not" => {
            let clause = query_expr(b, payload, &format!("{path}.not"))?;
            let node = api::QueryNot::create(
                b,
                &api::QueryNotArgs {
                    clause: Some(clause),
                },
            );
            (api::QueryExprKind::QueryNot, offset(node))
        }
        _ => return Err(contract(format!("unknown query variant `{path}.{kind}`"))),
    };
    Ok(api::QueryExpr::create(
        b,
        &api::QueryExprArgs {
            expr_type,
            expr: Some(expr),
        },
    ))
}

fn term_query<'a>(
    b: &mut FlatBufferBuilder<'a>,
    o: &Map<String, Value>,
    path: &str,
) -> Result<WIPOffset<api::TermQuery<'a>>, JsonCodecError> {
    check_keys(o, "SearchKernel_V1_TermQuery", path)?;
    let field = b.create_string(&string(
        required(o, "field", path)?,
        &format!("{path}.field"),
    )?);
    let value = value(b, required(o, "value", path)?)?;
    Ok(api::TermQuery::create(
        b,
        &api::TermQueryArgs {
            field: Some(field),
            value: Some(value),
        },
    ))
}

fn analyze<'a>(
    b: &mut FlatBufferBuilder<'a>,
    o: &Map<String, Value>,
) -> Result<WIPOffset<UnionWIPOffset>, JsonCodecError> {
    check_keys(o, "SearchKernel_V1_AnalyzeRequest", "analyze")?;
    let (target_type, target) = match field(o, "target") {
        None => (api::AnalyzeTarget::NONE, None),
        Some(value) => {
            let (kind, payload) = one_key(value, "analyze.target")?;
            match kind {
                "analyzer" => {
                    let analyzer = analyzer(b, object(payload, "analyze.target.analyzer")?)?;
                    (api::AnalyzeTarget::Analyzer, Some(offset(analyzer)))
                }
                "field" => {
                    let target = analyze_field_target(b, object(payload, "analyze.target.field")?)?;
                    (api::AnalyzeTarget::AnalyzeFieldTarget, Some(offset(target)))
                }
                _ => return Err(contract(format!("unknown analyze target `{kind}`"))),
            }
        }
    };
    let text = optional_string(o, "text", "analyze")?.map(|v| b.create_string(&v));
    let language = optional_string(o, "language", "analyze")?.map(|v| b.create_string(&v));
    Ok(offset(api::AnalyzeRequest::create(
        b,
        &api::AnalyzeRequestArgs {
            target_type,
            target,
            text,
            language,
        },
    )))
}

fn analyzer<'a>(
    b: &mut FlatBufferBuilder<'a>,
    o: &Map<String, Value>,
) -> Result<WIPOffset<api::Analyzer<'a>>, JsonCodecError> {
    check_keys(o, "SearchKernel_V1_Analyzer", "analyzer")?;
    let name = optional_string(o, "name", "analyzer")?.map(|v| b.create_string(&v));
    let version = field(o, "version")
        .map(|v| u32_value(v, "analyzer.version"))
        .transpose()?
        .unwrap_or(0);
    let entries = map_entries(b, o.get("options"), "analyzer.options")?;
    let options = match o.get("options") {
        None => None,
        Some(value) if is_absent_vector(value, "analyzer.options")? => None,
        Some(_) => Some(b.create_vector(&entries)),
    };
    Ok(api::Analyzer::create(
        b,
        &api::AnalyzerArgs {
            name,
            version,
            options,
        },
    ))
}

fn analyze_field_target<'a>(
    b: &mut FlatBufferBuilder<'a>,
    o: &Map<String, Value>,
) -> Result<WIPOffset<api::AnalyzeFieldTarget<'a>>, JsonCodecError> {
    check_keys(
        o,
        "SearchKernel_V1_AnalyzeFieldTarget",
        "analyze.target.field",
    )?;
    let field = b.create_string(&string(
        required(o, "field", "analyze.target.field")?,
        "analyze.target.field.field",
    )?);
    Ok(api::AnalyzeFieldTarget::create(
        b,
        &api::AnalyzeFieldTargetArgs { field: Some(field) },
    ))
}

fn enum_name(value: &Value, path: &str) -> Result<String, JsonCodecError> {
    string(value, path)
}

fn parse_comparison(value: &Value, path: &str) -> Result<api::ComparisonOp, JsonCodecError> {
    match enum_name(value, path)?.as_str() {
        "unspecified" => Ok(api::ComparisonOp::Unspecified),
        "eq" => Ok(api::ComparisonOp::Eq),
        "ne" => Ok(api::ComparisonOp::Ne),
        "lt" => Ok(api::ComparisonOp::Lt),
        "lte" => Ok(api::ComparisonOp::Lte),
        "gt" => Ok(api::ComparisonOp::Gt),
        "gte" => Ok(api::ComparisonOp::Gte),
        other => Err(contract(format!("unknown enum `{path}` value `{other}`"))),
    }
}

fn parse_direction(value: &Value, path: &str) -> Result<api::SortDirection, JsonCodecError> {
    match enum_name(value, path)?.as_str() {
        "unspecified" => Ok(api::SortDirection::Unspecified),
        "asc" => Ok(api::SortDirection::Asc),
        "desc" => Ok(api::SortDirection::Desc),
        other => Err(contract(format!("unknown enum `{path}` value `{other}`"))),
    }
}

fn parse_missing(value: &Value, path: &str) -> Result<api::MissingPlacement, JsonCodecError> {
    match enum_name(value, path)?.as_str() {
        "unspecified" => Ok(api::MissingPlacement::Unspecified),
        "first" => Ok(api::MissingPlacement::First),
        "last" => Ok(api::MissingPlacement::Last),
        other => Err(contract(format!("unknown enum `{path}` value `{other}`"))),
    }
}

fn parse_facet_sort(value: &Value, path: &str) -> Result<api::FacetSort, JsonCodecError> {
    match enum_name(value, path)?.as_str() {
        "unspecified" => Ok(api::FacetSort::Unspecified),
        "count_desc" => Ok(api::FacetSort::CountDesc),
        "count_asc" => Ok(api::FacetSort::CountAsc),
        "value_asc" => Ok(api::FacetSort::ValueAsc),
        "value_desc" => Ok(api::FacetSort::ValueDesc),
        other => Err(contract(format!("unknown enum `{path}` value `{other}`"))),
    }
}

fn describe<'a>(
    b: &mut FlatBufferBuilder<'a>,
    o: &Map<String, Value>,
) -> Result<WIPOffset<UnionWIPOffset>, JsonCodecError> {
    check_keys(o, "SearchKernel_V1_DescribeRequest", "describe")?;
    Ok(offset(api::DescribeRequest::create(
        b,
        &api::DescribeRequestArgs {},
    )))
}

fn inspect_query<'a>(
    b: &mut FlatBufferBuilder<'a>,
    o: &Map<String, Value>,
) -> Result<WIPOffset<UnionWIPOffset>, JsonCodecError> {
    check_keys(o, "SearchKernel_V1_InspectQueryRequest", "inspect_query")?;
    let request = search(
        b,
        object(
            required(o, "request", "inspect_query")?,
            "inspect_query.request",
        )?,
    )?;
    Ok(offset(api::InspectQueryRequest::create(
        b,
        &api::InspectQueryRequestArgs {
            request: Some(request),
        },
    )))
}
