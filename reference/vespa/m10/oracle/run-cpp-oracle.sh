#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
M10="$(cd "$SCRIPT_DIR/.." && pwd)"; ROOT="$(cd "$M10/../../.." && pwd)"
source "$ROOT/reference/vespa/common/verify-common.sh"
SCENARIO="${1:?usage: run-cpp-oracle.sh SCENARIO_JSON}"
BIN="$SCRIPT_DIR/target/m10_cpp_oracle"; SRC="$SCRIPT_DIR/m10_ranking_oracle.cpp"; PROV="$BIN.provenance"
IMAGE="${M10_VESPA_DEV_IMAGE:-docker.io/vespaengine/vespa-dev-almalinux-8:latest}"
[[ -x "$BIN" ]] || fail_verification "M10 C++ oracle binary missing; build explicitly with oracle/build-cpp-oracle.sh"
[[ -s "$PROV" && -f "$SRC" ]] || fail_verification "M10 oracle provenance/source missing"
python3 - "$PROV" "$ROOT/reference/vespa/PINNED_VESPA.json" <<'PY'
import json,sys
p={}
for line in open(sys.argv[1],encoding='utf-8'):
 k,v=line.rstrip('\n').split('=',1); p[k]=v
expected=json.load(open(sys.argv[2],encoding='utf-8'))['commit']
if p.get('vespa_commit')!=expected: raise SystemExit('M10 oracle not built from pinned Vespa revision')
PY
scenario_abs="$(cd "$(dirname "$SCENARIO")" && pwd -P)/$(basename "$SCENARIO")"
case "$scenario_abs" in "$ROOT"/*) rel="${scenario_abs#"$ROOT"/}";; *) fail_verification "scenario must be inside repository";; esac
if [[ -f /.dockerenv ]]; then
 work="$(mktemp -d /tmp/searchkernel-m10-oracle.XXXXXX)"; trap 'rm -rf "$work"' EXIT; cd "$work"
 export M10_ORACLE_FIXTURE_ROOT="$M10/fixture"
 exec "$BIN" "$scenario_abs"
fi
exec docker run --rm --privileged --pids-limit -1 \
  -v "$ROOT":/work:ro \
  -e M10_ORACLE_FIXTURE_ROOT=/work/reference/vespa/m10/fixture \
  "$IMAGE" /bin/bash -lc "set -e; w=\$(mktemp -d /tmp/searchkernel-m10-oracle.XXXXXX); cd \"\$w\"; exec /work/reference/vespa/m10/oracle/target/m10_cpp_oracle /work/$rel"
