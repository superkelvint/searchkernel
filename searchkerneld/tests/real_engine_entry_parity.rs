#![cfg(feature = "native")]

use std::{
    collections::BTreeSet,
    net::TcpListener,
    path::PathBuf,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use searchkernel::{field, ErrorCode, Field, Index, IndexDefinition, RemoteClient, Sort};
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

struct Daemon(Child);
impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn unused_tcp_addr() -> std::net::SocketAddr {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
}

fn launch_production_daemon(data_root: &std::path::Path) -> (Daemon, String) {
    let addr = unused_tcp_addr();
    let child = Command::new(env!("CARGO_BIN_EXE_searchkerneld"))
        .arg("--data-root")
        .arg(data_root)
        .arg("--listen")
        .arg(addr.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("launch production searchkerneld binary");
    let mut daemon = Daemon(child);
    let endpoint = format!("http://{addr}");
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(status) = daemon.0.try_wait().unwrap() {
            panic!("production searchkerneld exited before accepting gRPC: {status}");
        }
        if std::net::TcpStream::connect(addr).is_ok() {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "production searchkerneld did not start"
        );
        thread::sleep(Duration::from_millis(25));
    }
    (daemon, endpoint)
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
        .map(|hit| json!({"id":hit.id.as_str(), "document":hit.document}))
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
        "schema_name": described.index_name(),
        "field_names": described.fields().iter().map(|field| field.name().to_owned()).collect::<BTreeSet<_>>(),
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
    let (_daemon, endpoint) = launch_production_daemon(&grpc_path.0);
    let client = RemoteClient::connect(endpoint).unwrap();
    let remote = client
        .create_index("entry-parity", definition())
        .expect("create target through canonical daemon lifecycle");
    let grpc_outcome = exercise(&remote);
    remote.close().unwrap();

    assert_eq!(
        grpc_outcome, direct_outcome,
        "real-engine public entry paths diverged"
    );
}
