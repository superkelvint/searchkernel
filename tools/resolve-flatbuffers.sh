#!/usr/bin/env bash
set -euo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
expected=25.12.19
mode=${1:-flatc}

case "$mode" in
    flatc|root) ;;
    *)
        echo "usage: tools/resolve-flatbuffers.sh flatc|root" >&2
        exit 2
        ;;
esac

is_expected_flatc() {
    local candidate=$1
    [[ -x "$candidate" ]] &&
        [[ "$("$candidate" --version 2>/dev/null || true)" == "flatc version $expected" ]]
}

resolve_flatc() {
    local common build_env shared candidate

    if [[ -n "${FLATC_PATH:-}" ]]; then
        if is_expected_flatc "$FLATC_PATH"; then
            printf '%s\n' "$FLATC_PATH"
            return 0
        fi
        echo "error: FLATC_PATH='$FLATC_PATH' is not executable FlatBuffers $expected flatc" >&2
        return 1
    fi

    if [[ -n "${FLATBUFFERS_CPP_ROOT:-}" ]] &&
       is_expected_flatc "$FLATBUFFERS_CPP_ROOT/bin/flatc"; then
        printf '%s\n' "$FLATBUFFERS_CPP_ROOT/bin/flatc"
        return 0
    fi

    common=$(git -C "$root" rev-parse --git-common-dir)
    if [[ "$common" != /* ]]; then
        common="$root/$common"
    fi
    common=$(cd -- "$common" && pwd)
    build_env=${SEARCHKERNEL_BUILD_ENV:-host}
    shared="$common/searchkernel/toolchains/flatbuffers/$expected/$build_env/bin/flatc"
    if is_expected_flatc "$shared"; then
        printf '%s\n' "$shared"
        return 0
    fi

    if command -v flatc >/dev/null 2>&1; then
        candidate=$(command -v flatc)
        if is_expected_flatc "$candidate"; then
            printf '%s\n' "$candidate"
            return 0
        fi
    fi

    if is_expected_flatc /usr/local/bin/flatc; then
        printf '%s\n' /usr/local/bin/flatc
        return 0
    fi

    echo "error: FlatBuffers $expected flatc was not found" >&2
    echo "       Run './dev install flatbuffers' once; the versioned install is shared by the clone." >&2
    return 1
}

flatc=$(resolve_flatc)

if [[ "$mode" == flatc ]]; then
    printf '%s\n' "$flatc"
    exit 0
fi

if [[ -n "${FLATBUFFERS_CPP_ROOT:-}" ]]; then
    prefix=$FLATBUFFERS_CPP_ROOT
else
    prefix=$(cd -- "$(dirname -- "$flatc")/.." && pwd)
fi

[[ -f "$prefix/include/flatbuffers/base.h" &&
   -f "$prefix/include/flatbuffers/idl.h" &&
   -f "$prefix/lib/libflatbuffers.a" ]] || {
    echo "error: resolved FlatBuffers C++ root is incomplete: $prefix" >&2
    exit 1
}

printf '%s\n' "$prefix"
