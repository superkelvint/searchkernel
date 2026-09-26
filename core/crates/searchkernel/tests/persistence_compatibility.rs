#![cfg(feature = "native")]

use searchkernel::{field, ErrorCode, Index, Patch};
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

fn snapshot_tree(root: &Path) -> std::collections::BTreeMap<PathBuf, Vec<u8>> {
    fn visit(root: &Path, path: &Path, output: &mut std::collections::BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if entry.file_type().unwrap().is_dir() {
                visit(root, &path, output);
            } else {
                output.insert(
                    path.strip_prefix(root).unwrap().to_path_buf(),
                    fs::read(path).unwrap(),
                );
            }
        }
    }

    let mut output = std::collections::BTreeMap::new();
    visit(root, root, &mut output);
    output
}

fn materialize_fixture(version: u32) -> PathBuf {
    let fixture = repo_root()
        .join("reference/persistence/fixtures")
        .join(format!("manifest-v{version}-storage-v1"));
    let target = temporary_path(&format!("v{version}"));
    copy_tree(&fixture, &target);
    target
}

fn search_ids(index: &Index, value: i64) -> Vec<String> {
    let result = index
        .browse()
        .filter(field("attr1").eq(value))
        .execute::<serde_json::Value>()
        .unwrap();
    let mut ids = result
        .hits
        .into_iter()
        .map(|hit| hit.id.to_string())
        .collect::<Vec<_>>();
    ids.sort();
    ids
}

fn assert_expected_searches(index: &Index) {
    for (value, expected) in [
        (10_i64, vec!["id:test:typea::2", "id:test:typea::4"]),
        (20_i64, vec![]),
        (30_i64, vec![]),
        (40_i64, vec!["id:test:typea::1"]),
    ] {
        assert_eq!(
            search_ids(index, value),
            expected.into_iter().map(str::to_owned).collect::<Vec<_>>(),
            "unexpected persisted search result for attr1={value}"
        );
    }
}

#[test]
fn every_supported_historical_fixture_reopens_searches_mutates_and_reopens_again() {
    for version in VERSIONS {
        let root = materialize_fixture(version);
        let index = Index::open(&root).unwrap_or_else(|error| panic!("v{version} open: {error}"));
        let described = index.describe().unwrap();
        assert_eq!(described.fields()[0].name(), "attr1");
        assert_expected_searches(&index);

        index
            .patch("id:test:typea::2", Patch::new().set("attr1", 50_i64))
            .unwrap();
        index
            .upsert("id:test:typea::5", &json!({"attr1": 10_i64}))
            .unwrap();
        index.close().unwrap();

        let reopened =
            Index::open(&root).unwrap_or_else(|error| panic!("v{version} reopen: {error}"));
        assert_eq!(
            search_ids(&reopened, 50),
            vec!["id:test:typea::2".to_owned()]
        );
        assert_eq!(
            search_ids(&reopened, 10),
            vec!["id:test:typea::4".to_owned(), "id:test:typea::5".to_owned(),]
        );
        reopened.close().unwrap();
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn v3_and_v4_fixtures_keep_omitted_document_type_while_reopening() {
    for version in [3, 4] {
        let root = materialize_fixture(version);
        let index = Index::open(&root).unwrap_or_else(|error| panic!("v{version} open: {error}"));
        let described = index.describe().expect("describe historical index");
        assert_eq!(described.document_type, None);
        assert_expected_searches(&index);
        index.close().unwrap();
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn repeated_failed_real_opens_do_not_mutate_or_destroy_a_valid_fixture() {
    let root = materialize_fixture(4);
    let manifest = root.join("manifest/index.json");
    let valid_manifest = fs::read(&manifest).unwrap();
    fs::write(&manifest, b"{").unwrap();
    let corrupt_snapshot = snapshot_tree(&root);

    for attempt in 0..3 {
        let error = match Index::open(&root) {
            Ok(index) => {
                index.close().unwrap();
                panic!("corrupt fixture unexpectedly opened on attempt {attempt}");
            }
            Err(error) => error,
        };
        assert_eq!(error.code(), ErrorCode::CorruptIndex);
        assert_eq!(
            snapshot_tree(&root),
            corrupt_snapshot,
            "failed open attempt {attempt} mutated persisted fixture bytes"
        );
    }

    fs::write(&manifest, valid_manifest).unwrap();
    let index = Index::open(&root).expect("restored fixture must reopen after repeated failures");
    assert_expected_searches(&index);
    index.close().unwrap();
    fs::remove_dir_all(root).unwrap();
}
