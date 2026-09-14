#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
M7="$(cd "$SCRIPT_DIR/.." && pwd)"
SCENARIO="${1:?usage: run-java-oracle.sh SCENARIO.json}"
"$SCRIPT_DIR/build-java-oracle.sh" >&2
mapfile -t META < <(python3 - "$SCENARIO" <<'PY'
import json,sys
x=json.load(open(sys.argv[1]))
print(x['id']); print(x['schema']); print(','.join(f"{f['name']}:{f['type']}" for f in x['fields'])); print(x['yql'])
PY
)
CP="$(cat "$SCRIPT_DIR/target/runtime-classpath.txt"):$SCRIPT_DIR/target/classes"
exec java -cp "$CP" searchkernel.m7.QueryOracle "${META[0]}" "${META[1]}" "${META[2]}" "${META[3]}"
