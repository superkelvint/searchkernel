#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
REPO=$(CDPATH= cd -- "$ROOT/../.." && pwd)
PYTHON=$(bash "$REPO/tools/resolve-python.sh")
CARGO=$(bash "$REPO/tools/resolve-rust.sh" cargo)

"$ROOT/verify-fixtures.sh"

"$PYTHON" "$REPO/tools/check-testkit-backend-free.py" \
    "$REPO/core/crates/searchkernel-testkit/Cargo.toml" \
    "$REPO/core/crates/searchkernel-testkit/src"

(cd "$REPO" && "$CARGO" run -q -p searchkernel-testkit --bin searchkernel-conformance -- "$ROOT")
