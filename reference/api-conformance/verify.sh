#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)

"$ROOT/verify-fixtures.sh"

python3 "$ROOT/../../tools/check-testkit-backend-free.py" \
    "$ROOT/../../core/crates/searchkernel-testkit/Cargo.toml" \
    "$ROOT/../../core/crates/searchkernel-testkit/src"

(cd "$ROOT/../.." && cargo run -q -p searchkernel-testkit --bin searchkernel-conformance -- "$ROOT")
