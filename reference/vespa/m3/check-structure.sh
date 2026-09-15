#!/usr/bin/env bash
set -euo pipefail
ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
PROD="$ROOT/core/crates/searchkernel-vespa-engine"

[[ -d "$PROD" ]] || { echo "missing $PROD" >&2; exit 1; }

if grep -RInE --exclude-dir=target --exclude='*.md' \
    'reference/vespa/m3|expected/(document|query)\.jsonl|LinguisticsOracle|Command::new\("java"\)|ProcessBuilder' \
    "$PROD/src" "$PROD/Cargo.toml" 2>/dev/null; then
  echo "M3 production code appears to depend on the oracle/goldens/Java runtime" >&2
  exit 1
fi

if grep -nE 'searchkernel-ir|searchkernel_ir' "$PROD/Cargo.toml" >/dev/null 2>&1; then
  echo "M-series dependency firewall violated: searchkernel-vespa-engine depends on searchkernel-ir" >&2
  exit 1
fi

source "$ROOT/reference/vespa/common/verify-common.sh"
check_fixture_literals "$ROOT/reference/vespa/m3/scenarios/cases.jsonl" \
  "$ROOT/core/crates/searchkernel-vespa-engine/src" "$ROOT/core/crates/searchkernel-vespa-bindings/src" "$ROOT/core/native/vespa"
