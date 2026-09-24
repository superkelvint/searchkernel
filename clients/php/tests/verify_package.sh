#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

composer validate "$ROOT/clients/php/composer.json" --strict --no-interaction

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

composer install   --working-dir "$TMP"   --no-interaction   --no-progress   --prefer-dist

php "$ROOT/clients/php/tests/package_consumer.php" "$TMP"
