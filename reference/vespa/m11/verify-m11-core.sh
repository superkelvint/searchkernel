#!/usr/bin/env bash
set -euo pipefail

M11="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$M11/../../.." && pwd)"
source "$M11/verify-m11-common.sh"

require_pinned_vespa
python3 "$M11/validate.py"
python3 "$M11/test_structure.py"
"$M11/check-structure.sh"
prepare_hardening_sdk
build_m11_driver

cargo check --quiet -p searchkernel-vespa-engine --example api-contract-m11
LD_LIBRARY_PATH="$M11_SDK/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
  cargo test --quiet --no-run -p searchkernel-vespa-engine --all-targets --all-features
# Cargo test executables resolve the adapter beside themselves through their
# $ORIGIN/.searchkernel-native DT_NEEDED entry, which LD_LIBRARY_PATH cannot
# override. Cumulative milestone builds can leave a normal adapter staged in
# deps, so restage the hardening runtime after Cargo has finished compiling.
stage_m11_adapter_for_executable_dir "$M11_CARGO_TARGET/debug/deps" "$M11_SDK"
LD_LIBRARY_PATH="$M11_SDK/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
  cargo test --quiet -p searchkernel-vespa-engine --all-targets --all-features

TARGET="$ROOT/target/vespa-verifier/m11/core"
rm -rf "$TARGET"
mkdir -p "$TARGET"
run_m11_driver failures "$TARGET/failures"
run_m11_driver lifecycle "$TARGET/lifecycle"
run_m11_driver reopen "$TARGET/reopen"
run_m11_driver advanced "$TARGET/advanced"
run_m11_driver adapter "$TARGET/adapter"
run_m11_driver malformed "$TARGET/malformed"

echo "M11 core verification passed"
