#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
python=${PYTHON:-python3}

"$python" "$root/tools/check-python-dependencies.py" --codegen

"$python" -m grpc_tools.protoc \
    --proto_path="$root/api" \
    --python_out="$root/clients/python/searchkernel" \
    --grpc_python_out="$root/clients/python/searchkernel" \
    "$root/api/rpc/searchkernel.proto"
