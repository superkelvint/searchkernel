#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
cd "$ROOT"
source "$ROOT/reference/vespa/common/verify-common.sh"

require_pinned_vespa
RUN_LIVE_ORACLE=0
if live_oracle_enabled; then
  RUN_LIVE_ORACLE=1
else
  announce_frozen_oracle_replay
fi

"$SCRIPT_DIR/validate_scenarios.py"
"$SCRIPT_DIR/validate_expected.py"

for scenario in "$SCRIPT_DIR/scenarios"/[0-9][0-9]-*.json; do
  check_fixture_literals --keys text,query "$scenario" "$ROOT/core/crates/searchkernel-vespa-engine/src" \
    "$ROOT/core/crates/searchkernel-vespa-bindings/src" "$ROOT/core/native/vespa"
done

if [[ -x reference/vespa/m5/verify-m5.sh ]]; then
  reference/vespa/m5/verify-m5.sh
else
  echo "missing cumulative M5 verifier" >&2
  exit 1
fi

run_workspace_quality
cargo check --manifest-path reference/vespa/api-contract/Cargo.toml --bin m6

"$SCRIPT_DIR/check-structure.sh"

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

for scenario in "$SCRIPT_DIR"/scenarios/[0-9][0-9]-*.json; do
  id="$(python3 "$ROOT/tools/json-cli.py" get "$scenario" id)"
  schema="$(python3 "$ROOT/tools/json-cli.py" get "$scenario" schema)"
  cfg="$TMP/$id-config"
  data="$TMP/$id-data"
  mkdir -p "$cfg" "$data"

  cargo run --quiet --manifest-path "$SCRIPT_DIR/rust-driver/Cargo.toml" -- compile "$scenario" "$cfg"

  # Compiler must refuse stale/non-empty output rather than merging configs.
  if cargo run --quiet --manifest-path "$SCRIPT_DIR/rust-driver/Cargo.toml" -- compile "$scenario" "$cfg" >/dev/null 2>&1; then
    echo "M6 compiler incorrectly accepted non-empty output dir for $id" >&2
    exit 1
  fi

  actual="$TMP/$id-rust.json"
  "$SCRIPT_DIR/canonicalize_cfg.py" "$scenario" "$cfg" > "$actual"
  "$SCRIPT_DIR/compare.py" "$SCRIPT_DIR/expected/$id.json" "$actual"

  if [[ "$RUN_LIVE_ORACLE" == 1 ]]; then
    live="$TMP/$id-java.json"
    "$SCRIPT_DIR/oracle/run-java-oracle.sh" "$id" "$schema" "$SCRIPT_DIR/scenarios/rendered/$id.sd" \
      | python3 "$ROOT/tools/json-cli.py" canonicalize - > "$live"
    "$SCRIPT_DIR/compare.py" "$SCRIPT_DIR/expected/$id.json" "$live"
    "$SCRIPT_DIR/compare.py" "$live" "$actual"
  fi

  if find "$cfg" -type l -print -quit | grep -q .; then
    echo "generated config contains symlink for $id" >&2
    exit 1
  fi
  if grep -R -n -E 'reference/vespa|upstream/vespa|m4/fixture|m5/' "$cfg" 2>/dev/null; then
    echo "generated config contains forbidden fixture/source path for $id" >&2
    exit 1
  fi

  # This is a separate cargo-run process from compilation. Engine::open must
  # bootstrap from the generated directory alone.
  cargo run --quiet --manifest-path "$SCRIPT_DIR/rust-driver/Cargo.toml" -- smoke "$scenario" "$cfg" "$data"
done

# The two fixtures intentionally have identical fields but different document
# types. Their private document-model manifests must differ, while their
# Java-derived index/attribute/summary semantics remain identical.
TYPEA_CFG="$TMP/text-summary-config/searchkernel-schema.json"
PRODUCTS_CFG="$TMP/products-text-summary-config/searchkernel-schema.json"
if cmp -s "$TYPEA_CFG" "$PRODUCTS_CFG"; then
  echo "M6 discarded document type: identical-field typea/products manifests are identical" >&2
  exit 1
fi

python3 "reference/vespa/m6/verify-m6-python-1.py" "$TYPEA_CFG" "$PRODUCTS_CFG"

echo "M6 verification passed"
