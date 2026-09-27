#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$ROOT"

source "$ROOT/tools/client-e2e-runtime.sh"
prepare_client_e2e_runtime

host_binary="$SEARCHKERNEL_LOCAL_HOST"

npm ci --prefix clients/typescript
npm run build --prefix clients/typescript

SEARCHKERNEL_LOCAL_HOST="$host_binary" \
SEARCHKERNEL_SDK_LIB="$SEARCHKERNEL_SDK_LIB" \
  node clients/typescript/scripts/verify-installed-managed-local-e2e.cjs

SEARCHKERNEL_LOCAL_HOST="$host_binary" \
SEARCHKERNEL_SDK_LIB="$SEARCHKERNEL_SDK_LIB" \
  npm run example:e2e --prefix clients/typescript
