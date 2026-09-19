#!/usr/bin/env bash
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
root=$(cd "$here/../.." && pwd)

"$here/scripts/check-required.sh"
"$here/scripts/check-bfbs.sh"
"$here/scripts/check-generation.sh"
"$here/scripts/check-fail-closed.sh"
python3 "$here/scripts/check-operation-map.py"
python3 "$here/scripts/check-generated-metadata.py"

cargo run -q --manifest-path "$here/codec-contract/Cargo.toml"

"$root/reference/api-conformance/verify.sh"

echo "C6 BFBS/model/JSON conformance PASS"
