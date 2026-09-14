//! Corpus-runner API.

use searchkernel_core::{
    analyzer_to_ir, api, error_to_api, field_to_ir, index_definition_to_ir, AnalyzeRequestIr,
    AnalyzerIdentity, ComparisonIr, ComparisonOp, DeleteRequestIr, DocumentIr, FacetIr, FacetSort,
    FieldCardinality, FieldIr, FieldKind, FieldValueIr, FilterIr, GetRequestIr, HighlightingIr,
    IndexContext, IndexDefinitionIr, IndexName, InsertRequestIr, LanguageHint, MissingPlacement,
    PatchOperationIr, PatchRequestIr, PortableError, ProjectionIr, QueryIr, RangeIr,
    RankingPolicyIr, ScalarValueIr, SearchFieldIr, SearchRequestIr, SortDirection, SortIr,
    UpsertRequestIr, WriteRequestIr, WriteRequestRef,
};
use serde_json::{json, Map, Value as JsonValue};
use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::result::Result as StdResult;

#[allow(warnings)]
mod transport_bindings {
    pub mod search_kernel {
        pub mod ffi {
            pub mod v1 {
                pub use searchkernel_core::api::*;

                mod request_payload_generated {
                    include!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../searchkernel-ffi/generated/search_kernel/ffi/v1/request_payload_generated.rs"
                    ));
                }
                pub use request_payload_generated::*;

                mod request_envelope_generated {
                    include!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../searchkernel-ffi/generated/search_kernel/ffi/v1/request_envelope_generated.rs"
                    ));
                }
                pub use request_envelope_generated::*;

                mod response_payload_generated {
                    include!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../searchkernel-ffi/generated/search_kernel/ffi/v1/response_payload_generated.rs"
                    ));
                }
                pub use response_payload_generated::*;

                mod response_envelope_generated {
                    include!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../searchkernel-ffi/generated/search_kernel/ffi/v1/response_envelope_generated.rs"
                    ));
                }
                pub use response_envelope_generated::*;
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// The number of conformance cases that passed, grouped by category.
pub struct ConformanceReport {
    pub passed: usize,
    pub categories: Vec<(String, usize)>,
}

/// A component corpus case embedded in a valid transport envelope for JSON
/// round-trip testing. Direct component tables are not themselves transport
/// roots, so this preserves their wire shape inside the closest protocol root.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransportCase {
    Request(Vec<u8>),
    Response(Vec<u8>),
}

/// Build a component conformance case inside a canonical SKRQ or SKRS root.
/// This is test infrastructure; production JSON conversion remains BFBS-only.
pub fn build_component_transport_case(
    operation: &str,
    input: &JsonValue,
) -> StdResult<TransportCase, String> {
    let mut builder = WireBuilder::new();
    let request = |builder: &mut WireBuilder,
                   payload_type: transport_bindings::search_kernel::ffi::v1::RequestPayload,
                   payload: flatbuffers::WIPOffset<flatbuffers::UnionWIPOffset>| {
        let envelope = transport_bindings::search_kernel::ffi::v1::RequestEnvelope::create(
            &mut builder.builder,
            &transport_bindings::search_kernel::ffi::v1::RequestEnvelopeArgs {
                protocol_version: 1,
                payload_type,
                payload: Some(payload),
            },
        );
        builder
            .finish_with_identifier(envelope, Some("SKRQ"))
            .map(TransportCase::Request)
    };
    let response =
        |builder: &mut WireBuilder,
         payload_type: transport_bindings::search_kernel::ffi::v1::ResponsePayload,
         payload: flatbuffers::WIPOffset<flatbuffers::UnionWIPOffset>| {
            let envelope = transport_bindings::search_kernel::ffi::v1::ResponseEnvelope::create(
                &mut builder.builder,
                &transport_bindings::search_kernel::ffi::v1::ResponseEnvelopeArgs {
                    protocol_version: 1,
                    payload_type,
                    payload: Some(payload),
                },
            );
            builder
                .finish_with_identifier(envelope, Some("SKRS"))
                .map(TransportCase::Response)
        };

    match operation {
        "index_definition_to_ir" => {
            let index = builder.index(input)?;
            let payload = transport_bindings::search_kernel::ffi::v1::DescribeResult::create(
                &mut builder.builder,
                &transport_bindings::search_kernel::ffi::v1::DescribeResultArgs {
                    index: Some(index),
                },
            );
            response(
                &mut builder,
                transport_bindings::search_kernel::ffi::v1::ResponsePayload::SearchKernel_V1_DescribeResult,
                payload.as_union_value(),
            )
        }
        "field_to_ir" => {
            let field = input.get("field").unwrap_or(input);
            let field = builder.field(field)?;
            let fields = builder.builder.create_vector(&[field]);
            let index = api::IndexDefinition::create(
                &mut builder.builder,
                &api::IndexDefinitionArgs {
                    fields: Some(fields),
                    ..Default::default()
                },
            );
            let payload = transport_bindings::search_kernel::ffi::v1::DescribeResult::create(
                &mut builder.builder,
                &transport_bindings::search_kernel::ffi::v1::DescribeResultArgs {
                    index: Some(index),
                },
            );
            response(
                &mut builder,
                transport_bindings::search_kernel::ffi::v1::ResponsePayload::SearchKernel_V1_DescribeResult,
                payload.as_union_value(),
            )
        }
        "analyzer_to_ir" => {
            let payload = builder.analyze(input)?;
            request(
                &mut builder,
                transport_bindings::search_kernel::ffi::v1::RequestPayload::SearchKernel_V1_AnalyzeRequest,
                payload.as_union_value(),
            )
        }
        "document_to_ir" => {
            let document = builder.document(input.get("document").unwrap_or(input))?;
            let payload = api::InsertRequest::create(
                &mut builder.builder,
                &api::InsertRequestArgs {
                    document: Some(document),
                },
            );
            request(
                &mut builder,
                transport_bindings::search_kernel::ffi::v1::RequestPayload::SearchKernel_V1_InsertRequest,
                payload.as_union_value(),
            )
        }
        "value_to_ir" => {
            let field = input
                .get("field")
                .and_then(JsonValue::as_str)
                .unwrap_or("value");
            let value = input.get("value").cloned().unwrap_or_else(|| json!({}));
            let mut fields = Map::new();
            fields.insert(field.to_owned(), value);
            let document = builder.document(&json!({
                "id": "component-value",
                "fields": fields,
            }))?;
            let payload = api::InsertRequest::create(
                &mut builder.builder,
                &api::InsertRequestArgs {
                    document: Some(document),
                },
            );
            request(
                &mut builder,
                transport_bindings::search_kernel::ffi::v1::RequestPayload::SearchKernel_V1_InsertRequest,
                payload.as_union_value(),
            )
        }
        "search_fields_to_ir"
        | "projection_to_ir"
        | "highlighting_to_ir"
        | "comparison_to_ir"
        | "range_to_ir"
        | "filter_to_ir"
        | "sort_to_ir"
        | "facet_to_ir"
        | "ranking_to_ir"
        | "query_to_ir" => {
            let payload = builder.search(&component_search_input(operation, input))?;
            request(
                &mut builder,
                transport_bindings::search_kernel::ffi::v1::RequestPayload::SearchKernel_V1_SearchRequest,
                payload.as_union_value(),
            )
        }
        other => Err(format!("no component transport embedding for {other}")),
    }
}

fn component_search_input(operation: &str, input: &JsonValue) -> JsonValue {
    let mut object = input.as_object().cloned().unwrap_or_default();
    match operation {
        "search_fields_to_ir" => {
            object.remove("selection");
            if let Some(selection) = input.get("selection").filter(|value| !value.is_null()) {
                object.insert("search_fields".to_owned(), selection.clone());
            }
        }
        "projection_to_ir" => {
            object.remove("projection");
            if let Some(projection) = input.get("projection").filter(|value| !value.is_null()) {
                object.insert("projection".to_owned(), projection.clone());
            }
        }
        "highlighting_to_ir" => {
            object.remove("highlighting");
            if let Some(highlighting) = input.get("highlighting").filter(|value| !value.is_null()) {
                object.insert("highlighting".to_owned(), highlighting.clone());
            }
        }
        "comparison_to_ir" => {
            object.clear();
            object.insert(
                "filter".to_owned(),
                json!({"comparison": input.get("comparison").unwrap_or(input)}),
            );
        }
        "range_to_ir" => {
            object.clear();
            object.insert(
                "filter".to_owned(),
                json!({"range": input.get("range").unwrap_or(input)}),
            );
        }
        "filter_to_ir" => {
            object.clear();
            object.insert("filter".to_owned(), input.clone());
        }
        "sort_to_ir" => {
            object.clear();
            object.insert("sort".to_owned(), json!([input]));
        }
        "facet_to_ir" => {
            object.clear();
            object.insert("facets".to_owned(), json!([input]));
        }
        "ranking_to_ir" => {
            object.clear();
            object.insert("ranking".to_owned(), input.clone());
        }
        "query_to_ir" => {
            object.clear();
            if let Some(query) = input.get("query") {
                object.insert("query".to_owned(), query.clone());
            }
            if let Some(query_expr) = input.get("query_expr") {
                object.insert("query_expr".to_owned(), query_expr.clone());
            }
        }
        _ => {}
    }
    JsonValue::Object(object)
}

#[derive(Debug)]
pub enum RunnerError {
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    Json {
        path: PathBuf,
        source: serde_json::Error,
    },
    Case {
        id: String,
        message: String,
    },
}

#[derive(Debug)]
enum ExecError {
    Message(String),
    Portable(PortableError),
}

impl From<String> for ExecError {
    fn from(value: String) -> Self {
        Self::Message(value)
    }
}
impl From<&'static str> for ExecError {
    fn from(value: &'static str) -> Self {
        Self::Message(value.to_owned())
    }
}
impl fmt::Display for ExecError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Message(value) => formatter.write_str(value),
            Self::Portable(value) => value.fmt(formatter),
        }
    }
}

impl fmt::Display for RunnerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => write!(formatter, "{}: {source}", path.display()),
            Self::Json { path, source } => write!(formatter, "{}: {source}", path.display()),
            Self::Case { id, message } => write!(formatter, "{id}: {message}"),
        }
    }
}
impl std::error::Error for RunnerError {}

/// Entry point for a frozen conformance corpus.
#[derive(Clone, Debug)]
pub struct ConformanceRunner {
    root: PathBuf,
}

impl ConformanceRunner {
    /// Create a runner for the corpus stored below `root`.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
    /// Return the corpus directory used by this runner.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Run every case in the corpus and compare it with its golden output.
    pub fn run(&self) -> StdResult<ConformanceReport, RunnerError> {
        let cases_path = self.root.join("cases.json");
        let cases = read_json(&cases_path)?;
        let cases = cases
            .get("cases")
            .and_then(JsonValue::as_array)
            .ok_or_else(|| RunnerError::Case {
                id: "runner".to_owned(),
                message: "cases.json must contain a cases array".to_owned(),
            })?;
        let mut categories = BTreeMap::new();
        for entry in cases {
            let id = required_string(entry, "id")?.to_owned();
            let case = read_json(&self.root.join(required_string(entry, "case")?))?;
            let golden = read_json(&self.root.join(required_string(entry, "golden")?))?;
            run_case(&self.root, &case, &golden).map_err(|message| RunnerError::Case {
                id: id.clone(),
                message,
            })?;
            *categories
                .entry(required_string(&case, "category")?.to_owned())
                .or_insert(0) += 1;
        }
        Ok(ConformanceReport {
            passed: cases.len(),
            categories: categories.into_iter().collect(),
        })
    }
}

fn read_json(path: &Path) -> StdResult<JsonValue, RunnerError> {
    let text = fs::read_to_string(path).map_err(|source| RunnerError::Io {
        path: path.to_owned(),
        source,
    })?;
    serde_json::from_str(&text).map_err(|source| RunnerError::Json {
        path: path.to_owned(),
        source,
    })
}

fn required_string<'a>(value: &'a JsonValue, key: &str) -> StdResult<&'a str, RunnerError> {
    value
        .get(key)
        .and_then(JsonValue::as_str)
        .ok_or_else(|| RunnerError::Case {
            id: "runner".to_owned(),
            message: format!("missing string property {key}"),
        })
}

fn run_case(root: &Path, case: &JsonValue, golden: &JsonValue) -> StdResult<(), String> {
    let operation = case
        .get("operation")
        .and_then(JsonValue::as_str)
        .ok_or("missing operation")?;
    let input = case.get("input").cloned().unwrap_or_else(|| json!({}));
    let index_json = match case.get("index_fixture").and_then(JsonValue::as_str) {
        Some(name) => Some(
            read_json(&root.join("fixtures/indexes").join(format!("{name}.json")))
                .map_err(|error| error.to_string())?,
        ),
        None => None,
    };
    let actual = match execute(operation, &input, index_json.as_ref()) {
        Ok(value) => value,
        Err(ExecError::Portable(error)) => json!({"error": error_json(&error)}),
        Err(error) => return Err(error.to_string()),
    };
    if &actual != golden {
        return Err(format!(
            "golden mismatch\nexpected: {golden}\nactual:   {actual}"
        ));
    }
    Ok(())
}

fn execute(
    operation: &str,
    input: &JsonValue,
    index_json: Option<&JsonValue>,
) -> StdResult<JsonValue, ExecError> {
    let bytes = if matches!(operation, "describe" | "inspect_query") {
        Vec::new()
    } else {
        WireBuilder::new().build(operation, input)?
    };
    let context = index_json
        .map(|index| {
            let bytes = WireBuilder::new().build("index_definition_to_ir", index)?;
            let root = flatbuffers::root::<api::IndexDefinition>(&bytes)
                .map_err(|error| error.to_string())?;
            IndexContext::from_api(root).map_err(ExecError::Portable)
        })
        .transpose()?;
    let result = match operation {
        "describe" => ok(describe_json(
            &context.ok_or("describe requires index fixture")?.describe(),
        )),
        "inspect_query" => {
            let context = context.ok_or("inspect_query requires index fixture")?;
            let request = input
                .get("request")
                .cloned()
                .unwrap_or_else(|| input.clone());
            let bytes = WireBuilder::new().build("search_to_ir", &request)?;
            let root = flatbuffers::root::<api::SearchRequest>(&bytes)
                .map_err(|error| error.to_string())?;
            let ir = context.inspect_query(root).map_err(ExecError::Portable)?;
            ok(inspect_json(&ir))
        }
        "index_definition_to_ir" => {
            let root = flatbuffers::root::<api::IndexDefinition>(&bytes)
                .map_err(|error| error.to_string())?;
            ok(index_json_value(
                &index_definition_to_ir(root).map_err(ExecError::Portable)?,
            ))
        }
        "field_to_ir" => {
            let root = flatbuffers::root::<api::FieldDefinition>(&bytes)
                .map_err(|error| error.to_string())?;
            ok(field_json(&field_to_ir(root).map_err(ExecError::Portable)?))
        }
        "analyzer_to_ir" => {
            let kind = parse_field_kind(
                input
                    .get("field_kind")
                    .and_then(JsonValue::as_str)
                    .unwrap_or("text"),
            )?;
            let analyzer = if input.get("analyzer").is_some_and(|value| !value.is_null()) {
                let root = flatbuffers::root::<api::Analyzer>(&bytes)
                    .map_err(|error| error.to_string())?;
                analyzer_to_ir(Some(root), kind).map_err(ExecError::Portable)?
            } else {
                analyzer_to_ir(None, kind).map_err(ExecError::Portable)?
            };
            ok(json!(analyzer_name(analyzer)))
        }
        "value_to_ir" => {
            let context = context.ok_or("operation requires index fixture")?;
            let root =
                flatbuffers::root::<api::Value>(&bytes).map_err(|error| error.to_string())?;
            ok(field_value_json(
                &context
                    .value_to_ir(
                        input
                            .get("field")
                            .and_then(JsonValue::as_str)
                            .unwrap_or_default(),
                        root,
                    )
                    .map_err(ExecError::Portable)?,
            ))
        }
        "document_to_ir" => {
            let context = context.ok_or("operation requires index fixture")?;
            let root =
                flatbuffers::root::<api::Document>(&bytes).map_err(|error| error.to_string())?;
            ok(document_json(
                &context.document_to_ir(root).map_err(ExecError::Portable)?,
            ))
        }
        "search_fields_to_ir" => {
            let context = context.ok_or("operation requires index fixture")?;
            let fields = if input.get("selection").is_some_and(|value| !value.is_null()) {
                let root = flatbuffers::root::<api::SearchFieldSelection>(&bytes)
                    .map_err(|error| error.to_string())?;
                context
                    .search_fields_to_ir(root)
                    .map_err(ExecError::Portable)?
            } else {
                context.definition().default_search_fields().to_vec()
            };
            ok(search_fields_json(&fields))
        }
        "projection_to_ir" => {
            let context = context.ok_or("operation requires index fixture")?;
            let projection = if input
                .get("projection")
                .is_some_and(|value| !value.is_null())
            {
                let root = flatbuffers::root::<api::FieldProjection>(&bytes)
                    .map_err(|error| error.to_string())?;
                context
                    .projection_to_ir(Some(root))
                    .map_err(ExecError::Portable)?
            } else {
                context
                    .projection_to_ir(None)
                    .map_err(ExecError::Portable)?
            };
            ok(projection_json(&projection))
        }
        "highlighting_to_ir" => {
            let context = context.ok_or("operation requires index fixture")?;
            let highlighting = if input
                .get("highlighting")
                .is_some_and(|value| !value.is_null())
            {
                let root = flatbuffers::root::<api::HighlightRequest>(&bytes)
                    .map_err(|error| error.to_string())?;
                context
                    .highlighting_to_ir(Some(root))
                    .map_err(ExecError::Portable)?
            } else {
                context
                    .highlighting_to_ir(None)
                    .map_err(ExecError::Portable)?
            };
            ok(highlighting_json(&highlighting))
        }
        "comparison_to_ir" => {
            let context = context.ok_or("operation requires index fixture")?;
            let root = flatbuffers::root::<api::FilterComparison>(&bytes)
                .map_err(|error| error.to_string())?;
            ok(comparison_json(
                &context
                    .comparison_to_ir(root)
                    .map_err(ExecError::Portable)?,
            ))
        }
        "range_to_ir" => {
            let context = context.ok_or("operation requires index fixture")?;
            let root =
                flatbuffers::root::<api::FilterRange>(&bytes).map_err(|error| error.to_string())?;
            ok(range_json(
                &context.range_to_ir(root).map_err(ExecError::Portable)?,
            ))
        }
        "filter_to_ir" => {
            let context = context.ok_or("operation requires index fixture")?;
            let root =
                flatbuffers::root::<api::FilterExpr>(&bytes).map_err(|error| error.to_string())?;
            ok(filter_json(
                &context.filter_to_ir(root).map_err(ExecError::Portable)?,
            ))
        }
        "sort_to_ir" => {
            let context = context.ok_or("operation requires index fixture")?;
            let root =
                flatbuffers::root::<api::SortExpr>(&bytes).map_err(|error| error.to_string())?;
            ok(sort_json(
                &context.sort_to_ir(root).map_err(ExecError::Portable)?,
            ))
        }
        "facet_to_ir" => {
            let context = context.ok_or("operation requires index fixture")?;
            let root = flatbuffers::root::<api::FacetRequest>(&bytes)
                .map_err(|error| error.to_string())?;
            ok(facet_json(
                &context.facet_to_ir(root).map_err(ExecError::Portable)?,
            ))
        }
        "ranking_to_ir" => {
            let context = context.ok_or("operation requires index fixture")?;
            let root = flatbuffers::root::<api::RankingPolicy>(&bytes)
                .map_err(|error| error.to_string())?;
            ok(json!(ranking_name(
                context
                    .ranking_to_ir(Some(root))
                    .map_err(ExecError::Portable)?
            )))
        }
        "query_to_ir" => {
            let context = context.ok_or("operation requires index fixture")?;
            let query = if input
                .get("query_expr")
                .is_some_and(|value| !value.is_null())
            {
                let root = flatbuffers::root::<api::QueryExpr>(&bytes)
                    .map_err(|error| error.to_string())?;
                context
                    .query_to_ir(
                        input
                            .get("query")
                            .and_then(JsonValue::as_str)
                            .unwrap_or_default(),
                        Some(root),
                    )
                    .map_err(ExecError::Portable)?
            } else {
                context
                    .query_to_ir(
                        input
                            .get("query")
                            .and_then(JsonValue::as_str)
                            .unwrap_or_default(),
                        None,
                    )
                    .map_err(ExecError::Portable)?
            };
            ok(query_json(&query))
        }
        "search_to_ir" => {
            let context = context.ok_or("operation requires index fixture")?;
            let root = flatbuffers::root::<api::SearchRequest>(&bytes)
                .map_err(|error| error.to_string())?;
            ok(search_json(
                &context.search_to_ir(root).map_err(ExecError::Portable)?,
            ))
        }
        "analyze_to_ir" => {
            let context = context.ok_or("operation requires index fixture")?;
            let root = flatbuffers::root::<api::AnalyzeRequest>(&bytes)
                .map_err(|error| error.to_string())?;
            ok(analyze_json(
                &context.analyze_to_ir(root).map_err(ExecError::Portable)?,
            ))
        }
        "insert_to_ir" => {
            let context = context.ok_or("operation requires index fixture")?;
            let root = flatbuffers::root::<api::InsertRequest>(&bytes)
                .map_err(|error| error.to_string())?;
            ok(insert_json(
                &context.insert_to_ir(root).map_err(ExecError::Portable)?,
            ))
        }
        "upsert_to_ir" => {
            let context = context.ok_or("operation requires index fixture")?;
            let root = flatbuffers::root::<api::UpsertRequest>(&bytes)
                .map_err(|error| error.to_string())?;
            ok(upsert_json(
                &context.upsert_to_ir(root).map_err(ExecError::Portable)?,
            ))
        }
        "patch_to_ir" => {
            let context = context.ok_or("operation requires index fixture")?;
            let root = flatbuffers::root::<api::PatchRequest>(&bytes)
                .map_err(|error| error.to_string())?;
            ok(patch_json(
                &context.patch_to_ir(root).map_err(ExecError::Portable)?,
            ))
        }
        "delete_to_ir" => {
            let context = context.ok_or("operation requires index fixture")?;
            let root = flatbuffers::root::<api::DeleteRequest>(&bytes)
                .map_err(|error| error.to_string())?;
            ok(delete_json(
                &context.delete_to_ir(root).map_err(ExecError::Portable)?,
            ))
        }
        "get_to_ir" => {
            let context = context.ok_or("operation requires index fixture")?;
            let root =
                flatbuffers::root::<api::GetRequest>(&bytes).map_err(|error| error.to_string())?;
            ok(get_json(
                &context.get_to_ir(root).map_err(ExecError::Portable)?,
            ))
        }
        "write_to_ir" => {
            let context = context.ok_or("operation requires index fixture")?;
            let kind = input
                .get("kind")
                .and_then(JsonValue::as_str)
                .unwrap_or("insert");
            let write = match kind {
                "insert" => {
                    let root = flatbuffers::root::<api::InsertRequest>(&bytes)
                        .map_err(|error| error.to_string())?;
                    context.write_to_ir(WriteRequestRef::Insert(root))
                }
                "upsert" => {
                    let root = flatbuffers::root::<api::UpsertRequest>(&bytes)
                        .map_err(|error| error.to_string())?;
                    context.write_to_ir(WriteRequestRef::Upsert(root))
                }
                "patch" => {
                    let root = flatbuffers::root::<api::PatchRequest>(&bytes)
                        .map_err(|error| error.to_string())?;
                    context.write_to_ir(WriteRequestRef::Patch(root))
                }
                "delete" => {
                    let root = flatbuffers::root::<api::DeleteRequest>(&bytes)
                        .map_err(|error| error.to_string())?;
                    context.write_to_ir(WriteRequestRef::Delete(root))
                }
                other => return Err(format!("unknown write kind {other}").into()),
            }
            .map_err(ExecError::Portable)?;
            ok(write_json(&write))
        }
        _ => return Err(format!("operation {operation} is not an API→IR corpus operation").into()),
    };
    Ok(result)
}

fn ok(value: JsonValue) -> JsonValue {
    json!({"ok": value})
}
fn error_json(error: &PortableError) -> JsonValue {
    let bytes = error_to_api(error);
    let encoded = flatbuffers::root::<api::Error>(&bytes)
        .expect("production error encoder must emit a valid Error table");
    let details = encoded
        .details()
        .map(|values| {
            values
                .iter()
                .map(|detail| json!({"code":detail.code(),"path":detail.path()}))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    json!({"code":api_error_code_name(encoded.code()),"details":details,"retryable":encoded.retryable()})
}
fn api_error_code_name(code: api::ErrorCode) -> &'static str {
    match code {
        api::ErrorCode::InvalidArgument => "invalid_argument",
        api::ErrorCode::NotFound => "not_found",
        api::ErrorCode::AlreadyExists => "already_exists",
        api::ErrorCode::FailedPrecondition => "failed_precondition",
        api::ErrorCode::Conflict => "conflict",
        api::ErrorCode::Unsupported => "unsupported",
        api::ErrorCode::ResourceExhausted => "resource_exhausted",
        api::ErrorCode::Internal => "internal",
        api::ErrorCode::Unavailable => "unavailable",
        api::ErrorCode::Unspecified => "unspecified",
        _ => "unspecified",
    }
}
fn parse_field_kind(value: &str) -> StdResult<FieldKind, String> {
    match value {
        "text" => Ok(FieldKind::Text),
        "keyword" => Ok(FieldKind::Keyword),
        "int64" => Ok(FieldKind::Int64),
        "float64" => Ok(FieldKind::Float64),
        "bool" => Ok(FieldKind::Bool),
        _ => Err(format!("unknown field kind {value}")),
    }
}
fn scalar_json(value: &ScalarValueIr) -> JsonValue {
    match value {
        ScalarValueIr::Bool(v) => json!({"bool":v}),
        ScalarValueIr::Int64(v) => json!({"int64":v}),
        ScalarValueIr::Float64(v) => json!({"float64":v}),
        ScalarValueIr::String(v) => json!({"string":v}),
    }
}
fn field_value_json(value: &FieldValueIr) -> JsonValue {
    match value {
        FieldValueIr::Single(v) => json!({"single":scalar_json(v)}),
        FieldValueIr::Multi(v) => json!({"multi":v.iter().map(scalar_json).collect::<Vec<_>>()}),
    }
}
fn search_fields_json(values: &[SearchFieldIr]) -> JsonValue {
    json!(values
        .iter()
        .map(|v| json!({"field":v.field().as_str(),"weight":v.weight().get()}))
        .collect::<Vec<_>>())
}
fn projection_json(value: &ProjectionIr) -> JsonValue {
    json!({"fields":value.fields().iter().map(|v|v.as_str()).collect::<Vec<_>>()})
}
fn highlighting_json(value: &HighlightingIr) -> JsonValue {
    match value {
        HighlightingIr::Disabled => json!({"kind":"disabled"}),
        HighlightingIr::Requested { fields } => {
            json!({"kind":"requested","fields":fields.iter().map(|v|v.as_str()).collect::<Vec<_>>()})
        }
    }
}
fn ranking_name(value: RankingPolicyIr) -> &'static str {
    match value {
        RankingPolicyIr::DefaultRelevanceV1 => "default_relevance_v1",
        RankingPolicyIr::Bm25 => "bm25",
    }
}
fn analyzer_name(value: Option<AnalyzerIdentity>) -> Option<&'static str> {
    value.map(|v| match v {
        AnalyzerIdentity::StandardV1 => "standard_v1",
    })
}
fn field_kind_name(value: FieldKind) -> &'static str {
    match value {
        FieldKind::Text => "text",
        FieldKind::Keyword => "keyword",
        FieldKind::Int64 => "int64",
        FieldKind::Float64 => "float64",
        FieldKind::Bool => "bool",
    }
}
fn cardinality_name(value: FieldCardinality) -> &'static str {
    match value {
        FieldCardinality::Single => "single",
        FieldCardinality::Multi => "multi",
    }
}
fn field_json(value: &FieldIr) -> JsonValue {
    let capabilities = value.capabilities();
    json!({"name":value.name().as_str(),"kind":field_kind_name(value.kind()),"cardinality":cardinality_name(value.cardinality()),"analyzer":analyzer_name(value.analyzer()),"capabilities":{"stored":capabilities.stored(),"searchable":capabilities.searchable(),"filterable":capabilities.filterable(),"sortable":capabilities.sortable(),"facetable":capabilities.facetable()}})
}
fn index_json_value(value: &IndexDefinitionIr) -> JsonValue {
    json!({"name":value.name().map(IndexName::as_str),"fields":value.fields().iter().map(field_json).collect::<Vec<_>>(),"default_search_fields":search_fields_json(value.default_search_fields()),"default_ranking":ranking_name(value.default_ranking()),"default_prefix_policy":"disabled","default_typo_policy":"disabled"})
}
fn describe_json(value: &IndexDefinitionIr) -> JsonValue {
    let fields = value.fields().iter().map(|field| {
        let capabilities = field.capabilities();
        let analyzer = field.analyzer().map(|_| json!({"name":"standard","options":{},"version":1}));
        json!({"name":field.name().as_str(),"kind":field_kind_name(field.kind()),"cardinality":cardinality_name(field.cardinality()),"stored":capabilities.stored(),"searchable":capabilities.searchable(),"filterable":capabilities.filterable(),"sortable":capabilities.sortable(),"facetable":capabilities.facetable(),"analyzer":analyzer})
    }).collect::<Vec<_>>();
    json!({"index":{"name":value.name().map(IndexName::as_str),"fields":fields,"default_search_fields":{"fields":value.default_search_fields().iter().map(|field|json!({"field":field.field().as_str(),"weight":field.weight().get()})).collect::<Vec<_>>()}}})
}
fn inspect_json(value: &SearchRequestIr) -> JsonValue {
    let query = match value.query() {
        QueryIr::Text(text) => json!(text),
        QueryIr::MatchAll => json!(""),
        other => query_json(other),
    };
    let ranking = json!({ranking_name(value.ranking()):{}});
    let highlighting = match value.highlighting() {
        HighlightingIr::Disabled => JsonValue::Null,
        HighlightingIr::Requested { fields } => {
            json!({"fields":fields.iter().map(|field|field.as_str()).collect::<Vec<_>>()})
        }
    };
    json!({"resolved_request":{"query":query,"search_fields":{"fields":value.search_fields().iter().map(|field|json!({"field":field.field().as_str(),"weight":field.weight().get()})).collect::<Vec<_>>()},"filter":value.filter().map(filter_json),"sort":value.sort().iter().map(sort_json).collect::<Vec<_>>(),"facets":value.facets().iter().map(facet_json).collect::<Vec<_>>(),"ranking":ranking,"limit":value.limit(),"offset":value.offset(),"projection":projection_json(value.projection()),"highlighting":highlighting,"language":value.language().map(LanguageHint::as_str)}})
}
fn document_json(value: &DocumentIr) -> JsonValue {
    let fields: Map<String, JsonValue> = value
        .fields()
        .iter()
        .map(|(k, v)| (k.as_str().to_owned(), field_value_json(v)))
        .collect();
    json!({"id":value.id().as_str(),"fields":fields})
}
fn insert_json(value: &InsertRequestIr) -> JsonValue {
    json!({"document":document_json(value.document())})
}
fn upsert_json(value: &UpsertRequestIr) -> JsonValue {
    json!({"document":document_json(value.document())})
}
fn delete_json(value: &DeleteRequestIr) -> JsonValue {
    json!({"id":value.id().as_str()})
}
fn get_json(value: &GetRequestIr) -> JsonValue {
    json!({"id":value.id().as_str(),"projection":projection_json(value.projection())})
}
fn patch_operation_json(value: &PatchOperationIr) -> JsonValue {
    match value {
        PatchOperationIr::Set { field, value } => {
            json!({"set":{"field":field.as_str(),"value":field_value_json(value)}})
        }
        PatchOperationIr::Unset { field } => json!({"unset":{"field":field.as_str()}}),
    }
}
fn patch_json(value: &PatchRequestIr) -> JsonValue {
    json!({"id":value.id().as_str(),"operations":value.operations().iter().map(patch_operation_json).collect::<Vec<_>>()})
}
fn write_json(value: &WriteRequestIr) -> JsonValue {
    match value {
        WriteRequestIr::Insert(value) => json!({"insert":insert_json(value)}),
        WriteRequestIr::Upsert(value) => json!({"upsert":upsert_json(value)}),
        WriteRequestIr::Patch(value) => json!({"patch":patch_json(value)}),
        WriteRequestIr::Delete(value) => json!({"delete":delete_json(value)}),
    }
}
fn comparison_name(value: ComparisonOp) -> &'static str {
    match value {
        ComparisonOp::Eq => "eq",
        ComparisonOp::Ne => "ne",
        ComparisonOp::Lt => "lt",
        ComparisonOp::Lte => "lte",
        ComparisonOp::Gt => "gt",
        ComparisonOp::Gte => "gte",
    }
}
fn comparison_json(value: &ComparisonIr) -> JsonValue {
    json!({"field":value.field().as_str(),"op":comparison_name(value.op()),"value":scalar_json(value.value())})
}
fn range_json(value: &RangeIr) -> JsonValue {
    json!({"field":value.field().as_str(),"lower":value.lower().map(scalar_json),"upper":value.upper().map(scalar_json),"include_lower":value.include_lower(),"include_upper":value.include_upper()})
}
fn filter_json(value: &FilterIr) -> JsonValue {
    match value {
        FilterIr::And(v) => json!({"and":v.iter().map(filter_json).collect::<Vec<_>>() }),
        FilterIr::Or(v) => json!({"or":v.iter().map(filter_json).collect::<Vec<_>>() }),
        FilterIr::Not(v) => json!({"not":filter_json(v)}),
        FilterIr::Comparison(v) => json!({"comparison":comparison_json(v)}),
        FilterIr::Range(v) => json!({"range":range_json(v)}),
        FilterIr::Exists { field } => json!({"exists":{"field":field.as_str()}}),
    }
}
fn query_json(value: &QueryIr) -> JsonValue {
    match value {
        QueryIr::MatchAll => json!({"match_all":{}}),
        QueryIr::Text(v) => json!({"text":v}),
        QueryIr::Term { field, value } => {
            json!({"term":{"field":field.as_str(),"value":scalar_json(value)}})
        }
        QueryIr::And(v) => json!({"and":v.iter().map(query_json).collect::<Vec<_>>() }),
        QueryIr::Or(v) => json!({"or":v.iter().map(query_json).collect::<Vec<_>>() }),
        QueryIr::Not(v) => json!({"not":query_json(v)}),
    }
}
fn sort_json(value: &SortIr) -> JsonValue {
    json!({"field":value.field().as_str(),"direction":match value.direction() {SortDirection::Asc=>"asc",SortDirection::Desc=>"desc"},"missing":match value.missing() {MissingPlacement::First=>"first",MissingPlacement::Last=>"last"}})
}
fn facet_json(value: &FacetIr) -> JsonValue {
    json!({"field":value.field().as_str(),"limit":value.limit(),"sort":match value.sort() {FacetSort::CountDesc=>"count_desc",FacetSort::CountAsc=>"count_asc",FacetSort::ValueAsc=>"value_asc",FacetSort::ValueDesc=>"value_desc"}})
}
fn analyze_json(value: &AnalyzeRequestIr) -> JsonValue {
    json!({"analyzer":analyzer_name(Some(value.analyzer())),"text":value.text(),"language":value.language().map(LanguageHint::as_str)})
}
fn search_json(value: &SearchRequestIr) -> JsonValue {
    json!({"query":query_json(value.query()),"search_fields":search_fields_json(value.search_fields()),"filter":value.filter().map(filter_json),"sort":value.sort().iter().map(sort_json).collect::<Vec<_>>(),"facets":value.facets().iter().map(facet_json).collect::<Vec<_>>(),"ranking":ranking_name(value.ranking()),"limit":value.limit(),"offset":value.offset(),"projection":projection_json(value.projection()),"highlighting":highlighting_json(value.highlighting()),"language":value.language().map(LanguageHint::as_str),"prefix_policy":"disabled","typo_policy":"disabled"})
}

struct WireBuilder {
    builder: flatbuffers::FlatBufferBuilder<'static>,
}

impl WireBuilder {
    fn new() -> Self {
        Self {
            builder: flatbuffers::FlatBufferBuilder::new(),
        }
    }

    fn text(&mut self, value: &str) -> flatbuffers::WIPOffset<&'static str> {
        self.builder.create_string(value)
    }

    fn finish<T: flatbuffers::Follow<'static>>(
        &mut self,
        root: flatbuffers::WIPOffset<T>,
    ) -> StdResult<Vec<u8>, String> {
        self.finish_with_identifier(root, None)
    }

    fn finish_with_identifier<T: flatbuffers::Follow<'static>>(
        &mut self,
        root: flatbuffers::WIPOffset<T>,
        identifier: Option<&str>,
    ) -> StdResult<Vec<u8>, String> {
        self.builder.finish(root, identifier);
        Ok(self.builder.finished_data().to_vec())
    }

    fn build(mut self, operation: &str, input: &JsonValue) -> StdResult<Vec<u8>, String> {
        match operation {
            "index_definition_to_ir" => {
                let root = self.index(input)?;
                self.finish(root)
            }
            "field_to_ir" => {
                let root = self.field(input.get("field").unwrap_or(input))?;
                self.finish(root)
            }
            "analyzer_to_ir" => {
                let root = self.analyzer(input.get("analyzer"))?;
                self.finish(root)
            }
            "value_to_ir" => {
                let root = self.value(input.get("value").unwrap_or(input))?;
                self.finish(root)
            }
            "document_to_ir" => {
                let root = self.document(input.get("document").unwrap_or(input))?;
                self.finish(root)
            }
            "search_fields_to_ir" => {
                let root = self.selection(input.get("selection").unwrap_or(input))?;
                self.finish(root)
            }
            "projection_to_ir" => {
                let root = self.projection(input.get("projection").unwrap_or(input))?;
                self.finish(root)
            }
            "highlighting_to_ir" => {
                let root = self.highlighting(input.get("highlighting"))?;
                self.finish(root)
            }
            "comparison_to_ir" => {
                let root = self.comparison(input.get("comparison").unwrap_or(input))?;
                self.finish(root)
            }
            "range_to_ir" => {
                let root = self.range(input.get("range").unwrap_or(input))?;
                self.finish(root)
            }
            "filter_to_ir" => {
                let root = self.filter(input.get("filter").unwrap_or(input))?;
                self.finish(root)
            }
            "sort_to_ir" => {
                let root = self.sort(input)?;
                self.finish(root)
            }
            "facet_to_ir" => {
                let root = self.facet(input)?;
                self.finish(root)
            }
            "ranking_to_ir" => {
                let root = self.ranking(input)?;
                self.finish(root)
            }
            "query_to_ir" => {
                let root = self.query_expr(input.get("query_expr").unwrap_or(input))?;
                self.finish(root)
            }
            "search_to_ir" => {
                let root = self.search(input)?;
                self.finish(root)
            }
            "analyze_to_ir" => {
                let root = self.analyze(input)?;
                self.finish(root)
            }
            "insert_to_ir" | "upsert_to_ir" => {
                let document = self.document(input.get("document").unwrap_or(input))?;
                if operation == "insert_to_ir" {
                    let root = api::InsertRequest::create(
                        &mut self.builder,
                        &api::InsertRequestArgs {
                            document: Some(document),
                        },
                    );
                    self.finish(root)
                } else {
                    let root = api::UpsertRequest::create(
                        &mut self.builder,
                        &api::UpsertRequestArgs {
                            document: Some(document),
                        },
                    );
                    self.finish(root)
                }
            }
            "delete_to_ir" => {
                let id = input
                    .get("id")
                    .and_then(JsonValue::as_str)
                    .map(|v| self.text(v));
                let root =
                    api::DeleteRequest::create(&mut self.builder, &api::DeleteRequestArgs { id });
                self.finish(root)
            }
            "get_to_ir" => {
                let id = input
                    .get("id")
                    .and_then(JsonValue::as_str)
                    .map(|v| self.text(v));
                let projection = input
                    .get("projection")
                    .filter(|v| !v.is_null())
                    .map(|v| self.projection(v))
                    .transpose()?;
                let root = api::GetRequest::create(
                    &mut self.builder,
                    &api::GetRequestArgs { id, projection },
                );
                self.finish(root)
            }
            "patch_to_ir" => {
                let operations = input
                    .get("operations")
                    .and_then(JsonValue::as_array)
                    .map(|values| {
                        values
                            .iter()
                            .map(|v| self.patch_operation(v))
                            .collect::<StdResult<Vec<_>, _>>()
                    })
                    .transpose()?
                    .map(|v| self.builder.create_vector(&v));
                let id = input
                    .get("id")
                    .and_then(JsonValue::as_str)
                    .map(|v| self.text(v));
                let root = api::PatchRequest::create(
                    &mut self.builder,
                    &api::PatchRequestArgs { id, operations },
                );
                self.finish(root)
            }
            "write_to_ir" => {
                let request = input.get("request").unwrap_or(input);
                match input
                    .get("kind")
                    .and_then(JsonValue::as_str)
                    .unwrap_or("insert")
                {
                    "insert" => {
                        let document = self.document(request.get("document").unwrap_or(request))?;
                        let root = api::InsertRequest::create(
                            &mut self.builder,
                            &api::InsertRequestArgs {
                                document: Some(document),
                            },
                        );
                        self.finish(root)
                    }
                    "upsert" => {
                        let document = self.document(request.get("document").unwrap_or(request))?;
                        let root = api::UpsertRequest::create(
                            &mut self.builder,
                            &api::UpsertRequestArgs {
                                document: Some(document),
                            },
                        );
                        self.finish(root)
                    }
                    "delete" => {
                        let id = request
                            .get("id")
                            .and_then(JsonValue::as_str)
                            .map(|v| self.text(v));
                        let root = api::DeleteRequest::create(
                            &mut self.builder,
                            &api::DeleteRequestArgs { id },
                        );
                        self.finish(root)
                    }
                    "patch" => {
                        let operations = request
                            .get("operations")
                            .and_then(JsonValue::as_array)
                            .map(|values| {
                                values
                                    .iter()
                                    .map(|v| self.patch_operation(v))
                                    .collect::<StdResult<Vec<_>, _>>()
                            })
                            .transpose()?
                            .map(|v| self.builder.create_vector(&v));
                        let id = request
                            .get("id")
                            .and_then(JsonValue::as_str)
                            .map(|v| self.text(v));
                        let root = api::PatchRequest::create(
                            &mut self.builder,
                            &api::PatchRequestArgs { id, operations },
                        );
                        self.finish(root)
                    }
                    kind => Err(format!("unknown write kind {kind}")),
                }
            }
            _ => Err(format!("no wire builder for {operation}")),
        }
    }

    fn value(
        &mut self,
        input: &JsonValue,
    ) -> StdResult<flatbuffers::WIPOffset<api::Value<'static>>, String> {
        let object = input.as_object().ok_or("value must be an object")?;
        if object.get("_union").and_then(JsonValue::as_str) == Some("NONE") {
            return Ok(api::Value::create(
                &mut self.builder,
                &api::ValueArgs::default(),
            ));
        }
        let (kind_type, kind) = if let Some(value) = object.get("bool") {
            (
                api::ValueKind::BoolValue,
                api::BoolValue::create(
                    &mut self.builder,
                    &api::BoolValueArgs {
                        value: value.as_bool().ok_or("invalid bool")?,
                    },
                )
                .as_union_value(),
            )
        } else if let Some(value) = object.get("int64") {
            (
                api::ValueKind::Int64Value,
                api::Int64Value::create(
                    &mut self.builder,
                    &api::Int64ValueArgs {
                        value: value.as_i64().ok_or("invalid int64")?,
                    },
                )
                .as_union_value(),
            )
        } else if let Some(value) = object.get("uint64") {
            (
                api::ValueKind::UInt64Value,
                api::UInt64Value::create(
                    &mut self.builder,
                    &api::UInt64ValueArgs {
                        value: value.as_u64().ok_or("invalid uint64")?,
                    },
                )
                .as_union_value(),
            )
        } else if let Some(value) = object.get("float64") {
            (
                api::ValueKind::Float64Value,
                api::Float64Value::create(
                    &mut self.builder,
                    &api::Float64ValueArgs {
                        value: parse_float(value)?,
                    },
                )
                .as_union_value(),
            )
        } else if let Some(value) = object.get("string") {
            let value = self.text(value.as_str().ok_or("invalid string")?);
            (
                api::ValueKind::StringValue,
                api::StringValue::create(
                    &mut self.builder,
                    &api::StringValueArgs { value: Some(value) },
                )
                .as_union_value(),
            )
        } else if let Some(values) = object.get("list") {
            let values = values
                .as_array()
                .ok_or("invalid list")?
                .iter()
                .map(|v| self.value(v))
                .collect::<StdResult<Vec<_>, _>>()?;
            let values = self.builder.create_vector(&values);
            (
                api::ValueKind::ValueList,
                api::ValueList::create(
                    &mut self.builder,
                    &api::ValueListArgs {
                        values: Some(values),
                    },
                )
                .as_union_value(),
            )
        } else if object.contains_key("null") {
            (
                api::ValueKind::NullValueValue,
                api::NullValueValue::create(
                    &mut self.builder,
                    &api::NullValueValueArgs {
                        value: api::NullValue::Null,
                    },
                )
                .as_union_value(),
            )
        } else {
            return Err("unknown value kind".to_owned());
        };
        Ok(api::Value::create(
            &mut self.builder,
            &api::ValueArgs {
                kind_type,
                kind: Some(kind),
            },
        ))
    }

    fn entries(
        &mut self,
        input: Option<&JsonValue>,
    ) -> StdResult<
        flatbuffers::WIPOffset<
            flatbuffers::Vector<'static, flatbuffers::ForwardsUOffset<api::ValueEntry<'static>>>,
        >,
        String,
    > {
        let mut result = Vec::new();
        if let Some(fields) = input.and_then(JsonValue::as_object) {
            for (key, value) in fields {
                let key = self.text(key);
                let value = self.value(value)?;
                result.push(api::ValueEntry::create(
                    &mut self.builder,
                    &api::ValueEntryArgs {
                        key: Some(key),
                        value: Some(value),
                    },
                ));
            }
        }
        Ok(self.builder.create_vector(&result))
    }

    fn document(
        &mut self,
        input: &JsonValue,
    ) -> StdResult<flatbuffers::WIPOffset<api::Document<'static>>, String> {
        let id = input
            .get("id")
            .and_then(JsonValue::as_str)
            .map(|v| self.text(v));
        let fields = self.entries(input.get("fields"))?;
        Ok(api::Document::create(
            &mut self.builder,
            &api::DocumentArgs {
                id,
                fields: Some(fields),
            },
        ))
    }

    fn analyzer(
        &mut self,
        input: Option<&JsonValue>,
    ) -> StdResult<flatbuffers::WIPOffset<api::Analyzer<'static>>, String> {
        let input = input.unwrap_or(&JsonValue::Null);
        let name = input
            .get("name")
            .and_then(JsonValue::as_str)
            .map(|v| self.text(v));
        let options = self.entries(input.get("options"))?;
        Ok(api::Analyzer::create(
            &mut self.builder,
            &api::AnalyzerArgs {
                name,
                version: input
                    .get("version")
                    .and_then(JsonValue::as_u64)
                    .unwrap_or(0) as u32,
                options: Some(options),
            },
        ))
    }

    fn field(
        &mut self,
        input: &JsonValue,
    ) -> StdResult<flatbuffers::WIPOffset<api::FieldDefinition<'static>>, String> {
        let kind = match input
            .get("kind")
            .and_then(JsonValue::as_str)
            .unwrap_or("unspecified")
        {
            "text" => api::FieldKind::Text,
            "keyword" => api::FieldKind::Keyword,
            "int64" => api::FieldKind::Int64,
            "float64" => api::FieldKind::Float64,
            "bool" => api::FieldKind::Bool,
            _ => api::FieldKind::Unspecified,
        };
        let cardinality = match input
            .get("cardinality")
            .and_then(JsonValue::as_str)
            .unwrap_or("single")
        {
            "multi" => api::FieldCardinality::Multi,
            "single" => api::FieldCardinality::Single,
            _ => api::FieldCardinality::Unspecified,
        };
        let stored = input
            .get("stored")
            .and_then(JsonValue::as_bool)
            .map(api::OptionalBool::new);
        let searchable = input
            .get("searchable")
            .and_then(JsonValue::as_bool)
            .map(api::OptionalBool::new);
        let filterable = input
            .get("filterable")
            .and_then(JsonValue::as_bool)
            .map(api::OptionalBool::new);
        let sortable = input
            .get("sortable")
            .and_then(JsonValue::as_bool)
            .map(api::OptionalBool::new);
        let facetable = input
            .get("facetable")
            .and_then(JsonValue::as_bool)
            .map(api::OptionalBool::new);
        let analyzer = input
            .get("analyzer")
            .map(|v| self.analyzer(Some(v)))
            .transpose()?;
        let name = input
            .get("name")
            .and_then(JsonValue::as_str)
            .map(|v| self.text(v));
        Ok(api::FieldDefinition::create(
            &mut self.builder,
            &api::FieldDefinitionArgs {
                name,
                kind,
                cardinality,
                stored: stored.as_ref(),
                searchable: searchable.as_ref(),
                filterable: filterable.as_ref(),
                sortable: sortable.as_ref(),
                facetable: facetable.as_ref(),
                analyzer,
            },
        ))
    }

    fn selection(
        &mut self,
        input: &JsonValue,
    ) -> StdResult<flatbuffers::WIPOffset<api::SearchFieldSelection<'static>>, String> {
        let fields = input
            .get("fields")
            .and_then(JsonValue::as_array)
            .map(|values| {
                values
                    .iter()
                    .map(|v| self.search_field(v))
                    .collect::<StdResult<Vec<_>, _>>()
            })
            .transpose()?;
        let fields = fields.map(|values| self.builder.create_vector(&values));
        Ok(api::SearchFieldSelection::create(
            &mut self.builder,
            &api::SearchFieldSelectionArgs { fields },
        ))
    }

    fn search_field(
        &mut self,
        input: &JsonValue,
    ) -> StdResult<flatbuffers::WIPOffset<api::SearchField<'static>>, String> {
        let field = input
            .get("field")
            .and_then(JsonValue::as_str)
            .map(|v| self.text(v));
        let weight = input
            .get("weight")
            .map(parse_float)
            .transpose()?
            .map(api::OptionalDouble::new);
        Ok(api::SearchField::create(
            &mut self.builder,
            &api::SearchFieldArgs {
                field,
                weight: weight.as_ref(),
            },
        ))
    }

    fn projection(
        &mut self,
        input: &JsonValue,
    ) -> StdResult<flatbuffers::WIPOffset<api::FieldProjection<'static>>, String> {
        let fields = input
            .get("fields")
            .and_then(JsonValue::as_array)
            .map(|values| {
                values
                    .iter()
                    .map(|v| self.text(v.as_str().unwrap_or_default()))
                    .collect::<Vec<_>>()
            })
            .map(|values| self.builder.create_vector(&values));
        Ok(api::FieldProjection::create(
            &mut self.builder,
            &api::FieldProjectionArgs { fields },
        ))
    }

    fn highlighting(
        &mut self,
        input: Option<&JsonValue>,
    ) -> StdResult<flatbuffers::WIPOffset<api::HighlightRequest<'static>>, String> {
        self.projection(input.unwrap_or(&JsonValue::Null))
            .map(|projection| flatbuffers::WIPOffset::new(projection.value()))
    }

    fn index(
        &mut self,
        input: &JsonValue,
    ) -> StdResult<flatbuffers::WIPOffset<api::IndexDefinition<'static>>, String> {
        let fields = input
            .get("fields")
            .and_then(JsonValue::as_array)
            .map(|values| {
                values
                    .iter()
                    .map(|v| self.field(v))
                    .collect::<StdResult<Vec<_>, _>>()
            })
            .transpose()?;
        let fields = fields.map(|values| self.builder.create_vector(&values));
        let default_search_fields = input
            .get("default_search_fields")
            .map(|v| self.selection(v))
            .transpose()?;
        let name = input
            .get("name")
            .and_then(JsonValue::as_str)
            .map(|v| self.text(v));
        Ok(api::IndexDefinition::create(
            &mut self.builder,
            &api::IndexDefinitionArgs {
                name,
                fields,
                default_search_fields,
            },
        ))
    }

    fn comparison(
        &mut self,
        input: &JsonValue,
    ) -> StdResult<flatbuffers::WIPOffset<api::FilterComparison<'static>>, String> {
        let field = input
            .get("field")
            .and_then(JsonValue::as_str)
            .map(|v| self.text(v));
        let op = match input
            .get("op")
            .and_then(JsonValue::as_str)
            .unwrap_or("unspecified")
        {
            "eq" => api::ComparisonOp::Eq,
            "ne" => api::ComparisonOp::Ne,
            "lt" => api::ComparisonOp::Lt,
            "lte" => api::ComparisonOp::Lte,
            "gt" => api::ComparisonOp::Gt,
            "gte" => api::ComparisonOp::Gte,
            _ => api::ComparisonOp::Unspecified,
        };
        let value = input.get("value").map(|v| self.value(v)).transpose()?;
        Ok(api::FilterComparison::create(
            &mut self.builder,
            &api::FilterComparisonArgs { field, op, value },
        ))
    }

    fn range(
        &mut self,
        input: &JsonValue,
    ) -> StdResult<flatbuffers::WIPOffset<api::FilterRange<'static>>, String> {
        let field = input
            .get("field")
            .and_then(JsonValue::as_str)
            .map(|v| self.text(v));
        let lower = input.get("lower").map(|v| self.value(v)).transpose()?;
        let upper = input.get("upper").map(|v| self.value(v)).transpose()?;
        let include_lower = input
            .get("include_lower")
            .and_then(JsonValue::as_bool)
            .map(api::OptionalBool::new);
        let include_upper = input
            .get("include_upper")
            .and_then(JsonValue::as_bool)
            .map(api::OptionalBool::new);
        Ok(api::FilterRange::create(
            &mut self.builder,
            &api::FilterRangeArgs {
                field,
                lower,
                upper,
                include_lower: include_lower.as_ref(),
                include_upper: include_upper.as_ref(),
            },
        ))
    }

    fn filter(
        &mut self,
        input: &JsonValue,
    ) -> StdResult<flatbuffers::WIPOffset<api::FilterExpr<'static>>, String> {
        let object = input.as_object().ok_or("filter must be an object")?;
        let (expr_type, expr) =
            if let Some(values) = object.get("all").or_else(|| object.get("and")) {
                let values = values
                    .as_array()
                    .ok_or("and must be an array")?
                    .iter()
                    .map(|v| self.filter(v))
                    .collect::<StdResult<Vec<_>, _>>()?;
                let values = self.builder.create_vector(&values);
                (
                    api::FilterExprKind::FilterAnd,
                    api::FilterAnd::create(
                        &mut self.builder,
                        &api::FilterAndArgs {
                            clauses: Some(values),
                        },
                    )
                    .as_union_value(),
                )
            } else if let Some(values) = object.get("any").or_else(|| object.get("or")) {
                let values = values
                    .as_array()
                    .ok_or("or must be an array")?
                    .iter()
                    .map(|v| self.filter(v))
                    .collect::<StdResult<Vec<_>, _>>()?;
                let values = self.builder.create_vector(&values);
                (
                    api::FilterExprKind::FilterOr,
                    api::FilterOr::create(
                        &mut self.builder,
                        &api::FilterOrArgs {
                            clauses: Some(values),
                        },
                    )
                    .as_union_value(),
                )
            } else if let Some(value) = object
                .get("not")
                .or_else(|| object.get("negation").and_then(|v| v.get("clause")))
            {
                let value = self.filter(value)?;
                (
                    api::FilterExprKind::FilterNot,
                    api::FilterNot::create(
                        &mut self.builder,
                        &api::FilterNotArgs {
                            clause: Some(value),
                        },
                    )
                    .as_union_value(),
                )
            } else if let Some(value) = object.get("comparison") {
                (
                    api::FilterExprKind::FilterComparison,
                    self.comparison(value)?.as_union_value(),
                )
            } else if let Some(value) = object.get("range") {
                (
                    api::FilterExprKind::FilterRange,
                    self.range(value)?.as_union_value(),
                )
            } else if let Some(value) = object.get("exists") {
                let field = value
                    .get("field")
                    .and_then(JsonValue::as_str)
                    .map(|v| self.text(v));
                (
                    api::FilterExprKind::FilterExists,
                    api::FilterExists::create(&mut self.builder, &api::FilterExistsArgs { field })
                        .as_union_value(),
                )
            } else {
                return Ok(api::FilterExpr::create(
                    &mut self.builder,
                    &api::FilterExprArgs::default(),
                ));
            };
        Ok(api::FilterExpr::create(
            &mut self.builder,
            &api::FilterExprArgs {
                expr_type,
                expr: Some(expr),
            },
        ))
    }

    fn sort(
        &mut self,
        input: &JsonValue,
    ) -> StdResult<flatbuffers::WIPOffset<api::SortExpr<'static>>, String> {
        let field = input
            .get("field")
            .and_then(JsonValue::as_str)
            .map(|v| self.text(v));
        let direction = match input
            .get("direction")
            .and_then(JsonValue::as_str)
            .unwrap_or("unspecified")
        {
            "asc" => api::SortDirection::Asc,
            "desc" => api::SortDirection::Desc,
            _ => api::SortDirection::Unspecified,
        };
        let missing = input.get("missing").and_then(JsonValue::as_str).map(|v| {
            api::OptionalMissingPlacement::new(match v {
                "first" => api::MissingPlacement::First,
                "last" => api::MissingPlacement::Last,
                _ => api::MissingPlacement::Unspecified,
            })
        });
        Ok(api::SortExpr::create(
            &mut self.builder,
            &api::SortExprArgs {
                field,
                direction,
                missing: missing.as_ref(),
            },
        ))
    }

    fn facet(
        &mut self,
        input: &JsonValue,
    ) -> StdResult<flatbuffers::WIPOffset<api::FacetRequest<'static>>, String> {
        let field = input
            .get("field")
            .and_then(JsonValue::as_str)
            .map(|v| self.text(v));
        let limit = input
            .get("limit")
            .and_then(JsonValue::as_u64)
            .map(|v| api::OptionalUInt32::new(v as u32));
        let sort = input.get("sort").and_then(JsonValue::as_str).map(|v| {
            api::OptionalFacetSort::new(match v {
                "count_desc" => api::FacetSort::CountDesc,
                "count_asc" => api::FacetSort::CountAsc,
                "value_asc" => api::FacetSort::ValueAsc,
                "value_desc" => api::FacetSort::ValueDesc,
                _ => api::FacetSort::Unspecified,
            })
        });
        Ok(api::FacetRequest::create(
            &mut self.builder,
            &api::FacetRequestArgs {
                field,
                limit: limit.as_ref(),
                sort: sort.as_ref(),
            },
        ))
    }

    fn ranking(
        &mut self,
        input: &JsonValue,
    ) -> StdResult<flatbuffers::WIPOffset<api::RankingPolicy<'static>>, String> {
        let (policy_type, policy) = if input.get("bm25").is_some() {
            (
                api::RankingPolicyKind::Bm25Ranking,
                Some(
                    api::Bm25Ranking::create(&mut self.builder, &api::Bm25RankingArgs {})
                        .as_union_value(),
                ),
            )
        } else if input.get("default").is_some() || input.as_object().is_some_and(|v| v.is_empty())
        {
            (
                api::RankingPolicyKind::DefaultRelevanceV1,
                Some(
                    api::DefaultRelevanceV1::create(
                        &mut self.builder,
                        &api::DefaultRelevanceV1Args {},
                    )
                    .as_union_value(),
                ),
            )
        } else {
            (api::RankingPolicyKind::NONE, None)
        };
        Ok(api::RankingPolicy::create(
            &mut self.builder,
            &api::RankingPolicyArgs {
                policy_type,
                policy,
            },
        ))
    }

    fn query_expr(
        &mut self,
        input: &JsonValue,
    ) -> StdResult<flatbuffers::WIPOffset<api::QueryExpr<'static>>, String> {
        let object = input
            .as_object()
            .ok_or("query expression must be an object")?;
        let (expr_type, expr) = if let Some(value) = object.get("term") {
            let field = value
                .get("field")
                .and_then(JsonValue::as_str)
                .map(|v| self.text(v));
            let value = value.get("value").map(|v| self.value(v)).transpose()?;
            (
                api::QueryExprKind::TermQuery,
                api::TermQuery::create(&mut self.builder, &api::TermQueryArgs { field, value })
                    .as_union_value(),
            )
        } else if let Some(values) = object.get("all").or_else(|| object.get("and")) {
            let values = values
                .as_array()
                .ok_or("query and must be an array")?
                .iter()
                .map(|v| self.query_expr(v))
                .collect::<StdResult<Vec<_>, _>>()?;
            let values = self.builder.create_vector(&values);
            (
                api::QueryExprKind::QueryAnd,
                api::QueryAnd::create(
                    &mut self.builder,
                    &api::QueryAndArgs {
                        clauses: Some(values),
                    },
                )
                .as_union_value(),
            )
        } else if let Some(values) = object.get("any").or_else(|| object.get("or")) {
            let values = values
                .as_array()
                .ok_or("query or must be an array")?
                .iter()
                .map(|v| self.query_expr(v))
                .collect::<StdResult<Vec<_>, _>>()?;
            let values = self.builder.create_vector(&values);
            (
                api::QueryExprKind::QueryOr,
                api::QueryOr::create(
                    &mut self.builder,
                    &api::QueryOrArgs {
                        clauses: Some(values),
                    },
                )
                .as_union_value(),
            )
        } else if let Some(value) = object
            .get("not")
            .or_else(|| object.get("negation").and_then(|v| v.get("clause")))
        {
            let value = self.query_expr(value)?;
            (
                api::QueryExprKind::QueryNot,
                api::QueryNot::create(
                    &mut self.builder,
                    &api::QueryNotArgs {
                        clause: Some(value),
                    },
                )
                .as_union_value(),
            )
        } else {
            return Ok(api::QueryExpr::create(
                &mut self.builder,
                &api::QueryExprArgs::default(),
            ));
        };
        Ok(api::QueryExpr::create(
            &mut self.builder,
            &api::QueryExprArgs {
                expr_type,
                expr: Some(expr),
            },
        ))
    }

    fn search(
        &mut self,
        input: &JsonValue,
    ) -> StdResult<flatbuffers::WIPOffset<api::SearchRequest<'static>>, String> {
        let query = input
            .get("query")
            .and_then(JsonValue::as_str)
            .map(|v| self.text(v));
        let search_fields = input
            .get("search_fields")
            .map(|v| self.selection(v))
            .transpose()?;
        let filter = input.get("filter").map(|v| self.filter(v)).transpose()?;
        let sorts = input
            .get("sort")
            .and_then(JsonValue::as_array)
            .map(|v| {
                v.iter()
                    .map(|x| self.sort(x))
                    .collect::<StdResult<Vec<_>, _>>()
            })
            .transpose()?
            .map(|v| self.builder.create_vector(&v));
        let facets = input
            .get("facets")
            .and_then(JsonValue::as_array)
            .map(|v| {
                v.iter()
                    .map(|x| self.facet(x))
                    .collect::<StdResult<Vec<_>, _>>()
            })
            .transpose()?
            .map(|v| self.builder.create_vector(&v));
        let ranking = input.get("ranking").map(|v| self.ranking(v)).transpose()?;
        let limit = input
            .get("limit")
            .and_then(JsonValue::as_u64)
            .map(|v| api::OptionalUInt32::new(v as u32));
        let offset = input
            .get("offset")
            .and_then(JsonValue::as_u64)
            .map(|v| api::OptionalUInt32::new(v as u32));
        let projection = input
            .get("projection")
            .map(|v| self.projection(v))
            .transpose()?;
        let highlighting = input
            .get("highlighting")
            .map(|v| self.highlighting(Some(v)))
            .transpose()?;
        let language = input
            .get("language")
            .and_then(JsonValue::as_str)
            .map(|v| self.text(v));
        let query_expr = input
            .get("query_expr")
            .map(|v| self.query_expr(v))
            .transpose()?;
        Ok(api::SearchRequest::create(
            &mut self.builder,
            &api::SearchRequestArgs {
                query,
                search_fields,
                filter,
                sort: sorts,
                facets,
                ranking,
                limit: limit.as_ref(),
                offset: offset.as_ref(),
                projection,
                highlighting,
                language,
                query_expr,
            },
        ))
    }

    fn analyze(
        &mut self,
        input: &JsonValue,
    ) -> StdResult<flatbuffers::WIPOffset<api::AnalyzeRequest<'static>>, String> {
        let (target_type, target) = if input.get("field").is_some() {
            let field = input
                .get("field")
                .and_then(JsonValue::as_str)
                .map(|v| self.text(v));
            (
                api::AnalyzeTarget::AnalyzeFieldTarget,
                Some(
                    api::AnalyzeFieldTarget::create(
                        &mut self.builder,
                        &api::AnalyzeFieldTargetArgs { field },
                    )
                    .as_union_value(),
                ),
            )
        } else if input.get("analyzer").is_some() {
            (
                api::AnalyzeTarget::Analyzer,
                Some(self.analyzer(input.get("analyzer"))?.as_union_value()),
            )
        } else {
            (api::AnalyzeTarget::NONE, None)
        };
        let text = input
            .get("text")
            .and_then(JsonValue::as_str)
            .map(|v| self.text(v));
        let language = input
            .get("language")
            .and_then(JsonValue::as_str)
            .map(|v| self.text(v));
        Ok(api::AnalyzeRequest::create(
            &mut self.builder,
            &api::AnalyzeRequestArgs {
                target_type,
                target,
                text,
                language,
            },
        ))
    }

    fn patch_operation(
        &mut self,
        input: &JsonValue,
    ) -> StdResult<flatbuffers::WIPOffset<api::PatchOperation<'static>>, String> {
        let object = input
            .as_object()
            .ok_or("patch operation must be an object")?;
        if object.get("_union").and_then(JsonValue::as_str) == Some("NONE") {
            return Ok(api::PatchOperation::create(
                &mut self.builder,
                &api::PatchOperationArgs::default(),
            ));
        }
        if let Some(value) = object.get("set") {
            let field = value
                .get("field")
                .and_then(JsonValue::as_str)
                .map(|v| self.text(v));
            let value = value.get("value").map(|v| self.value(v)).transpose()?;
            let set = api::SetField::create(&mut self.builder, &api::SetFieldArgs { field, value });
            Ok(api::PatchOperation::create(
                &mut self.builder,
                &api::PatchOperationArgs {
                    operation_type: api::PatchOperationKind::SetField,
                    operation: Some(set.as_union_value()),
                },
            ))
        } else if let Some(value) = object.get("unset") {
            let field = value
                .get("field")
                .and_then(JsonValue::as_str)
                .map(|v| self.text(v));
            let unset = api::UnsetField::create(&mut self.builder, &api::UnsetFieldArgs { field });
            Ok(api::PatchOperation::create(
                &mut self.builder,
                &api::PatchOperationArgs {
                    operation_type: api::PatchOperationKind::UnsetField,
                    operation: Some(unset.as_union_value()),
                },
            ))
        } else {
            Ok(api::PatchOperation::create(
                &mut self.builder,
                &api::PatchOperationArgs::default(),
            ))
        }
    }
}

fn parse_float(value: &JsonValue) -> StdResult<f64, String> {
    match value {
        JsonValue::Number(value) => value.as_f64().ok_or("invalid float64".to_owned()),
        JsonValue::String(value) if value == "NaN" => Ok(f64::NAN),
        JsonValue::String(value) if value == "Infinity" => Ok(f64::INFINITY),
        JsonValue::String(value) if value == "-Infinity" => Ok(f64::NEG_INFINITY),
        _ => Err("invalid float64".to_owned()),
    }
}
