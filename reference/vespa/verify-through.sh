#!/usr/bin/env bash
set -euo pipefail

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
cd "$ROOT/../.."

list_targets() {
    "$ROOT/../../tools/dev-targets.py" list verify
}

target="${1:-}"
if [[ "$target" != "--list" && "$target" != "list" ]]; then
    canonical=$("$ROOT/../../tools/dev-targets.py" resolve verify "$target") || exit $?
    if [[ "$canonical" != "$target" ]]; then
        shift
        set -- "$canonical" "$@"
        target=$canonical
    fi
fi
case "$target" in
    --list|list)
        ;;
    vespa.native-sdk|vespa.rust-ownership|vespa.document-mutation|vespa.linguistics|vespa.text-indexing|vespa.persistence.reopen|vespa.schema-config|vespa.query-compilation|vespa.results|vespa.grouping|vespa.grouping.recursive|vespa.grouping.yql|vespa.ranking|vespa.ranking.feature-values|vespa.search-reply|vespa.hardening|vespa.hardening.core|vespa.hardening.stress|vespa.hardening.sanitizers|vespa.nearest-neighbor.query|vespa.tensor.document-crud|vespa.nearest-neighbor.hnsw|vespa.recovery|product.rust-real-engine|product.integration|milestone.m0|milestone.m1|milestone.m2|milestone.m3|milestone.m4|milestone.m5|milestone.m6|milestone.m7|milestone.m8|milestone.m9|milestone.m10|milestone.m11|milestone.m12|milestone.m13|milestone.m14|m0|m1|m2|m3|m4|m5|m6|m7|m8|m9|m10|m11|m12|m13|m14|searchreply|phase10|integration|i)
        ;;
    *)
        ;;
esac

case "${1:-}" in
    --list|list)
        list_targets
        ;;
    vespa.native-sdk)
        "$ROOT/verifiers/native-sdk/verify.sh" "${@:2}"
        ;;
    vespa.rust-ownership)
        "$ROOT/verifiers/rust-ownership/verify.sh" "${@:2}"
        ;;
    vespa.document-mutation)
        "$ROOT/verifiers/document-mutation/verify.sh" "${@:2}"
        ;;
    vespa.linguistics)
        "$ROOT/verifiers/linguistics/verify.sh" "${@:2}"
        ;;
    vespa.text-indexing)
        "$ROOT/verifiers/text-indexing/verify.sh" "${@:2}"
        ;;
    vespa.persistence.reopen)
        "$ROOT/verifiers/persistence/reopen/verify.sh" "${@:2}"
        ;;
    vespa.schema-config)
        "$ROOT/verifiers/schema-config/verify.sh" "${@:2}"
        ;;
    vespa.query-compilation)
        "$ROOT/verifiers/query-compilation/verify.sh" "${@:2}"
        ;;
    vespa.results)
        "$ROOT/verifiers/results/verify.sh" "${@:2}"
        ;;
    vespa.grouping)
        "$ROOT/verifiers/grouping/attributes-facets/verify.sh" "${@:2}"
        ;;
    vespa.grouping.recursive)
        "$ROOT/verifiers/grouping/recursive/verify.sh" "${@:2}"
        ;;
    vespa.grouping.yql)
        "$ROOT/verifiers/grouping/yql/verify.sh" "${@:2}"
        ;;
    vespa.ranking)
        "$ROOT/verifiers/ranking/verify.sh" "${@:2}"
        ;;
    vespa.ranking.feature-values)
        "$ROOT/verifiers/ranking/feature-values/verify.sh" "${@:2}"
        ;;
    vespa.search-reply)
        "$ROOT/verifiers/search-reply/verify.sh" "${@:2}"
        ;;
    vespa.hardening)
        "$ROOT/verifiers/hardening/full/verify.sh" "${@:2}"
        ;;
    vespa.hardening.core)
        "$ROOT/verifiers/hardening/core/verify.sh" "${@:2}"
        ;;
    vespa.hardening.stress)
        "$ROOT/verifiers/hardening/stress/verify.sh" "${@:2}"
        ;;
    vespa.hardening.sanitizers)
        "$ROOT/verifiers/hardening/sanitizers/verify.sh" "${@:2}"
        ;;
    vespa.hardening.tsan)
        M11_SANITIZER_LANES=tsan "$ROOT/verifiers/hardening/sanitizers/verify.sh" "${@:2}"
        ;;
    vespa.nearest-neighbor.query)
        "$ROOT/verifiers/nearest-neighbor/query/verify.sh" "${@:2}"
        ;;
    vespa.tensor.document-crud)
        "$ROOT/verifiers/tensor/document-crud/verify.sh" "${@:2}"
        ;;
    vespa.nearest-neighbor.hnsw)
        "$ROOT/verifiers/nearest-neighbor/hnsw/verify.sh" "${@:2}"
        ;;
    vespa.recovery)
        "$ROOT/verifiers/recovery/verify.sh" "${@:2}"
        ;;
    product.rust-real-engine)
        "$ROOT/verifiers/product/rust-real-engine/verify.sh" "${@:2}"
        ;;
    product.integration)
        "$ROOT/verifiers/product/integration/verify.sh" "${@:2}"
        ;;
    milestone.m0|milestone.m1|milestone.m2|milestone.m3|milestone.m4|milestone.m5|milestone.m6|milestone.m7|milestone.m8|milestone.m9|milestone.m10|milestone.m11|milestone.m12|milestone.m13|milestone.m14)
        milestone="${1#milestone.}"
        "$ROOT/$milestone/verify-$milestone.sh" "${@:2}"
        ;;
    m0|m1|m2|m3|m4|m5|m6|m7|m8|m9|m10|m11|m12|m13|m14)
        "$ROOT/$1/verify-$1.sh" "${@:2}"
        ;;
    searchreply)
        "$ROOT/verifiers/search-reply/verify.sh" "${@:2}"
        ;;
    phase10)
        "$ROOT/verifiers/product/rust-real-engine/verify.sh" "${@:2}"
        ;;
    integration|i)
        "$ROOT/verifiers/product/integration/verify.sh" "${@:2}"
        ;;
    *)
        echo "unknown verifier target: ${1:-<empty>}" >&2
        echo "run '$0 --list' for canonical semantic targets" >&2
        exit 2
        ;;
esac
