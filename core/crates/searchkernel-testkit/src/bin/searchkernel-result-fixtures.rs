//! Generate and check the canonical encoded result corpus.

use flatbuffers::FlatBufferBuilder;
use searchkernel_protocol::protocol::v2 as wire;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const DEFAULT_OUTPUT: &str = "core/crates/searchkernel-dispatch/tests/fixtures/portable/responses";
const RESPONSE_IDENTIFIER: &str = "SKRS";

fn encode_fixture(result: &searchkernel_ir::SearchResult) -> Vec<u8> {
    let mut builder = FlatBufferBuilder::new();
    let payload = searchkernel_ir::search_result_offset(&mut builder, result)
        .expect("canonical result fixture must validate")
        .as_union_value();
    let envelope = wire::ResponseEnvelope::create(
        &mut builder,
        &wire::ResponseEnvelopeArgs {
            protocol_version: searchkernel_protocol::PROTOCOL_VERSION,
            payload_type: wire::ResponsePayload::SearchKernel_V2_SearchResult,
            payload: Some(payload),
        },
    );
    builder.finish(envelope, Some(RESPONSE_IDENTIFIER));
    builder.finished_data().to_vec()
}

fn main() {
    let mut arguments = std::env::args_os().skip(1);
    let check = arguments
        .next()
        .is_some_and(|argument| argument == "--check");
    let output = arguments
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_OUTPUT));
    assert!(
        arguments.next().is_none(),
        "usage: searchkernel-result-fixtures [--check] [output-directory]"
    );

    if !check {
        std::fs::create_dir_all(&output).expect("create fixture output directory");
    }

    let fixtures = searchkernel_testkit::result_fixtures();
    let expected = fixtures
        .iter()
        .map(|fixture| format!("result-{}.skrs", fixture.name))
        .collect::<BTreeSet<_>>();
    for fixture in fixtures {
        let path = output.join(format!("result-{}.skrs", fixture.name));
        let bytes = encode_fixture(&fixture.result);
        if check {
            let existing = std::fs::read(&path)
                .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
            assert_eq!(existing, bytes, "fixture drifted: {}", path.display());
        } else {
            write_fixture(&path, &bytes);
        }
    }
    if check {
        for entry in std::fs::read_dir(&output).expect("read fixture output directory") {
            let name = entry
                .expect("read fixture directory entry")
                .file_name()
                .to_string_lossy()
                .into_owned();
            if name.starts_with("result-") && name.ends_with(".skrs") {
                assert!(
                    expected.contains(&name),
                    "unexpected result fixture: {name}"
                );
            }
        }
    }
}

fn write_fixture(path: &Path, bytes: &[u8]) {
    std::fs::write(path, bytes).unwrap_or_else(|error| panic!("write {}: {error}", path.display()));
}
