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
  rm -rf "$work/schema"
}
probe="$tmp/probe"; make_bfbs probe "$probe"; mkdir -p "$probe/out"
( cd "$tmp/run"; "$binary" generate --bfbs-root "$probe/bfbs" --output-root "$probe/out" )
opmap="$probe/out/reference/json-conformance/generated/operation-map.json"; client="$probe/out/crates/searchkernel-client/src/generated.rs"; schema="$probe/out/reference/json-conformance/generated/searchkernel.schema.json"
json_generated="$probe/out/crates/searchkernel-json/src/generated.rs"
[[ -f "$opmap" && -f "$client" && -f "$schema" && -f "$json_generated" ]] || { echo 'BFBS-only probe generation missing artifacts' >&2; exit 1; }
python3 - "$opmap" "$schema" <<'PY2'
import json,sys
opmap=json.load(open(sys.argv[1])); ops={x['operation']:x for x in opmap.get('operations',[])}; probe=ops.get('verifier_probe')
if not probe: raise SystemExit('missing verifier_probe derived from mutated BFBS')
if not probe.get('request','').endswith('VerifierProbeRequest'): raise SystemExit(f'wrong probe request: {probe}')
if not probe.get('response','').endswith('VerifierProbeResult'): raise SystemExit(f'wrong probe result: {probe}')
schema=json.load(open(sys.argv[2]));
if 'verifier_probe_field' not in json.dumps(schema,sort_keys=True): raise SystemExit('missing verifier_probe_field derived from mutated BFBS')
PY2
grep -Eq '\bfn[[:space:]]+verifier_probe[[:space:]]*\(' "$client" || { echo 'generated client missing verifier_probe' >&2; exit 1; }
grep -q 'verifier_probe_field' "$client" || { echo 'generated client model missing verifier_probe_field' >&2; exit 1; }
grep -q 'verifier_probe_field' "$json_generated" || { echo 'generated JSON codec missing verifier_probe_field' >&2; exit 1; }
orphan="$tmp/orphan"; make_bfbs orphan "$orphan"; mkdir -p "$orphan/out"
set +e; ( cd "$tmp/run"; "$binary" generate --bfbs-root "$orphan/bfbs" --output-root "$orphan/out" ) >"$orphan/stdout" 2>"$orphan/stderr"; status=$?; set -e
[[ $status -ne 0 ]] || { echo 'bindgen accepted unpaired VerifierOrphanRequest' >&2; exit 1; }
grep -q 'VerifierOrphanRequest' "$orphan/stderr" "$orphan/stdout" || { echo 'orphan diagnostic did not identify VerifierOrphanRequest' >&2; exit 1; }
bad="$tmp/bad"; mkdir -p "$bad/bfbs" "$bad/out"; printf 'not-bfbs\n' > "$bad/bfbs/searchkernel_ffi_request.bfbs"; cp "$root/crates/searchkernel-json/bfbs/searchkernel_ffi_response.bfbs" "$bad/bfbs/"
set +e; ( cd "$tmp/run"; "$binary" generate --bfbs-root "$bad/bfbs" --output-root "$bad/out" ) >"$bad/stdout" 2>"$bad/stderr"; status=$?; set -e
[[ $status -ne 0 ]] || { echo 'bindgen accepted malformed BFBS' >&2; exit 1; }
echo 'C6 BFBS-only + mutation/fail-closed generation PASS'
