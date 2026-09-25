#!/usr/bin/env bash
set -euo pipefail

ROOT=$(git rev-parse --show-toplevel)
cd "$ROOT"
PYTHON_BIN=$(bash "$ROOT/tools/resolve-python.sh")

./dev native-sdk ensure
SEARCHKERNEL_RUN_NATIVE_E2E=1 "$PYTHON_BIN" -m unittest discover \
    -s clients/python/tests \
    -p 'test_grpc_e2e.py'
