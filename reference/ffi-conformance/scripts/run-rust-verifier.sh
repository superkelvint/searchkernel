#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "$0")/common.sh"
root="$(repo_root)"
cd "$root"
command -v cargo >/dev/null 2>&1 || fail "cargo not found"
cargo test --manifest-path reference/ffi-conformance/rust/Cargo.toml --all-targets --all-features
