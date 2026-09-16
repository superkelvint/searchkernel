#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../../.." && pwd)"; cd "$ROOT"
bad=0
roots=(core/crates/searchkernel-vespa-engine core/crates/searchkernel-vespa-bindings core/native/vespa)
for d in "${roots[@]}"; do
  if [[ ! -d "$d" ]]; then echo "M10 production source root is missing: $d" >&2; bad=1; fi
done
check_absent(){ local p="$1"; if grep -R -n -E "$p" "${roots[@]}" --include='*.rs' --include='*.cc' --include='*.cpp' --include='*.h' --include='*.hpp' --include='Cargo.toml' 2>/dev/null; then echo "forbidden M10 production pattern: $p" >&2; bad=1; fi; }
check_present(){ local where="$1" p="$2" label="$3"; if ! grep -R -q -E "$p" "$where" --include='*.rs' --include='*.cc' --include='*.cpp' --include='*.h' --include='*.hpp' 2>/dev/null; then echo "M10 implementation is missing $label ($p)" >&2; bad=1; fi; }

# The low-level M track stays native-only and independent from C-series semantics.
check_absent 'Command::new\("java"\)|Command::new\("mvn"\)|JNI_CreateJavaVM|libjvm'
check_absent 'reference/vespa/m10/(expected|scenarios|oracle)'
check_absent 'std::process::Command|popen\(|system\('
check_absent '(^|[^[:alnum:]_])(rankscript|RankScript|meval|evalexpr|rhai)([^[:alnum:]_]|$)'
for manifest in core/crates/searchkernel-vespa-engine/Cargo.toml core/crates/searchkernel-vespa-bindings/Cargo.toml; do
  if [[ ! -f "$manifest" ]]; then echo "M10 production manifest is missing: $manifest" >&2; bad=1
  elif grep -n 'searchkernel-ir' "$manifest"; then echo 'M-series dependency firewall violated' >&2; bad=1; fi
done

# Structural evidence that M10 is compiled into Proton config and transported
# through the native request/reply boundary, rather than handled in Rust only.
check_present core/crates/searchkernel-vespa-engine/src 'vespa\.rank\.firstphase' 'rank-profile first-phase config emission'
check_present core/crates/searchkernel-vespa-engine/src 'vespa\.match\.feature' 'match-feature config emission'
check_present core/native/vespa 'propertiesMap|MapNames::RANK' 'native rank-property plumbing'
check_present core/native/vespa 'match_features|FeatureValues' 'native match-feature result plumbing'
check_present core/native/vespa 'ranking' 'native rank-profile selection plumbing'

# The public generic M API must own the M10 concepts. A parallel M10-specific
# request/result family would recreate the milestone-shaped API we are removing.
check_present core/crates/searchkernel-vespa-engine/src 'struct[[:space:]]+RankProfile' 'RankProfile Rust API'
check_present core/crates/searchkernel-vespa-engine/src 'query_feature' 'SearchRequest query-feature API'
check_present core/crates/searchkernel-vespa-engine/src 'rank_property' 'SearchRequest rank-property API'
check_present core/crates/searchkernel-vespa-engine/src 'match_features' 'Hit match-feature API'
check_absent 'M10(Search|Rank|Ranking)(Request|Result)|LocalRank(Request|Result)|RankingRequest'

exit "$bad"
