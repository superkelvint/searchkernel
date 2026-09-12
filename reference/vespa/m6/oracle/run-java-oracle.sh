#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
"$SCRIPT_DIR/build-java-oracle.sh" >&2
CP="$(cat "$SCRIPT_DIR/target/runtime-classpath.txt"):$SCRIPT_DIR/target/classes"
exec java -cp "$CP" searchkernel.m6.SchemaOracle "$@"
