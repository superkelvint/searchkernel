#!/usr/bin/env bash
set -euo pipefail

M11="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$M11/../../.." && pwd)"
source "$M11/verify-m11-common.sh"

require_pinned_vespa
python3 "$M11/validate.py"
"$M11/check-structure.sh"
prepare_hardening_sdk
build_m11_driver

cargo check --quiet --manifest-path "$ROOT/reference/vespa/api-contract/Cargo.toml" --bin m11
LD_LIBRARY_PATH="$M11_SDK/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
  cargo test --quiet -p searchkernel-vespa-engine --all-targets --all-features
LD_LIBRARY_PATH="$M11_SDK/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
  cargo test --quiet -p searchkernel-ir-adapter --features hardening \
    malformed_native_result_maps_to_portable_internal_with_detail

TARGET="$ROOT/target/vespa-verifier/m11/core"
rm -rf "$TARGET"
mkdir -p "$TARGET"
run_m11_driver lifecycle "$TARGET/lifecycle"
run_m11_driver reopen "$TARGET/reopen"
run_m11_driver advanced "$TARGET/advanced"
run_m11_driver adapter "$TARGET/adapter"
run_m11_driver malformed "$TARGET/malformed"
run_m11_driver failures "$TARGET/failures"

echo "M11 core verification passed"
