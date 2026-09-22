use searchkernel_ir::{
    index_definition_ir_to_api, resolve_rust_index_definition, snapshot, RustFieldInput,
    RustFieldKind, RustIndexDefinitionInput,
};
use std::fs;
use std::path::{Path, PathBuf};

const COMMITTED_OUTPUT: &str = "reference/persistence/fixtures";
const STORAGE_SOURCE: &str = "reference/vespa/m5/fixtures/persisted-v1/index";
const VESPA_REVISION: &str = "2300ef0412bdb76f227ae3b497c8ca5e73c5da4e";

fn main() {
    let output = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            panic!("usage: searchkernel-persistence-fixtures <candidate-output-directory>")
        });
    let committed = repo_root().join(COMMITTED_OUTPUT);
    let candidate = if output.is_absolute() {
        output.clone()
    } else {
        std::env::current_dir()
            .expect("resolve current directory")
            .join(&output)
    };
    if candidate == committed {
        panic!("refusing to rewrite the committed persistence fixture corpus");
    }
    fs::create_dir_all(&output).expect("create persistence fixture directory");

    // The preserved M5 corpus used Search Kernel's historical integer facade
    // over a physical Vespa INT32 attribute. Keep the portable/resolved schema
    // at that historical Int64 meaning; the physical config identity below is
    // intentionally frozen as INT32.
    let input = RustIndexDefinitionInput {
        name: None,
        fields: vec![RustFieldInput {
            name: "attr1".to_owned(),
            kind: RustFieldKind::Int64,
            multi: false,
            analyzer: None,
            stored: Some(false),
            searchable: Some(false),
            filterable: Some(true),
            sortable: Some(true),
            facetable: Some(false),
            distance_metric: None,
            hnsw: None,
        }],
        default_search_fields: None,
    };
    let resolved =
        resolve_rust_index_definition(input).expect("resolve persistence fixture schema");
    let definition_bytes = index_definition_ir_to_api(&resolved);
    let schema_snapshot = snapshot::index_definition(&resolved);
    let config_identity = embedded_config_identity();
    let repo_root = repo_root();

    for (version, manifest_revision, manifest_abi) in [
        (2_u32, "a7abfe6213dea5e7fc1132d52c1fef163b42f729", 5_u32),
        (3_u32, "e5b7f60cdf837866355756b55d3cb1e77d5c9b62", 6_u32),
        (4_u32, "e02da1cf4c91544a7dbab120e961c47d26b67278", 12_u32),
    ] {
        let directory = output.join(format!("manifest-v{version}-storage-v1"));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(directory.join("manifest")).expect("create fixture manifest");
        fs::create_dir_all(directory.join("config")).expect("create fixture config");

        copy_tree(&repo_root.join(STORAGE_SOURCE), &directory.join("data"));

        for (name, contents) in embedded_config_files() {
            fs::write(directory.join("config").join(name), contents).expect("write fixture config");
        }

        let manifest = match version {
            2 => serde_json::json!({
                "format_version": 2,
                "searchkernel_version": "0.1.0",
                "native_abi_version": manifest_abi,
                "vespa_revision": VESPA_REVISION,
                "portable_index_definition": legacy_definition(),
                "resolved_index_definition": legacy_definition(),
                "resolved_engine_config_identity": config_identity
            }),
            3 => {
                fs::write(directory.join("manifest/index-definition.bfbs"), &definition_bytes)
                    .expect("write v3 canonical definition");
                fs::write(
                    directory.join("manifest/portable-index-definition.bfbs"),
                    &definition_bytes,
                )
                .expect("write v3 portable definition");
                serde_json::json!({
                    "manifest_format_version": 3,
                    "storage_compatibility_generation": 1,
                    "provenance": {
                        "created_by_searchkernel_version": "0.1.0",
                        "created_by_native_abi": manifest_abi,
                        "created_by_vespa_revision": VESPA_REVISION
                    },
                    "portable_index_definition": "portable-index-definition.bfbs",
                    "canonical_index_definition": "index-definition.bfbs",
                    "resolved_engine_config_identity": config_identity
                })
            }
            4 => {
                fs::write(directory.join("manifest/index-definition.bfbs"), &definition_bytes)
                    .expect("write v4 canonical definition");
                fs::write(
                    directory.join("manifest/portable-index-definition.bfbs"),
                    &definition_bytes,
                )
                .expect("write v4 portable definition");
                serde_json::json!({
                    "manifest_format_version": 4,
                    "storage_compatibility_generation": 1,
                    "resolved_schema_snapshot_version": 1,
                    "resolved_schema_snapshot": schema_snapshot,
                    "provenance": {
                        "created_by_searchkernel_version": "0.1.0",
                        "created_by_native_abi": manifest_abi,
                        "created_by_vespa_revision": VESPA_REVISION
                    },
                    "portable_index_definition": "portable-index-definition.bfbs",
                    "canonical_index_definition": "index-definition.bfbs",
                    "resolved_engine_config_identity": config_identity
                })
            }
            _ => unreachable!(),
        };
        fs::write(
            directory.join("manifest/index.json"),
            serde_json::to_vec_pretty(&manifest).expect("encode manifest"),
        )
        .expect("write manifest");

        let metadata = serde_json::json!({
            "fixture_format": 1,
            "manifest_format_revision": manifest_revision,
            "manifest_version": version,
            "storage_compatibility_generation": 1,
            "manifest_native_abi_version": manifest_abi,
            "storage_searchkernel_revision": "7ec21655f2c5a311aa2fe4fe65861f9a8f2e3487",
            "storage_native_abi_version": 4,
            "vespa_revision": VESPA_REVISION,
            "assembly": "one-time compatibility fixture assembled from preserved M5 storage and the historical manifest contract; runtime tests never reconstruct it",
            "schema": {
                "fields": [{
                    "name": "attr1",
                    "portable_type": "int64",
                    "physical_vespa_type": "INT32",
                    "stored": false,
                    "searchable": false,
                    "filterable": true,
                    "sortable": true,
                    "facetable": false
                }]
            },
            "mutations": [
                "put id:test:typea::1 attr1=10",
                "put id:test:typea::2 attr1=20",
                "put id:test:typea::3 attr1=30",
                "update id:test:typea::2 attr1=10",
                "delete id:test:typea::3",
                "update id:test:typea::1 attr1=40",
                "put id:test:typea::4 attr1=10"
            ],
            "expected_queries": {
                "10": ["id:test:typea::2", "id:test:typea::4"],
                "20": [],
                "30": [],
                "40": ["id:test:typea::1"]
            }
        });
        fs::write(
            directory.join("METADATA.json"),
            serde_json::to_vec_pretty(&metadata).expect("encode metadata"),
        )
        .expect("write metadata");
    }

    write_readme(&output);
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .expect("workspace root")
        .to_path_buf()
}

fn copy_tree(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).expect("create fixture storage directory");
    for entry in fs::read_dir(source).expect("read preserved storage fixture") {
        let entry = entry.expect("read storage fixture entry");
        let target = destination.join(entry.file_name());
        if entry.file_type().expect("read storage fixture type").is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).expect("copy preserved storage fixture file");
        }
    }
}

fn legacy_definition() -> serde_json::Value {
    serde_json::json!({
        "fields": [{
            "name": "attr1",
            "kind": "Int64",
            "stored": false,
            "searchable": false,
            "filterable": true,
            "sortable": true,
            "facetable": false
        }]
    })
}

fn embedded_config_files() -> [(&'static str, &'static str); 7] {
    [
        (
            "searchkernel-schema.json",
            "{\"version\":1,\"document_type\":\"typea\",\"fields\":[{\"name\":\"attr1\",\"type\":\"integer\",\"index\":false,\"attribute\":true,\"summary\":false}]}\n",
        ),
        ("indexschema.cfg", "indexfield[0]\n"),
        (
            "attributes.cfg",
            "attribute[1]\nattribute[0].name \"attr1\"\nattribute[0].datatype INT32\nattribute[0].collectiontype SINGLE\n",
        ),
        (
            "summary.cfg",
            "defaultsummaryid 1\nclasses[1]\nclasses[0].id 1\nclasses[0].name \"default\"\nclasses[0].fields[3]\nclasses[0].fields[0].name \"documentid\"\nclasses[0].fields[0].type \"longstring\"\nclasses[0].fields[0].command \"documentid\"\nclasses[0].fields[0].source \"\"\nclasses[0].fields[1].name \"rankfeatures\"\nclasses[0].fields[1].type \"featuredata\"\nclasses[0].fields[1].command \"rankfeatures\"\nclasses[0].fields[1].source \"\"\nclasses[0].fields[2].name \"summaryfeatures\"\nclasses[0].fields[2].type \"featuredata\"\nclasses[0].fields[2].command \"summaryfeatures\"\nclasses[0].fields[2].source \"\"\n",
        ),
        (
            "rank-profiles.cfg",
            "rankprofile[1]\nrankprofile[0].name \"default\"\n",
        ),
        ("juniperrc.cfg", ""),
        (
            "proton.cfg",
            "documentdb[1]\ndocumentdb[0].inputdoctypename \"typea\"\ndocumentdb[0].document_id_attribute true\n",
        ),
    ]
}

fn embedded_config_identity() -> serde_json::Value {
    embedded_config_files()
        .into_iter()
        .map(|(name, contents)| {
            (
                name.to_owned(),
                serde_json::Value::String(contents.to_owned()),
            )
        })
        .collect::<serde_json::Map<_, _>>()
        .into()
}

fn write_readme(output: &Path) {
    let readme = r#"# Persistence compatibility fixtures

Each version directory is a complete persisted Search Kernel index fixture:
checked-in manifest state, frozen generated engine config, and the preserved M5
Vespa storage payload. Tests copy a complete fixture before mutation; they do
not reconstruct historical state at runtime.

The corpus is append-only. Once a version fixture lands, do not regenerate or
edit it in place. A new manifest/storage generation adds a new directory.

The metadata records manifest-contract provenance separately from the older M5
storage provenance because these compatibility fixtures were assembled once
from preserved historical components rather than captured by a single original
process invocation.
"#;
    fs::write(output.join("README.md"), readme).expect("write fixture README");
}
