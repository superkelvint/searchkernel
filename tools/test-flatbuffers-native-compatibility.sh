#!/usr/bin/env bash
set -euo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
common=$(git -C "$root" rev-parse --git-common-dir)
if [[ "$common" != /* ]]; then
    common="$root/$common"
fi
prefix="$common/searchkernel/toolchains/flatbuffers/25.12.19"
archive="$prefix/lib/libflatbuffers.a"

if ! command -v nm >/dev/null 2>&1; then
    echo 'error: nm is required to inspect the FlatBuffers archive' >&2
    exit 1
fi

if nm -u "$archive" | grep -F '__isoc23_' >/dev/null; then
    echo "FlatBuffers archive contains glibc C23-only symbols: $archive" >&2
    exit 1
fi

echo 'FlatBuffers native compatibility PASS'
