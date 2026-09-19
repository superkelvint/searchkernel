//! Corpus-runner API.

use searchkernel_ir::{
    analyzer_to_ir, api, error_to_api, field_to_ir, index_definition_to_ir, AnalyzeRequestIr,
    AnalyzerIdentity, ComparisonIr, ComparisonOp, DeleteRequestIr, DocumentIr, FieldIr, FieldKind,
    FieldValueIr, FilterIr, GetRequestIr, HighlightingIr, IndexContext, IndexDefinitionIr,
    InsertRequestIr, LanguageHint, PatchOperationIr, PatchRequestIr, PortableError, ProjectionIr,
    QueryIr, RangeIr, ScalarValueIr, SearchFieldIr, SearchRequestIr, SortIr, UpsertRequestIr,
    WriteRequestIr, WriteRequestRef,
};
use searchkernel_protocol::{protocol::v2 as transport, PROTOCOL_VERSION};
use serde_json::{json, Map, Value as JsonValue};
use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::result::Result as StdResult;

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
                   payload_type: transport::RequestPayload,
                   payload: flatbuffers::WIPOffset<flatbuffers::UnionWIPOffset>| {
        let envelope = transport::RequestEnvelope::create(
            &mut builder.builder,
            &transport::RequestEnvelopeArgs {
                protocol_version: PROTOCOL_VERSION,
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
         payload_type: transport::ResponsePayload,
         payload: flatbuffers::WIPOffset<flatbuffers::UnionWIPOffset>| {
            let envelope = transport::ResponseEnvelope::create(
                &mut builder.builder,
                &transport::ResponseEnvelopeArgs {
                    protocol_version: PROTOCOL_VERSION,
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
            let payload = transport::DescribeResult::create(
                &mut builder.builder,
                &transport::DescribeResultArgs { index: Some(index) },
            );
            response(
                &mut builder,
                transport::ResponsePayload::SearchKernel_V2_DescribeResult,
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
            let payload = transport::DescribeResult::create(
                &mut builder.builder,
                &transport::DescribeResultArgs { index: Some(index) },
            );
            response(
                &mut builder,
                transport::ResponsePayload::SearchKernel_V2_DescribeResult,
                payload.as_union_value(),
            )
        }
        "analyzer_to_ir" => {
            let payload = builder.analyze(input)?;
            request(
                &mut builder,
                transport::RequestPayload::SearchKernel_V2_AnalyzeRequest,
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
                transport::RequestPayload::SearchKernel_V2_InsertRequest,
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
                transport::RequestPayload::SearchKernel_V2_InsertRequest,
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
                transport::RequestPayload::SearchKernel_V2_SearchRequest,
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

    /// Regenerate checked-in internal IR snapshots for an explicitly approved
    /// corpus update. This is intentionally separate from `run` so normal
    /// verification never mutates acceptance artifacts.
    pub fn regenerate_goldens(&self) -> StdResult<ConformanceReport, RunnerError> {
        let manifest = read_json(&self.root.join("manifest.json"))?;
        if manifest.get("version").and_then(JsonValue::as_str) != Some("0.6.0")
            || manifest.get("snapshot_format").and_then(JsonValue::as_str)
                != Some("searchkernel-ir-internal/v2")
        {
            return Err(RunnerError::Case {
                id: "runner".to_owned(),
                message: "golden regeneration requires the v0.6 internal snapshot corpus"
                    .to_owned(),
            });
        }
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
            let case_path = self.root.join(required_string(entry, "case")?);
            let golden_path = self.root.join(required_string(entry, "golden")?);
            let case = read_json(&case_path)?;
            let actual = case_output(&self.root, &case).map_err(|message| RunnerError::Case {
                id: id.clone(),
                message,
            })?;
            let contents =
                serde_json::to_string_pretty(&actual).map_err(|source| RunnerError::Json {
                    path: golden_path.clone(),
                    source,
                })?;
            fs::write(&golden_path, format!("{contents}\n")).map_err(|source| RunnerError::Io {
                path: golden_path,
                source,
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
    let actual = case_output(root, case)?;
    if &actual != golden {
        return Err(format!(
            "golden mismatch\nexpected: {golden}\nactual:   {actual}"
        ));
    }
    Ok(())
}

fn case_output(root: &Path, case: &JsonValue) -> StdResult<JsonValue, String> {
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
    match execute(operation, &input, index_json.as_ref()) {
        Ok(value) => Ok(value),
        Err(ExecError::Portable(error)) => Ok(json!({"error": error_json(&error)})),
        Err(error) => Err(error.to_string()),
    }
}

fn execute(
    operation: &str,
    input: &JsonValue,
    index_json: Option<&JsonValue>,
) -> StdResult<JsonValue, ExecError> {
    let bytes = if matches!(operation, "describe" | "inspect_query" | "facet_to_ir") {
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
        "filters_to_ir" => {
            let context = context.ok_or("operation requires index fixture")?;
            let root = flatbuffers::root::<api::SearchRequest>(&bytes)
                .map_err(|error| error.to_string())?;
            let request = context.search_to_ir(root).map_err(ExecError::Portable)?;
            ok(request.filter().map(filter_json).unwrap_or(JsonValue::Null))
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
            let context = context.ok_or("facet_to_ir requires index fixture")?;
            ok(facet_from_json(input, &context).map_err(ExecError::Portable)?)
        }
        "ranking_to_ir" => {
            let context = context.ok_or("operation requires index fixture")?;
            let root = flatbuffers::root::<api::RankingRequest>(&bytes)
                .map_err(|error| error.to_string())?;
            let ranking = context
                .ranking_request_to_ir(Some(root))
                .map_err(ExecError::Portable)?;
            ok(json!({"profile": ranking.profile, "properties": ranking.properties}))
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
            let mut output = search_json(&context.search_to_ir(root).map_err(ExecError::Portable)?);
            if let Some(items) = input.get("facets").and_then(JsonValue::as_array) {
                let mut seen = std::collections::BTreeSet::new();
                let facets = items
                    .iter()
                    .enumerate()
                    .map(|(index, item)| {
                        let facet = facet_from_json_at(item, &context, &format!("facets[{index}]"))
                            .map_err(ExecError::Portable)?;
                        let field = facet
                            .get("field")
                            .and_then(JsonValue::as_str)
                            .unwrap_or_default();
                        if !seen.insert(field.to_owned()) {
                            return Err(ExecError::Portable(PortableError::invalid(
                                "duplicate_facet_field",
                                Some(format!("facets[{index}].field")),
                                "facet fields must be unique",
                            )));
                        }
                        Ok(facet)
                    })
                    .collect::<StdResult<Vec<_>, ExecError>>()?;
                output["facets"] = JsonValue::Array(facets);
            }
            ok(output)
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
        FieldValueIr::Tensor(value) => {
            let payload = match &value.payload {
                searchkernel_ir::RequestTensorPayloadIr::Dense(values) => json!({"dense": values}),
                searchkernel_ir::RequestTensorPayloadIr::Sparse(values) => json!({
                    "sparse": values.iter().map(|cell| json!({
                        "address": cell.address,
                        "value": cell.value,
                    })).collect::<Vec<_>>()
                }),
                searchkernel_ir::RequestTensorPayloadIr::Mixed(values) => json!({
                    "mixed": values.iter().map(|block| json!({
                        "address": block.address,
                        "values": block.values,
                    })).collect::<Vec<_>>()
                }),
            };
            json!({"tensor":{"type_spec":value.type_spec,"payload":payload}})
        }
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
fn analyzer_name(value: Option<AnalyzerIdentity>) -> Option<&'static str> {
    value.map(|value| match value {
        AnalyzerIdentity::StandardV1 => "standard_v1",
    })
}
fn field_json(value: &FieldIr) -> JsonValue {
    searchkernel_ir::snapshot::field(value)
}
fn index_json_value(value: &IndexDefinitionIr) -> JsonValue {
    searchkernel_ir::snapshot::index_definition(value)
}
fn describe_json(value: &IndexDefinitionIr) -> JsonValue {
    json!({"index":searchkernel_ir::snapshot::index_definition(value)})
}
fn inspect_json(value: &SearchRequestIr) -> JsonValue {
    json!({"resolved_request":searchkernel_ir::snapshot::search_request(value)})
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
    json!({"document":document_json(value.document()),"condition":value.condition()})
}
fn delete_json(value: &DeleteRequestIr) -> JsonValue {
    json!({"id":value.id().as_str(),"condition":value.condition()})
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
    fn bound_json(value: &searchkernel_ir::FilterRangeBoundIr) -> JsonValue {
        match value {
            searchkernel_ir::FilterRangeBoundIr::Finite(value) => scalar_json(value),
            searchkernel_ir::FilterRangeBoundIr::NegativeInfinity => {
                json!("negative_infinity")
            }
            searchkernel_ir::FilterRangeBoundIr::PositiveInfinity => {
                json!("positive_infinity")
            }
            searchkernel_ir::FilterRangeBoundIr::Unbounded => JsonValue::Null,
        }
    }
    json!({"field":value.field().as_str(),"lower":bound_json(value.lower_bound()),"upper":bound_json(value.upper_bound()),"include_lower":value.include_lower(),"include_upper":value.include_upper()})
}
fn filter_json(value: &FilterIr) -> JsonValue {
    searchkernel_ir::snapshot::filter(value)
}
fn query_json(value: &QueryIr) -> JsonValue {
    searchkernel_ir::snapshot::query(value)
}

fn sort_json(value: &SortIr) -> JsonValue {
    searchkernel_ir::snapshot::sort(value)
}
fn facet_from_json(
    input: &JsonValue,
    context: &IndexContext,
) -> StdResult<JsonValue, PortableError> {
    facet_from_json_at(input, context, "facet")
}

fn facet_from_json_at(
    input: &JsonValue,
    context: &IndexContext,
    path: &str,
) -> StdResult<JsonValue, PortableError> {
    let field_name = input
        .get("field")
        .and_then(JsonValue::as_str)
        .unwrap_or_default();
    let field = context
        .definition()
        .fields()
        .iter()
        .find(|field| field.name().as_str() == field_name)
        .ok_or_else(|| {
            PortableError::invalid(
                "unknown_field",
                Some(format!("{path}.field")),
                "facet field does not exist",
            )
        })?;
    if !field.capabilities().facetable() {
        return Err(PortableError::invalid(
            "field_not_facetable",
            Some(format!("{path}.field")),
            "field is not facetable",
        ));
    }
    let sort = match input.get("sort").and_then(JsonValue::as_str) {
        None => "count_desc",
        Some("count_desc") => "count_desc",
        Some("count_asc") => "count_asc",
        Some("value_asc") => "value_asc",
        Some("value_desc") => "value_desc",
        Some(_) => {
            return Err(PortableError::invalid(
                "invalid_facet",
                Some(format!("{path}.sort")),
                "facet sort is unknown",
            ))
        }
    };
    let limit = input.get("limit").and_then(JsonValue::as_u64).unwrap_or(10);
    let limit = u32::try_from(limit).map_err(|_| {
        PortableError::invalid(
            "invalid_facet",
            Some(format!("{path}.limit")),
            "facet limit is out of range",
        )
    })?;
    Ok(json!({"field": field.name().as_str(), "limit": limit, "sort": sort}))
}
fn analyze_json(value: &AnalyzeRequestIr) -> JsonValue {
    json!({"analyzer":analyzer_name(Some(value.analyzer())),"text":value.text(),"language":value.language().map(LanguageHint::as_str)})
}
fn search_json(value: &SearchRequestIr) -> JsonValue {
    searchkernel_ir::snapshot::search_request(value)
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

    fn query_field(&mut self, value: &str) -> flatbuffers::WIPOffset<api::QueryFieldRef<'static>> {
        let path = self.text(value);
        api::QueryFieldRef::create(
            &mut self.builder,
            &api::QueryFieldRefArgs {
                path: Some(path),
                element_index: None,
            },
        )
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
            "filters_to_ir" => {
                let root = self.search(input)?;
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
            "facet_to_ir" => Err("facet_to_ir is handled directly by the corpus runner".to_owned()),
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
                    let condition = input
                        .get("condition")
                        .and_then(JsonValue::as_str)
                        .map(|value| self.text(value));
                    let root = api::UpsertRequest::create(
                        &mut self.builder,
                        &api::UpsertRequestArgs {
                            document: Some(document),
                            condition,
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
                let condition = input
                    .get("condition")
                    .and_then(JsonValue::as_str)
                    .map(|value| self.text(value));
                let root = api::DeleteRequest::create(
                    &mut self.builder,
                    &api::DeleteRequestArgs { id, condition },
                );
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
                        let condition = request
                            .get("condition")
                            .and_then(JsonValue::as_str)
                            .map(|value| self.text(value));
                        let root = api::UpsertRequest::create(
                            &mut self.builder,
                            &api::UpsertRequestArgs {
                                document: Some(document),
                                condition,
                            },
                        );
                        self.finish(root)
                    }
                    "delete" => {
                        let id = request
                            .get("id")
                            .and_then(JsonValue::as_str)
                            .map(|v| self.text(v));
                        let condition = request
                            .get("condition")
                            .and_then(JsonValue::as_str)
                            .map(|value| self.text(value));
                        let root = api::DeleteRequest::create(
                            &mut self.builder,
                            &api::DeleteRequestArgs { id, condition },
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
        let kind = input
            .get("kind")
            .and_then(JsonValue::as_str)
            .unwrap_or("string");
        let matching_mode = match kind {
            "keyword" => api::MatchingMode::Exact,
            _ => api::MatchingMode::Text,
        };
        let kind = match kind {
            "int64" => api::PrimitiveDataTypeKind::Int64,
            "float64" => api::PrimitiveDataTypeKind::Float64,
            "bool" => api::PrimitiveDataTypeKind::Bool,
            _ => api::PrimitiveDataTypeKind::String,
        };
        let is_string = matches!(kind, api::PrimitiveDataTypeKind::String);
        let primitive =
            api::PrimitiveDataType::create(&mut self.builder, &api::PrimitiveDataTypeArgs { kind });
        let element_type = api::DataType::create(
            &mut self.builder,
            &api::DataTypeArgs {
                kind_type: api::DataTypeKind::PrimitiveDataType,
                kind: Some(primitive.as_union_value()),
            },
        );
        let data_type = if input.get("cardinality").and_then(JsonValue::as_str) == Some("multi") {
            let array = api::ArrayDataType::create(
                &mut self.builder,
                &api::ArrayDataTypeArgs {
                    element_type: Some(element_type),
                },
            );
            api::DataType::create(
                &mut self.builder,
                &api::DataTypeArgs {
                    kind_type: api::DataTypeKind::ArrayDataType,
                    kind: Some(array.as_union_value()),
                },
            )
        } else {
            element_type
        };
        let matching = is_string.then(|| {
            api::MatchingConfig::create(
                &mut self.builder,
                &api::MatchingConfigArgs {
                    mode: matching_mode,
                    options: None,
                },
            )
        });
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
        let indexing_script = input
            .get("indexing_script")
            .and_then(JsonValue::as_str)
            .map(|value| self.text(value));
        let properties = input
            .get("properties")
            .map(|value| self.entries(Some(value)))
            .transpose()?;
        let name = input
            .get("name")
            .and_then(JsonValue::as_str)
            .map(|v| self.text(v));
        Ok(api::FieldDefinition::create(
            &mut self.builder,
            &api::FieldDefinitionArgs {
                name,
                data_type: Some(data_type),
                stored: stored.as_ref(),
                searchable: searchable.as_ref(),
                filterable: filterable.as_ref(),
                sortable: sortable.as_ref(),
                facetable: facetable.as_ref(),
                analyzer,
                matching,
                indexing_script,
                properties,
                ..Default::default()
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
        let properties = input
            .get("properties")
            .map(|value| self.entries(Some(value)))
            .transpose()?;
        let document_type = input
            .get("document_type")
            .and_then(JsonValue::as_str)
            .map(|value| self.text(value));
        let schema_inherits = input
            .get("schema_inherits")
            .map(|value| self.string_vector(value))
            .transpose()?;
        let document_inherits = input
            .get("document_inherits")
            .map(|value| self.string_vector(value))
            .transpose()?;
        let schema_fields = input
            .get("schema_fields")
            .map(|value| self.schema_fields(value))
            .transpose()?
            .map(|value| self.builder.create_vector(&value));
        let fieldsets = input
            .get("fieldsets")
            .map(|value| self.fieldsets(value))
            .transpose()?
            .map(|value| self.builder.create_vector(&value));
        Ok(api::IndexDefinition::create(
            &mut self.builder,
            &api::IndexDefinitionArgs {
                name,
                fields,
                default_search_fields,
                properties,
                document_type,
                schema_inherits,
                document_inherits,
                fieldsets,
                schema_fields,
                ..Default::default()
            },
        ))
    }

    fn schema_fields(
        &mut self,
        input: &JsonValue,
    ) -> StdResult<Vec<flatbuffers::WIPOffset<api::SearchOnlyFieldDefinition<'static>>>, String>
    {
        let values = input.as_array().ok_or("schema_fields must be an array")?;
        values
            .iter()
            .map(|value| {
                let field = value.get("field").unwrap_or(value);
                let field = self.field(field)?;
                Ok(api::SearchOnlyFieldDefinition::create(
                    &mut self.builder,
                    &api::SearchOnlyFieldDefinitionArgs { field: Some(field) },
                ))
            })
            .collect()
    }

    fn fieldsets(
        &mut self,
        input: &JsonValue,
    ) -> StdResult<Vec<flatbuffers::WIPOffset<api::FieldSetDefinition<'static>>>, String> {
        let values = input.as_array().ok_or("fieldsets must be an array")?;
        values
            .iter()
            .map(|value| {
                let value = value.as_object().ok_or("fieldset must be an object")?;
                let name = value
                    .get("name")
                    .and_then(JsonValue::as_str)
                    .ok_or("fieldset name is required")?;
                let fields = value
                    .get("fields")
                    .map(|value| self.string_vector(value))
                    .transpose()?;
                let query_commands = value
                    .get("query_commands")
                    .map(|value| self.string_vector(value))
                    .transpose()?;
                let properties = value
                    .get("properties")
                    .map(|value| self.entries(Some(value)))
                    .transpose()?;
                let name = self.text(name);
                Ok(api::FieldSetDefinition::create(
                    &mut self.builder,
                    &api::FieldSetDefinitionArgs {
                        name: Some(name),
                        fields,
                        matching: None,
                        query_commands,
                        properties,
                    },
                ))
            })
            .collect()
    }

    fn comparison(
        &mut self,
        input: &JsonValue,
    ) -> StdResult<flatbuffers::WIPOffset<api::FilterComparison<'static>>, String> {
        let field = input
            .get("field")
            .and_then(JsonValue::as_str)
            .map(|v| self.query_field(v));
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
            .map(|v| self.query_field(v));
        let lower = input
            .get("lower")
            .map(|v| self.range_bound(v))
            .transpose()?;
        let upper = input
            .get("upper")
            .map(|v| self.range_bound(v))
            .transpose()?;
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

    fn range_bound(
        &mut self,
        input: &JsonValue,
    ) -> StdResult<flatbuffers::WIPOffset<api::RangeBound<'static>>, String> {
        let value = self.value(input)?;
        let finite = api::FiniteRangeBound::create(
            &mut self.builder,
            &api::FiniteRangeBoundArgs { value: Some(value) },
        );
        Ok(api::RangeBound::create(
            &mut self.builder,
            &api::RangeBoundArgs {
                bound_type: api::RangeBoundKind::FiniteRangeBound,
                bound: Some(finite.as_union_value()),
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
                    .map(|v| self.query_field(v));
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
            let (policy_type, policy) = match v {
                "first" => {
                    let policy =
                        api::MissingFirst::create(&mut self.builder, &api::MissingFirstArgs {});
                    (
                        api::MissingPolicyKind::MissingFirst,
                        Some(policy.as_union_value()),
                    )
                }
                "last" => {
                    let policy =
                        api::MissingLast::create(&mut self.builder, &api::MissingLastArgs {});
                    (
                        api::MissingPolicyKind::MissingLast,
                        Some(policy.as_union_value()),
                    )
                }
                "unspecified" => (api::MissingPolicyKind(0), None),
                _ => (api::MissingPolicyKind(0), None),
            };
            api::MissingPolicy::create(
                &mut self.builder,
                &api::MissingPolicyArgs {
                    policy_type,
                    policy,
                },
            )
        });
        let target = field.map(|field| {
            let field = api::FieldSortTarget::create(
                &mut self.builder,
                &api::FieldSortTargetArgs { field: Some(field) },
            );
            api::SortTarget::create(
                &mut self.builder,
                &api::SortTargetArgs {
                    target_type: api::SortTargetKind::FieldSortTarget,
                    target: Some(field.as_union_value()),
                },
            )
        });
        Ok(api::SortExpr::create(
            &mut self.builder,
            &api::SortExprArgs {
                target,
                direction,
                missing,
                annotations: None,
            },
        ))
    }

    fn ranking(
        &mut self,
        input: &JsonValue,
    ) -> StdResult<flatbuffers::WIPOffset<api::RankingRequest<'static>>, String> {
        let profile = if input.get("_union").and_then(JsonValue::as_str) == Some("NONE") {
            None
        } else {
            input
                .get("profile")
                .and_then(JsonValue::as_str)
                .or_else(|| input.get("bm25").map(|_| "bm25"))
                .or(Some("default"))
        };
        let profile = profile.map(|value| self.text(value));
        let properties = input
            .get("properties")
            .map(|value| self.string_entries(value))
            .transpose()?;
        Ok(api::RankingRequest::create(
            &mut self.builder,
            &api::RankingRequestArgs {
                profile,
                properties,
            },
        ))
    }

    fn string_entries(
        &mut self,
        input: &JsonValue,
    ) -> StdResult<
        flatbuffers::WIPOffset<
            flatbuffers::Vector<'static, flatbuffers::ForwardsUOffset<api::StringEntry<'static>>>,
        >,
        String,
    > {
        let mut result = Vec::new();
        if let Some(values) = input.as_object() {
            for (key, value) in values {
                let key = self.text(key);
                let value = value.as_str().ok_or("string map values must be strings")?;
                let value = self.text(value);
                result.push(api::StringEntry::create(
                    &mut self.builder,
                    &api::StringEntryArgs {
                        key: Some(key),
                        value: Some(value),
                    },
                ));
            }
        }
        Ok(self.builder.create_vector(&result))
    }

    fn query_inputs(
        &mut self,
        input: &JsonValue,
    ) -> StdResult<Vec<flatbuffers::WIPOffset<api::QueryInput<'static>>>, String> {
        let values = input.as_object().ok_or("inputs must be an object")?;
        values
            .iter()
            .map(|(name, value)| {
                let (input_type, input) =
                    if let Some(expression) = value.get("expression").and_then(JsonValue::as_str) {
                        let expression = self.text(expression);
                        let expression = api::ExpressionQueryInput::create(
                            &mut self.builder,
                            &api::ExpressionQueryInputArgs {
                                expression: Some(expression),
                            },
                        );
                        (
                            api::QueryInputKind::ExpressionQueryInput,
                            expression.as_union_value(),
                        )
                    } else {
                        let value = self.value(value)?;
                        let literal = api::LiteralQueryInput::create(
                            &mut self.builder,
                            &api::LiteralQueryInputArgs { value: Some(value) },
                        );
                        (
                            api::QueryInputKind::LiteralQueryInput,
                            literal.as_union_value(),
                        )
                    };
                let name = self.text(name);
                Ok(api::QueryInput::create(
                    &mut self.builder,
                    &api::QueryInputArgs {
                        name: Some(name),
                        input_type,
                        input: Some(input),
                    },
                ))
            })
            .collect()
    }

    fn query_model(
        &mut self,
        input: &JsonValue,
    ) -> StdResult<flatbuffers::WIPOffset<api::QueryModel<'static>>, String> {
        let input = input.as_object().ok_or("query_model must be an object")?;
        let get_text = |this: &mut Self, key: &str| {
            input
                .get(key)
                .and_then(JsonValue::as_str)
                .map(|value| this.text(value))
        };
        let query_type = if let Some(value) = input.get("type") {
            let value = value
                .as_object()
                .ok_or("query_model.type must be an object")?;
            let is_yql_default = value
                .get("is_yql_default")
                .and_then(JsonValue::as_bool)
                .map(api::OptionalBool::new);
            let composite = value
                .get("composite")
                .and_then(JsonValue::as_str)
                .map(|v| self.text(v));
            let tokenization = value
                .get("tokenization")
                .and_then(JsonValue::as_str)
                .map(|v| self.text(v));
            let syntax = value
                .get("syntax")
                .and_then(JsonValue::as_str)
                .map(|v| self.text(v));
            let profile = value
                .get("profile")
                .and_then(JsonValue::as_str)
                .map(|v| self.text(v));
            Some(api::QueryModelType::create(
                &mut self.builder,
                &api::QueryModelTypeArgs {
                    composite,
                    tokenization,
                    syntax,
                    profile,
                    is_yql_default: is_yql_default.as_ref(),
                },
            ))
        } else {
            None
        };
        let sources = input
            .get("sources")
            .map(|value| self.string_vector(value))
            .transpose()?;
        let restrict = input
            .get("restrict")
            .map(|value| self.string_vector(value))
            .transpose()?;
        let search_group = input
            .get("search_group")
            .and_then(JsonValue::as_u64)
            .map(|value| api::OptionalUInt32::new(value as u32));
        let query_string = get_text(self, "query_string");
        let default_index = get_text(self, "default_index");
        let encoding = get_text(self, "encoding");
        let filter = get_text(self, "filter");
        let language = get_text(self, "language");
        let locale = get_text(self, "locale");
        let search_path = get_text(self, "search_path");
        Ok(api::QueryModel::create(
            &mut self.builder,
            &api::QueryModelArgs {
                query_string,
                default_index,
                encoding,
                filter,
                language,
                locale,
                type_: query_type,
                sources,
                restrict,
                search_path,
                search_group: search_group.as_ref(),
            },
        ))
    }

    fn string_vector(
        &mut self,
        input: &JsonValue,
    ) -> StdResult<
        flatbuffers::WIPOffset<
            flatbuffers::Vector<'static, flatbuffers::ForwardsUOffset<&'static str>>,
        >,
        String,
    > {
        let values = input.as_array().ok_or("string vector must be an array")?;
        let values = values
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .map(|value| self.text(value))
                    .ok_or("string vector values must be strings")
            })
            .collect::<StdResult<Vec<_>, _>>()?;
        Ok(self.builder.create_vector(&values))
    }

    fn presentation(
        &mut self,
        input: &JsonValue,
        fallback_projection: Option<flatbuffers::WIPOffset<api::FieldProjection<'static>>>,
        fallback_highlighting: Option<flatbuffers::WIPOffset<api::HighlightRequest<'static>>>,
    ) -> StdResult<flatbuffers::WIPOffset<api::PresentationRequest<'static>>, String> {
        let input = input.as_object().ok_or("presentation must be an object")?;
        let fields = input
            .get("fields")
            .map(|value| self.projection(value))
            .transpose()?
            .or(fallback_projection);
        let highlighting = input
            .get("highlighting")
            .map(|value| self.highlighting(Some(value)))
            .transpose()?
            .or(fallback_highlighting);
        let bolding = input
            .get("bolding")
            .and_then(JsonValue::as_bool)
            .map(api::OptionalBool::new);
        let timing = input
            .get("timing")
            .and_then(JsonValue::as_bool)
            .map(api::OptionalBool::new);
        let summary = input
            .get("summary")
            .and_then(JsonValue::as_str)
            .map(|value| self.text(value));
        let format = input
            .get("format")
            .and_then(JsonValue::as_str)
            .map(|value| self.text(value));
        Ok(api::PresentationRequest::create(
            &mut self.builder,
            &api::PresentationRequestArgs {
                summary,
                fields,
                highlighting,
                format,
                bolding: bolding.as_ref(),
                timing: timing.as_ref(),
            },
        ))
    }

    fn grouping_requests(
        &mut self,
        input: &JsonValue,
    ) -> StdResult<Vec<flatbuffers::WIPOffset<api::GroupingRequest<'static>>>, String> {
        let values = input.as_array().ok_or("grouping must be an array")?;
        values
            .iter()
            .map(|value| self.grouping_request(value))
            .collect()
    }

    fn grouping_request(
        &mut self,
        input: &JsonValue,
    ) -> StdResult<flatbuffers::WIPOffset<api::GroupingRequest<'static>>, String> {
        let input = input
            .as_object()
            .ok_or("grouping request must be an object")?;
        let operations = input
            .get("operations")
            .or_else(|| input.get("root").and_then(|root| root.get("operations")))
            .ok_or("grouping request requires operations")?;
        let root = self.grouping_all(operations, input.get("label"))?;
        let source = api::StructuredGroupingSource::create(
            &mut self.builder,
            &api::StructuredGroupingSourceArgs { root: Some(root) },
        );
        let continuations = input
            .get("continuations")
            .map(|value| self.string_vector(value))
            .transpose()?;
        Ok(api::GroupingRequest::create(
            &mut self.builder,
            &api::GroupingRequestArgs {
                source_type: api::GroupingSourceKind::StructuredGroupingSource,
                source: Some(source.as_union_value()),
                continuations,
            },
        ))
    }

    fn grouping_all(
        &mut self,
        input: &JsonValue,
        label: Option<&JsonValue>,
    ) -> StdResult<flatbuffers::WIPOffset<api::GroupingAllOperation<'static>>, String> {
        let values = input
            .as_array()
            .ok_or("grouping operations must be an array")?;
        let operations = values
            .iter()
            .map(|value| self.grouping_operation(value))
            .collect::<StdResult<Vec<_>, _>>()?;
        let operations = self.builder.create_vector(&operations);
        let label = label
            .and_then(JsonValue::as_str)
            .map(|value| self.text(value));
        Ok(api::GroupingAllOperation::create(
            &mut self.builder,
            &api::GroupingAllOperationArgs {
                operations: Some(operations),
                label,
            },
        ))
    }

    fn grouping_operation(
        &mut self,
        input: &JsonValue,
    ) -> StdResult<flatbuffers::WIPOffset<api::GroupingOperation<'static>>, String> {
        let input = input
            .as_object()
            .ok_or("grouping operation must be an object")?;
        let (operation_type, operation) = if let Some(value) = input.get("all") {
            (
                api::GroupingOperationKind::GroupingAllOperation,
                self.grouping_all(value.get("operations").unwrap_or(value), value.get("label"))?
                    .as_union_value(),
            )
        } else if let Some(value) = input.get("each") {
            let value = value.as_object().ok_or("grouping each must be an object")?;
            let operations = self.grouping_operations_vector(
                value
                    .get("operations")
                    .ok_or("grouping each requires operations")?,
            )?;
            let label = value
                .get("label")
                .and_then(JsonValue::as_str)
                .map(|v| self.text(v));
            (
                api::GroupingOperationKind::GroupingEachOperation,
                api::GroupingEachOperation::create(
                    &mut self.builder,
                    &api::GroupingEachOperationArgs {
                        operations: Some(operations),
                        label,
                    },
                )
                .as_union_value(),
            )
        } else if let Some(value) = input.get("group") {
            let expression = self.grouping_expression(value)?;
            (
                api::GroupingOperationKind::GroupingGroupOperation,
                api::GroupingGroupOperation::create(
                    &mut self.builder,
                    &api::GroupingGroupOperationArgs {
                        expression: Some(expression),
                    },
                )
                .as_union_value(),
            )
        } else if let Some(value) = input.get("max") {
            let value = value.as_u64().ok_or("grouping max must be an integer")? as u32;
            let max = api::FiniteGroupingMax::create(
                &mut self.builder,
                &api::FiniteGroupingMaxArgs { value },
            );
            (
                api::GroupingOperationKind::GroupingMaxOperation,
                api::GroupingMaxOperation::create(
                    &mut self.builder,
                    &api::GroupingMaxOperationArgs {
                        value_type: api::GroupingMaxKind::FiniteGroupingMax,
                        value: Some(max.as_union_value()),
                    },
                )
                .as_union_value(),
            )
        } else if let Some(value) = input.get("output") {
            let values = value.as_array().ok_or("grouping output must be an array")?;
            let outputs = values
                .iter()
                .map(|value| self.grouping_output(value))
                .collect::<StdResult<Vec<_>, _>>()?;
            let outputs = self.builder.create_vector(&outputs);
            (
                api::GroupingOperationKind::GroupingOutputOperation,
                api::GroupingOutputOperation::create(
                    &mut self.builder,
                    &api::GroupingOutputOperationArgs {
                        outputs: Some(outputs),
                    },
                )
                .as_union_value(),
            )
        } else {
            return Err("unsupported grouping operation builder shape".to_owned());
        };
        Ok(api::GroupingOperation::create(
            &mut self.builder,
            &api::GroupingOperationArgs {
                operation_type,
                operation: Some(operation),
            },
        ))
    }

    fn grouping_operations_vector(
        &mut self,
        input: &JsonValue,
    ) -> StdResult<
        flatbuffers::WIPOffset<
            flatbuffers::Vector<
                'static,
                flatbuffers::ForwardsUOffset<api::GroupingOperation<'static>>,
            >,
        >,
        String,
    > {
        let values = input
            .as_array()
            .ok_or("grouping operations must be an array")?;
        let values = values
            .iter()
            .map(|value| self.grouping_operation(value))
            .collect::<StdResult<Vec<_>, _>>()?;
        Ok(self.builder.create_vector(&values))
    }

    fn grouping_output(
        &mut self,
        input: &JsonValue,
    ) -> StdResult<flatbuffers::WIPOffset<api::GroupingOutput<'static>>, String> {
        let input = input
            .as_object()
            .ok_or("grouping output must be an object")?;
        let expression = input
            .get("expression")
            .or_else(|| input.get("aggregation"))
            .ok_or("grouping output requires expression")?;
        let expression = self.grouping_expression(expression)?;
        let label = input
            .get("label")
            .and_then(JsonValue::as_str)
            .map(|v| self.text(v));
        Ok(api::GroupingOutput::create(
            &mut self.builder,
            &api::GroupingOutputArgs {
                label,
                expression: Some(expression),
            },
        ))
    }

    fn grouping_expression(
        &mut self,
        input: &JsonValue,
    ) -> StdResult<flatbuffers::WIPOffset<api::GroupingExpression<'static>>, String> {
        let input = input
            .as_object()
            .ok_or("grouping expression must be an object")?;
        let (expr_type, expr) = if let Some(value) = input.get("attribute") {
            let field = value
                .as_str()
                .ok_or("grouping attribute must be a string")?;
            let field = self.text(field);
            let value = api::GroupingAttributeExpression::create(
                &mut self.builder,
                &api::GroupingAttributeExpressionArgs { field: Some(field) },
            );
            (
                api::GroupingExpressionKind::GroupingAttributeExpression,
                value.as_union_value(),
            )
        } else if input.get("aggregation").and_then(JsonValue::as_str) == Some("count") {
            let count =
                api::CountAggregation::create(&mut self.builder, &api::CountAggregationArgs {});
            let aggregation = api::GroupingAggregationExpression::create(
                &mut self.builder,
                &api::GroupingAggregationExpressionArgs {
                    aggregation_type: api::AggregationKind::CountAggregation,
                    aggregation: Some(count.as_union_value()),
                },
            );
            (
                api::GroupingExpressionKind::GroupingAggregationExpression,
                aggregation.as_union_value(),
            )
        } else if let Some(value) = input.get("negative") {
            let expression = self.grouping_expression(value)?;
            let value = api::GroupingUnaryExpression::create(
                &mut self.builder,
                &api::GroupingUnaryExpressionArgs {
                    op: api::GroupingUnaryOperator::Negative,
                    expression: Some(expression),
                },
            );
            (
                api::GroupingExpressionKind::GroupingUnaryExpression,
                value.as_union_value(),
            )
        } else {
            return Err("unsupported grouping expression builder shape".to_owned());
        };
        Ok(api::GroupingExpression::create(
            &mut self.builder,
            &api::GroupingExpressionArgs {
                expr_type,
                expr: Some(expr),
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
                .ok_or("query term requires a field")?;
            let field = self.query_field(field);
            let value = value.get("value").map(|v| self.value(v)).transpose()?;
            let term = api::TermQuery::create(&mut self.builder, &api::TermQueryArgs { value });
            let item = api::QueryExpr::create(
                &mut self.builder,
                &api::QueryExprArgs {
                    expr_type: api::QueryExprKind::TermQuery,
                    expr: Some(term.as_union_value()),
                    annotations: None,
                },
            );
            (
                api::QueryExprKind::QueryContains,
                api::QueryContains::create(
                    &mut self.builder,
                    &api::QueryContainsArgs {
                        field: Some(field),
                        item: Some(item),
                    },
                )
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
                annotations: None,
            },
        ))
    }

    fn filter_clause(
        &mut self,
        input: &JsonValue,
    ) -> StdResult<flatbuffers::WIPOffset<api::FilterClause<'static>>, String> {
        let source = input
            .get("source")
            .and_then(JsonValue::as_str)
            .map(|value| self.text(value));
        let bindings = input
            .get("bindings")
            .map(|value| self.entries(Some(value)))
            .transpose()?;
        Ok(api::FilterClause::create(
            &mut self.builder,
            &api::FilterClauseArgs { source, bindings },
        ))
    }

    fn search(
        &mut self,
        input: &JsonValue,
    ) -> StdResult<flatbuffers::WIPOffset<api::SearchRequest<'static>>, String> {
        let query_text = input
            .get("query")
            .and_then(JsonValue::as_str)
            .map(|v| self.text(v));
        let query_expr = input
            .get("query_expr")
            .map(|v| self.query_expr(v))
            .transpose()?;
        let query_source = if let Some(query_expr) = query_expr {
            let typed = api::TypedQuery::create(
                &mut self.builder,
                &api::TypedQueryArgs {
                    query: Some(query_expr),
                },
            );
            Some(api::QuerySource::create(
                &mut self.builder,
                &api::QuerySourceArgs {
                    source_type: api::QuerySourceKind::TypedQuery,
                    source: Some(typed.as_union_value()),
                },
            ))
        } else if let Some(text) = query_text {
            let source = api::SearchKernelTextQuery::create(
                &mut self.builder,
                &api::SearchKernelTextQueryArgs { text: Some(text) },
            );
            Some(api::QuerySource::create(
                &mut self.builder,
                &api::QuerySourceArgs {
                    source_type: api::QuerySourceKind::SearchKernelTextQuery,
                    source: Some(source.as_union_value()),
                },
            ))
        } else {
            // A nullable legacy `query` is the wire spelling of an empty text
            // query. Keep the text-source provenance so the IR applies the
            // same default search-field selection as an explicit empty query.
            let text = self.text("");
            let source = api::SearchKernelTextQuery::create(
                &mut self.builder,
                &api::SearchKernelTextQueryArgs { text: Some(text) },
            );
            Some(api::QuerySource::create(
                &mut self.builder,
                &api::QuerySourceArgs {
                    source_type: api::QuerySourceKind::SearchKernelTextQuery,
                    source: Some(source.as_union_value()),
                },
            ))
        };
        let search_fields = input
            .get("search_fields")
            .map(|v| self.selection(v))
            .transpose()?;
        let structured = input.get("filter").map(|v| self.filter(v)).transpose()?;
        let structured = structured.map(|filter| self.builder.create_vector(&[filter]));
        let textual = input
            .get("textual")
            .map(|values| {
                values
                    .as_array()
                    .ok_or("textual must be an array")?
                    .iter()
                    .map(|value| self.filter_clause(value))
                    .collect::<StdResult<Vec<_>, _>>()
            })
            .transpose()?
            .map(|values| self.builder.create_vector(&values));
        let legacy_sources = input
            .get("sources")
            .map(|values| {
                values
                    .as_array()
                    .ok_or("sources must be an array")?
                    .iter()
                    .map(|value| {
                        value
                            .as_str()
                            .map(|value| self.text(value))
                            .ok_or("source must be a string")
                    })
                    .collect::<StdResult<Vec<_>, _>>()
            })
            .transpose()?
            .map(|values| self.builder.create_vector(&values));
        let legacy_parameters = input
            .get("parameters")
            .map(|values| self.entries(Some(values)))
            .transpose()?;
        let filters = (structured.is_some()
            || textual.is_some()
            || legacy_sources.is_some()
            || legacy_parameters.is_some())
        .then(|| {
            api::FilterRequest::create(
                &mut self.builder,
                &api::FilterRequestArgs {
                    structured,
                    textual,
                    sources: legacy_sources,
                    parameters: legacy_parameters,
                },
            )
        });
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
        let ranking = input.get("ranking").map(|v| self.ranking(v)).transpose()?;
        let inputs = input
            .get("inputs")
            .map(|v| self.query_inputs(v))
            .transpose()?
            .map(|v| self.builder.create_vector(&v));
        let limit = input
            .get("limit")
            .and_then(JsonValue::as_u64)
            .map(|v| api::OptionalUInt32::new(v as u32));
        let offset = input
            .get("offset")
            .and_then(JsonValue::as_u64)
            .map(|v| api::OptionalUInt32::new(v as u32));
        let timeout = input
            .get("timeout")
            .and_then(JsonValue::as_u64)
            .map(api::OptionalUInt64::new);
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
        let query_model = input
            .get("query_model")
            .map(|value| self.query_model(value))
            .transpose()?;
        let query_model = query_model.or_else(|| {
            language.map(|language| {
                api::QueryModel::create(
                    &mut self.builder,
                    &api::QueryModelArgs {
                        language: Some(language),
                        ..Default::default()
                    },
                )
            })
        });
        let presentation = if let Some(value) = input.get("presentation") {
            Some(self.presentation(value, projection, highlighting)?)
        } else if projection.is_some() || highlighting.is_some() {
            Some(api::PresentationRequest::create(
                &mut self.builder,
                &api::PresentationRequestArgs {
                    fields: projection,
                    highlighting,
                    ..Default::default()
                },
            ))
        } else {
            None
        };
        let properties = input
            .get("properties")
            .map(|value| self.entries(Some(value)))
            .transpose()?;
        let grouping = input
            .get("grouping")
            .map(|value| self.grouping_requests(value))
            .transpose()?
            .map(|value| self.builder.create_vector(&value));
        let query_profile = input
            .get("query_profile")
            .and_then(JsonValue::as_str)
            .map(|value| self.text(value));
        let search_chain = input
            .get("search_chain")
            .and_then(JsonValue::as_str)
            .map(|value| self.text(value));
        Ok(api::SearchRequest::create(
            &mut self.builder,
            &api::SearchRequestArgs {
                query: query_source,
                search_fields,
                filters,
                sort: sorts,
                grouping,
                ranking,
                inputs,
                limit: limit.as_ref(),
                offset: offset.as_ref(),
                timeout_ms: timeout.as_ref(),
                query_model,
                presentation,
                properties,
                query_profile,
                search_chain,
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
