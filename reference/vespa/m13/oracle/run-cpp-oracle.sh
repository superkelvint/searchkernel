#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/../../../.." && pwd)"
[[ -x "$SCRIPT_DIR/target/m13_cpp_oracle" ]] || {
  echo "M13 direct oracle is not built; run oracle/build-cpp-oracle.sh" >&2
  exit 1
}
export LD_LIBRARY_PATH="$ROOT/vespa-deps/lib64:$ROOT/upstream/vespa/document/src/vespa/document:$ROOT/upstream/vespa/eval/src/vespa/eval:$ROOT/upstream/vespa/vespalib/src/vespa/vespalib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export LD_PRELOAD="$ROOT/vespa-deps/lib64/libLLVM-21.so${LD_PRELOAD:+:$LD_PRELOAD}"
exec "$SCRIPT_DIR/target/m13_cpp_oracle" "$@"
