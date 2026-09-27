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
        PYTHON="$executable"
        export PYTHON
        ;;
    node|npm)
        NODE=$(bash "$ROOT/tools/resolve-node.sh" node)
        NPM=$(bash "$ROOT/tools/resolve-node.sh" npm)
        PATH="$(dirname -- "$NODE"):$PATH"
        export NODE NPM PATH
        if [[ "$runtime" == node ]]; then
            executable=$NODE
        else
            executable=$NPM
        fi
        ;;
    php)
        executable=$(bash "$ROOT/tools/resolve-runtime.sh" php)
        PHP="$executable"
        export PHP
        ;;
    ruby)
        executable=$(bash "$ROOT/tools/resolve-runtime.sh" ruby)
        RUBY="$executable"
        export RUBY
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
        if [[ -n "${SEARCHKERNEL_RUNTIME_LD_PRELOAD:-}" ]]; then
            export LD_PRELOAD="$SEARCHKERNEL_RUNTIME_LD_PRELOAD"
        else
            unset LD_PRELOAD || true
        fi

        if command -v readelf >/dev/null 2>&1 &&
           command -v ldd >/dev/null 2>&1 &&
           readelf -h "$executable" >/dev/null 2>&1; then
            missing=$(ldd "$executable" 2>&1 | sed -n '/=> not found/p')
            if [[ -n "$missing" ]]; then
                echo "native runtime dependency closure is incomplete for $executable:" >&2
                printf '%s\n' "$missing" >&2
                echo "resolved runtime library directories: $runtime_libs" >&2
                exit 127
            fi
        fi

        exec "$executable" "$@"
        ;;
    *)
        echo "unknown repository runtime: $runtime" >&2
        echo "usage: tools/run-runtime.sh python|node|npm|php|ruby|native <args...>" >&2
        exit 2
        ;;
esac

exec "$executable" "$@"
