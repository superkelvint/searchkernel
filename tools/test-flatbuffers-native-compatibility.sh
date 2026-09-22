#!/usr/bin/env bash
set -euo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
common=$(git -C "$root" rev-parse --git-common-dir)
if [[ "$common" != /* ]]; then
    common="$root/$common"
fi
prefix="$common/searchkernel/toolchains/flatbuffers/25.12.19"
archive="$prefix/lib/libflatbuffers.a"

archive_is_compatible() {
    local candidate=$1
    local undefined

    [[ -f "$candidate" ]] || return 1
    command -v nm >/dev/null 2>&1 || return 1
    if ! undefined=$(nm -u "$candidate" 2>/dev/null); then
        return 1
    fi
    ! grep -F '__isoc23_' <<<"$undefined" >/dev/null
}

tmpdir=$(mktemp -d "${TMPDIR:-/tmp}/searchkernel-flatbuffers-compat.XXXXXX")
trap 'rm -rf -- "$tmpdir"' EXIT

if archive_is_compatible "$tmpdir/missing.a"; then
    echo 'error: missing FlatBuffers archive was accepted' >&2
    exit 1
fi

printf 'not an object archive\n' >"$tmpdir/corrupt.a"
if archive_is_compatible "$tmpdir/corrupt.a"; then
    echo 'error: uninspectable FlatBuffers archive was accepted' >&2
    exit 1
fi

if ! archive_is_compatible "$archive"; then
    echo "FlatBuffers archive is missing, unreadable, or contains glibc C23-only symbols: $archive" >&2
    exit 1
fi

echo 'FlatBuffers native compatibility PASS'
