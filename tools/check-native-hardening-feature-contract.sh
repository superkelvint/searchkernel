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
grep -q 'explicit_sdk_path = Some(configured.clone())' "$BUILD_RS" || {
    echo 'explicit native SDK is not retained as a separate hardening runtime base' >&2
    exit 1
}
grep -q 'build_local_adapter(&manifest_dir, &configured, &target, true)' "$BUILD_RS" || {
    echo 'hardening does not build a private adapter over an explicit native SDK' >&2
    exit 1
}
if grep -q 'explicit native SDK lacks hardening failpoints; rebuilding it once' "$BUILD_RS"; then
    echo 'hardening must not rewrite an explicitly managed production SDK in place' >&2
    exit 1
fi
grep -q 'SK_NATIVE_ENABLE_FAILPOINTS", "0"' "$BUILD_RS" || {
    echo 'full native SDK repair does not explicitly stay in production mode' >&2
    exit 1
}

echo 'native hardening feature contract PASS'
