#![cfg(feature = "native")]

use std::{
    collections::BTreeSet,
    net::SocketAddr,
    path::PathBuf,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use searchkernel::{field, ErrorCode, Field, Index, IndexDefinition, Sort};
use searchkernel_dispatch::Dispatcher;
use searchkerneld::{start, ServerConfig};
use serde_json::{json, Value};

struct TempIndex(PathBuf);
impl TempIndex {
    fn new(label: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        Self(std::env::temp_dir().join(format!(
            "searchkernel-entry-parity-{label}-{}-{nonce}",
            std::process::id()
        )))
    }
}
impl Drop for TempIndex {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn definition() -> IndexDefinition {
    IndexDefinition::new()
        .name("entry_parity")
        .field(Field::text("title").stored(true).searchable(true))
        .field(
            Field::keyword("category")
                .stored(true)
                .filterable(true)
                .sortable(true),
        )
        .field(
            Field::float64("price")
                .stored(true)
                .filterable(true)
                .sortable(true),
        )
        .default_search_fields(["title"])
}

fn exercise(index: &Index) -> Value {
    index
        .upsert(
            "doc:1",
            &json!({"title":"hello world", "category":"a", "price":5.0}),
        )
        .unwrap();
    index
        .upsert(
            "doc:2",
            &json!({"title":"hello again", "category":"b", "price":2.0}),
        )
        .unwrap();
    index
        .upsert(
            "doc:3",
            &json!({"title":"other", "category":"a", "price":1.0}),
        )
        .unwrap();

    let result = index
        .search("hello")
        .filter(field("price").lte(5.0))
        .sort(Sort::asc("price"))
        .limit(10)
        .execute::<Value>()
        .unwrap();
    let hits: Vec<Value> = result
        .hits
        .iter()
        .map(|hit| json!({"id":hit.id, "document":hit.document}))
        .collect();

    let bad = index
        .search("hello")
        .filter(field("missing").eq("x"))
        .execute::<Value>()
        .unwrap_err();
    let detail_codes: BTreeSet<_> = bad
        .details()
        .iter()
        .map(|detail| detail.code.clone())
        .collect();
    assert_eq!(bad.code(), ErrorCode::InvalidArgument);

    let described = index.describe().unwrap();
    json!({
        "schema_name": described.name(),
        "field_names": described.fields().map(|field| field.name().to_owned()).collect::<BTreeSet<_>>(),
        "total_hits": result.total_hits.value,
        "hits": hits,
        "negative": {"code": format!("{:?}", bad.code()), "details": detail_codes},
    })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn public_rust_direct_and_generated_grpc_match_on_real_engine() {
    let direct_path = TempIndex::new("direct");
    let direct = Index::create(&direct_path.0, definition()).unwrap();
    let direct_outcome = exercise(&direct);
    direct.close().unwrap();

    let grpc_path = TempIndex::new("grpc");
    let hosted = Index::create(&grpc_path.0, definition()).unwrap();
    let (context, session) = hosted.clone().into_session_parts();
    let dispatcher = Arc::new(Dispatcher::new(context, session));
    let mut server = start(
        ServerConfig::new("127.0.0.1:0".parse::<SocketAddr>().unwrap()),
        dispatcher,
    )
    .await
    .unwrap();
    let endpoint = format!("http://{}", server.tcp_addr().unwrap());

    let remote = Index::connect(endpoint).unwrap();
    let mut grpc_outcome = exercise(&remote);
    if std::env::var_os("SEARCHKERNEL_TEST_PERTURB_GRPC_PARITY").is_some() {
        grpc_outcome["total_hits"] = json!(999_u64);
    }
    remote.close().unwrap();
    server.shutdown().await.unwrap();
    hosted.close().unwrap();

    assert_eq!(
        grpc_outcome, direct_outcome,
        "real-engine public entry paths diverged"
    );
}
