use searchkernel_vespa_engine::{
    ConfigCompiler, Document, Engine, Field, FieldType, Query, RangeBound, Schema, SearchRequest,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{collections::HashSet, fs, path::PathBuf};

#[derive(Deserialize)]
struct Scenario {
    id: String,
    schema: String,
    fields: Vec<FieldSpec>,
    documents: Vec<Document>,
    typed: Value,
    yql: String,
}
#[derive(Deserialize)]
struct FieldSpec {
    name: String,
    #[serde(rename = "type")]
    kind: String,
    index: bool,
    attribute: bool,
    summary: bool,
}
#[derive(Deserialize)]
struct Document {
    id: String,
    kind: String,
    field: String,
    value: Value,
}
fn load(p: &str) -> Scenario {
    serde_json::from_str(&fs::read_to_string(p).expect("read scenario")).expect("parse scenario")
}
fn schema(s: &Scenario) -> Schema {
    let mut out = Schema::new(&s.schema);
    for f in &s.fields {
        let k = match f.kind.as_str() {
            "text" => FieldType::Text,
            "integer" => FieldType::Integer,
            _ => panic!("unsupported"),
        };
        let mut x = Field::new(&f.name, k);
        if f.index {
            x = x.indexed();
        }
        if f.attribute {
            x = x.attribute();
        }
        if f.summary {
            x = x.summary();
        }
        out = out.field(x);
    }
    out
}
fn bound(v: &Value) -> RangeBound {
    match v["kind"].as_str().unwrap() {
        "unbounded" => RangeBound::Unbounded,
        "included" => RangeBound::Included(v["value"].as_i64().unwrap()),
        "excluded" => RangeBound::Excluded(v["value"].as_i64().unwrap()),
        _ => panic!("bad bound"),
    }
}
fn query(v: &Value) -> Query {
    match v["kind"].as_str().unwrap() {
        "term" => Query::Term {
            field: v["field"].as_str().unwrap().into(),
            text: v["text"].as_str().unwrap().into(),
        },
        "phrase" => Query::Phrase {
            field: v["field"].as_str().unwrap().into(),
            text: v["text"].as_str().unwrap().into(),
        },
        "and" => Query::And(
            v["children"]
                .as_array()
                .unwrap()
                .iter()
                .map(query)
                .collect(),
        ),
        "or" => Query::Or(
            v["children"]
                .as_array()
                .unwrap()
                .iter()
                .map(query)
                .collect(),
        ),
        "not" => Query::Not(Box::new(query(&v["child"]))),
        "equals" => Query::Equals {
            field: v["field"].as_str().unwrap().into(),
            value: v["value"].as_i64().unwrap(),
        },
        "range" => Query::Range {
            field: v["field"].as_str().unwrap().into(),
            lower: bound(&v["lower"]),
            upper: bound(&v["upper"]),
        },
        "exists" => Query::Exists {
            field: v["field"].as_str().unwrap().into(),
        },
        _ => panic!("bad query"),
    }
}
fn canon_bound(b: &RangeBound) -> Value {
    match b {
        RangeBound::Unbounded => json!({"kind":"unbounded"}),
        RangeBound::Included(v) => json!({"kind":"included","value":v}),
        RangeBound::Excluded(v) => json!({"kind":"excluded","value":v}),
    }
}
fn canon(q: &Query) -> Value {
    match q {
        Query::True => json!({"kind":"true"}),
        Query::Term { field, text } => json!({"kind":"term","field":field,"text":text}),
        Query::Phrase { field, text } => json!({"kind":"phrase","field":field,"text":text}),
        Query::And(xs) => json!({"kind":"and","children":xs.iter().map(canon).collect::<Vec<_>>()}),
        Query::Or(xs) => json!({"kind":"or","children":xs.iter().map(canon).collect::<Vec<_>>()}),
        Query::Not(x) => json!({"kind":"not","child":canon(x)}),
        Query::Equals { field, value } => json!({"kind":"equals","field":field,"value":value}),
        Query::Range {
            field,
            lower,
            upper,
        } => {
            json!({"kind":"range","field":field,"lower":canon_bound(lower),"upper":canon_bound(upper)})
        }
        Query::Exists { field } => json!({"kind":"exists","field":field}),
    }
}
fn populate(engine: &mut Engine, s: &Scenario) {
    let mut created = HashSet::new();
    for d in &s.documents {
        if created.insert(d.id.clone()) {
            let mut document = Document::new(&d.id);
            for field in s.documents.iter().filter(|field| field.id == d.id) {
                document = match field.kind.as_str() {
                    "text" => document.field(&field.field, field.value.as_str().unwrap()),
                    "integer" => document.field(&field.field, field.value.as_i64().unwrap()),
                    _ => panic!(),
                };
            }
            engine.put(document).expect("put document");
        }
    }
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 3 {
        panic!("usage")
    };
    match a[1].as_str() {
        "canonical" => {
            let s = load(&a[2]);
            let t = query(&s.typed);
            let y = Query::from_yql(&s.yql).expect("parse yql");
            println!(
                "{}",
                serde_json::to_string(
                    &json!({"id":s.id,"schema":s.schema,"typed":canon(&t),"yql":canon(&y)})
                )
                .unwrap()
            );
        }
        "execute" => {
            if a.len() != 6 {
                panic!("execute SCENARIO CONFIG DATA typed|yql")
            };
            let s = load(&a[2]);
            let cfg = PathBuf::from(&a[3]);
            let data = PathBuf::from(&a[4]);
            ConfigCompiler::default()
                .compile_to(&schema(&s), &cfg)
                .expect("compile");
            let mut e = Engine::open(&data, &cfg).expect("open");
            populate(&mut e, &s);
            let result = if a[5] == "typed" {
                e.search(SearchRequest::new(query(&s.typed)))
            } else {
                e.search(SearchRequest::new(Query::from_yql(&s.yql).expect("parse yql")))
            }
            .expect("query");
            let mut hits = result
                .hits
                .into_iter()
                .map(|h| h.document_id)
                .collect::<Vec<_>>();
            hits.sort();
            println!(
                "{}",
                serde_json::to_string(&json!({"id":s.id,"hits":hits})).unwrap()
            );
        }
        _ => panic!("unknown mode"),
    }
}
