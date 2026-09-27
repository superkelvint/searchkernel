#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
cd "$ROOT"
TEST="$ROOT/searchkerneld/tests/real_engine_entry_parity.rs"
TMP="$(mktemp)"
trap 'rm -f "$TMP"' EXIT
sed '/let (_daemon, endpoint) = launch_production_daemon(&grpc_path.0);/c\    let endpoint = format!("http://127.0.0.1:{}", unused_tcp_addr().port());' "$TEST" > "$TMP"
if cmp -s "$TEST" "$TMP"; then
  echo "entry parity deliberate-regression proof did not perturb the remote path" >&2
  exit 1
fi
ORIGINAL="$(mktemp)"
cp "$TEST" "$ORIGINAL"
trap 'cp "$ORIGINAL" "$TEST"; rm -f "$ORIGINAL" "$TMP"' EXIT
cp "$TMP" "$TEST"
if cargo test -p searchkerneld --test real_engine_entry_parity --features native -- --nocapture; then
  echo "entry parity gate passed after bypassing production searchkerneld" >&2
  exit 1
fi
echo "entry parity remote-path deliberate-regression proof passed"
