#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/../../../.." && pwd)"
TARGET_DIR="$SCRIPT_DIR/target"
OUT="$TARGET_DIR/searchreply_cpp_oracle"

mkdir -p "$TARGET_DIR"
M8_DIRECT_ORACLE="$SCRIPT_DIR/searchreply_oracle.cpp" \
M8_DIRECT_TARGET_DIR="$TARGET_DIR" \
M8_DIRECT_OUT="$OUT" \
  "$ROOT/reference/vespa/m8/oracle/build-cpp-oracle.sh"

printf 'SearchReply direct C++ oracle: %s\n' "$OUT" >&2
