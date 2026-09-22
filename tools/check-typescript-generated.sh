#!/usr/bin/env bash
set -euo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
client="$root/clients/typescript"
generated="$client/src/generated"
fresh=$(mktemp -d)
trap 'rm -rf -- "$fresh"' EXIT

# The generated tree is intentionally ignored, so a clean checkout needs one
# bootstrap generation before it can be compared with an independently fresh
# tree. Once bootstrapped, never overwrite the consumed tree during the check:
# that is what makes stale local output observable.
if [[ ! -d "$generated" ]]; then
    npm run generate --prefix "$client"
fi

TYPESCRIPT_GENERATED_DIR="$fresh" npm run generate --prefix "$client"
diff -ruN -- "$generated" "$fresh"
