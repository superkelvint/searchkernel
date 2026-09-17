#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
root=$(c6_root); flatc=$(find_flatc); binary=$(bindgen_binary)
tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
mkdir -p "$tmp/bin" "$tmp/run"; cp "$binary" "$tmp/bin/searchkernel-bindgen"; binary="$tmp/bin/searchkernel-bindgen"
make_bfbs() {
  local mode=$1 work=$2
  mkdir -p "$work/schema" "$work/bfbs"; cp "$root/api/"*.fbs "$work/schema/"
  python3 "$(dirname "${BASH_SOURCE[0]}")/mutate-schema.py" "$mode" "$work/schema"
  generate_bfbs "$flatc" "$work/schema" "$work/bfbs"
}
probe="$tmp/probe"; make_bfbs probe "$probe"; mkdir -p "$probe/out"
( cd "$tmp/run"; "$binary" generate --bfbs-root "$probe/bfbs" --output-root "$probe/out" )
model="$probe/out/reference/json-conformance/generated/schema-model.json"; client="$probe/out/core/crates/searchkernel-client/src/generated.rs"; python_client="$probe/out/clients/python/searchkernel/generated/client.py"
[[ -f "$model" && -f "$client" && -f "$python_client" ]] || { echo 'BFBS-only probe generation missing artifacts' >&2; exit 1; }
python3 - "$model" <<'PY2'
import json,sys
model=json.load(open(sys.argv[1])); ops={x['operation']:x for x in model.get('operations',[])}; probe=ops.get('verifier_probe')
if not probe: raise SystemExit('missing verifier_probe derived from mutated BFBS')
if not probe.get('request','').endswith('VerifierProbeRequest'): raise SystemExit(f'wrong probe request: {probe}')
if not probe.get('response','').endswith('VerifierProbeResult'): raise SystemExit(f'wrong probe result: {probe}')
if 'verifier_probe_field' not in json.dumps(model,sort_keys=True): raise SystemExit('missing verifier_probe_field derived from mutated BFBS')
PY2
grep -Eq '^[[:space:]]*pub fn verifier_probe[[:space:]]*\(' "$client" || { echo 'generated Rust client missing executable verifier_probe method' >&2; exit 1; }
grep -Eq '^[[:space:]]*pub verifier_probe_field:' "$client" || { echo 'generated Rust model missing verifier_probe_field member' >&2; exit 1; }
grep -Eq '^[[:space:]]*def verifier_probe[[:space:]]*\(' "$python_client" || { echo 'generated Python client missing executable verifier_probe method' >&2; exit 1; }
grep -Eq '^[[:space:]]*verifier_probe_field:' "$python_client" || { echo 'generated Python model missing verifier_probe_field member' >&2; exit 1; }

# Compile the generated client against bindings produced from the same mutated
# schema, then exercise the new field and operation over real FlatBuffers.
bindings="$probe/bindings"
"$flatc" --rust --rust-module-root-file --gen-all -o "$bindings" "$probe/schema/searchkernel_ffi_request.fbs"
"$flatc" --rust --rust-module-root-file --gen-all -o "$bindings" "$probe/schema/searchkernel_ffi_response.fbs"
python3 - "$bindings/mod.rs" <<'PYRUSTMOD'
from pathlib import Path
import sys

path = Path(sys.argv[1])
text = path.read_text()
needle = "    pub mod v1 {\n      use super::*;\n"
replacement = needle + "      pub use super::super::v1::*;\n      mod request_payload_generated;\n      pub use self::request_payload_generated::*;\n      mod request_envelope_generated;\n      pub use self::request_envelope_generated::*;\n"
if text.count(needle) != 1:
    raise SystemExit("could not merge request and response Rust binding modules")
path.write_text(text.replace(needle, replacement, 1))
PYRUSTMOD
crate="$probe/rust-client"
mkdir -p "$crate/src" "$probe/searchkernel-ffi/generated"
cp -R "$bindings/." "$probe/searchkernel-ffi/generated/"
cp "$client" "$crate/src/generated.rs"
cat > "$crate/src/lib.rs" <<'RUSTLIB'
#[allow(warnings)]
mod generated;
RUSTLIB
python3 - "$crate/src/generated.rs" <<'PYRUSTTEST'
from pathlib import Path
import sys

path = Path(sys.argv[1])
with path.open("a") as output:
    output.write(r'''

#[cfg(test)]
mod mutation_wire_test {
    use super::*;

    struct ProbeTransport;

    impl Transport for ProbeTransport {
        type Error = ();

        fn call(&self, request: &[u8]) -> Result<Vec<u8>, Self::Error> {
            let envelope = root::<wire::RequestEnvelope<'_>>(request).unwrap();
            assert_eq!(
                envelope.payload_type(),
                wire::RequestPayload::SearchKernel_V1_VerifierProbeRequest
            );
            let request = envelope
                .payload_as_search_kernel_v1_verifier_probe_request()
                .unwrap();
            assert_eq!(request.token(), Some("descriptor-round-trip"));
            let request_payload = request.payload().unwrap();
            assert_eq!(request_payload.signed_value(), -9_223_372_036_854_775_000);
            assert_eq!(request_payload.ratio(), 125.5);
            assert!(request_payload.enabled());
            assert_eq!(request_payload.mode(), wire::FieldKind::Bool);
            assert_eq!(
                request.value_type(),
                wire::VerifierProbeValue::VerifierProbePayload
            );
            assert_eq!(
                request
                    .value_as_verifier_probe_payload()
                    .unwrap()
                    .mode(),
                wire::FieldKind::Bool
            );

            let mut builder = FlatBufferBuilder::new();
            let token = builder.create_string(request.token().unwrap());
            let scalar_payload = wire::VerifierProbePayload::create(
                &mut builder,
                &wire::VerifierProbePayloadArgs {
                    signed_value: -9_223_372_036_854_775_000,
                    ratio: 125.5,
                    enabled: true,
                    mode: wire::FieldKind::Bool,
                },
            );
            let payload = wire::VerifierProbeResult::create(
                &mut builder,
                &wire::VerifierProbeResultArgs {
                    token: Some(token),
                    payload: Some(scalar_payload),
                    value_type: wire::VerifierProbeValue::VerifierProbePayload,
                    value: Some(scalar_payload.as_union_value()),
                },
            );
            let response = wire::ResponseEnvelope::create(
                &mut builder,
                &wire::ResponseEnvelopeArgs {
                    protocol_version: 1,
                    payload_type: wire::ResponsePayload::SearchKernel_V1_VerifierProbeResult,
                    payload: Some(payload.as_union_value()),
                },
            );
            wire::finish_response_envelope_buffer(&mut builder, response);
            Ok(builder.finished_data().to_vec())
        }
    }

    #[test]
    fn mutated_field_and_operation_compile_and_round_trip() {
        let search = model::SearchRequest {
            verifier_probe_field: 0xfeed_beef,
            ..Default::default()
        };
        let encoded = encode_operation_search(search).unwrap();
        let envelope = root::<wire::RequestEnvelope<'_>>(&encoded).unwrap();
        assert_eq!(
            envelope.payload_type(),
            wire::RequestPayload::SearchKernel_V1_SearchRequest
        );
        assert_eq!(
            envelope
                .payload_as_search_kernel_v1_search_request()
                .unwrap()
                .verifier_probe_field(),
            0xfeed_beef
        );

        let scalar_payload = model::VerifierProbePayload {
            signed_value: -9_223_372_036_854_775_000,
            ratio: 125.5,
            enabled: true,
            mode: model::FieldKind::Bool,
        };
        let expected = model::VerifierProbeResult {
            token: Some("descriptor-round-trip".to_owned()),
            payload: Some(scalar_payload.clone()),
            value: model::VerifierProbeValue::VerifierProbePayload(scalar_payload.clone()),
        };
        let actual = Client::new(ProbeTransport)
            .verifier_probe(model::VerifierProbeRequest {
                token: Some("descriptor-round-trip".to_owned()),
                payload: Some(scalar_payload.clone()),
                value: model::VerifierProbeValue::VerifierProbePayload(scalar_payload),
            })
            .unwrap();
        assert_eq!(actual, expected);
    }
}
''')
PYRUSTTEST
cat > "$crate/Cargo.toml" <<'TOML'
[package]
name = "searchkernel-bindgen-mutation-proof"
version = "0.0.0"
edition = "2021"

[dependencies]
flatbuffers = "=25.12.19"

[workspace]
TOML
cargo test --quiet --manifest-path "$crate/Cargo.toml"
enum="$tmp/enum"; make_bfbs enum "$enum"; mkdir -p "$enum/out"
if ( cd "$tmp/run"; "$binary" generate --bfbs-root "$enum/bfbs" --output-root "$enum/out" ) >"$enum/stdout" 2>"$enum/stderr"; then
  enum_client="$enum/out/core/crates/searchkernel-client/src/generated.rs"
  enum_python="$enum/out/clients/python/searchkernel/generated/client.py"
  enum_model="$enum/out/reference/json-conformance/generated/schema-model.json"
  grep -q 'VerifierSynthetic' "$enum_client" || { echo 'generated client missing synthetic enum variant' >&2; exit 1; }
  grep -q 'model::ErrorCode::VerifierSynthetic => wire::ErrorCode(11)' "$enum_client" || { echo 'generated client missing synthetic enum encode mapping' >&2; exit 1; }
  grep -q '11 => Ok(model::ErrorCode::VerifierSynthetic)' "$enum_client" || { echo 'generated client missing synthetic enum decode mapping' >&2; exit 1; }
  grep -q 'verifier_synthetic' "$enum_python" || { echo 'generated Python missing synthetic enum mapping' >&2; exit 1; }
  python3 - "$enum_model" <<'PY3'
import json
import sys

model = json.load(open(sys.argv[1], encoding="utf-8"))
if "VerifierSynthetic" not in json.dumps(model, sort_keys=True):
    raise SystemExit("normalized model missing synthetic enum variant")
PY3
else
  grep -q 'ErrorCode' "$enum/stderr" "$enum/stdout" || { echo 'enum rejection did not identify ErrorCode' >&2; exit 1; }
  grep -q 'VerifierSynthetic' "$enum/stderr" "$enum/stdout" || { echo 'enum rejection did not identify VerifierSynthetic' >&2; exit 1; }
fi
orphan="$tmp/orphan"; make_bfbs orphan "$orphan"; mkdir -p "$orphan/out"
set +e; ( cd "$tmp/run"; "$binary" generate --bfbs-root "$orphan/bfbs" --output-root "$orphan/out" ) >"$orphan/stdout" 2>"$orphan/stderr"; status=$?; set -e
[[ $status -ne 0 ]] || { echo 'bindgen accepted unpaired VerifierOrphanRequest' >&2; exit 1; }
grep -q 'VerifierOrphanRequest' "$orphan/stderr" "$orphan/stdout" || { echo 'orphan diagnostic did not identify VerifierOrphanRequest' >&2; exit 1; }
bad="$tmp/bad"; mkdir -p "$bad/bfbs" "$bad/out"; printf 'not-bfbs\n' > "$bad/bfbs/searchkernel_ffi_request.bfbs"; cp "$root/core/crates/searchkernel-json/bfbs/searchkernel_ffi_response.bfbs" "$bad/bfbs/"
set +e; ( cd "$tmp/run"; "$binary" generate --bfbs-root "$bad/bfbs" --output-root "$bad/out" ) >"$bad/stdout" 2>"$bad/stderr"; status=$?; set -e
[[ $status -ne 0 ]] || { echo 'bindgen accepted malformed BFBS' >&2; exit 1; }
echo 'C6 BFBS-only + mutation/fail-closed generation PASS'
