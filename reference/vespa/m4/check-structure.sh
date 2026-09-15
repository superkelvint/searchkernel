#!/usr/bin/env bash
set -euo pipefail
M4="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$M4/../../.." && pwd)"
RUST="$ROOT/core/crates/searchkernel-vespa-engine"
SYS="$ROOT/core/crates/searchkernel-vespa-bindings"
NATIVE="$ROOT/core/native/vespa"
SCEN="$ROOT/reference/vespa/m4/scenarios/cases.json"

[[ -d "$RUST" && -d "$SYS" && -d "$NATIVE" ]] || {
  echo "expected M-series crate/native layout is missing" >&2
  exit 1
}

if grep -nE 'searchkernel-ir|searchkernel_ir' "$RUST/Cargo.toml" >/dev/null 2>&1; then
  echo "M-series firewall violated: searchkernel-vespa-engine depends on searchkernel-ir" >&2
  exit 1
fi

# Production may not depend on verifier/oracle/goldens.
if grep -RInE --exclude-dir=target --exclude='*.md' \
    'reference/vespa/m4|m4_text_oracle|expected/results\.jsonl' \
    "$RUST/src" "$SYS/src" "$NATIVE" 2>/dev/null; then
  echo "M4 production code appears to depend on acceptance artifacts" >&2
  exit 1
fi

# Java/oracle execution belongs only to reference tooling, never production.
if grep -RInE --exclude-dir=target --exclude='*.md' \
    'Command::new\("java"\)|ProcessBuilder|LinguisticsOracle' \
    "$RUST/src" "$SYS/src" "$NATIVE" 2>/dev/null; then
  echo "production M4 path invokes Java/oracle" >&2
  exit 1
fi

# Toy-tokenizer removal is verified by code review plus the M3-sensitive runtime
# scenario; do not grep generic Unicode implementation primitives here.

source "$ROOT/reference/vespa/common/verify-common.sh"
check_fixture_literals "$SCEN" "$RUST/src" "$SYS/src" "$NATIVE"

# Vespa headers belong only to core/native/vespa, never the safe Rust crate.
if grep -RInE '#include[[:space:]]*[<\"]vespa/' "$RUST" "$SYS" --include='*.rs' 2>/dev/null; then
  echo "Vespa C++ headers leaked outside native adapter" >&2
  exit 1
fi

# Raw extern declarations belong in sys/raw layer, not safe wrapper.
if grep -RInE 'extern[[:space:]]+"C"' "$RUST/src" 2>/dev/null; then
  echo "raw FFI found in safe searchkernel-vespa-engine crate" >&2
  exit 1
fi

echo "M4 structural checks passed"
