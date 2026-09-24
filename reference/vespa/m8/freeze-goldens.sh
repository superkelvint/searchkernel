#!/usr/bin/env bash
set -euo pipefail
M8="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"; ROOT="$(cd "$M8/../../.." && pwd)"; cd "$ROOT"
source "$ROOT/reference/vespa/common/verify-common.sh"
require_pinned_vespa
"$M8/validate_scenarios.py"
[[ -x "$M8/oracle/target/m8_cpp_oracle" ]] || fail_verification "build/review the M8 direct C++ oracle before freezing goldens"
command -v timeout >/dev/null 2>&1 || fail_verification "timeout is required to bound direct oracle execution"
command -v setsid >/dev/null 2>&1 || fail_verification "setsid is required for interruptible direct oracle execution"
ORACLE_TIMEOUT="${SK_GOLDEN_ORACLE_TIMEOUT_SECONDS:-120}"
ORACLE_PID=
CURRENT_TMP=
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
 ORACLE_PID=$!
 trap interrupt INT TERM
 local status=0
 wait "$ORACLE_PID" || status=$?
 ORACLE_PID=
 trap - INT TERM
 return "$status"
}
mkdir -p "$M8/expected"
for s in "$M8"/scenarios/[0-9][0-9]-*.json; do
 id="$(python3 "$ROOT/tools/json-cli.py" get "$s" id)"
 echo "M8 freezing $id" >&2
 CURRENT_TMP="$(mktemp)"
 run_oracle "$M8/oracle/run-cpp-oracle.sh" "$s" > "$CURRENT_TMP"
 python3 "reference/vespa/m8/freeze-goldens-python-1.py" "$CURRENT_TMP" "$M8/expected/$id.json"
 rm -f "$CURRENT_TMP"
 CURRENT_TMP=
done
"$M8/validate_expected.py"
echo 'M8 goldens frozen from direct pinned Vespa C++ oracle'
