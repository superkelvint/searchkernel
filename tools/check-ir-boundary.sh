#!/bin/sh
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
CORE="$ROOT/core/crates/searchkernel-ir/src"

if rg -n 'Resolved[A-Z]|IndexSemantics|pub fn resolve_' "$CORE"; then
  echo "legacy IR names remain" >&2; exit 1
fi
if rg -n 'searchkernel_protocol|flatbuffers::' "$CORE/model.rs" "$CORE/backend.rs" "$CORE/error.rs"; then
  echo "wire types leaked into owned core modules" >&2; exit 1
fi
if rg -n 'searchkernel_protocol::.*(Insert|Upsert|Patch|Delete|Get|Search|Analyze)Result' "$CORE/backend.rs"; then
  echo "generated result type in backend trait" >&2; exit 1
fi
UNION='(ValueKind|PatchOperationKind|FilterExprKind|RankingPolicyKind|QueryExprKind|AnalyzeTarget)'
if rg -n "$UNION" "$CORE" -g '*.rs' -g '!api_to_ir.rs' -g '!ir_to_api.rs'; then
  echo "generated union discriminator interpreted past conversion boundary" >&2; exit 1
fi
echo "IR boundary PASS"
