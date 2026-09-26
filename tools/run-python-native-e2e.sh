#!/usr/bin/env bash
set -euo pipefail

ROOT=$(git rev-parse --show-toplevel)
cd "$ROOT"
PYTHON_BIN=$(bash "$ROOT/tools/resolve-python.sh")

prevalidated_native_sdk=0
while (($#)); do
    case "$1" in
        --prevalidated-native-sdk)
            prevalidated_native_sdk=1
            ;;
        *)
            echo "usage: $0 [--prevalidated-native-sdk]" >&2
            exit 2
            ;;
    esac
    shift
done

source "$ROOT/tools/client-e2e-runtime.sh"
prepare_client_e2e_runtime "$prevalidated_native_sdk"

SEARCHKERNEL_RUN_NATIVE_E2E=1 "$PYTHON_BIN" -m unittest discover \
    -s clients/python/tests \
    -p 'test_grpc_e2e.py'
