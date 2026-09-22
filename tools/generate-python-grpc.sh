#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)

python3 "$root/tools/check-python-dependencies.py" --codegen

python3 -m grpc_tools.protoc \
    --proto_path="$root/api" \
    --python_out="$root/clients/python/searchkernel" \
    --grpc_python_out="$root/clients/python/searchkernel" \
    "$root/api/rpc/searchkernel.proto"
