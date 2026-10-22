#!/usr/bin/env bash
set -euo pipefail

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
. "$ROOT/common/verify-common.sh"
cd "$REPO_ROOT"
parse_verifier_args "$@"

TARGET=$REPO_ROOT/target/vespa-verifier/m5
CONFIG="$SDK/share/embedded_crud_cfg"
mkdir -p "$TARGET"

"$ROOT/m4/verify-m4.sh" --skip-workspace-quality
run_workspace_quality

for scenario in "$ROOT/m5/scenarios"/*.json; do
    check_fixture_literals "$scenario" "$REPO_ROOT/core/crates/searchkernel-vespa-engine/src" \
        "$REPO_ROOT/core/crates/searchkernel-vespa-bindings/src" "$REPO_ROOT/core/native/vespa"
done

# The M5 persistence verifier is a workspace example owned by the native engine.
M5_DRIVER="$REPO_ROOT/core/crates/searchkernel-vespa-engine/examples/searchkernel-m5-driver.rs"
if rg -n 'searchkernel-(core|testkit)|searchkernel-ir|searchkernel-testkit|serde_json|HashMap|HashSet' \
    "$M5_DRIVER"; then
    echo "M5 driver has a forbidden dependency or persistence shortcut" >&2
    exit 1
fi
if ! rg -q 'std::process::abort\(\)' "$M5_DRIVER"; then
    echo "M5 crash writer does not terminate without the ordinary close path" >&2
    exit 1
fi

cargo build --quiet -p searchkernel-vespa-engine --example searchkernel-m5-driver
BIN=$CARGO_TARGET_DIR/debug/examples/searchkernel-m5-driver
if [ ! -x "$BIN" ]; then
    echo "M5 driver binary not found at $BIN" >&2
    exit 1
fi

require_native_driver "$BIN" sk_engine_open sk_engine_put sk_engine_update sk_engine_remove sk_engine_search_generic

if [ ! -d "$CONFIG" ]; then
    echo "M5 config directory not found: $CONFIG" >&2
    exit 1
fi

run_driver() {
    mode=$1
    data=$2
    shift 2
    run_driver_config "$mode" "$data" "$CONFIG" "$@"
}

run_driver_config() {
    mode=$1
    data=$2
    config=$3
    shift 3
    cargo run --quiet -p searchkernel-vespa-engine --example searchkernel-m5-driver -- \
        "$mode" "$data" "$config" "$@"
}

compare() {
    python3 "$ROOT/common/compare_json.py" "$1" "$2"
}

rm -rf "$TARGET/graceful" "$TARGET/repeated" "$TARGET/crash" "$TARGET/fixture" \
    "$TARGET/corrupt" "$TARGET/new" "$TARGET/bad-config" "$TARGET/bad-config-data"
mkdir -p "$TARGET/graceful" "$TARGET/repeated" "$TARGET/crash"

run_driver seed "$TARGET/graceful" > "$TARGET/graceful/seed.json"
compare "$ROOT/m5/expected/graceful-seed.json" "$TARGET/graceful/seed.json"
run_driver verify "$TARGET/graceful" > "$TARGET/graceful/verify.json"
compare "$ROOT/m5/expected/graceful-verify.json" "$TARGET/graceful/verify.json"
run_driver mutate "$TARGET/graceful" > "$TARGET/graceful/mutate.json"
compare "$ROOT/m5/expected/graceful-mutate.json" "$TARGET/graceful/mutate.json"
run_driver verify-final "$TARGET/graceful" > "$TARGET/graceful/final.json"
compare "$ROOT/m5/expected/graceful-final.json" "$TARGET/graceful/final.json"

run_driver open-only "$TARGET/new" > "$TARGET/new.json"
run_driver open-only "$TARGET/new" > "$TARGET/new-reopen.json"

run_driver repeat "$TARGET/repeated" 5 > "$TARGET/repeated.json"
compare "$ROOT/m5/expected/repeated-reopen.json" "$TARGET/repeated.json"

run_driver seed "$TARGET/crash" > "$TARGET/crash/seed.json"
set +e
run_driver crash-update "$TARGET/crash" > "$TARGET/crash/update.log" 2> "$TARGET/crash/update.err"
crash_status=$?
set -e
if [ "$crash_status" -eq 0 ]; then
    echo "crash-update unexpectedly returned successfully" >&2
    exit 1
fi
run_driver crash-verify "$TARGET/crash" > "$TARGET/crash/verify.json"
python3 - "$TARGET/crash/verify.json" \
    "$ROOT/m5/expected/crash-pre-update.json" \
    "$ROOT/m5/expected/crash-post-update.json" <<'PY'
import json
import sys

actual = json.load(open(sys.argv[1]))
allowed = [json.load(open(path)) for path in sys.argv[2:]]
if actual not in allowed:
    raise SystemExit(f"unexpected crash recovery state: {actual!r}")
PY

printf '%s\n' invalid > "$TARGET/not-a-directory"
if run_driver open-only "$TARGET/not-a-directory" > "$TARGET/invalid.out" 2> "$TARGET/invalid.err"; then
    echo "opening a regular file unexpectedly succeeded" >&2
    exit 1
fi
if [ ! -s "$TARGET/invalid.err" ]; then
    echo "invalid path did not preserve an error message" >&2
    exit 1
fi

if run_driver_config open-only "$TARGET/bad-config-data" "$TARGET/bad-config" \
    > "$TARGET/bad-config.out" 2> "$TARGET/bad-config.err"; then
    echo "opening with a missing config unexpectedly succeeded" >&2
    exit 1
fi
if [ ! -s "$TARGET/bad-config.err" ]; then
    echo "invalid configuration did not preserve an error message" >&2
    exit 1
fi

FIXTURE=$ROOT/m5/fixtures/persisted-v1/index
if [ ! -d "$FIXTURE" ]; then
    echo "persisted M5 fixture is missing: $FIXTURE" >&2
    exit 1
fi
python3 - "$ROOT/m5/fixtures/persisted-v1/METADATA.json" "$SDK/manifest.json" <<'PY'
import json
import sys

metadata = json.load(open(sys.argv[1]))
manifest = json.load(open(sys.argv[2]))
if metadata["created_by_native_abi"] != 4:
    raise SystemExit("fixture provenance was rewritten")
if metadata["created_by_vespa_revision"] != manifest["vespa_commit"]:
    raise SystemExit("fixture/Vespa revision metadata mismatch")
if metadata["target"] != "linux-x86_64":
    raise SystemExit("fixture target is not explicitly recorded")
PY
cp -a "$FIXTURE" "$TARGET/fixture"
run_driver verify-final "$TARGET/fixture" > "$TARGET/fixture.json"
compare "$ROOT/m5/expected/graceful-final.json" "$TARGET/fixture.json"

CORRUPT=$TARGET/corrupt
cp -a "$FIXTURE" "$CORRUPT"
CORRUPT_TLS=$(find "$CORRUPT" -path '*/tmp/typea/typea-*' -type f | sort | head -n 1)
if [ -z "$CORRUPT_TLS" ]; then
    echo "fixture transaction log part not found for corruption probe" >&2
    exit 1
fi
truncate -s 1 "$CORRUPT_TLS"
if run_driver open-only "$CORRUPT" > "$TARGET/corrupt.out" 2> "$TARGET/corrupt.err"; then
    echo "opening a corrupted transaction log unexpectedly succeeded" >&2
    exit 1
fi
if [ ! -s "$TARGET/corrupt.err" ]; then
    echo "corrupted transaction log did not preserve native failure context" >&2
    exit 1
fi

echo "M5 PASS"
