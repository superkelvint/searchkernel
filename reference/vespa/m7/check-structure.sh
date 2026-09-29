#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../../.." && pwd)"; cd "$ROOT"
bad=0

roots=()
for d in core/crates/searchkernel-vespa-engine core/crates/searchkernel-vespa-bindings core/native/vespa; do
  [[ -e "$d" ]] && roots+=("$d")
done

runtime_roots=()
for d in core/crates/searchkernel-vespa-engine/src core/crates/searchkernel-vespa-bindings/src core/native/vespa; do
  [[ -e "$d" ]] && runtime_roots+=("$d")
done

check_absent() {
  local p="$1"
  if ((${#roots[@]})) && grep -R -n -E "$p" "${roots[@]}" --include='*.rs' --include='*.cc' --include='*.cpp' --include='*.h' --include='*.hpp' 2>/dev/null; then
    echo "forbidden M7 production pattern: $p" >&2
    bad=1
  fi
}

check_runtime_absent() {
  local p="$1"
  if ((${#runtime_roots[@]})) && grep -R -n -E "$p" "${runtime_roots[@]}" --include='*.rs' --include='*.cc' --include='*.cpp' --include='*.h' --include='*.hpp' 2>/dev/null; then
    echo "forbidden M7 runtime production pattern: $p" >&2
    bad=1
  fi
}

check_absent 'Command::new\("java"\)|Command::new\("mvn"\)|JNI_CreateJavaVM|libjvm'
check_absent 'reference/vespa/m7/(expected|scenarios|oracle)'
for manifest in core/crates/searchkernel-vespa-engine/Cargo.toml core/crates/searchkernel-vespa-bindings/Cargo.toml; do
  if [[ -f "$manifest" ]] && grep -n 'searchkernel-ir' "$manifest"; then
    echo 'M-series dependency firewall violated' >&2
    bad=1
  fi
done
# Build scripts may invoke repository tooling (for example native SDK resolution).
# Runtime production code must not spawn subprocesses.
check_runtime_absent 'std::process::Command|popen\(|system\('
exit "$bad"
