#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
M8="$(cd "$SCRIPT_DIR/.." && pwd)"; ROOT="$(cd "$M8/../../.." && pwd)"
source "$ROOT/reference/vespa/common/verify-common.sh"
SCENARIO="${1:?usage: run-cpp-oracle.sh SCENARIO_JSON}"
BIN="$SCRIPT_DIR/target/m8_cpp_oracle"; SRC="$SCRIPT_DIR/m8_local_oracle.cpp"; PROV="$BIN.provenance"
IMAGE="${M8_VESPA_DEV_IMAGE:-docker.io/vespaengine/vespa-dev-almalinux-8:latest}"
[[ -x "$BIN" ]] || fail_verification "M8 C++ oracle binary missing; build explicitly with oracle/build-cpp-oracle.sh"
[[ -s "$PROV" && -f "$SRC" ]] || fail_verification "M8 oracle provenance/source missing"
python3 - "$PROV" "$BIN" "$SRC" "$ROOT/reference/vespa/PINNED_VESPA.json" <<'PY'
import hashlib,json,sys
p={}
for line in open(sys.argv[1],encoding='utf-8'):
 k,v=line.rstrip('\n').split('=',1); p[k]=v
expected=json.load(open(sys.argv[4],encoding='utf-8'))['commit']
sha=lambda x:hashlib.sha256(open(x,'rb').read()).hexdigest()
if p.get('vespa_commit')!=expected: raise SystemExit('M8 oracle not built from pinned Vespa revision')
if p.get('oracle_sha256')!=sha(sys.argv[3]): raise SystemExit('M8 oracle source provenance mismatch')
if p.get('binary_sha256')!=sha(sys.argv[2]): raise SystemExit('M8 oracle binary provenance mismatch')
PY
scenario_abs="$(cd "$(dirname "$SCENARIO")" && pwd -P)/$(basename "$SCENARIO")"
case "$scenario_abs" in "$ROOT"/*) rel="${scenario_abs#"$ROOT"/}";; *) fail_verification "scenario must be inside repository";; esac
if [[ -f /.dockerenv ]]; then
 work="$(mktemp -d /tmp/searchkernel-m8-oracle.XXXXXX)"; trap 'rm -rf "$work"' EXIT; cd "$work"
 export M8_ORACLE_FIXTURE_ROOT="$M8/fixture"
 exec "$BIN" "$scenario_abs"
fi
exec docker run --rm --privileged --pids-limit -1 \
  -v "$ROOT":/work:ro \
  -e M8_ORACLE_FIXTURE_ROOT=/work/reference/vespa/m8/fixture \
  "$IMAGE" /bin/bash -lc "set -e; w=\$(mktemp -d /tmp/searchkernel-m8-oracle.XXXXXX); cd \"\$w\"; exec /work/reference/vespa/m8/oracle/target/m8_cpp_oracle /work/$rel"
