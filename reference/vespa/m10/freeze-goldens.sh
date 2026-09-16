#!/usr/bin/env bash
set -euo pipefail
M10="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"; ROOT="$(cd "$M10/../../.." && pwd)"; cd "$ROOT"
source "$ROOT/reference/vespa/common/verify-common.sh"
require_pinned_vespa
"$M10/validate_scenarios.py"
[[ -x "$M10/oracle/target/m10_cpp_oracle" ]] || fail_verification "build/review the M10 direct C++ oracle before freezing goldens"
command -v timeout >/dev/null 2>&1 || fail_verification "timeout is required to bound direct oracle execution"
command -v setsid >/dev/null 2>&1 || fail_verification "setsid is required for interruptible direct oracle execution"
ORACLE_TIMEOUT="${SK_GOLDEN_ORACLE_TIMEOUT_SECONDS:-120}"
ORACLE_PID=; CURRENT_TMP=
interrupt() {
 trap - INT TERM
 if [[ -n "$ORACLE_PID" ]]; then
  kill -TERM -- "-$ORACLE_PID" 2>/dev/null || kill -TERM "$ORACLE_PID" 2>/dev/null || true
  sleep 1
  kill -KILL -- "-$ORACLE_PID" 2>/dev/null || kill -KILL "$ORACLE_PID" 2>/dev/null || true
 fi
 [[ -z "$CURRENT_TMP" ]] || rm -f "$CURRENT_TMP"
 exit 130
}
run_oracle() {
 setsid timeout --foreground --kill-after=10s "$ORACLE_TIMEOUT" "$@" &
 ORACLE_PID=$!; trap interrupt INT TERM
 local status=0; wait "$ORACLE_PID" || status=$?
 ORACLE_PID=; trap - INT TERM; return "$status"
}
mkdir -p "$M10/expected"
rm -f "$M10"/expected/*.json
for s in "$M10"/scenarios/[0-9][0-9]-*.json; do
 id="$(python3 -c 'import json,sys;print(json.load(open(sys.argv[1]))["id"])' "$s")"
 echo "M10 freezing $id" >&2
 CURRENT_TMP="$(mktemp)"
 run_oracle "$M10/oracle/run-cpp-oracle.sh" "$s" > "$CURRENT_TMP"
 "$M10/canonicalize.py" < "$CURRENT_TMP" > "$M10/expected/$id.json"
 rm -f "$CURRENT_TMP"; CURRENT_TMP=
done
"$M10/validate_expected.py"
echo 'M10 goldens frozen from direct pinned Vespa C++ oracle'
