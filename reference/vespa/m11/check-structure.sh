#!/usr/bin/env bash
set -euo pipefail

M11="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$M11/../../.." && pwd)"
cd "$ROOT"

bad=0
fail() { echo "M11 structural check: $*" >&2; bad=1; }

if rg -n --glob '*.rs' --glob '*.toml' \
    'searchkernel-ir|searchkernel_ir|searchkernel-testkit|searchkernel_testkit' \
    "$ROOT/core/crates/searchkernel-vespa-engine/src" "$ROOT/core/crates/searchkernel-vespa-bindings/src" \
    "$ROOT/core/crates/searchkernel-vespa-engine/Cargo.toml" "$ROOT/core/crates/searchkernel-vespa-bindings/Cargo.toml"; then
  fail "forbidden IR/testkit/FFI dependency escaped the native engine boundary"
fi

if rg -n --glob '*.rs' --glob '*.toml' \
    'searchkernel-testkit|searchkernel_testkit' "$ROOT/core/crates/searchkernel/examples/searchkernel-m11-driver.rs"; then
  fail "M11 driver uses a testkit or alternate backend"
fi

if rg -n --glob '*.rs' \
    'Command::new|std::process::Command|ProcessBuilder|JNI_CreateJavaVM|libjvm' \
    "$ROOT/core/crates/searchkernel/examples/searchkernel-m11-driver.rs" "$ROOT/core/crates/searchkernel-vespa-engine/src" "$ROOT/core/crates/searchkernel-vespa-bindings/src"; then
  fail "test driver or production wrapper has an alternate runtime/oracle path"
fi

# The driver must derive expectations from native behavior, not a JSON oracle.
# Raw bindings must remain free of a second data path. The engine may decode
# JSON field values before passing them to Vespa.
if rg -n --glob '*.rs' --glob '*.toml' 'serde_json' \
    "$ROOT/core/crates/searchkernel/examples/searchkernel-m11-driver.rs" "$ROOT/core/crates/searchkernel-vespa-bindings/src"; then
  fail "test driver or production wrapper has an alternate runtime/oracle path"
fi

if rg -n --glob '*.rs' '#include[[:space:]]*[<\"]vespa/|extern[[:space:]]+"C"' \
    "$ROOT/core/crates/searchkernel-vespa-engine/src"; then
  fail "Vespa headers or raw extern declarations escaped the sys boundary"
fi

if rg -n 'reference/vespa/m11|expected/|scenarios/' \
    "$ROOT/core/crates/searchkernel-vespa-engine/src" "$ROOT/core/crates/searchkernel-vespa-bindings/src" \
    "$ROOT/core/native/vespa"; then
  fail "production implementation refers to M11 verifier artifacts"
fi

if rg -n '_operation_mutex|std::mutex[[:space:]]+_.*engine' \
    --glob '*.cc' --glob '*.h' "$ROOT/core/native/vespa"; then
  fail "process-wide or whole-engine operation mutex remains"
fi

if ! rg -n 'std::shared_mutex|std::shared_lock|std::unique_lock' \
    --glob '*.cc' --glob '*.h' "$ROOT/core/native/vespa" >/dev/null; then
  fail "native lifecycle gate is missing"
fi

engine_source="$ROOT/core/crates/searchkernel-vespa-engine/src/engine.rs"
for impl in 'unsafe impl Send for Engine' 'unsafe impl Sync for Engine'; do
  matches="$(rg -n -F "$impl" "$engine_source" || true)"
  match_count="$(printf '%s\n' "$matches" | sed '/^$/d' | wc -l | tr -d '[:space:]')"
  if [[ "$match_count" != "1" ]]; then
    if [[ "$match_count" == "0" ]]; then
      fail "missing $impl"
    else
      fail "$impl must appear exactly once"
    fi
    continue
  fi

  line="${matches%%:*}"
  cursor=$((line - 1))
  found_safety=0
  while ((cursor >= 1)); do
    context="$(sed -n "${cursor}p" "$engine_source")"
    if [[ "$context" =~ ^[[:space:]]*// ]]; then
      if [[ "$context" == *SAFETY:* ]]; then
        found_safety=1
      fi
      cursor=$((cursor - 1))
      continue
    fi
    break
  done

  ((found_safety == 1)) ||
    fail "$impl has no adjacent SAFETY justification"
done

if rg -n '^[[:space:]]*[^#[:space:]].*(wildcard|\*|(^|:)all($|:))' \
    "$M11/sanitizer/lsan.supp" "$M11/sanitizer/tsan.supp" >/dev/null; then
  fail "sanitizer suppression is broader than a documented symbol/stack"
fi

if [[ -f "$ROOT/core/native/sdk/linux-x86_64/lib/libsearchkernel_vespa.so" ]] &&
   nm -D "$ROOT/core/native/sdk/linux-x86_64/lib/libsearchkernel_vespa.so" 2>/dev/null |
     grep -q 'sk_test_failpoint'; then
  fail "production SDK exports hardening failpoints"
fi

exit "$bad"
