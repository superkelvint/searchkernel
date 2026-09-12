#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$ROOT"

bad=0
existing_roots=()
for dir in crates/searchkernel-vespa crates/searchkernel-vespa-sys native/vespa; do
  [[ -e "$dir" ]] && existing_roots+=("$dir")
done

check_absent() {
  local pattern="$1"
  if ((${#existing_roots[@]})) && grep -R -n -E "$pattern" "${existing_roots[@]}" \
      --include='*.rs' --include='*.cc' --include='*.cpp' --include='*.cxx' --include='*.h' --include='*.hpp' 2>/dev/null; then
    echo "forbidden M6 production pattern: $pattern" >&2
    bad=1
  fi
}

check_absent 'Command::new\("java"\)|Command::new\("mvn"\)|JNI_CreateJavaVM|libjvm'
check_absent 'reference/vespa/m6/(expected|scenarios|oracle)|reference/vespa/m4/fixture'

for manifest in crates/searchkernel-vespa/Cargo.toml crates/searchkernel-vespa-sys/Cargo.toml; do
  if [[ -f "$manifest" ]] && grep -n 'searchkernel-core' "$manifest"; then
    echo 'M-series dependency firewall violated: searchkernel-core dependency found' >&2
    bad=1
  fi
done

# M6 removes the historical native document-type fixture identity. Production
# native adapter sources may no longer hard-code "typea". Tests/reference files
# are intentionally outside native/vespa production sources checked here.
if [[ -d native/vespa ]]; then
  while IFS= read -r -d '' file; do
    case "$file" in
      */test/*|*/tests/*|*_test.cc|*_test.cpp|*_test.cxx) continue ;;
    esac
    if grep -n -E '"typea"|document:typea|:typea:' "$file"; then
      echo "M6 forbids hard-coded historical document type in native production source: $file" >&2
      bad=1
    fi
  done < <(find native/vespa -type f \( -name '*.cc' -o -name '*.cpp' -o -name '*.cxx' -o -name '*.h' -o -name '*.hpp' \) -print0)
fi

# The compiler must produce the frozen private manifest, not merely validate and
# discard document_type.
if [[ -d crates/searchkernel-vespa/src ]] && ! grep -R -q 'searchkernel-schema\.json' crates/searchkernel-vespa/src --include='*.rs'; then
  echo 'M6 compiler does not reference required searchkernel-schema.json manifest' >&2
  bad=1
fi

exit "$bad"
