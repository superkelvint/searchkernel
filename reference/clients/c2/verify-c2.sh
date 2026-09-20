#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
REPO_ROOT=$(CDPATH= cd -- "$ROOT/../../.." && pwd)

cargo check --quiet --manifest-path "$ROOT/api-contract/Cargo.toml" --bin c2

IR_SOURCES=$(find "$REPO_ROOT/core/crates/searchkernel-ir/src" -type f -name '*.rs' -print)
rustfmt --edition 2021 --check \
    "$REPO_ROOT/core/crates/searchkernel-protocol/src/lib.rs" \
    $IR_SOURCES \
    "$REPO_ROOT/core/crates/searchkernel-ir/tests/semantics.rs" \
    "$ROOT/api-contract/src/bin/c2.rs" \
    "$ROOT/api-contract/tests/semantics.rs"
cargo clippy --quiet --manifest-path "$ROOT/api-contract/Cargo.toml" \
    --all-targets -- -D warnings
cargo test --quiet --manifest-path "$ROOT/api-contract/Cargo.toml" \
    --all-targets

TREE=$(cargo tree --quiet --manifest-path "$REPO_ROOT/core/crates/searchkernel-ir/Cargo.toml")
if printf '%s\n' "$TREE" | grep -Eiq 'vespa|searchkernel-(sys|vespa)'; then
    echo "C2 dependency graph contains a Vespa dependency" >&2
    printf '%s\n' "$TREE" >&2
    exit 1
fi

if rg -n -i '(^|[[:space:]])(use|extern[[:space:]]+crate)[[:space:]]+(vespa|searchkernel_(sys|vespa))|vespa::|searchkernel_(sys|vespa)' \
    "$REPO_ROOT/core/crates/searchkernel-protocol" "$REPO_ROOT/core/crates/searchkernel-ir"; then
    echo "C2 source contains a Vespa dependency or namespace" >&2
    exit 1
fi

echo "C2 PASS"
