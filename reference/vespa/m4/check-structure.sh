#!/usr/bin/env bash
set -euo pipefail
M4="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$M4/../../.." && pwd)"
RUST="$ROOT/crates/searchkernel-vespa"
SYS="$ROOT/crates/searchkernel-vespa-sys"
NATIVE="$ROOT/native/vespa"
SCEN="$ROOT/reference/vespa/m4/scenarios/cases.json"

[[ -d "$RUST" && -d "$SYS" && -d "$NATIVE" ]] || {
  echo "expected M-series crate/native layout is missing" >&2
  exit 1
}

if grep -nE 'searchkernel-core|searchkernel_core' "$RUST/Cargo.toml" >/dev/null 2>&1; then
  echo "M-series firewall violated: searchkernel-vespa depends on searchkernel-core" >&2
  exit 1
fi

# Production may not depend on verifier/oracle/goldens.
if grep -RInE --exclude-dir=target --exclude='*.md' \
    'reference/vespa/m4|M4-IMMUTABLE|m4_text_oracle|expected/results\.jsonl' \
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

# Frozen aliases/scenario ids may appear in the driver, never production.
python3 - "$SCEN" <<'PY' | while IFS= read -r needle; do
import json, sys
s=json.load(open(sys.argv[1], encoding='utf-8'))
for x in s['documents']:
    print(x)
for st in s['steps']:
    if 'id' in st: print(st['id'])
PY
  [[ -z "$needle" ]] && continue
  if grep -RInF --exclude-dir=target -- "$needle" "$RUST/src" "$SYS/src" "$NATIVE" >/dev/null 2>&1; then
    echo "M4 fixture identifier '$needle' appears in production source" >&2
    exit 1
  fi
done

# Vespa headers belong only to native/vespa, never the safe Rust crate.
if grep -RInE '#include[[:space:]]*[<\"]vespa/' "$RUST" "$SYS" --include='*.rs' 2>/dev/null; then
  echo "Vespa C++ headers leaked outside native adapter" >&2
  exit 1
fi

# Raw extern declarations belong in sys/raw layer, not safe wrapper.
if grep -RInE 'extern[[:space:]]+"C"' "$RUST/src" 2>/dev/null; then
  echo "raw FFI found in safe searchkernel-vespa crate" >&2
  exit 1
fi

echo "M4 structural checks passed"
