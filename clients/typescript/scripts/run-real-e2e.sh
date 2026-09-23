#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$ROOT"

./dev native-sdk ensure
cargo build -p searchkerneld

target_root="${CARGO_TARGET_DIR:-target/host}"
case "$target_root" in
  /*) host_binary="$target_root/debug/searchkerneld" ;;
  *) host_binary="$ROOT/$target_root/debug/searchkerneld" ;;
esac

if [[ ! -x "$host_binary" ]]; then
  echo "TypeScript E2E host binary missing after build: $host_binary" >&2
  exit 1
fi

SEARCHKERNEL_LOCAL_HOST="$host_binary" \
  npm run example:e2e --prefix clients/typescript
