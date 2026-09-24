#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
M9="$(cd "$SCRIPT_DIR/.." && pwd)"; ROOT="$(cd "$M9/../../.." && pwd)"
source "$ROOT/reference/vespa/common/verify-common.sh"
SCENARIO="${1:?usage: run-cpp-oracle.sh SCENARIO_JSON}"
BIN="$SCRIPT_DIR/target/m9_cpp_oracle"; SRC="$SCRIPT_DIR/m9_attribute_grouping_oracle.cpp"; PROV="$BIN.provenance"
IMAGE="${M9_VESPA_DEV_IMAGE:-docker.io/vespaengine/vespa-dev-almalinux-8:latest}"
[[ -x "$BIN" ]] || fail_verification "M9 C++ oracle binary missing; build explicitly with oracle/build-cpp-oracle.sh"
[[ -s "$PROV" && -f "$SRC" ]] || fail_verification "M9 oracle provenance/source missing"
python3 "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/run-cpp-oracle-python-1.py" "$PROV" "$ROOT/reference/vespa/PINNED_VESPA.json"
scenario_abs="$(cd "$(dirname "$SCENARIO")" && pwd -P)/$(basename "$SCENARIO")"
case "$scenario_abs" in "$ROOT"/*) rel="${scenario_abs#"$ROOT"/}";; *) fail_verification "scenario must be inside repository";; esac
if [[ -f /.dockerenv ]]; then
 work="$(mktemp -d /tmp/searchkernel-m9-oracle.XXXXXX)"; trap 'rm -rf "$work"' EXIT; cd "$work"
 export M9_ORACLE_FIXTURE_ROOT="$M9/fixture"
 exec "$BIN" "$scenario_abs"
fi
exec docker run --rm --privileged --pids-limit -1 \
  -v "$ROOT":/work:ro \
  -e M9_ORACLE_FIXTURE_ROOT=/work/reference/vespa/m9/fixture \
  "$IMAGE" /bin/bash -lc "set -e; w=\$(mktemp -d /tmp/searchkernel-m9-oracle.XXXXXX); cd \"\$w\"; exec /work/reference/vespa/m9/oracle/target/m9_cpp_oracle /work/$rel"
