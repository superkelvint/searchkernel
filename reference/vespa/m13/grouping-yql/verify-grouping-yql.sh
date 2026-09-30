#!/usr/bin/env bash
set -euo pipefail

DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$DIR/../../../.." && pwd)"
cd "$ROOT"
source "$ROOT/reference/vespa/common/verify-common.sh"
PYTHON=$(bash "$ROOT/tools/resolve-python.sh")

require_pinned_vespa
"$PYTHON" "$DIR/validate.py"

# The low-level grouping case is independently compared with a direct pinned
# Vespa oracle and exercises the real native grouping ABI. M9 is frozen and is
# intentionally reused rather than modified by this additive M13 coverage.
"$ROOT/reference/vespa/m9-general-grouping-verifier/verify-recursive-grouping.sh"

# The portable showcase drives the daemon/native Vespa path through the shared
# FlatBuffer compiler. It covers the currently executable root/grouping and YQL
# cases. Summary-hit grouping is an executable coverage case. Continuation
# round-trips remain intentionally deferred until their translation path exists.
PYTHON_LOG="$(mktemp)"
trap 'rm -f "$PYTHON_LOG"' EXIT
bash "$ROOT/tools/run-python-native-e2e.sh" \
  --prevalidated-native-sdk --include-facade | tee "$PYTHON_LOG"
rg -F 'textual filter parser, clause-local bindings, grammar, and syntax errors PASS' "$PYTHON_LOG" >/dev/null ||
  fail_verification "canonical Python E2E did not report textual filter parser coverage"

echo "M13 grouping/YQL acceptance coverage passed for currently executable cases"
