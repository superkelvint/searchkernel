#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

cargo run --quiet -p searchkernel-testkit --bin searchkernel-persistence-fixtures -- "$TMP/fixtures"
diff -ru "$ROOT/reference/persistence/fixtures" "$TMP/fixtures"

echo "Persistence fixture generation PASS"
