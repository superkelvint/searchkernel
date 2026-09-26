#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
CHECK="$ROOT/tools/check-flatbuffers-cpp.sh"

tmp=$(mktemp -d "${TMPDIR:-/tmp}/searchkernel-flatbuffers-test.XXXXXX")
trap 'rm -rf -- "$tmp"' EXIT

prefix="$tmp/prefix"
mkdir -p "$prefix/bin" "$prefix/include/flatbuffers" "$prefix/lib" "$tmp/build"

cat >"$prefix/bin/flatc" <<'EOF'
#!/usr/bin/env bash
if [[ "${1:-}" == "--version" ]]; then
    echo "flatc version 25.12.19"
    exit 0
fi
exit 2
EOF
chmod +x "$prefix/bin/flatc"

cat >"$prefix/include/flatbuffers/base.h" <<'EOF'
#define FLATBUFFERS_VERSION_MAJOR 25
#define FLATBUFFERS_VERSION_MINOR 12
#define FLATBUFFERS_VERSION_REVISION 19
EOF

cat >"$prefix/include/flatbuffers/idl.h" <<'EOF'
#pragma once
namespace flatbuffers {
class Parser {
public:
    Parser();
    bool Parse(const char *);
};
}
EOF

cat >"$tmp/build/parser.cc" <<'EOF'
#include <flatbuffers/idl.h>

extern "C" void searchkernel_missing_flatbuffers_abi_symbol();

flatbuffers::Parser::Parser() = default;

bool flatbuffers::Parser::Parse(const char *) {
    searchkernel_missing_flatbuffers_abi_symbol();
    return true;
}
EOF

"${CXX:-c++}" -std=c++17 -I"$prefix/include" -c "$tmp/build/parser.cc" -o "$tmp/build/parser.o"
"${AR:-ar}" rcs "$prefix/lib/libflatbuffers.a" "$tmp/build/parser.o"

if bash "$CHECK" "$prefix" 25.12.19 >"$tmp/check.out" 2>"$tmp/check.err"; then
    echo "expected ABI-incompatible FlatBuffers archive to be rejected" >&2
    cat "$tmp/check.out" >&2
    cat "$tmp/check.err" >&2
    exit 1
fi

grep -q "FlatBuffers C++ compile/link probe failed" "$tmp/check.err"

cat >"$tmp/build/abi.cc" <<'EOF'
extern "C" void searchkernel_missing_flatbuffers_abi_symbol() {}
EOF
"${CXX:-c++}" -std=c++17 -c "$tmp/build/abi.cc" -o "$tmp/build/abi.o"
"${AR:-ar}" rcs "$prefix/lib/libflatbuffers.a" "$tmp/build/abi.o"

bash "$CHECK" "$prefix" 25.12.19

mkdir -p "$tmp/compiler/bin"
mv "$prefix/bin/flatc" "$tmp/compiler/bin/flatc"
bash "$CHECK" "$prefix" 25.12.19 "$tmp/compiler/bin/flatc"

echo "PASS incompatible FlatBuffers C++ archive is rejected, compatible archive is accepted, and split overrides remain valid"


# Regression: the installer must authenticate the pinned FlatBuffers source
# archive before any extraction is attempted.
fake_bin="$tmp/fake-bin"
mkdir -p "$fake_bin"

cat >"$fake_bin/curl" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
output=
while (($#)); do
    case "$1" in
        --output)
            output=$2
            shift 2
            ;;
        *)
            shift
            ;;
    esac
done
[[ -n "$output" ]] || {
    echo "fake curl did not receive --output" >&2
    exit 2
}
printf 'corrupted flatbuffers source archive\n' >"$output"
EOF
chmod +x "$fake_bin/curl"

cat >"$fake_bin/tar" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
: "${SEARCHKERNEL_TEST_TAR_MARKER:?}"
: >"$SEARCHKERNEL_TEST_TAR_MARKER"
exit 1
EOF
chmod +x "$fake_bin/tar"

corrupt_prefix="$tmp/corrupt-prefix"
corrupt_marker="$tmp/tar-was-called"
if env \
    PATH="$fake_bin:$PATH" \
    FLATBUFFERS_INSTALL_PREFIX="$corrupt_prefix" \
    SEARCHKERNEL_TEST_TAR_MARKER="$corrupt_marker" \
    "$ROOT/dev" install flatbuffers >"$tmp/corrupt.out" 2>"$tmp/corrupt.err"; then
    echo "expected corrupted FlatBuffers source archive to be rejected" >&2
    exit 1
fi

if [[ -e "$corrupt_marker" ]]; then
    echo "expected corrupted FlatBuffers source archive to be rejected before extraction" >&2
    cat "$tmp/corrupt.err" >&2
    exit 1
fi

grep -q "FlatBuffers source checksum mismatch" "$tmp/corrupt.err"

echo "PASS corrupted FlatBuffers source bytes are rejected before extraction"

# Atomic publication regression.
bash "$ROOT/tools/test-flatbuffers-publication.sh"
