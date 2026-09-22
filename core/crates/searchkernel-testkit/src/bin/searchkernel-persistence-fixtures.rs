use searchkernel_ir::{
    index_definition_ir_to_api, resolve_rust_index_definition, snapshot, RustFieldInput,
    RustFieldKind, RustIndexDefinitionInput,
};
use std::fs;
use std::path::{Path, PathBuf};

const DEFAULT_OUTPUT: &str = "reference/persistence/fixtures";

fn main() {
    let output = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_OUTPUT));
    fs::create_dir_all(&output).expect("create persistence fixture directory");

    let input = RustIndexDefinitionInput {
        name: None,
        fields: vec![RustFieldInput {
            name: "attr1".to_owned(),
            kind: RustFieldKind::Int64,
            multi: false,
            analyzer: None,
            stored: false,
            searchable: false,
            filterable: true,
            sortable: true,
            facetable: false,
        }],
        default_search_fields: None,
    };
    let mut resolved =
        resolve_rust_index_definition(&input).expect("resolve persistence fixture schema");
    resolved.set_document_type("typea");
    let canonical = index_definition_ir_to_api(&resolved);
    let schema_snapshot = snapshot::index_definition(&resolved);

    let config_identity = embedded_config_identity();
    let config_identity = &config_identity;

    for (version, revision) in [
        (2_u32, "a7abfe6213dea5e7fc1132d52c1fef163b42f729"),
        (3_u32, "e5b7f60cdf837866355756b55d3cb1e77d5c9b62"),
        (4_u32, "e02da1cf4c91544a7dbab120e961c47d26b67278"),
    ] {
        let directory = output.join(format!("manifest-v{version}-storage-v1"));
        fs::create_dir_all(&directory).expect("create version fixture");
        if version != 2 {
            fs::write(directory.join("index-definition.bfbs"), &canonical)
                .expect("write canonical definition");
            fs::write(directory.join("portable-index-definition.bfbs"), &canonical)
                .expect("write portable definition");
        }

        let manifest = match version {
            2 => serde_json::json!({
                "format_version": 2,
                "searchkernel_version": "0.1.0",
                "native_abi_version": 4,
                "vespa_revision": "2300ef0412bdb76f227ae3b497c8ca5e73c5da4e",
                "portable_index_definition": {
                    "fields": [{
                        "name": "attr1",
                        "kind": "Int64",
                        "stored": true,
                        "searchable": false,
                        "filterable": true,
                        "sortable": true,
                        "facetable": false
                    }]
                },
                "resolved_engine_config_identity": config_identity,
                "resolved_index_definition": {
                    "fields": [{
                        "name": "attr1",
                        "kind": "Int64",
                        "stored": true,
                        "searchable": false,
                        "filterable": true,
                        "sortable": false,
                        "facetable": false
                    }]
                }
            }),
            3 => serde_json::json!({
                "manifest_format_version": 3,
                "storage_compatibility_generation": 1,
                "provenance": {
                    "created_by_searchkernel_version": "0.1.0",
                    "created_by_native_abi": 4,
                    "created_by_vespa_revision": "2300ef0412bdb76f227ae3b497c8ca5e73c5da4e"
                },
                "portable_index_definition": "portable-index-definition.bfbs",
                "canonical_index_definition": "index-definition.bfbs",
                "resolved_engine_config_identity": config_identity
            }),
            4 => serde_json::json!({
                "manifest_format_version": 4,
                "storage_compatibility_generation": 1,
                "resolved_schema_snapshot_version": 1,
                "resolved_schema_snapshot": &schema_snapshot,
                "provenance": {
                    "created_by_searchkernel_version": "0.1.0",
                    "created_by_native_abi": 4,
                    "created_by_vespa_revision": "2300ef0412bdb76f227ae3b497c8ca5e73c5da4e"
                },
                "portable_index_definition": "portable-index-definition.bfbs",
                "canonical_index_definition": "index-definition.bfbs",
                "resolved_engine_config_identity": config_identity
            }),
            _ => unreachable!(),
        };
        fs::write(
            directory.join("manifest-template.json"),
            serde_json::to_vec_pretty(&manifest).expect("encode manifest"),
        )
        .expect("write manifest template");
        fs::create_dir_all(directory.join("config")).expect("create fixture config");
        for (name, contents) in embedded_config_files() {
            fs::write(directory.join("config").join(name), contents).expect("write fixture config");
        }

        let metadata = serde_json::json!({
            "fixture_format": 1,
            "searchkernel_revision": revision,
            "manifest_version": version,
            "storage_compatibility_generation": 1,
            "native_abi_version": 4,
            "vespa_revision": "2300ef0412bdb76f227ae3b497c8ca5e73c5da4e",
            "schema": {
                "fields": [{
                    "name": "attr1",
                    "type": "int64",
                    "stored": true,
                    "searchable": false,
                    "filterable": true,
                    "sortable": true,
                    "facetable": false
                }]
            },
            "storage_fixture": "reference/vespa/m5/fixtures/persisted-v1/index",
            "mutations": [
                "put doc1 attr1=10",
                "put doc2 attr1=20",
                "put doc3 attr1=30",
                "update doc2 attr1=10",
                "remove doc3",
                "update doc1 attr1=40",
                "put doc4 attr1=10"
            ],
            "expected_queries": {
                "10": ["doc2", "doc4"],
                "20": [],
                "30": [],
                "40": ["doc1"]
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

fn embedded_config_files() -> [(&'static str, &'static str); 7] {
    [
        ("searchkernel-schema.json", "{\"version\":1,\"document_type\":\"typea\",\"fields\":[{\"name\":\"attr1\",\"type\":\"integer\",\"index\":false,\"attribute\":true,\"summary\":true}]}\n"),
        ("indexschema.cfg", "indexfield[0]\n"),
        ("attributes.cfg", "attribute[1]\nattribute[0].name \"attr1\"\nattribute[0].datatype INT32\nattribute[0].collectiontype SINGLE\n"),
        ("summary.cfg", "defaultsummaryid 1\nclasses[1]\nclasses[0].id 1\nclasses[0].name \"default\"\nclasses[0].fields[4]\nclasses[0].fields[0].name \"documentid\"\nclasses[0].fields[0].type \"longstring\"\nclasses[0].fields[0].command \"documentid\"\nclasses[0].fields[0].source \"\"\nclasses[0].fields[1].name \"rankfeatures\"\nclasses[0].fields[1].type \"featuredata\"\nclasses[0].fields[1].command \"rankfeatures\"\nclasses[0].fields[1].source \"\"\nclasses[0].fields[2].name \"summaryfeatures\"\nclasses[0].fields[2].type \"featuredata\"\nclasses[0].fields[2].command \"summaryfeatures\"\nclasses[0].fields[2].source \"\"\nclasses[0].fields[3].name \"attr1\"\nclasses[0].fields[3].type \"integer\"\nclasses[0].fields[3].command \"\"\nclasses[0].fields[3].source \"attr1\"\n"),
        ("rank-profiles.cfg", "rankprofile[1]\nrankprofile[0].name \"default\"\n"),
        ("juniperrc.cfg", ""),
        ("proton.cfg", "documentdb[1]\ndocumentdb[0].inputdoctypename \"typea\"\ndocumentdb[0].document_id_attribute true\n"),
    ]
}

fn embedded_config_identity() -> serde_json::Value {
    serde_json::json!({
        "searchkernel-schema.json": "{\"version\":1,\"document_type\":\"typea\",\"fields\":[{\"name\":\"attr1\",\"type\":\"integer\",\"index\":false,\"attribute\":true,\"summary\":true}]}\n",
        "indexschema.cfg": "indexfield[0]\n",
        "attributes.cfg": "attribute[1]\nattribute[0].name \"attr1\"\nattribute[0].datatype INT32\nattribute[0].collectiontype SINGLE\n",
        "summary.cfg": "defaultsummaryid 1\nclasses[1]\nclasses[0].id 1\nclasses[0].name \"default\"\nclasses[0].fields[3]\nclasses[0].fields[0].name \"documentid\"\nclasses[0].fields[0].type \"longstring\"\nclasses[0].fields[0].command \"documentid\"\nclasses[0].fields[0].source \"\"\nclasses[0].fields[1].name \"rankfeatures\"\nclasses[0].fields[1].type \"featuredata\"\nclasses[0].fields[1].command \"rankfeatures\"\nclasses[0].fields[1].source \"\"\nclasses[0].fields[2].name \"summaryfeatures\"\nclasses[0].fields[2].type \"featuredata\"\nclasses[0].fields[2].command \"summaryfeatures\"\nclasses[0].fields[2].source \"\"\n",
        "rank-profiles.cfg": "rankprofile[1]\nrankprofile[0].name \"default\"\n",
        "juniperrc.cfg": "",
        "proton.cfg": "documentdb[1]\ndocumentdb[0].inputdoctypename \"typea\"\ndocumentdb[0].document_id_attribute true\n"
    })
}

fn write_readme(output: &Path) {
    let readme = r#"# Persistence compatibility fixtures

These files are immutable compatibility artifacts. The manifest version, canonical
schema bytes, provenance, mutation history, and expected queries are checked in.
The Vespa storage-generation-1 payload is the immutable M5 fixture referenced by
each metadata file and is copied by the verifier before mutation.

Do not regenerate or edit an existing version fixture after it lands. A format
change adds a new fixture directory.
"#;
    fs::write(output.join("README.md"), readme).expect("write fixture README");
}
