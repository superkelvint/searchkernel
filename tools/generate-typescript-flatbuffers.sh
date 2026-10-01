#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
flatc_bin=$(bash "$root/tools/resolve-flatbuffers.sh" flatc)
generated_root=${TYPESCRIPT_GENERATED_DIR:-"$root/clients/typescript/src/_wire/v1/generated"}
out="$generated_root/flatbuffers"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
"$flatc_bin" --ts --gen-object-api --gen-all -I "$root/api" -o "$tmp" \
  "$root/api/searchkernel.fbs" "$root/api/searchkernel_request.fbs" "$root/api/searchkernel_response.fbs"
rm -rf "$out"
mkdir -p "$(dirname "$out")"
mv "$tmp" "$out"
trap - EXIT
python=$(bash "$root/tools/resolve-python.sh")
"$python" "$root/tools/normalize-generated-text.py" "$out"
semantic_output=${TYPESCRIPT_SEMANTIC_MODEL_OUTPUT:-"$root/clients/typescript/src/semantic.ts"}
"$python" "$root/tools/generate-typescript-semantic-model.py" \
  "$out/search-kernel/wire" "$semantic_output"
