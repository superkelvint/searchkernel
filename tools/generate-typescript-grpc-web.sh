#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
client="$root/clients/typescript"
protoc_bin=${PROTOC_PATH:-"$client/node_modules/grpc-tools/bin/protoc"}
plugin=${GRPC_WEB_PLUGIN_PATH:-"$client/node_modules/protoc-gen-grpc-web/bin/protoc-gen-grpc-web"}
[[ -x "$protoc_bin" && -x "$plugin" ]] || { echo 'error: install TypeScript package generation dependencies' >&2; exit 1; }
out="$client/src/generated/grpc"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
"$protoc_bin" -I "$root/api" --js_out=import_style=commonjs,binary:"$tmp" \
  --plugin=protoc-gen-grpc-web="$plugin" --grpc-web_out=import_style=typescript,mode=grpcweb:"$tmp" \
  "$root/api/rpc/searchkernel.proto"
rm -rf "$out"
mkdir -p "$(dirname "$out")"
mv "$tmp" "$out"
trap - EXIT
