use searchkernel::{
    field, Analyzer, Error, Facet, FacetSort, Field, Filter, IndexDefinition, IndexRuntime, Patch,
    Range, Ranking, SearchField, Sort,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Deserialize)]
struct Scenario {
    id: String,
    schema: SchemaSpec,
    cases: Vec<CaseSpec>,
}

#[derive(Deserialize)]
struct SchemaSpec {
    name: Option<String>,
    fields: Vec<FieldSpec>,
    default_search_fields: Option<Vec<SearchFieldSpec>>,
}

#[derive(Deserialize)]
struct FieldSpec {
    name: String,
    kind: String,
    #[serde(default)]
    multi: bool,
    stored: Option<bool>,
    searchable: Option<bool>,
    filterable: Option<bool>,
    sortable: Option<bool>,
    facetable: Option<bool>,
    analyzer: Option<String>,
}

#[derive(Deserialize)]
struct SearchFieldSpec {
    field: String,
    weight: Option<f64>,
}

#[derive(Deserialize)]
struct CaseSpec {
    id: String,
    op: String,
    #[serde(default)]
    args: Value,
}

fn main() {
    let scenario_path = PathBuf::from(std::env::args_os().nth(1).expect("scenario path"));
    let data_root = PathBuf::from(std::env::args_os().nth(2).expect("data root"));
    let scenario: Scenario =
        serde_json::from_slice(&std::fs::read(&scenario_path).expect("read scenario"))
            .expect("parse scenario");
    std::fs::create_dir_all(&data_root).expect("create data root");
    let output = run_scenario(&scenario, &data_root);
    println!(
        "{}",
        serde_json::to_string_pretty(&output).expect("serialize output")
    );
}

fn run_scenario(scenario: &Scenario, data_root: &Path) -> Value {
    let index_path = data_root.join(&scenario.id);
    let definition = build_definition(&scenario.schema);
    let mut runtime =
        Some(IndexRuntime::create(&index_path, definition).expect("create native index"));
    let mut results = Vec::with_capacity(scenario.cases.len());
    for case in &scenario.cases {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            execute_case(case, &mut runtime, &index_path)
        }))
        .unwrap_or_else(|panic| {
            json!({
                "ok": false,
                "panic": format_panic(panic),
            })
        });
        results.push(json!({ "id": case.id, "result": result }));
    }
    if let Some(runtime) = runtime.take() {
        let _ = runtime.close();
    }
    json!({ "scenario": scenario.id, "cases": results })
}

fn build_definition(spec: &SchemaSpec) -> IndexDefinition {
    let mut definition = IndexDefinition::new();
    if let Some(name) = &spec.name {
        definition = definition.name(name);
    }
    for field_spec in &spec.fields {
        let mut field = match field_spec.kind.as_str() {
            "text" => Field::text(&field_spec.name),
            "keyword" => Field::keyword(&field_spec.name),
            "int64" => Field::int64(&field_spec.name),
            "float64" => Field::float64(&field_spec.name),
            "bool" => Field::bool(&field_spec.name),
            other => panic!("unknown field kind {other}"),
        };
        if field_spec.multi {
            field = field.multi();
        }
        if let Some(value) = field_spec.stored {
            field = field.stored(value);
        }
        if let Some(value) = field_spec.searchable {
            field = field.searchable(value);
        }
        if let Some(value) = field_spec.filterable {
            field = field.filterable(value);
        }
        if let Some(value) = field_spec.sortable {
            field = field.sortable(value);
        }
        if let Some(value) = field_spec.facetable {
            field = field.facetable(value);
        }
        if field_spec.analyzer.as_deref() == Some("standard_v1") {
            field = field.analyzer(Analyzer::standard_v1());
        }
        definition = definition.field(field);
    }
    if let Some(fields) = &spec.default_search_fields {
        definition = definition.default_search_fields(fields.iter().map(|field| {
            let value = SearchField::new(&field.field);
            field
                .weight
                .map_or(value.clone(), |weight| value.weight(weight))
        }));
    }
    definition
}

fn execute_case(case: &CaseSpec, runtime: &mut Option<IndexRuntime>, path: &Path) -> Value {
    match case.op.as_str() {
        "create_duplicate" => {
            match IndexRuntime::create(path, build_definition_from_args(&case.args)) {
                Ok(created) => {
                    let _ = created.close();
                    json!({"ok": true, "value": {"created": true}})
                }
                Err(error) => error_json(&error),
            }
        }
        "open_missing" => match IndexRuntime::open(path.join("missing")) {
            Ok(opened) => {
                let _ = opened.close();
                json!({"ok": true, "value": {"opened": true}})
            }
            Err(error) => error_json(&error),
        },
        "create_schema" => match IndexRuntime::create(
            path.join(format!("schema-case-{}", case.id)),
            build_definition_from_args(&case.args),
        ) {
            Ok(created) => {
                let _ = created.close();
                json!({"ok": true, "value": {"created": true}})
            }
            Err(error) => error_json(&error),
        },
        "close" => outcome(runtime.as_ref().expect("runtime").close()),
        "reopen" => {
            if let Some(old) = runtime.take() {
                let _ = old.close();
            }
            match IndexRuntime::open(path) {
                Ok(opened) => {
                    *runtime = Some(opened);
                    json!({"ok": true})
                }
                Err(error) => error_json(&error),
            }
        }
        "closed_search" => {
            let index = runtime.as_ref().expect("runtime").index();
            let _ = runtime.as_ref().expect("runtime").close();
            outcome(index.search("wireless").execute::<Value>().map(search_json))
        }
        _ => {
            let index = runtime.as_ref().expect("runtime").index();
            match case.op.as_str() {
                "insert" => outcome(index.insert(arg_str(&case.args, "id"), arg(&case.args, "document"))),
                "upsert" => outcome(index.upsert(arg_str(&case.args, "id"), arg(&case.args, "document"))),
                "patch" => outcome(index.patch(arg_str(&case.args, "id"), build_patch(arg(&case.args, "patch")))),
                "get" => outcome(index.get::<Value>(arg_str(&case.args, "id"))),
                "get_projection" => outcome(index.get_with_projection::<Value>(
                    arg_str(&case.args, "id"),
                    arg_array(&case.args, "fields").iter().map(|value| value.as_str().expect("field")),
                )),
                "delete" => outcome(index.delete(arg_str(&case.args, "id"))),
                "describe" => outcome(index.describe().map(|value| json!({"debug": format!("{value:?}")}))),
                "inspect" => outcome(build_search(&index, &case.args).inspect().map(|value| {
                    json!({
                        "query": value.query,
                        "search_fields": value.search_fields.iter().map(|field| json!({"field": field.field(), "weight": field.weight_value()})).collect::<Vec<_>>(),
                        "filter_debug": value.filter.map(|filter| format!("{filter:?}")),
                        "sort_debug": value.sorts.iter().map(|sort| format!("{sort:?}")).collect::<Vec<_>>(),
                        "facet_debug": value.facets.iter().map(|facet| format!("{facet:?}")).collect::<Vec<_>>(),
                        "ranking_debug": format!("{:?}", value.ranking),
                        "limit": value.limit,
                        "offset": value.offset,
                        "projection": value.projection,
                        "highlighting": value.highlighting,
                        "language": value.language,
                        "timeout_ms": value.timeout.map(|timeout| timeout.as_millis()),
                    })
                })),
                "analyze_field" => outcome(index.analyze(arg_str(&case.args, "text")).field(arg_str(&case.args, "field")).language_opt(arg_opt_str(&case.args, "language")).execute().map(analyze_json)),
                "analyze_analyzer" => outcome(index.analyze(arg_str(&case.args, "text")).analyzer(Analyzer::standard_v1()).language_opt(arg_opt_str(&case.args, "language")).execute().map(analyze_json)),
                "search" => outcome(build_search(&index, &case.args).execute::<Value>().map(search_json)),
                other => panic!("unknown case operation {other}"),
            }
        }
    }
}

trait AnalyzeLanguageOpt {
    fn language_opt(self, value: Option<&str>) -> Self;
}

impl AnalyzeLanguageOpt for searchkernel::AnalyzeBuilder {
    fn language_opt(self, value: Option<&str>) -> Self {
        match value {
            Some(value) => self.language(value),
            None => self,
        }
    }
}

fn build_definition_from_args(args: &Value) -> IndexDefinition {
    serde_json::from_value::<SchemaSpec>(args.get("schema").cloned().expect("schema args"))
        .map(|schema| build_definition(&schema))
        .expect("schema args")
}

fn build_patch(value: &Value) -> Patch {
    let mut patch = Patch::new();
    for operation in value.as_array().expect("patch array") {
        match operation
            .get("op")
            .and_then(Value::as_str)
            .expect("patch op")
        {
            "set" => {
                patch = patch.set(
                    operation
                        .get("field")
                        .and_then(Value::as_str)
                        .expect("patch field"),
                    operation.get("value").expect("patch value"),
                )
            }
            "unset" => {
                patch = patch.unset(
                    operation
                        .get("field")
                        .and_then(Value::as_str)
                        .expect("patch field"),
                )
            }
            other => panic!("unknown patch operation {other}"),
        }
    }
    patch
}

fn build_search(index: &searchkernel::Index, args: &Value) -> searchkernel::SearchBuilder {
    let mut search = if args.get("browse").and_then(Value::as_bool).unwrap_or(false) {
        index.browse()
    } else {
        index.search(arg_str(args, "query"))
    };
    if let Some(fields) = args.get("search_fields") {
        search = search.search_fields(fields.as_array().expect("search fields").iter().map(
            |field| {
                let mut value = SearchField::new(
                    field
                        .get("field")
                        .and_then(Value::as_str)
                        .expect("search field"),
                );
                if let Some(weight) = field.get("weight").and_then(Value::as_f64) {
                    value = value.weight(weight);
                }
                value
            },
        ));
    }
    if let Some(filter) = args.get("filter") {
        search = search.filter(build_filter(filter));
    }
    if let Some(sorts) = args.get("sorts") {
        for sort in sorts.as_array().expect("sorts") {
            let mut value = if sort.get("direction").and_then(Value::as_str) == Some("desc") {
                Sort::desc(arg_str(sort, "field"))
            } else {
                Sort::asc(arg_str(sort, "field"))
            };
            value = match sort.get("missing").and_then(Value::as_str) {
                Some("first") => value.missing_first(),
                Some("last") => value.missing_last(),
                _ => value,
            };
            search = search.sort(value);
        }
    }
    if let Some(facets) = args.get("facets") {
        for facet in facets.as_array().expect("facets") {
            let mut value = Facet::new(arg_str(facet, "field"));
            if let Some(limit) = facet.get("limit").and_then(Value::as_u64) {
                value = value.limit(limit as u32);
            }
            if let Some(sort) = facet.get("sort").and_then(Value::as_str) {
                value = value.sort(match sort {
                    "count_asc" => FacetSort::CountAsc,
                    "value_asc" => FacetSort::ValueAsc,
                    "value_desc" => FacetSort::ValueDesc,
                    _ => FacetSort::CountDesc,
                });
            }
            search = search.facet(value);
        }
    }
    if let Some(fields) = args.get("projection") {
        search = search.projection(
            fields
                .as_array()
                .expect("projection")
                .iter()
                .map(|field| field.as_str().expect("projection field")),
        );
    }
    if let Some(fields) = args.get("highlight") {
        search = search.highlight(
            fields
                .as_array()
                .expect("highlight")
                .iter()
                .map(|field| field.as_str().expect("highlight field")),
        );
    }
    if let Some(language) = args.get("language").and_then(Value::as_str) {
        search = search.language(language);
    }
    if args.get("ranking").and_then(Value::as_str) == Some("bm25") {
        search = search.ranking(Ranking::bm25());
    }
    if let Some(timeout) = args.get("timeout_ms").and_then(Value::as_u64) {
        search = search.timeout(Duration::from_millis(timeout));
    }
    if let Some(limit) = args.get("limit").and_then(Value::as_u64) {
        search = search.limit(limit as u32);
    }
    if let Some(offset) = args.get("offset").and_then(Value::as_u64) {
        search = search.offset(offset as u32);
    }
    search
}

fn build_filter(value: &Value) -> Filter {
    if let Some(value) = value.get("all") {
        return Filter::all(value.as_array().expect("all").iter().map(build_filter));
    }
    if let Some(value) = value.get("any") {
        return Filter::any(value.as_array().expect("any").iter().map(build_filter));
    }
    if let Some(value) = value.get("not") {
        return Filter::not(build_filter(value));
    }
    let field_name = value
        .get("field")
        .and_then(Value::as_str)
        .expect("filter field");
    if value
        .get("exists")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return field(field_name).exists();
    }
    if let Some(range) = value.get("range") {
        let mut result = Range::new();
        if let Some(lower) = range.get("lower") {
            result = result.lower(lower);
        }
        if let Some(upper) = range.get("upper") {
            result = result.upper(upper);
        }
        if let Some(include) = range.get("include_lower").and_then(Value::as_bool) {
            result = result.include_lower(include);
        }
        if let Some(include) = range.get("include_upper").and_then(Value::as_bool) {
            result = result.include_upper(include);
        }
        return field(field_name).range(result);
    }
    let op = value
        .get("op")
        .and_then(Value::as_str)
        .expect("comparison op");
    let operand = value.get("value").expect("comparison value");
    match op {
        "ne" => field(field_name).ne(operand),
        "lt" => field(field_name).lt(operand),
        "lte" => field(field_name).lte(operand),
        "gt" => field(field_name).gt(operand),
        "gte" => field(field_name).gte(operand),
        _ => field(field_name).eq(operand),
    }
}

fn search_json(result: searchkernel::SearchResult<Value>) -> Value {
    json!({
        "total_hits": {"value": result.total_hits.value, "relation": format!("{:?}", result.total_hits.relation)},
        "hits": result.hits.into_iter().map(|hit| json!({
            "id": hit.id.as_str(),
            "score": hit.score,
            "document": hit.document,
            "highlights": hit.highlights.into_iter().map(|highlight| json!({
                "field": highlight.field,
                "fragments": highlight.fragments.into_iter().map(|fragment| json!({
                    "text": fragment.text,
                    "matches": fragment.matches.into_iter().map(|span| json!({"start_byte": span.start_byte, "end_byte": span.end_byte})).collect::<Vec<_>>(),
                })).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "facets": result.facets.into_iter().map(|facet| json!({
            "field": facet.field,
            "values": facet.values.into_iter().map(|value| json!({"value": result_value_json(value.value), "count": value.count})).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
    })
}

fn result_value_json(value: searchkernel::ResultValue) -> Value {
    use searchkernel::ResultValue;

    match value {
        ResultValue::Null => Value::Null,
        ResultValue::Bool(value) => json!(value),
        ResultValue::Int64(value) => json!(value),
        ResultValue::UInt64(value) => json!(value),
        ResultValue::Float64(value) => json!(value),
        ResultValue::NonFiniteFloat(value) => json!(format!("{value:?}")),
        ResultValue::String(value) => json!(value),
        ResultValue::Bytes(value) => json!(value),
        ResultValue::List(values) => {
            Value::Array(values.into_iter().map(result_value_json).collect())
        }
        ResultValue::Object(values) => Value::Object(
            values
                .into_iter()
                .map(|(key, value)| (key, result_value_json(value)))
                .collect(),
        ),
        ResultValue::Tensor(value) => json!({"type_spec": value.type_spec}),
        _ => panic!("unsupported result value variant in trace snapshot"),
    }
}

fn analyze_json(result: searchkernel::AnalyzeResult) -> Value {
    json!({"tokens": result.tokens.into_iter().map(|token| json!({
        "surface": token.surface, "term": token.term, "position": token.position,
        "start_byte": token.start_byte, "end_byte": token.end_byte,
    })).collect::<Vec<_>>()})
}

fn outcome<T>(result: searchkernel::Result<T>) -> Value
where
    T: serde::Serialize,
{
    match result {
        Ok(value) => json!({"ok": true, "value": value}),
        Err(error) => error_json(&error),
    }
}

fn error_json(error: &Error) -> Value {
    json!({
        "ok": false,
        "error": {
            "code": error_code(error),
            "retryable": error.retryable(),
            "details": error.details().iter().map(|detail| json!({"code": detail.code, "path": detail.path, "metadata": detail.metadata})).collect::<Vec<_>>(),
        }
    })
}

fn error_code(error: &Error) -> &'static str {
    match error.code() {
        searchkernel::ErrorCode::InvalidArgument => "invalid_argument",
        searchkernel::ErrorCode::NotFound => "not_found",
        searchkernel::ErrorCode::AlreadyExists => "already_exists",
        searchkernel::ErrorCode::FailedPrecondition => "failed_precondition",
        searchkernel::ErrorCode::Conflict => "conflict",
        searchkernel::ErrorCode::Unsupported => "unsupported",
        searchkernel::ErrorCode::ResourceExhausted => "resource_exhausted",
        searchkernel::ErrorCode::Internal => "internal",
        searchkernel::ErrorCode::Unavailable => "unavailable",
        searchkernel::ErrorCode::DeadlineExceeded => "deadline_exceeded",
        searchkernel::ErrorCode::CorruptIndex => "corrupt_index",
        searchkernel::ErrorCode::IncompatibleIndex => "incompatible_index",
        searchkernel::ErrorCode::IndexClosed => "index_closed",
        _ => "internal",
    }
}

fn arg<'a>(args: &'a Value, name: &str) -> &'a Value {
    args.get(name).expect(name)
}
fn arg_str<'a>(args: &'a Value, name: &str) -> &'a str {
    arg(args, name).as_str().expect(name)
}
fn arg_array<'a>(args: &'a Value, name: &str) -> &'a [Value] {
    arg(args, name).as_array().expect(name)
}
fn arg_opt_str<'a>(args: &'a Value, name: &str) -> Option<&'a str> {
    args.get(name).and_then(Value::as_str)
}

fn format_panic(panic: Box<dyn std::any::Any + Send>) -> String {
    panic.downcast_ref::<&str>().map_or_else(
        || "non-string panic".to_owned(),
        |value| (*value).to_owned(),
    )
}
