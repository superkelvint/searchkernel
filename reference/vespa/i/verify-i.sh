#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
cd "$ROOT"
source "$ROOT/reference/vespa/common/verify-common.sh"
parse_cumulative_verifier_args "$@"
CHILD_VERIFIER_ARGS=()
if [[ "$SKIP_WORKSPACE_QUALITY" == 1 ]]; then
  CHILD_VERIFIER_ARGS+=(--skip-workspace-quality)
fi

require_pinned_vespa
require_sdk_manifest "$SDK/manifest.json"
python3 "$SCRIPT_DIR/validate.py"

# I-series is only meaningful when the latest frozen cumulative native
# baseline remains green. M13 is the current baseline; it transitively
# preserves M0-M12 before the public Phase 10 product-path check runs.
if [[ "$BASELINE_ALREADY_PROVED" != 1 ]]; then
  "$ROOT/reference/vespa/verify-through.sh" m13 "${CHILD_VERIFIER_ARGS[@]}"
fi
"$ROOT/reference/vespa/verify-through.sh" phase10 "${CHILD_VERIFIER_ARGS[@]}"

if rg -n 'searchkernel-(ir-adapter|vespa-engine|vespa-bindings)|searchkernel_ir|searchkernel_vespa|RecordingBackend|ScriptedBackend' \
    "$SCRIPT_DIR/rust-driver/Cargo.toml" "$SCRIPT_DIR/rust-driver/src"; then
  echo "I-series driver imports a forbidden lower-level crate or test backend" >&2
  exit 1
fi

TARGET="$REPO_ROOT/target/vespa-verifier/i"
DRIVER_TARGET="$CARGO_TARGET_DIR/i-driver"
mkdir -p "$TARGET"

cargo fmt --manifest-path "$SCRIPT_DIR/rust-driver/Cargo.toml" -- --check
cargo clippy --quiet --manifest-path "$SCRIPT_DIR/rust-driver/Cargo.toml" --all-targets --all-features -- -D warnings
cargo build --quiet --manifest-path "$SCRIPT_DIR/rust-driver/Cargo.toml" \
  --target-dir "$DRIVER_TARGET"
DRIVER="$DRIVER_TARGET/debug/searchkernel-i-driver"
require_native_driver "$DRIVER" \
  sk_engine_open sk_engine_put sk_engine_update sk_engine_remove sk_engine_search

for scenario in "$SCRIPT_DIR"/scenarios/*.json; do
  id="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["id"])' "$scenario")"
  data="$TARGET/$id-data"
  mkdir -p "$data"
  LD_LIBRARY_PATH="$SDK/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
    "$DRIVER" "$scenario" "$data" \
    > "$TARGET/$id.actual.json" 2> "$TARGET/$id.native.log"
  python3 "$SCRIPT_DIR/compare.py" \
    "$SCRIPT_DIR/expected/$id.json" "$TARGET/$id.actual.json"
done

echo "I-series product-path verification passed"
