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
        assert_eq!(input["protocol_version"], 2, "{name}: protocol version");
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
        count, 21,
        "expected all ten request/response operation discriminators plus canonical Error"
    );
}
