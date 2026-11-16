#!/usr/bin/env bash
set -euo pipefail

prefix=${1:-}
expected_version=${2:-25.12.19}
flatc=${3:-}

if [[ -z "$prefix" ]]; then
    echo "usage: $0 <flatbuffers-prefix> [expected-version] [flatc-path]" >&2
    exit 2
fi

if [[ -e "$prefix/.searchkernel-current" || -L "$prefix/.searchkernel-current" ]]; then
    [[ -L "$prefix/.searchkernel-current" ]] || {
        echo "invalid FlatBuffers publication pointer under '$prefix'" >&2
        exit 1
    }
    prefix=$(cd -- "$prefix/.searchkernel-current" && pwd -P)
fi
flatc=${flatc:-"$prefix/bin/flatc"}

header="$prefix/include/flatbuffers/base.h"
idl_header="$prefix/include/flatbuffers/idl.h"
library="$prefix/lib/libflatbuffers.a"

[[ -x "$flatc" ]] || {
    echo "FlatBuffers compiler is missing at '$flatc'" >&2
    exit 1
}
[[ -f "$header" ]] || {
    echo "FlatBuffers version header is missing at '$header'" >&2
    exit 1
}
[[ -f "$idl_header" ]] || {
    echo "FlatBuffers parser header is missing at '$idl_header'" >&2
    exit 1
}
[[ -f "$library" ]] || {
    echo "FlatBuffers static library is missing at '$library'" >&2
    exit 1
}

actual_flatc=$("$flatc" --version 2>/dev/null || true)
[[ "$actual_flatc" == "flatc version $expected_version" ]] || {
    echo "FlatBuffers compiler version mismatch: expected $expected_version, got '${actual_flatc:-unknown}'" >&2
    exit 1
}

major=$(sed -n 's/^#define FLATBUFFERS_VERSION_MAJOR //p' "$header")
minor=$(sed -n 's/^#define FLATBUFFERS_VERSION_MINOR //p' "$header")
revision=$(sed -n 's/^#define FLATBUFFERS_VERSION_REVISION //p' "$header")
actual_header="$major.$minor.$revision"
[[ "$actual_header" == "$expected_version" ]] || {
    echo "FlatBuffers C++ header version mismatch: expected $expected_version, got '$actual_header'" >&2
    exit 1
}

cxx=${CXX:-c++}
command -v "$cxx" >/dev/null 2>&1 || {
    echo "FlatBuffers C++ compatibility check requires a C++ compiler ('$cxx')" >&2
    exit 1
}

tmp=$(mktemp -d "${TMPDIR:-/tmp}/searchkernel-flatbuffers-probe.XXXXXX")
trap 'rm -rf -- "$tmp"' EXIT

cat >"$tmp/probe.cc" <<'EOF'
#include <flatbuffers/idl.h>

int main() {
    flatbuffers::Parser parser;
    return parser.Parse("table Probe { value:int; } root_type Probe;") ? 0 : 1;
}
EOF

if ! "$cxx" -std=c++17     -I"$prefix/include"     "$tmp/probe.cc"     "$library"     -o "$tmp/probe"     >"$tmp/link.out" 2>"$tmp/link.err"; then
    echo "FlatBuffers C++ compile/link probe failed for '$prefix'" >&2
    sed -n '1,20p' "$tmp/link.err" >&2
    exit 1
fi

if ! "$tmp/probe" >"$tmp/run.out" 2>"$tmp/run.err"; then
    echo "FlatBuffers C++ runtime probe failed for '$prefix'" >&2
    sed -n '1,20p' "$tmp/run.err" >&2
    exit 1
fi
