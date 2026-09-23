#!/usr/bin/env bash
set -euo pipefail

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)

list_targets() {
    cat <<'EOF'
Semantic verifier targets:
  vespa.native-sdk
  vespa.rust-ownership
  vespa.document-mutation
  vespa.linguistics
  vespa.text-indexing
  vespa.persistence.reopen
  vespa.schema-config
  vespa.query-compilation
  vespa.results
  vespa.grouping
  vespa.grouping.recursive
  vespa.grouping.yql
  vespa.ranking
  vespa.ranking.feature-values
  vespa.search-reply
  vespa.hardening
  vespa.hardening.core
  vespa.hardening.stress
  vespa.hardening.sanitizers
  vespa.nearest-neighbor.query
  vespa.tensor.document-crud
  vespa.nearest-neighbor.hnsw
  vespa.recovery
  product.rust-real-engine
  product.integration

Milestone aggregate targets:
  milestone.m0
  milestone.m1
  milestone.m2
  milestone.m3
  milestone.m4
  milestone.m5
  milestone.m6
  milestone.m7
  milestone.m8
  milestone.m9
  milestone.m10
  milestone.m11
  milestone.m12
  milestone.m13
  milestone.m14

Legacy aliases retained during migration:
  m0 ... m14
  searchreply
  phase10
  integration
  i
EOF
}

case "${1:-}" in
    --list|list)
        list_targets
        ;;
    vespa.native-sdk)
        "$ROOT/verifiers/native-sdk/verify.sh"
        ;;
    vespa.rust-ownership)
        "$ROOT/verifiers/rust-ownership/verify.sh"
        ;;
    vespa.document-mutation)
        "$ROOT/verifiers/document-mutation/verify.sh"
        ;;
    vespa.linguistics)
        "$ROOT/verifiers/linguistics/verify.sh"
        ;;
    vespa.text-indexing)
        "$ROOT/verifiers/text-indexing/verify.sh"
        ;;
    vespa.persistence.reopen)
        "$ROOT/verifiers/persistence/reopen/verify.sh"
        ;;
    vespa.schema-config)
        "$ROOT/verifiers/schema-config/verify.sh"
        ;;
    vespa.query-compilation)
        "$ROOT/verifiers/query-compilation/verify.sh"
        ;;
    vespa.results)
        "$ROOT/verifiers/results/verify.sh"
        ;;
    vespa.grouping)
        "$ROOT/verifiers/grouping/attributes-facets/verify.sh"
        ;;
    vespa.grouping.recursive)
        "$ROOT/verifiers/grouping/recursive/verify.sh"
        ;;
    vespa.grouping.yql)
        "$ROOT/verifiers/grouping/yql/verify.sh"
        ;;
    vespa.ranking)
        "$ROOT/verifiers/ranking/verify.sh"
        ;;
    vespa.ranking.feature-values)
        "$ROOT/verifiers/ranking/feature-values/verify.sh"
        ;;
    vespa.search-reply)
        "$ROOT/verifiers/search-reply/verify.sh"
        ;;
    vespa.hardening)
        "$ROOT/verifiers/hardening/full/verify.sh"
        ;;
    vespa.hardening.core)
        "$ROOT/verifiers/hardening/core/verify.sh"
        ;;
    vespa.hardening.stress)
        "$ROOT/verifiers/hardening/stress/verify.sh"
        ;;
    vespa.hardening.sanitizers)
        "$ROOT/verifiers/hardening/sanitizers/verify.sh"
        ;;
    vespa.nearest-neighbor.query)
        "$ROOT/verifiers/nearest-neighbor/query/verify.sh"
        ;;
    vespa.tensor.document-crud)
        "$ROOT/verifiers/tensor/document-crud/verify.sh"
        ;;
    vespa.nearest-neighbor.hnsw)
        "$ROOT/verifiers/nearest-neighbor/hnsw/verify.sh"
        ;;
    vespa.recovery)
        "$ROOT/verifiers/recovery/verify.sh"
        ;;
    product.rust-real-engine)
        "$ROOT/verifiers/product/rust-real-engine/verify.sh"
        ;;
    product.integration)
        "$ROOT/verifiers/product/integration/verify.sh"
        ;;
    milestone.m0|milestone.m1|milestone.m2|milestone.m3|milestone.m4|milestone.m5|milestone.m6|milestone.m7|milestone.m8|milestone.m9|milestone.m10|milestone.m11|milestone.m12|milestone.m13|milestone.m14)
        milestone="${1#milestone.}"
        "$ROOT/$milestone/verify-$milestone.sh"
        ;;
    m0|m1|m2|m3|m4|m5|m6|m7|m8|m9|m10|m11|m12|m13|m14)
        "$ROOT/$1/verify-$1.sh"
        ;;
    searchreply)
        "$ROOT/verifiers/search-reply/verify.sh"
        ;;
    phase10)
        "$ROOT/verifiers/product/rust-real-engine/verify.sh"
        ;;
    integration|i)
        "$ROOT/verifiers/product/integration/verify.sh"
        ;;
    *)
        echo "unknown verifier target: ${1:-<empty>}" >&2
        echo "run '$0 --list' for canonical semantic targets" >&2
        exit 2
        ;;
esac
