#!/bin/sh
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
CORE="$ROOT/core/crates/searchkernel-ir/src"

# `ir_to_api.rs` intentionally mentions generated V2 Resolved* wire tables
# while encoding the owned IR.  Only owned legacy IR names belong in this
# check; rejecting generated wire names made the verifier stale after the V2
# resolved-request encoder was introduced.
if rg -n 'Resolved[A-Z][A-Za-z0-9_]*Ir|IndexSemantics|pub fn resolve_' "$CORE"; then
  echo "legacy IR names remain" >&2; exit 1
fi
if rg -n 'searchkernel_protocol|flatbuffers::' "$CORE/model.rs" "$CORE/backend.rs" "$CORE/error.rs"; then
  echo "wire types leaked into owned core modules" >&2; exit 1
fi
if rg -n 'searchkernel_protocol::.*(Insert|Upsert|Patch|Delete|Get|Search|Analyze)Result' "$CORE/backend.rs"; then
  echo "generated result type in backend trait" >&2; exit 1
fi
# Only generated wire discriminators are boundary concerns.  Owned IR enums
# intentionally use the same semantic nouns, and filter_expression.rs is the
# API-side builder for textual filter clauses.
UNION='api::(ValueKind|PatchOperationKind|FilterExprKind|RankingPolicyKind|QueryExprKind|AnalyzeTarget)'
if rg -n "$UNION" "$CORE" -g '*.rs' -g '!api_to_ir.rs' -g '!ir_to_api.rs' -g '!filter_expression.rs'; then
  echo "generated union discriminator interpreted past conversion boundary" >&2; exit 1
fi
echo "IR boundary PASS"
