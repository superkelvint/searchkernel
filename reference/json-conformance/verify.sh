#!/usr/bin/env bash
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
root=$(cd "$here/../.." && pwd)

"$here/scripts/check-required.sh"
"$here/scripts/check-bfbs.sh"
"$here/scripts/check-generation.sh"
python3 "$here/scripts/check-operation-map.py"
python3 "$here/scripts/check-generated-metadata.py"
python3 "$here/scripts/check-generated-docs.py"

cargo check -q --manifest-path "$here/client-contract/Cargo.toml"
cargo run -q --manifest-path "$here/client-behavior/Cargo.toml"
cargo run -q --manifest-path "$here/codec-contract/Cargo.toml"

"$here/scripts/check-bfbs-only-and-mutations.sh"

(
  cd "$root"
  cargo test -q -p searchkernel-ffi --all-features --lib json_convergence_tests
)

"$root/reference/api-conformance/verify.sh"

echo "C6 BFBS/client/transport/JSON conformance PASS"
