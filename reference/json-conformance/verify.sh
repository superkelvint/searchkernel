#!/usr/bin/env bash
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
root=$(cd "$here/../.." && pwd)
if [[ -n "${FLATC_PATH:-}" ]]; then
    flatc_bin=$FLATC_PATH
elif command -v flatc >/dev/null 2>&1; then
    flatc_bin=flatc
else
    flatc_bin=$(find "$root/target/debug/build" -path '*/flatbuffers-build-*/out/bin/flatc' -type f -print -quit)
fi
expected_version='flatc version 25.12.19'

if [[ "$($flatc_bin --version)" != "$expected_version" ]]; then
    echo "json conformance: flatc version mismatch" >&2
    exit 1
fi

if rg -n 'searchkernel-(vespa|vespa-sys)|axum|hyper|actix|warp' \
    "$root/crates/searchkernel-json/Cargo.toml" \
    "$root/crates/searchkernel-json/src"; then
    echo "searchkernel-json must remain independent of Vespa and HTTP servers" >&2
    exit 1
fi

generated=$(mktemp -d)
trap 'rm -rf "$generated"' EXIT
"$flatc_bin" --rust --rust-module-root-file --gen-all -o "$generated" \
    "$root/api/searchkernel_ffi_request.fbs" \
    "$root/api/searchkernel_ffi_response.fbs"
rustfmt "$generated/search_kernel/ffi/v1/"*_generated.rs

for file in request_envelope_generated.rs request_payload_generated.rs \
    response_envelope_generated.rs response_payload_generated.rs; do
    cmp "$generated/search_kernel/ffi/v1/$file" \
        "$root/crates/searchkernel-json/src/generated/search_kernel/ffi/v1/$file"
done

FLATC_PATH="$flatc_bin" cargo run -q -p searchkernel-bindgen -- check
cargo test -q -p searchkernel-json --lib
cargo test -q -p searchkernel-ffi --all-features --lib

echo "C6 JSON conformance PASS"
