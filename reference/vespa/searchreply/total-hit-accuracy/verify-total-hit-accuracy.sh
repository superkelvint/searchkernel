#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/../../../.." && pwd)"
cd "$ROOT"
source "$ROOT/reference/vespa/common/verify-common.sh"
require_pinned_vespa

TRACE="$SCRIPT_DIR/SOURCE_TRACE.md"
grep -q 'totalHitCount' "$TRACE"
grep -q 'MATCH_PHASE.*0x01' "$ROOT/reference/vespa/searchreply/SOURCE_TRACE.md"
grep -q 'weak_and_search.cpp' "$TRACE"
grep -q 'nearest_neighbor_blueprint.cpp' "$TRACE"
grep -q 'match_phase_limiter.h' "$TRACE"
grep -q 'Unknown' "$TRACE"

grep -q 'r.totalHitCount = result.totalHits()' \
  "$ROOT/upstream/vespa/searchcore/src/vespa/searchcore/proton/matching/result_processor.cpp"
grep -q 'degradeMatchPhase' \
  "$ROOT/upstream/vespa/searchcore/src/vespa/searchcore/proton/matching/matcher.cpp"
grep -q 'approximate_nns_timed_out_queries' \
  "$ROOT/upstream/vespa/searchcore/src/vespa/searchcore/proton/matching/matcher.cpp"
grep -q 'TotalHitsRelation::Unknown' \
  "$ROOT/core/crates/searchkernel-vespa-engine/src/native.rs"
grep -q 'VespaTotalHitsRelation::Unknown' \
  "$ROOT/core/crates/searchkernel-ir-adapter/src/backend.rs"

cargo test -p searchkernel-vespa-engine --test native_crud \
  native_nearest_neighbor_uses_a_tensor_query_input_and_rank_profile -- --exact

echo 'total-hit accuracy source-trace verification passed'
