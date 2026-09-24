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

expect_regression_failure() {
    local label=$1
    local marker=$2
    shift 2
    local output
    output=$(mktemp)
    set +e
    "$@" >"$output" 2>&1
    local status=$?
    set -e
    if [[ $status -eq 0 ]]; then
        cat "$output" >&2
        rm -f "$output"
        echo "sensitivity failure: $label mutation remained green" >&2
        exit 1
    fi
    if ! grep -Fq "$marker" "$output" || ! grep -Fq "FAILED" "$output"; then
        cat "$output" >&2
        rm -f "$output"
        echo "sensitivity failure: $label failed outside the intended regression" >&2
        exit 1
    fi
    rm -f "$output"
    echo "Sensitivity PASS: $label mutation was detected by the intended regression"
}

python3 "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/check-persistence-sensitivity-python-1.py" "$MANIFEST"

expect_regression_failure     "removing claimed manifest-v3 support"     "v3 open:"     cargo test -p searchkernel --test persistence_compatibility --features native -- --nocapture

cp "$BACKUP" "$MANIFEST"

python3 "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/check-persistence-sensitivity-python-2.py" "$MANIFEST"

expect_regression_failure     "weakening persisted config-identity validation"     "manifest_round_trips_definition_and_config_identity"     cargo test -p searchkernel --features native --lib manifest_round_trips_definition_and_config_identity -- --nocapture

restore
trap - EXIT INT TERM
echo "Persistence sensitivity PASS"
