#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../../.." && pwd)"; cd "$ROOT"
bad=0
roots=(core/crates/searchkernel-vespa-engine core/crates/searchkernel-vespa-bindings core/native/vespa)
for d in "${roots[@]}"; do
  if [[ ! -d "$d" ]]; then
    echo "M9 production source root is missing: $d" >&2
    bad=1
  fi
done
check_absent(){ local p="$1"; if grep -R -n -E "$p" "${roots[@]}" --include='*.rs' --include='*.cc' --include='*.cpp' --include='*.h' --include='*.hpp' 2>/dev/null; then echo "forbidden M9 production pattern: $p" >&2; bad=1; fi; }
check_present_native(){ local p="$1"; if ! grep -R -q -E "$p" core/native/vespa --include='*.cc' --include='*.cpp' --include='*.h' --include='*.hpp' 2>/dev/null; then echo "M9 native adapter is missing expected local Vespa path marker: $p" >&2; bad=1; fi; }
check_absent 'Command::new\("java"\)|Command::new\("mvn"\)|JNI_CreateJavaVM|libjvm'
check_absent 'reference/vespa/m9/(expected|scenarios|oracle)'
check_absent 'std::process::Command|popen\(|system\('
for manifest in core/crates/searchkernel-vespa-engine/Cargo.toml core/crates/searchkernel-vespa-bindings/Cargo.toml; do
  if [[ ! -f "$manifest" ]]; then
    echo "M9 production manifest is missing: $manifest" >&2
    bad=1
  elif grep -n 'searchkernel-ir' "$manifest"; then
    echo 'M-series dependency firewall violated' >&2
    bad=1
  fi
done
check_present_native 'groupSpec'
check_present_native 'maxhits[^[:alnum:]_]*=[^0-9-]*0|\.maxhits[^[:alnum:]_]*=[^0-9-]*0'
check_present_native 'groupResult'
check_present_native 'attribute|Attribute'
# Do not attempt to prove absence of Rust aggregation with a fragile grep alone;
# every grouping scenario requests zero regular hits and is the normative trap.
exit "$bad"
