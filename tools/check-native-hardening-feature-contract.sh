#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
BUILD_RS="$ROOT/core/crates/searchkernel-vespa-bindings/build.rs"

grep -q 'CARGO_FEATURE_HARDENING' "$BUILD_RS" || {
    echo 'bindings build script does not detect the Cargo hardening feature' >&2
    exit 1
}
grep -q '"SK_NATIVE_ENABLE_FAILPOINTS"' "$BUILD_RS" || {
    echo 'bindings build script does not couple hardening to native failpoints' >&2
    exit 1
}
grep -q 'verify_hardening_sdk' "$BUILD_RS" || {
    echo 'bindings build script does not verify hardening SDK compatibility' >&2
    exit 1
}
grep -q 'check-native-sdk-hardening.sh' "$BUILD_RS" || {
    echo 'bindings build script does not use the canonical hardening SDK verifier' >&2
    exit 1
}

echo 'native hardening feature contract PASS'
