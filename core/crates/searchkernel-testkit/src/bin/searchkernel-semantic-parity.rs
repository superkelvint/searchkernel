use flatbuffers::root;
use searchkernel_ir::wire::v1::schema as api;
use searchkernel_ir::{
    snapshot, IndexContext, PresentationInput, QueryInputIr, QueryModelInput,
    QuerySourceInput, RankingInput, RequestValueIr, SearchInput, TextualFilterInput, Value,
};
use serde::Deserialize;
use std::{collections::BTreeMap, fs, path::PathBuf};

#[derive(Deserialize)]
struct Corpus {
    index: String,
    scenarios: Vec<Scenario>,
}
#[derive(Deserialize)]
struct Scenario {
    id: String,
    intent: Intent,
}
#[derive(Deserialize)]
struct Intent {
    kind: String,
    query: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
    filter: Option<Filter>,
    ranking: Option<String>,
    #[serde(default)]
    inputs: BTreeMap<String, Typed>,
    #[serde(default)]
    rank_properties: BTreeMap<String, String>,
}
#[derive(Deserialize)]
struct Filter {
    source: String,
    bindings: BTreeMap<String, Typed>,
}
#[derive(Deserialize)]
struct Typed {
    int64: Option<String>,
    uint64: Option<String>,
    string: Option<String>,
    float64: Option<f64>,
}
fn typed(v: &Typed) -> RequestValueIr {
    if let Some(x) = &v.int64 {
        RequestValueIr::Int64(x.parse().unwrap())
    } else if let Some(x) = &v.uint64 {
        RequestValueIr::UInt64(x.parse().unwrap())
    } else if let Some(x) = &v.string {
        RequestValueIr::String(x.clone())
    } else {
        RequestValueIr::Float64(v.float64.unwrap())
    }
}
fn filter_value(v: &Typed) -> Value {
    match typed(v) {
        RequestValueIr::Int64(x) => Value::Int64(x),
        RequestValueIr::UInt64(x) => Value::UInt64(x),
        RequestValueIr::String(x) => Value::String(x),
        RequestValueIr::Float64(x) => Value::Float64(x),
        _ => unreachable!(),
    }
}
fn main() {
    let mut a = std::env::args().skip(1);
    let mut corpus = None;
    let mut output = None;
    while let Some(x) = a.next() {
        match x.as_str() {
            "--corpus" => corpus = a.next().map(PathBuf::from),
            "--output" => output = a.next().map(PathBuf::from),
            _ => panic!("unknown argument {x}"),
        }
    }
    let path = corpus.unwrap();
    let c: Corpus = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    let root_dir = path.parent().unwrap().parent().unwrap().parent().unwrap();
    let index_json: serde_json::Value =
        serde_json::from_slice(&fs::read(root_dir.join(&c.index)).unwrap()).unwrap();
    let bytes = searchkernel_testkit::build_index_definition_from_json(&index_json).unwrap();
    let index = root::<api::IndexDefinition<'_>>(&bytes).unwrap();
    let ctx = IndexContext::from_api(index).unwrap();
    let mut out = serde_json::Map::new();
    for s in c.scenarios {
        let i = s.intent;
        let query = if i.kind == "browse" {
            QuerySourceInput::MatchAll
        } else {
            QuerySourceInput::Text(i.query.unwrap())
        };
        let textual_filters = i
            .filter
            .into_iter()
            .map(|f| TextualFilterInput {
                source: f.source,
                bindings: f
                    .bindings
                    .into_iter()
                    .map(|(k, v)| (k, filter_value(&v)))
                    .collect(),
            })
            .collect();
        let query_inputs = i
            .inputs
            .into_iter()
            .map(|(k, v)| (k, QueryInputIr::Literal(typed(&v))))
            .collect();
        let input = SearchInput {
            query,
            search_fields: None,
            filters: vec![],
            textual_filters,
            sorts: vec![],
            grouping: vec![],
            ranking: i.ranking.map(RankingInput::Named),
            query_inputs,
            rank_properties: i.rank_properties,
            query_model: QueryModelInput::default(),
            projection: None,
            highlighting: None,
            presentation: PresentationInput::default(),
            properties: BTreeMap::new(),
            query_profile: None,
            search_chain: None,
            timeout: None,
            limit: i.limit,
            offset: i.offset,
        };
        let ir = searchkernel_ir::resolve_search(&ctx, input).unwrap();
        out.insert(
            s.id,
            searchkernel_testkit::lossless_integer_tags(snapshot::search_request(&ir)),
        );
    }
    fs::write(output.unwrap(), serde_json::to_vec_pretty(&out).unwrap()).unwrap();
}
