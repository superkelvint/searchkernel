#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../../.." && pwd)"; cd "$ROOT"
bad=0
roots=(); for d in crates/searchkernel-vespa crates/searchkernel-vespa-sys native/vespa; do [[ -e "$d" ]] && roots+=("$d"); done
check_absent(){ local p="$1"; if ((${#roots[@]})) && grep -R -n -E "$p" "${roots[@]}" --include='*.rs' --include='*.cc' --include='*.cpp' --include='*.h' --include='*.hpp' 2>/dev/null; then echo "forbidden M7 production pattern: $p" >&2; bad=1; fi; }
check_absent 'Command::new\("java"\)|Command::new\("mvn"\)|JNI_CreateJavaVM|libjvm'
check_absent 'reference/vespa/m7/(expected|scenarios|oracle)'
for manifest in crates/searchkernel-vespa/Cargo.toml crates/searchkernel-vespa-sys/Cargo.toml; do if [[ -f "$manifest" ]] && grep -n 'searchkernel-core' "$manifest"; then echo 'M-series dependency firewall violated' >&2; bad=1; fi; done
check_absent 'std::process::Command|popen\(|system\('
exit "$bad"
