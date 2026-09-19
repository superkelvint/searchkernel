#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)

python3 - <<'PY'
import google.protobuf
import grpc

assert google.protobuf.__version__ == "7.35.1", google.protobuf.__version__
assert grpc.__version__ == "1.84.0", grpc.__version__
PY

python3 -m grpc_tools.protoc \
    --proto_path="$root/api" \
    --python_out="$root/clients/python/searchkernel" \
    --grpc_python_out="$root/clients/python/searchkernel" \
    "$root/api/rpc/searchkernel.proto"
