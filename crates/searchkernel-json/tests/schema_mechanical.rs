use std::path::{Path, PathBuf};

fn production_files() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    [root.join("src"), root.join("native")]
        .into_iter()
        .flat_map(|directory| walk(&directory))
        .collect()
}

fn walk(directory: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(directory)
        .unwrap()
        .flat_map(|entry| {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(&path)
            } else {
                vec![path]
            }
        })
        .collect()
}

#[test]
fn production_json_path_has_no_message_specific_codec() {
    let forbidden = [
        "InsertRequest",
        "UpsertRequest",
        "PatchRequest",
        "SearchRequest",
        "FilterExpr",
        "QueryExpr",
        "RankingPolicy",
        "SearchResult",
        "document_json",
        "value_json",
        "$absent",
        "operation",
    ];
    let obsolete_files = [
        "codec.rs",
        "generated_codec.rs",
        "json_types.rs",
        "schema_metadata.rs",
        "codec_dispatch.rs",
    ];
    for path in production_files() {
        assert!(
            !obsolete_files.contains(&path.file_name().unwrap().to_str().unwrap()),
            "obsolete JSON codec file: {}",
            path.display()
        );
        let text = std::fs::read_to_string(&path).unwrap();
        for term in forbidden {
            assert!(!text.contains(term), "{term} found in {}", path.display());
        }
    }
}
