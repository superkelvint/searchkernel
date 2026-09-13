#!/usr/bin/env bash
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../.." && pwd)"

"$here/scripts/check-header.sh"
"$here/scripts/check-schema.sh"
"$here/scripts/check-dependencies.sh"
"$here/scripts/check-source-invariants.sh"

cd "$root"
cargo build -p searchkernel-ffi --release --no-default-features

"$here/scripts/check-symbols.sh"
"$here/scripts/check-native-dependencies.sh"
"$here/scripts/run-c-smoke.sh"
"$here/scripts/run-rust-verifier.sh"

echo "C5 FFI conformance PASS"
