#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
client="$root/clients/typescript"
toolchain_node_modules=${SEARCHKERNEL_TYPESCRIPT_NODE_MODULES:-/opt/searchkernel/typescript/node_modules}
if [[ ! -e "$client/node_modules" && ! -L "$client/node_modules" ]]; then
    if [[ -d "$toolchain_node_modules" ]]; then
        ln -s "$toolchain_node_modules" "$client/node_modules"
    elif [[ "${SEARCHKERNEL_IN_CI_IMAGE:-0}" == "1" ]]; then
        npm=$(bash "$root/tools/resolve-node.sh" npm)
        echo 'warning: CI image predates baked TypeScript dependencies; bootstrapping locked packages once for this source snapshot' >&2
        "$npm" ci --prefix "$client" --no-audit --no-fund
    fi
fi
protoc_bin=${PROTOC_PATH:-"$client/node_modules/grpc-tools/bin/protoc"}
plugin=${GRPC_WEB_PLUGIN_PATH:-"$client/node_modules/protoc-gen-grpc-web/bin/protoc-gen-grpc-web"}
node_plugin=${GRPC_NODE_PLUGIN_PATH:-"$client/node_modules/grpc-tools/bin/grpc_node_plugin"}
[[ -x "$protoc_bin" && -x "$plugin" && -x "$node_plugin" ]] || {
    echo 'error: TypeScript generation dependencies are unavailable; use the canonical CI toolchain image or install the locked client dependencies' >&2
    exit 1
}
generated_root=${TYPESCRIPT_GENERATED_DIR:-"$client/src/generated"}
out="$generated_root/grpc"
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
