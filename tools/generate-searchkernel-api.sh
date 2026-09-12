#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
expected_version='25.12.19'
flatc_bin=${FLATC_PATH:-flatc}

actual_version=$($flatc_bin --version)
expected_output="flatc version ${expected_version}"
if [[ "$actual_version" != "$expected_output" ]]; then
    printf 'error: expected %s, got %s\n' "$expected_output" "$actual_version" >&2
    exit 1
fi

$flatc_bin \
    --rust \
    --rust-module-root-file \
    -o "$repo_root/crates/searchkernel-api/src/generated" \
    "$repo_root/api/searchkernel.fbs"

# Keep checked-in generated code consistent with the repository's Rust style.
rustfmt "$repo_root/crates/searchkernel-api/src/lib.rs" \
    "$repo_root/crates/searchkernel-api/src/generated/mod.rs" \
    "$repo_root/crates/searchkernel-api/src/generated/search_kernel/v1/"*_generated.rs
