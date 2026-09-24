#!/usr/bin/env bash
set -euo pipefail

M11="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$M11/../../.." && pwd)"
source "$M11/verify-m11-common.sh"

require_pinned_vespa
python3 "$M11/validate.py"
prepare_hardening_sdk
build_m11_driver

TARGET="$ROOT/target/vespa-verifier/m11/stress"
rm -rf "$TARGET"
mkdir -p "$TARGET"
run_m11_driver torture "$TARGET/torture"
run_m11_driver concurrency "$TARGET/concurrency"

resource_output="$TARGET/resources.json"
run_m11_driver resources "$TARGET/resources" > "$resource_output"
python3 "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/verify-m11-stress-python-1.py" "$resource_output" "$M11/expected/invariants.json"

echo "M11 stress/resource verification passed"
