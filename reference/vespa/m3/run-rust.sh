#!/usr/bin/env bash
set -euo pipefail
if [[ $# -ne 2 ]]; then
  echo "usage: $0 document|query OUTPUT.jsonl" >&2
  exit 2
fi
MODE="$1"
OUTPUT="$2"
[[ "$MODE" == document || "$MODE" == query ]] || { echo "invalid mode: $MODE" >&2; exit 2; }
ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
M3="$ROOT/reference/vespa/m3"

TARGET_DIR="${CARGO_TARGET_DIR:-$M3/rust-driver/target}"
mkdir -p "$TARGET_DIR"
CARGO_MESSAGES="$(mktemp "$TARGET_DIR/searchkernel-m3-cargo-messages.XXXXXX")"
trap 'rm -f "$CARGO_MESSAGES"' EXIT
cargo build --quiet \
  --message-format=json-render-diagnostics \
  --target-dir "$TARGET_DIR" \
  --manifest-path "$M3/rust-driver/Cargo.toml" \
  >"$CARGO_MESSAGES"
ADAPTER_LIB="$(python3 "$ROOT/.github/ci/resolve-native-adapter-lib.py" "$CARGO_MESSAGES" "$TARGET_DIR")"
BIN="$TARGET_DIR/debug/searchkernel-m3-conformance-driver"
LD_LIBRARY_PATH="$ADAPTER_LIB${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
python3 "$M3/run_cases.py" \
  --cases "$M3/scenarios/cases.jsonl" \
  --output "$OUTPUT" \
  "$BIN" "$MODE"
