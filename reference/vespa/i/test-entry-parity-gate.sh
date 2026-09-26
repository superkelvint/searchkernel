#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
cd "$ROOT"
if SEARCHKERNEL_TEST_PERTURB_GRPC_PARITY=1 cargo test -p searchkerneld --test real_engine_entry_parity --features native -- --nocapture; then
  echo "entry parity gate accepted a deliberately perturbed gRPC outcome" >&2
  exit 1
fi
echo "entry parity deliberate-regression proof passed"
