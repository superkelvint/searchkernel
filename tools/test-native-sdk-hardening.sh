#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
CHECKER="$ROOT/tools/check-native-sdk-hardening.sh"
[[ -x "$CHECKER" ]] || {
    echo "missing hardening SDK checker: $CHECKER" >&2
    exit 1
}
FIXTURE="$(mktemp -d "${TMPDIR:-/tmp}/searchkernel-sdk-hardening.XXXXXX")"
cleanup() {
    rm -rf "$FIXTURE"
}
trap cleanup EXIT

mkdir -p "$FIXTURE/sdk/lib"

mapfile -t required_symbols < <(
    sed -nE 's/^SK_API ([A-Za-z0-9_]+) (sk_test_[a-zA-Z0-9_]+)\(.*/\2/p' \
        "$ROOT/core/native/vespa/searchkernel_vespa_test.h"
)
[[ "${#required_symbols[@]}" -gt 0 ]]

write_fixture_source() {
    local omit="${1:-}"
    {
        printf '%s\n' '#include <cstdint>'
        printf '%s\n' '#define SK_API __attribute__((visibility("default")))'
        local symbol
        for symbol in "${required_symbols[@]}"; do
            [[ "$symbol" == "$omit" ]] && continue
            printf 'extern "C" SK_API int32_t %s() { return 0; }\n' "$symbol"
        done
    } >"$FIXTURE/abi.cc"
}

build_fixture() {
    local omit="${1:-}"
    write_fixture_source "$omit"
    printf '%s\n' 'SEARCHKERNEL_2 { global: sk_*; local: *; };' >"$FIXTURE/map"
    c++ -fvisibility=hidden -ffunction-sections -fdata-sections -fPIC -shared \
        -Wl,--gc-sections \
        -Wl,--version-script="$FIXTURE/map" \
        -o "$FIXTURE/sdk/lib/libsearchkernel_vespa.so.2" "$FIXTURE/abi.cc"
    cp "$FIXTURE/sdk/lib/libsearchkernel_vespa.so.2" \
        "$FIXTURE/sdk/lib/libsearchkernel_vespa.so"
}

write_manifest() {
    local hardening="$1"
    printf '{"hardening_failpoints":%s}\n' "$hardening" >"$FIXTURE/sdk/manifest.json"
}

build_fixture "${required_symbols[0]}"
write_manifest true
if "$CHECKER" "$FIXTURE/sdk" 2>"$FIXTURE/missing.stderr"; then
    echo "hardening SDK checker accepted a missing exported symbol" >&2
    exit 1
fi
grep -q "${required_symbols[0]}" "$FIXTURE/missing.stderr"

build_fixture
write_manifest false
if "$CHECKER" "$FIXTURE/sdk" 2>"$FIXTURE/manifest.stderr"; then
    echo "hardening SDK checker accepted a non-hardening manifest" >&2
    exit 1
fi
grep -q 'does not enable failpoints' "$FIXTURE/manifest.stderr"

write_manifest true
"$CHECKER" "$FIXTURE/sdk"
echo 'native SDK hardening export regression PASS'
