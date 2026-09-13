#!/usr/bin/env bash
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
root=$(cd "$here/../.." && pwd)

"$root/tools/check-json-bfbs.sh"
cargo test -q -p searchkernel-json --tests
cargo test -q -p searchkernel-ffi --all-features --lib
"$root/reference/api-conformance/verify.sh"

echo "C6 JSON conformance PASS"
