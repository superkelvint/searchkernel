#!/usr/bin/env bash
set -euo pipefail

M11="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$M11/../../.." && pwd)"

source "$ROOT/reference/vespa/common/verify-common.sh"
require_pinned_vespa
python3 "$M11/validate.py"
"$M11/check-immutable.sh"

# The current repository has M0–M7. The cumulative verifier is deliberately
# invoked at the highest available frozen milestone; M8–M10 are not silently
# recreated by M11.
"$ROOT/reference/vespa/m7/verify-m7.sh"
"$M11/verify-m11-core.sh"
"$M11/verify-m11-stress.sh"
"$M11/verify-m11-sanitizers.sh"

run_workspace_quality
echo "M11 verification passed (cumulative baseline M0-M7; M8-M10 absent)"
