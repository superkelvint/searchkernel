use std::{
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
};

use searchkernel_protocol::{
    json::{
        request_flatbuffer_to_json, request_json_to_flatbuffer, response_flatbuffer_to_json,
        response_json_to_flatbuffer,
    },
    protocol::v1::{RequestEnvelope, RequestPayload},
};

fn assert_php_consumes_printed_response(repo_root: &std::path::Path, name: &str, json: &[u8]) {
    let script = repo_root.join("clients/php/tests/consume_bfbs_response.php");
    let mut child = Command::new("php")
        .arg(&script)
        .arg(name)
        .current_dir(repo_root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|error| panic!("{name}: start PHP BFBS response consumer: {error}"));

    child
        .stdin
        .take()
        .expect("PHP consumer stdin")
        .write_all(json)
        .unwrap_or_else(|error| panic!("{name}: write BFBS-printed JSON to PHP: {error}"));

    let output = child
        .wait_with_output()
        .unwrap_or_else(|error| panic!("{name}: wait for PHP BFBS response consumer: {error}"));
    assert!(
        output.status.success(),
        "{name}: PHP rejected canonical BFBS-printed response:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

#[test]
#[ignore = "requires the PHP runtime; run explicitly from client verification"]
fn php_emitted_standard_json_matches_canonical_bfbs() {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("repository root");
    let script = repo_root.join("clients/php/tests/emit_bfbs_contract.php");

    let output = Command::new("php")
        .arg(&script)
        .current_dir(&repo_root)
        .output()
        .expect("run PHP BFBS contract fixture emitter");
    assert!(
        output.status.success(),
        "PHP BFBS fixture emitter failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("PHP fixture output is UTF-8");
    let mut count = 0usize;

    for (index, line) in stdout.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        count += 1;
        let mut parts = line.splitn(3, '\t');
        let schema = parts.next().expect("fixture schema");
        let name = parts.next().expect("fixture name");
        let json = parts
            .next()
            .unwrap_or_else(|| panic!("fixture {} missing JSON", index + 1))
            .as_bytes();

        let input: serde_json::Value = serde_json::from_slice(json)
            .unwrap_or_else(|error| panic!("{name}: emitted invalid JSON: {error}"));
        assert_eq!(input["protocol_version"], 1, "{name}: protocol version");
        let input_type = input["payload_type"]
            .as_str()
            .unwrap_or_else(|| panic!("{name}: payload_type missing"))
            .to_owned();

        let printed = match schema {
            "request" => {
                let buffer = request_json_to_flatbuffer(json)
                    .unwrap_or_else(|error| panic!("{name}: BFBS request parse failed: {error}"));
                assert_eq!(&buffer[4..8], b"SKRQ", "{name}: request identifier");
                let envelope =
                    flatbuffers::root::<RequestEnvelope<'_>>(&buffer).unwrap_or_else(|error| {
                        panic!("{name}: daemon request verifier rejected PHP JSON: {error}")
                    });
                if name == "create-index-e2e-schema" {
                    assert_eq!(
                        envelope.payload_type(),
                        RequestPayload::SearchKernel_Wire_CreateIndexRequest,
                        "{name}: daemon sees create-index discriminator"
                    );
                    let payload = envelope
                        .payload_as_search_kernel_wire_create_index_request()
                        .unwrap_or_else(|| {
                            panic!("{name}: daemon cannot access create-index payload")
                        });
                    let definition = payload
                        .definition()
                        .unwrap_or_else(|| panic!("{name}: create-index definition missing"));
                    assert_eq!(
                        definition.fields().map(|fields| fields.len()),
                        Some(13),
                        "{name}: exact PHP E2E field count"
                    );
                    assert_eq!(
                        definition.rank_profiles().map(|profiles| profiles.len()),
                        Some(6),
                        "{name}: exact PHP E2E rank-profile count"
                    );
                    assert_eq!(
                        envelope.target().map(|target| target.name()),
                        Some(Some("php-e2e")),
                        "{name}: exact PHP E2E target"
                    );
                }
                if name == "open-index-empty-table" {
                    assert_eq!(
                        envelope.payload_type(),
                        RequestPayload::SearchKernel_Wire_OpenIndexRequest,
                        "{name}: daemon sees open-index discriminator"
                    );
                    envelope
                        .payload_as_search_kernel_wire_open_index_request()
                        .unwrap_or_else(|| {
                            panic!("{name}: daemon cannot access open-index payload")
                        });
                    assert_eq!(
                        envelope.target().map(|target| target.name()),
                        Some(Some("php-e2e")),
                        "{name}: exact PHP E2E target"
                    );
                }
                request_flatbuffer_to_json(&buffer)
                    .unwrap_or_else(|error| panic!("{name}: request print failed: {error}"))
            }
            "response" => {
                let buffer = response_json_to_flatbuffer(json)
                    .unwrap_or_else(|error| panic!("{name}: BFBS response parse failed: {error}"));
                assert_eq!(&buffer[4..8], b"SKRS", "{name}: response identifier");
                response_flatbuffer_to_json(&buffer)
                    .unwrap_or_else(|error| panic!("{name}: response print failed: {error}"))
            }
            other => panic!("{name}: unknown fixture schema {other}"),
        };

        let round_trip: serde_json::Value = serde_json::from_slice(&printed)
            .unwrap_or_else(|error| panic!("{name}: BFBS printer emitted invalid JSON: {error}"));
        assert_eq!(
            round_trip["payload_type"].as_str(),
            Some(input_type.as_str()),
            "{name}: payload discriminator survives BFBS round trip"
        );

        match schema {
            "request" => {
                request_json_to_flatbuffer(&printed).unwrap_or_else(|error| {
                    panic!("{name}: BFBS-printed request is not readable: {error}")
                });
                if name == "insert-empty-string-bytes" {
                    let document = &round_trip["payload"]["document"];
                    let fields = document["fields"]
                        .as_array()
                        .expect("empty-value request fields survive as a vector");
                    let field = |key: &str| {
                        fields
                            .iter()
                            .find(|entry| entry["key"].as_str() == Some(key))
                            .unwrap_or_else(|| panic!("{name}: missing field {key}"))
                    };
                    assert_eq!(
                        field("title")["value"]["kind"]["value"].as_str(),
                        Some(""),
                        "{name}: empty StringValue must survive BFBS JSON parsing"
                    );
                    assert_eq!(
                        field("raw")["value"]["kind"]["value"]
                            .as_array()
                            .map(Vec::len),
                        Some(0),
                        "{name}: empty BytesValue must survive BFBS JSON parsing"
                    );
                }
            }
            "response" => {
                response_json_to_flatbuffer(&printed).unwrap_or_else(|error| {
                    panic!("{name}: BFBS-printed response is not readable: {error}")
                });
                assert_php_consumes_printed_response(&repo_root, name, &printed);
            }
            _ => unreachable!(),
        }
    }

    assert!(
        count >= 16,
        "expected representative bidirectional PHP/BFBS coverage, saw only {count} fixtures"
    );

    let fixture_root =
        repo_root.join("core/crates/searchkernel-dispatch/tests/fixtures/portable/responses");
    let mut facet_count = 0usize;
    for entry in std::fs::read_dir(&fixture_root).expect("read shared response fixture directory") {
        let entry = entry.expect("read shared response fixture entry");
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.starts_with("result-facet-") || !name.ends_with(".skrs") {
            continue;
        }
        facet_count += 1;
        let bytes = std::fs::read(entry.path())
            .unwrap_or_else(|error| panic!("{name}: read shared facet fixture: {error}"));
        let printed = response_flatbuffer_to_json(&bytes)
            .unwrap_or_else(|error| panic!("{name}: print shared facet fixture: {error}"));
        assert_php_consumes_printed_response(
            &repo_root,
            &format!("shared-facet:{}", name.trim_end_matches(".skrs")),
            &printed,
        );
    }
    assert!(
        facet_count >= 20,
        "expected the shared facet response corpus, saw only {facet_count} fixtures"
    );
}
