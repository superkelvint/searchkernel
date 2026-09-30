#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../../.." && pwd)"
DRIVER="$ROOT/core/crates/searchkernel/examples/searchkernel-recovery-driver.rs"

if rg -n --glob '*.rs' --glob '*.toml' \
    'searchkernel-testkit|searchkernel_testkit|searchkernel-vespa-engine|searchkernel_vespa_engine|VespaBackend|HashMap|HashSet' \
    "$DRIVER"; then
  echo "recovery driver uses a forbidden alternate backend or nondeterministic model" >&2
  exit 1
fi

if ! rg -n 'std::process::abort\(\)' "$DRIVER" >/dev/null; then
  echo "recovery driver has no real process abort" >&2
  exit 1
fi

if ! rg -n 'IndexRuntime::(create|open)' "$DRIVER" >/dev/null; then
  echo "recovery driver does not exercise the product lifecycle" >&2
  exit 1
fi
