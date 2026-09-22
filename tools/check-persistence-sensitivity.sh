#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
MANIFEST="$ROOT/core/crates/searchkernel/src/index/manifest.rs"
BACKUP=$(mktemp)
cp "$MANIFEST" "$BACKUP"

restore() {
    cp "$BACKUP" "$MANIFEST"
    rm -f "$BACKUP"
}
trap restore EXIT INT TERM

expect_failure() {
    local label=$1
    shift
    set +e
    "$@"
    local status=$?
    set -e
    if [[ $status -eq 0 ]]; then
        echo "sensitivity failure: $label mutation remained green" >&2
        exit 1
    fi
    echo "Sensitivity PASS: $label mutation was detected"
}

python3 - "$MANIFEST" <<'PY'
from pathlib import Path
import sys

path = Path(sys.argv[1])
text = path.read_text()
old = "pub(crate) const PREVIOUS_MANIFEST_FORMAT_VERSION: u32 = 3;"
new = "pub(crate) const PREVIOUS_MANIFEST_FORMAT_VERSION: u32 = 99;"
if text.count(old) != 1:
    raise SystemExit("could not locate unique previous-manifest compatibility constant")
path.write_text(text.replace(old, new))
PY

expect_failure     "removing claimed manifest-v3 support"     cargo test -p searchkernel --test persistence_compatibility --features native -- --nocapture

cp "$BACKUP" "$MANIFEST"

python3 - "$MANIFEST" <<'PY'
from pathlib import Path
import sys

path = Path(sys.argv[1])
text = path.read_text()
old = "        if expected != &actual {\n"
new = "        if false && expected != &actual {\n"
if text.count(old) != 1:
    raise SystemExit("could not locate unique config-identity validation branch")
path.write_text(text.replace(old, new))
PY

expect_failure     "weakening persisted config-identity validation"     cargo test -p searchkernel --features native --lib manifest_round_trips_definition_and_config_identity -- --nocapture

restore
trap - EXIT INT TERM
echo "Persistence sensitivity PASS"
