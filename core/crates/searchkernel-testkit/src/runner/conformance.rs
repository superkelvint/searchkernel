use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
/// The number of conformance cases that passed, grouped by category.
pub struct ConformanceReport {
    pub passed: usize,
    pub categories: Vec<(String, usize)>,
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
            wire_ir::index_context_from_api(root).map_err(ExecError::Portable)
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
            let ir = wire_ir::inspect_query(&context, root).map_err(ExecError::Portable)?;
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
                &wire_ir::value_to_ir(
                    &context,
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
                &wire_ir::document_to_ir(&context, root).map_err(ExecError::Portable)?,
            ))
        }
        "search_fields_to_ir" => {
            let context = context.ok_or("operation requires index fixture")?;
            let fields = if input.get("selection").is_some_and(|value| !value.is_null()) {
                let root = flatbuffers::root::<api::SearchFieldSelection>(&bytes)
                    .map_err(|error| error.to_string())?;
                wire_ir::search_fields_to_ir(&context, root).map_err(ExecError::Portable)?
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
                wire_ir::projection_to_ir(&context, Some(root)).map_err(ExecError::Portable)?
            } else {
                wire_ir::projection_to_ir(&context, None).map_err(ExecError::Portable)?
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
                wire_ir::highlighting_to_ir(&context, Some(root)).map_err(ExecError::Portable)?
            } else {
                wire_ir::highlighting_to_ir(&context, None).map_err(ExecError::Portable)?
            };
            ok(highlighting_json(&highlighting))
        }
        "comparison_to_ir" => {
            let context = context.ok_or("operation requires index fixture")?;
            let root = flatbuffers::root::<api::FilterComparison>(&bytes)
                .map_err(|error| error.to_string())?;
            ok(comparison_json(
                &wire_ir::comparison_to_ir(&context, root).map_err(ExecError::Portable)?,
            ))
        }
        "range_to_ir" => {
            let context = context.ok_or("operation requires index fixture")?;
            let root =
                flatbuffers::root::<api::FilterRange>(&bytes).map_err(|error| error.to_string())?;
            ok(range_json(
                &wire_ir::range_to_ir(&context, root).map_err(ExecError::Portable)?,
            ))
        }
        "filter_to_ir" => {
            let context = context.ok_or("operation requires index fixture")?;
            let root =
                flatbuffers::root::<api::FilterExpr>(&bytes).map_err(|error| error.to_string())?;
            ok(filter_json(
                &wire_ir::filter_to_ir(&context, root).map_err(ExecError::Portable)?,
            ))
        }
        "filters_to_ir" => {
            let context = context.ok_or("operation requires index fixture")?;
            let root = flatbuffers::root::<api::SearchRequest>(&bytes)
                .map_err(|error| error.to_string())?;
            let request = wire_ir::search_to_ir(&context, root).map_err(ExecError::Portable)?;
            ok(request.filter().map(filter_json).unwrap_or(JsonValue::Null))
        }
        "sort_to_ir" => {
            let context = context.ok_or("operation requires index fixture")?;
            let root =
                flatbuffers::root::<api::SortExpr>(&bytes).map_err(|error| error.to_string())?;
            ok(sort_json(
                &wire_ir::sort_to_ir(&context, root).map_err(ExecError::Portable)?,
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
            let ranking = wire_ir::ranking_request_to_ir(&context, Some(root))
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
                wire_ir::query_to_ir(
                    &context,
                    input
                        .get("query")
                        .and_then(JsonValue::as_str)
                        .unwrap_or_default(),
                    Some(root),
                )
                .map_err(ExecError::Portable)?
            } else {
                wire_ir::query_to_ir(
                    &context,
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
            let mut output =
                search_json(&wire_ir::search_to_ir(&context, root).map_err(ExecError::Portable)?);
            if let Some(items) = input.get("facets").and_then(JsonValue::as_array) {
                let parsed = items
                    .iter()
                    .enumerate()
                    .map(|(index, item)| {
                        facet_spec_from_json(item, &format!("facets[{index}]"))
                            .map_err(ExecError::Portable)
                    })
                    .collect::<StdResult<Vec<_>, ExecError>>()?;
                resolve_facets_at(
                    &context,
                    parsed.iter().map(|(facet, _)| facet.clone()).collect(),
                    "facets",
                )
                .map_err(ExecError::Portable)?;
                let facets = parsed.into_iter().map(|(_, json)| json).collect();
                output["facets"] = JsonValue::Array(facets);
            }
            ok(output)
        }
        "analyze_to_ir" => {
            let context = context.ok_or("operation requires index fixture")?;
            let root = flatbuffers::root::<api::AnalyzeRequest>(&bytes)
                .map_err(|error| error.to_string())?;
            ok(analyze_json(
                &wire_ir::analyze_to_ir(&context, root).map_err(ExecError::Portable)?,
            ))
        }
        "insert_to_ir" => {
            let context = context.ok_or("operation requires index fixture")?;
            let root = flatbuffers::root::<api::InsertRequest>(&bytes)
                .map_err(|error| error.to_string())?;
            ok(insert_json(
                &wire_ir::insert_to_ir(&context, root).map_err(ExecError::Portable)?,
            ))
        }
        "upsert_to_ir" => {
            let context = context.ok_or("operation requires index fixture")?;
            let root = flatbuffers::root::<api::UpsertRequest>(&bytes)
                .map_err(|error| error.to_string())?;
            ok(upsert_json(
                &wire_ir::upsert_to_ir(&context, root).map_err(ExecError::Portable)?,
            ))
        }
        "patch_to_ir" => {
            let context = context.ok_or("operation requires index fixture")?;
            let root = flatbuffers::root::<api::PatchRequest>(&bytes)
                .map_err(|error| error.to_string())?;
            ok(patch_json(
                &wire_ir::patch_to_ir(&context, root).map_err(ExecError::Portable)?,
            ))
        }
        "update_to_ir" => {
            let context = context.ok_or("operation requires index fixture")?;
            let root = flatbuffers::root::<api::UpdateRequest>(&bytes)
                .map_err(|error| error.to_string())?;
            ok(update_json(
                &wire_ir::update_to_ir(&context, root).map_err(ExecError::Portable)?,
            ))
        }
        "delete_to_ir" => {
            let context = context.ok_or("operation requires index fixture")?;
            let root = flatbuffers::root::<api::DeleteRequest>(&bytes)
                .map_err(|error| error.to_string())?;
            ok(delete_json(
                &wire_ir::delete_to_ir(&context, root).map_err(ExecError::Portable)?,
            ))
        }
        "get_to_ir" => {
            let context = context.ok_or("operation requires index fixture")?;
            let root =
                flatbuffers::root::<api::GetRequest>(&bytes).map_err(|error| error.to_string())?;
            ok(get_json(
                &wire_ir::get_to_ir(&context, root).map_err(ExecError::Portable)?,
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
                    wire_ir::write_to_ir(&context, WriteRequestRef::Insert(root))
                }
                "upsert" => {
                    let root = flatbuffers::root::<api::UpsertRequest>(&bytes)
                        .map_err(|error| error.to_string())?;
                    wire_ir::write_to_ir(&context, WriteRequestRef::Upsert(root))
                }
                "patch" => {
                    let root = flatbuffers::root::<api::PatchRequest>(&bytes)
                        .map_err(|error| error.to_string())?;
                    wire_ir::write_to_ir(&context, WriteRequestRef::Patch(root))
                }
                "update" => {
                    let root = flatbuffers::root::<api::UpdateRequest>(&bytes)
                        .map_err(|error| error.to_string())?;
                    wire_ir::write_to_ir(&context, WriteRequestRef::Update(root))
                }
                "delete" => {
                    let root = flatbuffers::root::<api::DeleteRequest>(&bytes)
                        .map_err(|error| error.to_string())?;
                    wire_ir::write_to_ir(&context, WriteRequestRef::Delete(root))
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

#[cfg(test)]
mod update_conformance_regression_tests {
    use super::*;

    #[test]
    fn frozen_update_presence_detects_false_to_absent_normalization() {
        let root =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../reference/api-conformance");
        let case = read_json(&root.join("cases/write/update-condition-create-false.json"))
            .expect("read frozen update case");
        let mut wrong_golden =
            read_json(&root.join("golden/write/update-condition-create-false.json"))
                .expect("read frozen update golden");
        wrong_golden["ok"]["create"] = JsonValue::Null;
        let error = run_case(&root, &case, &wrong_golden)
            .expect_err("normalizing present false to absent must fail conformance");
        assert!(error.contains("golden mismatch"));
    }
}
