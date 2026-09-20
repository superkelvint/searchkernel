#!/usr/bin/env bash
set -euo pipefail
ORACLE_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$ORACLE_DIR/../../../.." && pwd)"
BIN="$ORACLE_DIR/target/m12_cpp_oracle"
[[ -x "$BIN" ]] || { echo "M12 oracle is not built; run oracle/build-cpp-oracle.sh" >&2; exit 1; }
[[ $# -eq 1 ]] || { echo "usage: run-cpp-oracle.sh SCENARIO_JSON" >&2; exit 2; }
ORACLE_LIB_DIRS="$({
  find "$ROOT/vespa-deps/lib64" "$ROOT/upstream/vespa" -type f \( -name '*.so' -o -name '*.so.*' \) -printf '%h\n'
  find "$ROOT/vespa-deps/lib64" "$ROOT/upstream/vespa" -type l \( -name '*.so' -o -name '*.so.*' \) -printf '%h\n'
} | sort -u | paste -sd: -)"
export LD_LIBRARY_PATH="$ROOT/vespa-deps/lib64:$ORACLE_LIB_DIRS${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export LD_PRELOAD="$ROOT/vespa-deps/lib64/libLLVM-21.so${LD_PRELOAD:+:$LD_PRELOAD}"
exec "$BIN" run "$1"
