#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
fixture=$(mktemp -d)
trap 'rm -rf "$fixture"' EXIT
mkdir -p "$fixture/tools" "$fixture/core/native/vespa" \
  "$fixture/core/crates/searchkernel-vespa-bindings/src" "$fixture/sdk/include"
cp -a "$root/tools/check-native-bindings" "$fixture/tools/"
cp "$root/tools/check-native-bindings.sh" "$fixture/tools/"
cp "$root/core/native/vespa/searchkernel_vespa.h" "$fixture/core/native/vespa/"
cp "$root/core/native/vespa/searchkernel_vespa.h" "$fixture/sdk/include/override.h"
cp "$root/core/crates/searchkernel-vespa-bindings/src/bindings.rs" \
  "$fixture/core/crates/searchkernel-vespa-bindings/src/"

python3 - "$fixture/sdk/include/override.h" <<'PYFIXTURE'
from pathlib import Path
import sys
p = Path(sys.argv[1])
s = p.read_text()
needle = "uint32_t sk_native_abi_version(void) SK_NOEXCEPT;"
assert needle in s
p.write_text(s.replace(needle, "uint64_t sk_native_abi_version(void) SK_NOEXCEPT;", 1))
PYFIXTURE
cargo run --locked --quiet --manifest-path "$fixture/tools/check-native-bindings/Cargo.toml" -- \
  "$fixture/sdk/include/override.h" \
  "$fixture/core/crates/searchkernel-vespa-bindings/src/bindings.rs"

if SK_NATIVE_ABI_HEADER="$fixture/sdk/include/override.h" \
    bash "$fixture/tools/check-native-bindings.sh" >/dev/null 2>&1; then
  echo "bindings freshness accepted bindings generated from a stale/non-canonical header" >&2
  exit 1
fi

echo "native bindings canonical-source regression passed"
