#!/usr/bin/env bash
set -euo pipefail
M12="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$M12/../../.." && pwd)"
cd "$ROOT"
bad=0
fail(){ echo "M12 structural check: $*" >&2; bad=1; }

# Native M-layer remains independent from portable IR/testkit.
if rg -n --glob '*.rs' --glob '*.toml' \
    'searchkernel-ir|searchkernel_ir|searchkernel-testkit|searchkernel_testkit' \
    "$ROOT/core/crates/searchkernel-vespa-engine/src" \
    "$ROOT/core/crates/searchkernel-vespa-bindings/src" \
    "$ROOT/core/crates/searchkernel-vespa-engine/Cargo.toml" \
    "$ROOT/core/crates/searchkernel-vespa-bindings/Cargo.toml"; then
  fail "portable IR/testkit dependency escaped the M-layer boundary"
fi

# Acceptance drivers may not shell out to an oracle or use fake/test backends.
if rg -n --glob '*.rs' --glob '*.toml' \
    'searchkernel-testkit|searchkernel_testkit|RecordingBackend|ScriptedBackend|Command::new|std::process::Command' \
    "$ROOT/core/crates/searchkernel/examples/searchkernel-m12-driver.rs"; then
  fail "M12 driver uses an alternate backend/oracle path"
fi

# Production must never know fixture IDs or verifier paths.
if rg -n 'reference/vespa/m12|id:ns:typea::(x|mid|y)|exact-near-far|multiple-query-tensors' \
    "$ROOT/core/crates/searchkernel-vespa-engine/src" \
    "$ROOT/core/crates/searchkernel-vespa-bindings/src" \
    "$ROOT/core/crates/searchkernel-ir-adapter/src" \
    "$ROOT/core/native/vespa"; then
  fail "production code refers to M12 verifier artifacts/fixture literals"
fi

# No query-time vector scan/distance implementation should appear in Rust production layers.
if rg -n --glob '*.rs' \
    'euclidean_distance|cosine_distance|cosine_similarity|dot_product_distance|nearest_neighbor_scan|vector_distance|\.iter\(\).*zip\(.*embedding' \
    "$ROOT/core/crates/searchkernel-vespa-engine/src" "$ROOT/core/crates/searchkernel-ir-adapter/src"; then
  fail "Rust production code appears to implement vector distance/NN scanning"
fi

# The adapter should route dense tensors explicitly and preserve scalar query-feature support.
if ! rg -n 'query_feature\(' "$ROOT/core/crates/searchkernel-ir-adapter/src/search" --glob '*.rs' >/dev/null; then
  fail "adapter scalar query-feature lowering is missing"
fi
if ! rg -n 'query_tensor\(' "$ROOT/core/crates/searchkernel-ir-adapter/src/search" --glob '*.rs' >/dev/null; then
  fail "adapter dense query-tensor lowering is missing"
fi
if ! rg -n 'Query::NearestNeighbor|Query::NearestNeighborWithOptions' "$ROOT/core/crates/searchkernel-ir-adapter/src/search" --glob '*.rs' >/dev/null; then
  fail "adapter nearest-neighbor lowering is missing"
fi

# Tensor grammar belongs to native validation, not the IR adapter.
# Match parsing/shape-validation behavior, not ordinary tensor values or test
# fixtures. In particular, a loose `parse.*type_spec` pattern also matches
# `Sparse { type_spec, ... }`, which is just value transport.
if rg -n --glob '*.rs' --glob '!**/tests/**' --glob '!**/tests.rs' \
    '\bparse[A-Za-z0-9_]*\([^;\n]*type_spec|\bsplit[A-Za-z0-9_]*\([^;\n]*type_spec|type_spec[^;\n]*\.(split|split_once|splitn|rsplit|rsplit_once|rsplitn|chars|bytes|strip_prefix|strip_suffix|find)\b|mapped dimension|indexed dimension' \
    "$ROOT/core/crates/searchkernel-ir-adapter/src"; then
  fail "adapter appears to duplicate native tensor-type parsing/shape validation"
fi

# Ensure typed query is no longer rejected wholesale once M12 is implemented.
if rg -n 'QueryIr::Typed\(_\).*typed_query_not_supported' "$ROOT/core/crates/searchkernel-ir-adapter/src/search" --glob '*.rs' >/dev/null; then
  fail "adapter still rejects every typed query wholesale"
fi

exit "$bad"
