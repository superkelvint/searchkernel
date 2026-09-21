#!/usr/bin/env bash
set -euo pipefail
M14="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
[[ "${ALLOW_M14_GOLDEN_REGEN:-}" == "1" ]] || { echo 'set ALLOW_M14_GOLDEN_REGEN=1 to regenerate M14 goldens' >&2; exit 2; }
[[ -x "$M14/oracle/target/m14_cpp_oracle" ]] || { echo 'build the reviewed M14 oracle first' >&2; exit 2; }
TMP="$(mktemp -d)"; trap 'rm -rf "$TMP"' EXIT
for scenario in "$M14"/scenarios/[0-9][0-9]-*.json; do
  id="$(python3 -c 'import json,sys;print(json.load(open(sys.argv[1]))["id"])' "$scenario")"
  "$M14/oracle/run-cpp-oracle.sh" "$scenario" | "$M14/canonicalize_behavior.py" > "$M14/expected/$id.json"
done
cfg="$TMP/config"; data="$TMP/data"
"$M14/oracle/prepare-fixture.sh" "$M14/scenarios/01-hnsw-query-controls.json" "$cfg" "$data"
"$M14/canonicalize_cfg.py" "$cfg" > "$M14/expected/hnsw-config.json"
"$M14/validate_expected.py"
echo 'M14 goldens regenerated from reviewed direct oracle' >&2
