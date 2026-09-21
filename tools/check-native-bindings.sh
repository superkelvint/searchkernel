#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
source_header="$root/core/native/vespa/searchkernel_vespa.h"
build_env=${SEARCHKERNEL_BUILD_ENV:-host}
target_root=${CARGO_TARGET_DIR:-"$root/target/$build_env"}
case "$target_root" in
    /*) ;;
    *) target_root="$root/$target_root" ;;
esac
sdk=${SK_NATIVE_SDK:-"$target_root/native-sdk/linux-x86_64"}
case "$sdk" in
    /*) ;;
    *) sdk="$root/$sdk" ;;
esac
header=${SK_NATIVE_ABI_HEADER:-"$sdk/include/searchkernel_vespa.h"}
bindings="$root/core/crates/searchkernel-vespa-bindings/src/bindings.rs"
temporary=$(mktemp)
trap 'rm -f "$temporary"' EXIT

[[ -f "$source_header" ]] || { echo "native ABI source header is missing: $source_header" >&2; exit 1; }
[[ -f "$header" ]] || { echo "native ABI header is missing: $header" >&2; exit 1; }

cargo run --locked --quiet --manifest-path "$root/tools/check-native-bindings/Cargo.toml" -- \
    "$header" "$temporary"

if [[ "${SK_NATIVE_BINDINGS_REGENERATE:-0}" == 1 ]]; then
    mv "$temporary" "$bindings"
    trap - EXIT
    echo "regenerated $bindings"
elif ! cmp -s "$temporary" "$bindings"; then
    echo "checked-in native ABI bindings are stale; set SK_NATIVE_BINDINGS_REGENERATE=1 to regenerate" >&2
    exit 1
else
    echo "native ABI bindings are current"
fi
