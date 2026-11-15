use std::{
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
};

use searchkernel_protocol::json::{
    request_flatbuffer_to_json, request_json_to_flatbuffer, response_flatbuffer_to_json,
    response_json_to_flatbuffer,
};

fn assert_ruby_consumes_printed_response(repo_root: &std::path::Path, name: &str, json: &[u8]) {
    let script = repo_root.join("clients/ruby/test/consume_bfbs_response.rb");
    let mut child = Command::new("ruby")
        .arg(&script)
        .arg(name)
        .current_dir(repo_root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|error| panic!("{name}: start Ruby BFBS response consumer: {error}"));

    child
        .stdin
        .take()
        .expect("Ruby consumer stdin")
        .write_all(json)
        .unwrap_or_else(|error| panic!("{name}: write BFBS-printed JSON to Ruby: {error}"));

    let output = child
        .wait_with_output()
        .unwrap_or_else(|error| panic!("{name}: wait for Ruby BFBS response consumer: {error}"));
    assert!(
        output.status.success(),
        "{name}: Ruby rejected canonical BFBS-printed response:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

#[test]
#[ignore = "requires the Ruby runtime; run explicitly from client verification"]
fn ruby_emitted_standard_json_matches_canonical_bfbs() {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("repository root");
    let script = repo_root.join("clients/ruby/test/emit_bfbs_contract.rb");

    let output = Command::new("ruby")
        .arg(&script)
        .current_dir(&repo_root)
        .output()
        .expect("run Ruby BFBS contract fixture emitter");
    assert!(
        output.status.success(),
        "Ruby BFBS fixture emitter failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("Ruby fixture output is UTF-8");
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
                if name == "portable-values-request" {
                    let entries = round_trip["payload"]["document"]["fields"]
                        .as_array()
                        .expect("BFBS printed ValueEntry vector");
                    let value = |key: &str| {
                        &entries
                            .iter()
                            .find(|entry| entry["key"] == key)
                            .unwrap_or_else(|| panic!("missing {key}"))["value"]
                    };
                    assert_eq!(value("min")["kind_type"], "Int64Value");
                    assert_eq!(
                        value("min")["kind"]["value"].as_str(),
                        Some("-9223372036854775808")
                    );
                    assert_eq!(
                        value("max")["kind"]["value"].as_str(),
                        Some("9223372036854775807")
                    );
                    assert_eq!(value("unsigned")["kind_type"], "UInt64Value");
                    assert_eq!(
                        value("unsigned")["kind"]["value"].as_str(),
                        Some("18446744073709551615")
                    );
                    for (key, kind) in [
                        ("dense", "DenseTensorValues"),
                        ("sparse", "SparseTensorCells"),
                        ("mixed", "MixedTensorBlocks"),
                    ] {
                        assert_eq!(value(key)["kind"]["payload_type"], kind);
                    }
                    assert_eq!(value("bytes")["kind"]["value"], serde_json::json!([0, 255]));
                    assert_eq!(value("nested")["kind_type"], "ValueList");

                    let unicode_keys = |document: &serde_json::Value| {
                        document["payload"]["document"]["fields"]
                            .as_array()
                            .expect("ValueEntry vector")
                            .iter()
                            .find(|entry| entry["key"] == "unicode_object")
                            .expect("unicode_object field")["value"]["kind"]["fields"]
                            .as_array()
                            .expect("unicode ValueObject fields")
                            .iter()
                            .map(|entry| {
                                entry["key"]
                                    .as_str()
                                    .expect("unicode object key")
                                    .to_owned()
                            })
                            .collect::<Vec<_>>()
                    };
                    let expected_unicode_order = vec!["a", "z", "é", "😀"];
                    assert_eq!(
                        unicode_keys(&input),
                        expected_unicode_order,
                        "Ruby must emit canonical UTF-8 key ordering before BFBS parsing"
                    );
                    assert_eq!(
                        unicode_keys(&round_trip),
                        expected_unicode_order,
                        "canonical Unicode key ordering must survive BFBS parse/print"
                    );
                }
            }
            "response" => {
                response_json_to_flatbuffer(&printed).unwrap_or_else(|error| {
                    panic!("{name}: BFBS-printed response is not readable: {error}")
                });
                assert_ruby_consumes_printed_response(&repo_root, name, &printed);
            }
            _ => unreachable!(),
        }
    }

    assert_eq!(
        count, 34,
        "expected operation discriminators including diagnostics, Error, values and recursive result"
    );
}

#[test]
#[ignore = "requires the Ruby runtime; run explicitly from client verification"]
fn ruby_rejects_adversarial_values_after_bfbs_printing() {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("repository root");
    let cases: &[(&str, &[u8])] = &[
        (
            "duplicate nested object key",
            br#"{"protocol_version":1,"payload_type":"SearchKernel_Wire_GetResult","payload":{"document":{"id":"id:bad","fields":[{"key":"bad","value":{"kind_type":"ValueObject","kind":{"fields":[{"key":"x","value":{"kind_type":"Int64Value","kind":{"value":1}}},{"key":"x","value":{"kind_type":"Int64Value","kind":{"value":2}}}]}}}]}}}"#,
        ),
        (
            "missing Value discriminator",
            br#"{"protocol_version":1,"payload_type":"SearchKernel_Wire_GetResult","payload":{"document":{"id":"id:bad","fields":[{"key":"bad","value":{"kind":{"value":1}}}]}}}"#,
        ),
    ];

    for (name, input) in cases {
        // The BFBS parser may reject malformed unions itself. If it accepts
        // one structurally, the Ruby consumer must reject the printed form.
        if let Ok(buffer) = response_json_to_flatbuffer(input) {
            let printed = response_flatbuffer_to_json(&buffer)
                .unwrap_or_else(|error| panic!("{name}: BFBS print failed: {error}"));
            assert_ruby_consumes_printed_response(&repo_root, "rejected-value-response", &printed);
        }
    }

    // Scalar fields use FlatBuffers defaults. An Int64Value table with its
    // scalar omitted therefore means canonical zero, not a missing payload.
    // Verify the BFBS printer and Ruby decoder agree on that default.
    let canonical_default = br#"{"protocol_version":1,"payload_type":"SearchKernel_Wire_GetResult","payload":{"document":{"id":"id:default","fields":[{"key":"zero","value":{"kind_type":"Int64Value","kind":{}}}]}}}"#;
    let buffer = response_json_to_flatbuffer(canonical_default)
        .expect("canonical Int64Value default must parse");
    let printed =
        response_flatbuffer_to_json(&buffer).expect("canonical Int64Value default must print");
    assert_ruby_consumes_printed_response(&repo_root, "portable-int64-default-response", &printed);
}
