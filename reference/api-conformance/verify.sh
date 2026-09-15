#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)

"$ROOT/verify-fixtures.sh"

if rg -n 'searchkernel-(vespa|vespa-sys)' \
    "$ROOT/../../core/crates/searchkernel-testkit/Cargo.toml" \
    "$ROOT/../../core/crates/searchkernel-testkit/src"; then
    echo "searchkernel-testkit must remain backend-free" >&2
    exit 1
fi

(cd "$ROOT/../.." && cargo run -q -p searchkernel-testkit --bin searchkernel-conformance -- "$ROOT")
