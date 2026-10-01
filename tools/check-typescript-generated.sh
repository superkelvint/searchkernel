#!/usr/bin/env bash
set -euo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
node=$(bash "$root/tools/resolve-node.sh" node)
PATH="$(dirname -- "$node"):$PATH"
export PATH
npm=$(bash "$root/tools/resolve-node.sh" npm)
client="$root/clients/typescript"
generated="$client/src/_wire/v1/generated/flatbuffers"
fresh=$(mktemp -d)
semantic_fresh=$(mktemp)
trap 'rm -rf -- "$fresh"; rm -f -- "$semantic_fresh"' EXIT

# The generated tree is intentionally ignored, so a clean checkout needs one
# bootstrap generation before it can be compared with an independently fresh
# tree. Once bootstrapped, never overwrite the consumed tree during the check:
# that is what makes stale local output observable.
if [[ ! -d "$generated" ]]; then
    "$npm" run generate:flatbuffers --prefix "$client"
fi

TYPESCRIPT_GENERATED_DIR="$fresh" TYPESCRIPT_SEMANTIC_MODEL_OUTPUT="$semantic_fresh" "$npm" run generate:flatbuffers --prefix "$client"
diff -ruN -- "$generated" "$fresh/flatbuffers"
diff -u -- "$client/src/semantic.ts" "$semantic_fresh"
