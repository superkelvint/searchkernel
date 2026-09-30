#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
cd "$ROOT"
source "$ROOT/reference/vespa/common/verify-common.sh"

prepare_native_probe_sdk
require_pinned_vespa
require_sdk_manifest "$SDK/manifest.json"
NATIVE_LIBRARY="$SDK/lib/libsearchkernel_vespa.so"
[[ -f "$NATIVE_LIBRARY" ]] || fail_verification "native SDK library is missing: $NATIVE_LIBRARY"
printf 'Phase 10 native provenance: SDK=%s ABI=' "$SDK"
python3 - "$SDK/manifest.json" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as stream:
    print(json.load(stream)["native_abi_version"])
PY
printf 'library_sha256='
sha256sum "$NATIVE_LIBRARY" | awk '{print $1}'

EXAMPLE="$ROOT/core/crates/searchkernel/examples/try_searchkernel_client_api_e2e.rs"
[[ -f "$EXAMPLE" ]] || fail_verification "canonical Phase 10 Rust E2E is missing: $EXAMPLE"

# Keep this acceptance driver on the public facade. The executable itself is
# the scenario; these checks prevent an accidental low-level escape hatch from
# turning it into a second adapter or ABI test.
if rg -n 'searchkernel-(ir|ir-adapter|vespa-engine|vespa-bindings)|searchkernel_ir|searchkernel_vespa|RecordingBackend|ScriptedBackend|Dispatcher|sk_[a-zA-Z0-9_]+' "$EXAMPLE"; then
  fail_verification "canonical Phase 10 E2E references a lower-level crate, backend, dispatcher, or native ABI"
fi

rustfmt --edition 2021 --check "$EXAMPLE"
cargo clippy --quiet -p searchkernel --example try_searchkernel_client_api_e2e \
  --features native --all-targets -- -D warnings

LOG="$CARGO_TARGET_DIR/phase10-e2e.log"
mkdir -p "$CARGO_TARGET_DIR"
cargo run --quiet -p searchkernel --features native \
  --example try_searchkernel_client_api_e2e | tee "$LOG"

for marker in \
  'CREATE INDEX' \
  'typed term/phrase/AND/OR/NOT/equality/inclusive-exclusive range/IN/weakAnd/WAND/weighted-set/dot-product/fuzzy/literal prefix/substring/suffix PASS' \
  'textual filter parser, clause-local bindings, grammar, and syntax errors PASS' \
  'nested grouping keys, count, and average values PASS' \
  'valid degraded result retains hits, coverage, and degradation reason PASS' \
  'default summary selection + projection docsums PASS' \
  'search_fields/language/timeout/inspect/missing-first, field-aware highlighting, and Vespa highlighting PASS' \
  'ranking configuration, grouping source data, tensor persistence, and ANN after reopen PASS'; do
  rg -F "$marker" "$LOG" >/dev/null ||
    fail_verification "canonical Phase 10 E2E did not report required coverage: $marker"
done

echo 'Phase 10 public Rust real-engine E2E verification passed'
