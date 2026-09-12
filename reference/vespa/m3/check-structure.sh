#!/usr/bin/env bash
set -euo pipefail
ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
PROD="$ROOT/crates/searchkernel-vespa"

[[ -d "$PROD" ]] || { echo "missing $PROD" >&2; exit 1; }

if grep -RInE --exclude-dir=target --exclude='*.md' \
    'reference/vespa/m3|expected/(document|query)\.jsonl|LinguisticsOracle|Command::new\("java"\)|ProcessBuilder' \
    "$PROD/src" "$PROD/Cargo.toml" 2>/dev/null; then
  echo "M3 production code appears to depend on the oracle/goldens/Java runtime" >&2
  exit 1
fi

if grep -nE 'searchkernel-core|searchkernel_core' "$PROD/Cargo.toml" >/dev/null 2>&1; then
  echo "M-series dependency firewall violated: searchkernel-vespa depends on searchkernel-core" >&2
  exit 1
fi

# High-signal fixture-specific shortcuts. Scenario ids must never occur as
# hard-coded string values in production source. Match quoted literals rather
# than arbitrary substrings, since ids such as "empty" are ordinary words in
# legitimate error messages and test names.
while IFS= read -r id; do
  [[ -z "$id" ]] && continue
  if grep -RInF --exclude-dir=target -- "\"$id\"" "$PROD/src" >/dev/null 2>&1 ||
     grep -RInF --exclude-dir=target -- "'$id'" "$PROD/src" >/dev/null 2>&1; then
    echo "fixture scenario id '$id' appears in production source" >&2
    exit 1
  fi
done < <(python3 - "$ROOT/reference/vespa/m3/scenarios/cases.jsonl" <<'PY'
import json, sys
for line in open(sys.argv[1], encoding='utf-8'):
    line=line.strip()
    if line: print(json.loads(line)['id'])
PY
)
