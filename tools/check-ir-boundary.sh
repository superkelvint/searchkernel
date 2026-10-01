#!/bin/sh
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
CORE="$ROOT/core/crates/searchkernel-ir/src"
MODEL="$CORE/model"

# `api_to_ir/resolve/mod.rs` is the resolver façade; responsibility-specific
# submodules convert validated public inputs into owned IR. These are not legacy
# IR names. Keep this check limited to names from the removed pre-refactor model.
if grep -rEn 'Resolved[A-Z][A-Za-z0-9_]*Ir|IndexSemantics' "$CORE"; then
  echo "legacy IR names remain" >&2; exit 1
fi

# Owned model, backend, and error types must not carry generated wire values or
# FlatBuffer handles.  The conversion modules are checked separately below.
if grep -rEn 'searchkernel_protocol|flatbuffers::|api::' \
    "$MODEL" "$CORE/backend.rs" "$CORE/error.rs"; then
  echo "wire types leaked into owned core modules" >&2; exit 1
fi
if grep -nE 'searchkernel_protocol::.*(Insert|Upsert|Patch|Delete|Get|Search|Analyze)Result' "$CORE/backend.rs"; then
  echo "generated result type in backend trait" >&2; exit 1
fi
# Generated protocol types may appear only at the two conversion boundaries.
# The crate-level `api` and hidden FlatBuffers re-exports are compatibility
# entry points, not semantic-layer dependencies. The wire facade is itself a
# conversion boundary, while the IR refactor split conversion code into two
# modules; these paths are deliberately excluded from the semantic-layer scan.
WIRE='searchkernel_protocol|flatbuffers::|api::'
if grep -rEn --include='*.rs' --exclude='*_tests.rs' --exclude='lib.rs' --exclude='wire.rs' \
    --exclude-dir=api_to_ir --exclude-dir=ir_to_api "$WIRE" "$CORE"; then
  echo "generated protocol type interpreted past conversion boundary" >&2; exit 1
fi

# Aggregate IR construction belongs to the API-to-IR resolvers. Test-only
# constructors are permitted for focused adapter tests; SearchFieldWeight is
# a validated scalar value and is intentionally constructible in production.
if awk '
  /^impl [A-Za-z0-9_]+/ { type = $2; test_cfg = 0 }
  /^[[:space:]]*#\[cfg\(any\(test, feature = "test"\)\)\]/ { test_cfg = 1 }
  /^[[:space:]]*pub fn new\(/ {
    if (type != "SearchFieldWeight" && !test_cfg) {
      print FILENAME ":" FNR ":" $0
      bad = 1
    }
    test_cfg = 0
  }
  END { exit bad }
' "$MODEL"/*.rs; then
  :
else
  echo "ungated public aggregate IR constructor" >&2; exit 1
fi

# Structured query execution has one production representation: QueryIr::Typed.
# The old execution-shaped variants were easy to construct beside the typed
# tree and therefore created a second semantic path into the adapter. Keep
# this guard structural so a future production ingress cannot silently restore
# that split; focused tests may still use the backend model directly.
if [ -f "$MODEL/query.rs" ] && sed -n '/^pub enum QueryIr {/,/^}/p' "$MODEL/query.rs" \
  | grep -nE '^[[:space:]]*(Term|And|Or|Not)([^[:alnum:]_]|$)'; then
  echo "legacy structured QueryIr variants remain" >&2
  exit 1
fi
if grep -rEn --include='*.rs' --exclude='tests.rs' --exclude='*_tests.rs' \
  --exclude-dir=tests 'QueryIr::(Term|And|Or|Not)([^[:alnum:]_]|$)' \
  "$ROOT/core/crates"; then
  echo "legacy structured QueryIr construction remains in production" >&2
  exit 1
fi
echo "IR boundary PASS"
