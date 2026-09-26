#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cargo=$(bash "$root/tools/resolve-rust.sh" cargo)
checked_in="$root/core/crates/searchkernel-rpc/src/generated/rpc.rs"
temporary=$(mktemp -d)
trap 'rm -rf "$temporary"' EXIT

"$cargo" run --locked --quiet --manifest-path "$root/tools/rpc-codegen/Cargo.toml" -- "$temporary"
cmp "$temporary/searchkernel.rpc.v1.rs" "$checked_in"
echo "Search Kernel Rust gRPC bindings are current"
