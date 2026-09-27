#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
runtime=${1:-}
[[ -n "$runtime" ]] || {
    echo "usage: tools/run-runtime.sh python|node|npm|php|ruby|native <args...>" >&2
    exit 2
}
shift

if [[ "${1:-}" == "--" ]]; then
    shift
fi

case "$runtime" in
    python)
        executable=$(bash "$ROOT/tools/resolve-python.sh")
        ;;
    node|npm)
        executable=$(bash "$ROOT/tools/resolve-node.sh" "$runtime")
        ;;
    php|ruby)
        executable=$(bash "$ROOT/tools/resolve-runtime.sh" "$runtime")
        ;;
    native)
        [[ "$#" -ge 1 ]] || {
            echo "usage: tools/run-runtime.sh native <binary> [args...]" >&2
            exit 2
        }
        executable=$1
        shift
        if [[ "$executable" != */* ]]; then
            executable=$(command -v "$executable" 2>/dev/null || true)
        fi
        [[ -n "$executable" && -x "$executable" ]] || {
            echo "native executable is missing or not executable: ${executable:-<empty>}" >&2
            exit 1
        }
        runtime_libs=$(bash "$ROOT/tools/resolve-native-runtime-libs.sh")
        export SEARCHKERNEL_RUNTIME_LIB_DIRS="$runtime_libs"
        export LD_LIBRARY_PATH="$runtime_libs"
        unset LD_PRELOAD || true
        exec "$executable" "$@"
        ;;
    *)
        echo "unknown repository runtime: $runtime" >&2
        echo "usage: tools/run-runtime.sh python|node|npm|php|ruby|native <args...>" >&2
        exit 2
        ;;
esac

exec "$executable" "$@"
