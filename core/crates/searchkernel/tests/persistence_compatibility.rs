#![cfg(feature = "native")]

use searchkernel::{field, Index, Patch};
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const VERSIONS: [u32; 3] = [2, 3, 4];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .expect("workspace root")
        .to_path_buf()
}

fn temporary_path(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("searchkernel-persistence-{label}-{nonce}"))
}

fn copy_tree(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let source = entry.path();
        let destination = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&source, &destination);
        } else {
            fs::copy(source, destination).unwrap();
        }
    }
}

fn materialize_fixture(version: u32) -> PathBuf {
    let root = repo_root();
    let fixture = root
        .join("reference/persistence/fixtures")
        .join(format!("manifest-v{version}-storage-v1"));
    let target = temporary_path(&format!("v{version}"));
    fs::create_dir_all(target.join("manifest")).unwrap();
    fs::create_dir_all(target.join("config")).unwrap();

    copy_tree(
        &root.join("reference/vespa/m5/fixtures/persisted-v1/index"),
        &target.join("data"),
    );
    copy_tree(&fixture.join("config"), &target.join("config"));
    fs::copy(
        fixture.join("manifest-template.json"),
        target.join("manifest/index.json"),
    )
    .unwrap();
    if version != 2 {
        for name in ["index-definition.bfbs", "portable-index-definition.bfbs"] {
            fs::copy(fixture.join(name), target.join("manifest").join(name)).unwrap();
        }
    }
    target
}

fn assert_expected(index: &Index) {
    for (value, expected) in [
        (10_i64, vec!["doc2", "doc4"]),
        (20_i64, vec![]),
        (30_i64, vec![]),
        (40_i64, vec!["doc1"]),
    ] {
        let mut ids = index
            .browse()
            .filter(field("attr1").eq(value))
            .limit(20)
            .execute::<serde_json::Value>()
            .unwrap()
            .hits
            .into_iter()
            .map(|hit| hit.id.to_string())
            .collect::<Vec<_>>();
        ids.sort();
        assert_eq!(ids, expected, "unexpected attr1={value} result");
    }
}

#[test]
fn every_supported_historical_fixture_reopens_searches_mutates_and_reopens_again() {
    for version in VERSIONS {
        let root = materialize_fixture(version);
        let index = Index::open(&root).unwrap_or_else(|error| panic!("v{version} open: {error}"));
        assert_expected(&index);

        index
            .patch("doc2", Patch::new().set("attr1", 50_i64))
            .unwrap();
        index
            .upsert("doc5", &json!({"attr1": 10_i64}))
            .unwrap();
        index.close().unwrap();

        let reopened =
            Index::open(&root).unwrap_or_else(|error| panic!("v{version} reopen: {error}"));
        let ten = reopened
            .browse()
            .filter(field("attr1").eq(10_i64))
            .limit(20)
            .execute::<serde_json::Value>()
            .unwrap();
        let mut ids = ten
            .hits
            .into_iter()
            .map(|hit| hit.id.to_string())
            .collect::<Vec<_>>();
        ids.sort();
        assert_eq!(ids, vec!["doc4", "doc5"]);
        assert!(reopened
            .browse()
            .filter(field("attr1").eq(50_i64))
            .limit(20)
            .execute::<serde_json::Value>()
            .unwrap()
            .hits
            .iter()
            .any(|hit| hit.id.as_str() == "doc2"));
        reopened.close().unwrap();
        fs::remove_dir_all(root).unwrap();
    }
}
