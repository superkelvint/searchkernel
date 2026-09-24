#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/../../../.." && pwd)"
SCENARIO="${1:?usage: run-cpp-oracle.sh SCENARIO CONFIG_DIR DATA_DIR}"
CONFIG_DIR="${2:?usage: run-cpp-oracle.sh SCENARIO CONFIG_DIR DATA_DIR}"
DATA_DIR="${3:?usage: run-cpp-oracle.sh SCENARIO CONFIG_DIR DATA_DIR}"
BIN="$SCRIPT_DIR/target/searchreply_cpp_oracle"
PROV="$BIN.provenance"

source "$ROOT/reference/vespa/common/verify-common.sh"
require_pinned_vespa
[[ -x "$BIN" ]] || fail_verification "SearchReply C++ oracle binary missing; build it explicitly"
[[ -s "$PROV" ]] || fail_verification "SearchReply oracle provenance is missing"
python3 "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/run-cpp-oracle-python-1.py" "$PROV" "$ROOT/reference/vespa/PINNED_VESPA.json"

exec "$BIN" "$SCENARIO" "$DATA_DIR" "$CONFIG_DIR"
