#!/usr/bin/env bash
set -euo pipefail
ORACLE_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
BIN="$ORACLE_DIR/target/m12_cpp_oracle"
[[ -x "$BIN" ]] || { echo "M12 oracle is not built; run oracle/build-cpp-oracle.sh" >&2; exit 1; }
[[ $# -eq 3 ]] || { echo "usage: prepare-fixture.sh SCENARIO_JSON CONFIG_DIR DATA_DIR" >&2; exit 2; }
exec "$BIN" prepare "$1" "$2" "$3"
