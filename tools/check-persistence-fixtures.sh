#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
FIXTURES="$ROOT/reference/persistence/fixtures"
MANIFEST_RS="$ROOT/core/crates/searchkernel/src/index/manifest.rs"
M5_DATA="$ROOT/reference/vespa/m5/fixtures/persisted-v1/index"

[[ -d "$FIXTURES" ]] || { echo "missing persistence fixture corpus" >&2; exit 1; }
[[ -f "$FIXTURES/README.md" ]] || { echo "missing persistence fixture README" >&2; exit 1; }
[[ -d "$M5_DATA" ]] || { echo "missing preserved M5 storage fixture" >&2; exit 1; }

"${CI_PYTHON:-python3}" "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/check-persistence-fixtures-python-1.py" "$FIXTURES" "$MANIFEST_RS" "$M5_DATA"

echo "Persistence fixture structure PASS"
