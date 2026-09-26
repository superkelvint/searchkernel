#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$ROOT"

./dev native-sdk ensure

PYTHON="$(bash "$ROOT/tools/resolve-python.sh")"
target_root="${CARGO_TARGET_DIR:-target/host}"
cargo_messages="$(mktemp "${TMPDIR:-/tmp}/searchkernel-typescript-e2e-cargo.XXXXXX")"
trap 'rm -f "$cargo_messages"' EXIT

cargo build -p searchkerneld --features native --message-format=json-render-diagnostics | tee "$cargo_messages"
adapter_lib="$("$PYTHON" "$ROOT/.github/ci/resolve-native-adapter-lib.py" "$cargo_messages" "$target_root")"

target_root="${CARGO_TARGET_DIR:-target/host}"
case "$target_root" in
  /*) host_binary="$target_root/debug/searchkerneld" ;;
  *) host_binary="$ROOT/$target_root/debug/searchkerneld" ;;
esac

if [[ ! -x "$host_binary" ]]; then
  echo "TypeScript E2E host binary missing after build: $host_binary" >&2
  exit 1
fi

npm ci --prefix clients/typescript
npm run build --prefix clients/typescript

SEARCHKERNEL_LOCAL_HOST="$host_binary" \
SEARCHKERNEL_SDK_LIB="$adapter_lib" \
  node clients/typescript/scripts/verify-installed-managed-local-e2e.cjs

SEARCHKERNEL_LOCAL_HOST="$host_binary" \
SEARCHKERNEL_SDK_LIB="$adapter_lib" \
  npm run example:e2e --prefix clients/typescript
