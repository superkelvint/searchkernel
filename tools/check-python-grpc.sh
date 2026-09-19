#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
temporary=$(mktemp -d)
trap 'rm -rf "$temporary"' EXIT

python3 - <<'PY'
import google.protobuf
import grpc

assert google.protobuf.__version__ == "7.35.1", google.protobuf.__version__
assert grpc.__version__ == "1.84.0", grpc.__version__
PY

python3 -m grpc_tools.protoc \
    --proto_path="$root/api" \
    --python_out="$temporary" \
    --grpc_python_out="$temporary" \
    "$root/api/rpc/searchkernel.proto"

cmp "$temporary/rpc/searchkernel_pb2.py" \
    "$root/clients/python/searchkernel/rpc/searchkernel_pb2.py"
cmp "$temporary/rpc/searchkernel_pb2_grpc.py" \
    "$root/clients/python/searchkernel/rpc/searchkernel_pb2_grpc.py"
echo "Python gRPC generation reproducibility PASS"
