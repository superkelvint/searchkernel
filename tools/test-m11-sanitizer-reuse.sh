#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
test_root=$(mktemp -d "${TMPDIR:-/tmp}/searchkernel-m11-sanitizer-reuse.XXXXXX")
trap 'rm -rf "$test_root"' EXIT

fixture="$test_root/repo"
mkdir -p \
    "$fixture/reference/vespa/m11/sanitizer" \
    "$fixture/reference/vespa/common" \
    "$fixture/core/native/vespa" \
    "$fixture/tools/bin"

cp "$root/reference/vespa/m11/verify-m11-sanitizers.sh" \
    "$fixture/reference/vespa/m11/verify-m11-sanitizers.sh"

cat > "$fixture/reference/vespa/common/verify-common.sh" <<'SH'
#!/usr/bin/env bash
CARGO_TARGET_DIR="${CARGO_TARGET_DIR:?test must set CARGO_TARGET_DIR}"
VESPA_DEPS="${VESPA_DEPS:-/tmp/vespa-deps}"
require_pinned_vespa() { :; }
using_published_vespa_toolchain() { return 1; }
require_native_driver() { :; }
fail_verification() {
    echo "verification error: $*" >&2
    exit 1
}
SH

cat > "$fixture/core/native/vespa/build-m11-sanitizer-sdk.sh" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
lane=$1
sdk=$2
build=$3
mkdir -p "$sdk/lib" "$sdk/include" "$build"
printf '{"hardening_failpoints":true,"sanitizer":"%s"}\n' "$lane" > "$sdk/manifest.json"
: > "$sdk/lib/libsearchkernel_vespa.so.2"
: > "$sdk/include/searchkernel_vespa.h"
printf '%s\n' "$lane" >> "${M11_TEST_BUILD_LOG:?}"
SH
chmod +x "$fixture/core/native/vespa/build-m11-sanitizer-sdk.sh"

cat > "$fixture/tools/bin/python3" <<'SH'
#!/usr/bin/env bash
exit 0
SH

cat > "$fixture/tools/bin/cargo" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
mkdir -p "${CARGO_TARGET_DIR:?}/debug"
cat > "$CARGO_TARGET_DIR/debug/searchkernel-m11-driver" <<'DRIVER'
#!/usr/bin/env bash
exit 0
DRIVER
chmod +x "$CARGO_TARGET_DIR/debug/searchkernel-m11-driver"
SH

real_cc=$(command -v cc)
cat > "$test_root/preload.c" <<'C'
void searchkernel_test_preload(void) {}
C
"$real_cc" -shared -fPIC "$test_root/preload.c" -o "$test_root/libpreload.so"

cat > "$fixture/tools/bin/cc" <<SH
#!/usr/bin/env bash
case "${1:-}" in
    -print-file-name=libasan.so|-print-file-name=libtsan.so.2)
        printf '%s\n' "$test_root/libpreload.so"
        ;;
    *)
        exec "$real_cc" "$@"
        ;;
esac
SH

chmod +x "$fixture/tools/bin/python3" "$fixture/tools/bin/cargo" "$fixture/tools/bin/cc"

printf 'halt_on_error=1\n' > "$fixture/reference/vespa/m11/sanitizer/asan.options"
printf 'halt_on_error=1\n' > "$fixture/reference/vespa/m11/sanitizer/ubsan.options"
: > "$fixture/reference/vespa/m11/sanitizer/lsan.supp"
: > "$fixture/reference/vespa/m11/sanitizer/tsan.supp"

build_log="$test_root/build.log"
: > "$build_log"
export M11_TEST_BUILD_LOG="$build_log"
export PATH="$fixture/tools/bin:$PATH"
export M11_SANITIZER_LANES=asan-ubsan

plan_one="$test_root/plan-one"
plan_two="$test_root/plan-two"

CARGO_TARGET_DIR="$plan_one" bash "$fixture/reference/vespa/m11/verify-m11-sanitizers.sh"
CARGO_TARGET_DIR="$plan_one" bash "$fixture/reference/vespa/m11/verify-m11-sanitizers.sh"

plan_one_builds=$(wc -l < "$build_log")
if [[ "$plan_one_builds" -ne 1 ]]; then
    echo "expected one sanitizer SDK build across repeated verification in one plan, got $plan_one_builds" >&2
    exit 1
fi

CARGO_TARGET_DIR="$plan_two" bash "$fixture/reference/vespa/m11/verify-m11-sanitizers.sh"

all_builds=$(wc -l < "$build_log")
if [[ "$all_builds" -ne 2 ]]; then
    echo "expected a fresh sanitizer SDK build for a distinct verification plan, got $all_builds total builds" >&2
    exit 1
fi

echo "M11 sanitizer plan-local reuse regression PASS"
