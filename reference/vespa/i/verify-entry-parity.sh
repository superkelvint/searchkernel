#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
cd "$ROOT"
source "$ROOT/reference/vespa/common/verify-common.sh"
require_pinned_vespa
require_sdk_manifest "$SDK/manifest.json"
TEST="$ROOT/searchkerneld/tests/real_engine_entry_parity.rs"
[[ -f "$TEST" ]] || fail_verification "real-engine entry parity test is missing"
if grep -E 'RecordingBackend|ScriptedBackend|searchkernel_testkit' "$TEST"; then
  fail_verification "real-engine entry parity test references a fake backend"
fi
grep -F 'Index::create' "$TEST" >/dev/null || fail_verification "Rust-direct path is missing"
grep -F 'env!("CARGO_BIN_EXE_searchkerneld")' "$TEST" >/dev/null || fail_verification "production searchkerneld process launch is missing"
grep -F '.arg("--data-root")' "$TEST" >/dev/null || fail_verification "multi-index daemon data root is missing"
grep -F 'RemoteClient::connect' "$TEST" >/dev/null || fail_verification "Rust remote daemon client path is missing"
grep -F '.create_index("entry-parity", definition())' "$TEST" >/dev/null || fail_verification "canonical Rust CreateIndex lifecycle is missing"
grep -F 'assert_eq!(grpc_outcome, direct_outcome' "$TEST" >/dev/null || fail_verification "cross-entry semantic comparison is missing"
"$SCRIPT_DIR/test-entry-parity-gate.sh"
cargo test -p searchkerneld --test real_engine_entry_parity --features native -- --nocapture
echo 'real-engine Rust-direct vs gRPC semantic parity passed'
