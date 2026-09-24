#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
SDK="${1:?usage: check-native-sdk-hardening.sh <sdk-directory>}"
MANIFEST="$SDK/manifest.json"
LIBRARY="$SDK/lib/libsearchkernel_vespa.so.2"
TEST_HEADER="$ROOT/core/native/vespa/searchkernel_vespa_test.h"

[[ -f "$MANIFEST" ]] || {
    echo "hardening SDK manifest is missing: $MANIFEST" >&2
    exit 1
}
[[ -f "$LIBRARY" ]] || {
    echo "hardening SDK library is missing: $LIBRARY" >&2
    exit 1
}

python3 "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/check-native-sdk-hardening-python-1.py" "$MANIFEST"

mapfile -t required_symbols < <(
    sed -nE 's/^SK_API ([A-Za-z0-9_]+) (sk_test_[a-zA-Z0-9_]+)\(.*/\2/p' \
        "$TEST_HEADER"
)
[[ "${#required_symbols[@]}" -gt 0 ]] || {
    echo "no hardening ABI symbols found in $TEST_HEADER" >&2
    exit 1
}

symbols="$(nm -D --defined-only "$LIBRARY")"
for symbol in "${required_symbols[@]}"; do
    if ! grep -Eq "(^|[[:space:]])${symbol}(@@[^[:space:]]+)?$" <<<"$symbols"; then
        echo "hardening SDK does not export required symbol: $symbol" >&2
        exit 1
    fi
done
