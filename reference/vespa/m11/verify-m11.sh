#!/usr/bin/env bash
set -euo pipefail

M11="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$M11/../../.." && pwd)"

source "$ROOT/reference/vespa/common/verify-common.sh"
require_pinned_vespa
python3 "$M11/validate.py"

# M11 is cumulative through the highest completed Vespa milestone.
"$ROOT/reference/vespa/m10/verify-m10.sh"
"$M11/verify-m11-core.sh"
"$M11/verify-m11-stress.sh"
"$M11/verify-m11-sanitizers.sh"

run_workspace_quality
echo "M11 verification passed (cumulative baseline M0-M10)"
