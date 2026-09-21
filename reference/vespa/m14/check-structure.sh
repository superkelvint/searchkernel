#!/usr/bin/env bash
set -euo pipefail
M14="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$M14/../../.." && pwd)"
bad=0
fail(){ echo "M14 structural check: $*" >&2; bad=1; }

# M-layer dependency firewall remains intact.
if rg -n --glob '*.rs' --glob '*.toml' \
  'searchkernel-ir|searchkernel_ir|searchkernel-testkit|searchkernel_testkit' \
  "$ROOT/core/crates/searchkernel-vespa-engine/src" \
  "$ROOT/core/crates/searchkernel-vespa-bindings/src" \
  "$ROOT/core/crates/searchkernel-vespa-engine/Cargo.toml" \
  "$ROOT/core/crates/searchkernel-vespa-bindings/Cargo.toml"; then
  fail 'portable IR/testkit dependency escaped the M-layer boundary'
fi

# Production must not know verifier fixtures.
if rg -n 'reference/vespa/m14|id:ns:typea::[a-h]|hnsw-query-controls|hnsw-mutation-reopen' \
  "$ROOT/core/crates/searchkernel-vespa-engine/src" \
  "$ROOT/core/crates/searchkernel-vespa-bindings/src" \
  "$ROOT/core/crates/searchkernel-ir-adapter/src" \
  "$ROOT/core/native/vespa"; then
  fail 'production code refers to M14 verifier artifacts/fixture literals'
fi

# Rust production layers must not implement vector distance or HNSW themselves.
if rg -n --glob '*.rs' \
  'euclidean_distance|cosine_distance|cosine_similarity|nearest_neighbor_scan|vector_distance|struct Hnsw(Graph|Node)|hnsw_(insert|search|remove)|graph_neighbors' \
  "$ROOT/core/crates/searchkernel-vespa-engine/src" "$ROOT/core/crates/searchkernel-ir-adapter/src"; then
  fail 'Rust production code appears to implement vector/HNSW search'
fi

SCHEMA="$ROOT/core/crates/searchkernel-vespa-engine/src/schema.rs"
for needle in \
  'DistanceMetric' \
  'HnswIndexConfig' \
  'distance_metric' \
  'hnsw' \
  'index.hnsw.enabled' \
  'index.hnsw.maxlinkspernode' \
  'index.hnsw.neighborstoexploreatinsert' \
  'distancemetric'; do
  rg -n "$needle" "$SCHEMA" >/dev/null || fail "native schema compiler is missing $needle"
done

# Tensor HNSW must not be implemented by emitting an ordinary inverted index.
if rg -n 'FieldType::Tensor.*indexfield|indexfield.*FieldType::Tensor' "$SCHEMA" >/dev/null; then
  fail 'tensor HNSW appears to be emitted as an inverted index field'
fi

ADAPTER="$ROOT/core/crates/searchkernel-ir-adapter/src/schema.rs"
# The old blanket HNSW/distance rejection must be gone.  These exact predicates
# were the pre-M14 shortcut that rejected the already-represented portable fields.
if rg -n 'value\.hnsw\.is_some\(\) \|\| !value\.options\.is_empty\(\)' "$ADAPTER" >/dev/null; then
  fail 'adapter still blanket-rejects HNSW configuration'
fi
if rg -n 'value\.distance_metric != searchkernel_ir::DistanceMetricIr::Unspecified' "$ADAPTER" >/dev/null; then
  fail 'adapter still blanket-rejects distance metrics'
fi
for metric in Euclidean Angular DotProduct PrenormalizedAngular GeoDegrees Hamming; do
  rg -n "DistanceMetricIr::$metric" "$ADAPTER" >/dev/null || fail "adapter metric mapping is missing $metric"
done

# The already-existing query controls must remain wired into the native query builder.
rg -n 'u8::from\(options\.approximate\)|options\.approximate' \
  "$ROOT/core/crates/searchkernel-vespa-engine/src/query/native.rs" >/dev/null || fail 'approximate flag is not forwarded by low-level query lowering'
rg -n 'allow_approximate.*add_nearest_neighbor_term|add_nearest_neighbor_term\(' \
  "$ROOT/core/native/vespa/abi.cc" >/dev/null || fail 'native adapter no longer forwards nearest-neighbor approximate mode'
rg -n 'explore_additional_hits' "$ROOT/core/native/vespa/abi.cc" >/dev/null || fail 'native ANN exploration control missing'
rg -n 'distance_threshold' "$ROOT/core/native/vespa/abi.cc" >/dev/null || fail 'native ANN distance threshold control missing'

# Acceptance drivers may not invoke the oracle or fake backend.
if rg -n --glob '*.rs' --glob '*.toml' \
  'RecordingBackend|ScriptedBackend|searchkernel-testkit|searchkernel_testkit|Command::new|std::process::Command' \
  "$M14/rust-driver"; then
  fail 'M14 Rust driver uses an alternate backend/oracle path'
fi

exit "$bad"
