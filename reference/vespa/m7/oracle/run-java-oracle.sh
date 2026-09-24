#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
M7="$(cd "$SCRIPT_DIR/.." && pwd)"
SCENARIO="${1:?usage: run-java-oracle.sh SCENARIO.json}"
"$SCRIPT_DIR/build-java-oracle.sh" >&2
mapfile -t META < <(python3 "reference/vespa/m7/oracle/run-java-oracle-python-1.py" "$SCENARIO"
)
CP="$(cat "$SCRIPT_DIR/target/runtime-classpath.txt"):$SCRIPT_DIR/target/classes"
exec java -cp "$CP" searchkernel.m7.QueryOracle "${META[0]}" "${META[1]}" "${META[2]}" "${META[3]}"
