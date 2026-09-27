#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
TMP="$(mktemp -d)"
RUNTIME="$ROOT/tools/run-runtime.sh"
COMPOSER=$(command -v composer 2>/dev/null || true)
[[ -n "$COMPOSER" ]] || {
    echo "PHP package verification requires composer" >&2
    exit 1
}
trap 'rm -rf "$TMP"' EXIT

"$RUNTIME" php "$COMPOSER" validate "$ROOT/clients/php/composer.json" --strict --no-interaction

cat >"$TMP/composer.json" <<JSON
{
  "repositories": [
    {
      "type": "path",
      "url": "$ROOT/clients/php",
      "options": {
        "symlink": false
      }
    }
  ],
  "require": {
    "searchkernel/searchkernel": "*"
  },
  "minimum-stability": "dev",
  "prefer-stable": true
}
JSON

"$RUNTIME" php "$COMPOSER" install   --working-dir "$TMP"   --no-interaction   --no-progress   --prefer-dist

"$RUNTIME" php "$ROOT/clients/php/tests/package_consumer.php" "$TMP"
