#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
REPO_ROOT=$(CDPATH= cd -- "$ROOT/../../.." && pwd)

cargo check --quiet --manifest-path "$ROOT/api-contract/Cargo.toml" --bin c2
rustfmt --edition 2021 --check \
    "$REPO_ROOT/crates/searchkernel-api/src/lib.rs" \
    "$REPO_ROOT/crates/searchkernel-api/build.rs" \
    "$REPO_ROOT/crates/searchkernel-core/src/lib.rs" \
    "$REPO_ROOT/crates/searchkernel-core/src/error.rs" \
    "$REPO_ROOT/crates/searchkernel-core/src/model.rs" \
    "$REPO_ROOT/crates/searchkernel-core/src/resolve.rs" \
    "$REPO_ROOT/crates/searchkernel-core/tests/semantics.rs" \
    "$ROOT/api-contract/src/bin/c2.rs" \
    "$ROOT/api-contract/tests/semantics.rs"
cargo clippy --quiet --manifest-path "$ROOT/api-contract/Cargo.toml" \
    --all-targets -- -D warnings
cargo test --quiet --manifest-path "$ROOT/api-contract/Cargo.toml" \
    --all-targets

TREE=$(cargo tree --quiet --manifest-path "$REPO_ROOT/crates/searchkernel-core/Cargo.toml")
if printf '%s\n' "$TREE" | grep -Eiq 'vespa|searchkernel-(sys|vespa)'; then
    echo "C2 dependency graph contains a Vespa dependency" >&2
    printf '%s\n' "$TREE" >&2
    exit 1
fi

if rg -n -i 'vespa|searchkernel-(sys|vespa)' \
    "$REPO_ROOT/crates/searchkernel-api" "$REPO_ROOT/crates/searchkernel-core"; then
    echo "C2 source contains a Vespa dependency or term" >&2
    exit 1
fi

echo "C2 PASS"
