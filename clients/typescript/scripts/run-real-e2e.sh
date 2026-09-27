#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$ROOT"
RUNTIME="$ROOT/tools/run-runtime.sh"

source "$ROOT/tools/client-e2e-runtime.sh"
prepare_client_e2e_runtime

host_binary="$SEARCHKERNEL_LOCAL_HOST"

"$RUNTIME" npm ci --prefix clients/typescript
"$RUNTIME" npm run build --prefix clients/typescript

SEARCHKERNEL_LOCAL_HOST="$host_binary" \
  "$RUNTIME" node clients/typescript/scripts/verify-installed-managed-local-e2e.cjs

SEARCHKERNEL_LOCAL_HOST="$host_binary" \
  "$RUNTIME" npm run example:e2e --prefix clients/typescript
