#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
source_header="$root/core/native/vespa/searchkernel_vespa.h"
bindings="$root/core/crates/searchkernel-vespa-bindings/src/bindings.rs"
temporary=$(mktemp)
trap 'rm -f "$temporary"' EXIT

[[ -f "$source_header" ]] || { echo "native ABI source header is missing: $source_header" >&2; exit 1; }
cargo_bin=$(bash "$root/tools/resolve-rust.sh" cargo)
"$cargo_bin" run --locked --quiet --manifest-path "$root/tools/check-native-bindings/Cargo.toml" -- \
    "$source_header" "$temporary"

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
