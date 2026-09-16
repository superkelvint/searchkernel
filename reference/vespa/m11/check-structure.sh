#!/usr/bin/env bash
set -euo pipefail

M11="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$M11/../../.." && pwd)"
cd "$ROOT"

bad=0
fail() { echo "M11 structural check: $*" >&2; bad=1; }

if rg -n --glob '*.rs' --glob '*.toml' \
    'searchkernel-ir|searchkernel_ir|searchkernel-testkit|searchkernel_testkit|searchkernel-ffi|searchkernel_ffi' \
    "$ROOT/core/crates/searchkernel-vespa-engine/src" "$ROOT/core/crates/searchkernel-vespa-bindings/src" \
    "$ROOT/core/crates/searchkernel-vespa-engine/Cargo.toml" "$ROOT/core/crates/searchkernel-vespa-bindings/Cargo.toml"; then
  fail "forbidden IR/testkit/FFI dependency escaped the native engine boundary"
fi

if rg -n --glob '*.rs' --glob '*.toml' \
    'searchkernel-testkit|searchkernel_testkit' "$M11/rust-driver"; then
  fail "M11 driver uses a testkit or alternate backend"
fi

if rg -n --glob '*.rs' --glob '*.toml' \
    'Command::new|std::process::Command|ProcessBuilder|JNI_CreateJavaVM|libjvm|serde_json' \
    "$M11/rust-driver" "$ROOT/core/crates/searchkernel-vespa-engine" "$ROOT/core/crates/searchkernel-vespa-bindings"; then
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

if rg -n '_operation_mutex|std::mutex[[:space:]]+_.*engine' "$ROOT/core/native/vespa/searchkernel_vespa.cpp"; then
  fail "process-wide or whole-engine operation mutex remains"
fi

if ! rg -n 'std::shared_mutex|std::shared_lock|std::unique_lock' \
    "$ROOT/core/native/vespa/searchkernel_vespa.cpp" >/dev/null; then
  fail "native lifecycle gate is missing"
fi

for impl in 'unsafe impl Send for Engine' 'unsafe impl Sync for Engine'; do
  line="$(rg -n "$impl" "$ROOT/core/crates/searchkernel-vespa-engine/src/lib.rs" | cut -d: -f1)"
  [[ -n "$line" ]] || { fail "missing $impl"; continue; }
  previous=$((line - 4))
  sed -n "${previous},${line}p" "$ROOT/core/crates/searchkernel-vespa-engine/src/lib.rs" | grep -q 'SAFETY:' ||
    fail "$impl has no adjacent SAFETY justification"
done

if rg -n 'wildcard|\*|all' "$M11/sanitizer/lsan.supp" "$M11/sanitizer/tsan.supp" | \
    grep -v '^.*No broad\|^.*No blanket' >/dev/null; then
  fail "sanitizer suppression is broader than a documented symbol/stack"
fi

if [[ -f "$ROOT/core/native/sdk/linux-x86_64/lib/libsearchkernel_vespa.so" ]] &&
   nm -D "$ROOT/core/native/sdk/linux-x86_64/lib/libsearchkernel_vespa.so" 2>/dev/null |
     grep -q 'sk_test_failpoint'; then
  fail "production SDK exports hardening failpoints"
fi

exit "$bad"
