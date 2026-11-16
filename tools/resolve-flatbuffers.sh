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

pin_prefix() {
    local prefix=$1
    if [[ -e "$prefix/.searchkernel-current" || -L "$prefix/.searchkernel-current" ]]; then
        [[ -L "$prefix/.searchkernel-current" ]] || {
            echo "error: invalid FlatBuffers publication pointer under '$prefix'" >&2
            return 1
        }
        (cd -- "$prefix/.searchkernel-current" && pwd -P)
    else
        (cd -- "$prefix" && pwd -P)
    fi
}

cpp_prefix=
if [[ -n "${FLATBUFFERS_CPP_ROOT:-}" ]]; then
    cpp_prefix=$(pin_prefix "$FLATBUFFERS_CPP_ROOT")
fi

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

    if [[ -n "$cpp_prefix" ]] &&
       is_expected_flatc "$cpp_prefix/bin/flatc"; then
        printf '%s\n' "$cpp_prefix/bin/flatc"
        return 0
    fi

    common=$(git -C "$root" rev-parse --git-common-dir)
    if [[ "$common" != /* ]]; then
        common="$root/$common"
    fi
    common=$(cd -- "$common" && pwd)
    build_env=${SEARCHKERNEL_BUILD_ENV:-host}
    shared="$common/searchkernel/toolchains/flatbuffers/$expected/$build_env"
    if [[ -d "$shared" ]]; then
        shared=$(pin_prefix "$shared") || return $?
    fi
    shared="$shared/bin/flatc"
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
    prefix=$cpp_prefix
else
    prefix=$(cd -- "$(dirname -- "$flatc")/.." && pwd -P)
fi

[[ -f "$prefix/include/flatbuffers/base.h" &&
   -f "$prefix/include/flatbuffers/idl.h" &&
   -f "$prefix/lib/libflatbuffers.a" ]] || {
    echo "error: resolved FlatBuffers C++ root is incomplete: $prefix" >&2
    exit 1
}

printf '%s\n' "$prefix"
