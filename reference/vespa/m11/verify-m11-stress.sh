#!/usr/bin/env bash
set -euo pipefail

M11="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$M11/../../.." && pwd)"
source "$M11/verify-m11-common.sh"

require_pinned_vespa
python3 "$M11/validate.py"
"$M11/check-immutable.sh"
prepare_hardening_sdk
build_m11_driver

TARGET="$ROOT/target/vespa-verifier/m11/stress"
rm -rf "$TARGET"
mkdir -p "$TARGET"
run_m11_driver stress "$TARGET/state-machine"
run_m11_driver concurrency "$TARGET/concurrency"

resource_output="$TARGET/resources.json"
run_m11_driver resources "$TARGET/resources" > "$resource_output"
python3 - "$resource_output" "$M11/expected/invariants.json" <<'PY'
import json
import sys

actual = json.load(open(sys.argv[1], encoding="utf-8"))
limits = json.load(open(sys.argv[2], encoding="utf-8"))
if actual["final_fds"] > actual["baseline_fds"] + limits["fd_slack"]:
    raise SystemExit("file descriptor count grew beyond the frozen guardrail")
if actual["final_threads"] > actual["baseline_threads"] + 1:
    raise SystemExit("native thread count grew beyond the frozen guardrail")
allowed_pss = max(
    limits["pss_absolute_slack_bytes"] // 1024,
    int(actual["baseline_pss_kib"] * (1 + limits["pss_relative_slack"])),
)
if actual["final_pss_kib"] > actual["baseline_pss_kib"] + allowed_pss:
    raise SystemExit("PSS grew beyond the frozen guardrail")
if actual["final_files"] > actual["baseline_files"] + limits["disk_file_slack"]:
    raise SystemExit("read-only reopen created too many files")
if actual["final_bytes"] > actual["baseline_bytes"] + limits["disk_byte_slack"]:
    raise SystemExit("read-only reopen grew on-disk state beyond the guardrail")
PY

echo "M11 stress/resource verification passed"
