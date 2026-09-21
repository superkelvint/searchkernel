#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
client="$root/clients/typescript"
protoc_bin=${PROTOC_PATH:-"$client/node_modules/grpc-tools/bin/protoc"}
plugin=${GRPC_WEB_PLUGIN_PATH:-"$client/node_modules/protoc-gen-grpc-web/bin/protoc-gen-grpc-web"}
node_plugin=${GRPC_NODE_PLUGIN_PATH:-"$client/node_modules/grpc-tools/bin/grpc_node_plugin"}
[[ -x "$protoc_bin" && -x "$plugin" && -x "$node_plugin" ]] || { echo 'error: install TypeScript package generation dependencies' >&2; exit 1; }
out="$client/src/generated/grpc"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
# The protobuf message module is shared by the browser and Node stubs. Generate
# both service clients from the same schema in one pass so they cannot drift.
"$protoc_bin" -I "$root/api" --js_out=import_style=commonjs,binary:"$tmp" \
  --plugin=protoc-gen-grpc-web="$plugin" --grpc-web_out=import_style=typescript,mode=grpcweb:"$tmp" \
  --plugin=protoc-gen-grpc="$node_plugin" --grpc_out=grpc_js:"$tmp" \
  "$root/api/rpc/searchkernel.proto"
rm -rf "$out"
mkdir -p "$(dirname "$out")"
mv "$tmp" "$out"
trap - EXIT
