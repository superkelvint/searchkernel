#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
NODE="$(bash "$ROOT/tools/resolve-node.sh" node)"
NPM="$(bash "$ROOT/tools/resolve-node.sh" npm)"
CARGO="$(bash "$ROOT/tools/resolve-rust.sh" cargo)"
export NODE NPM CARGO
export PATH="$(dirname -- "$NODE"):$PATH"
package="$ROOT/integrations/instantsearch"
cd "$ROOT"
"$NPM" ci --prefix "$package"
"$NPM" run build --prefix "$package"
"$NPM" run typecheck --prefix "$package"
"$NPM" run check:contract --prefix "$package"
"$NPM" test --prefix "$package"
(cd "$package" && "$NODE" scripts/check-package.mjs)
"$CARGO" test -p searchkerneld --all-features --test public_search_binding --test instantsearch_compat --test instantsearch_product -- --include-ignored
