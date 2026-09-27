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

cp "$root/core/native/vespa/build-m11-sanitizer-sdk.sh" \
    "$fixture/core/native/vespa/build-m11-sanitizer-sdk.sh"
cat > "$fixture/core/native/vespa/build-sdk.sh" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
lane=${SK_NATIVE_SANITIZER:?}
sdk=${SK_NATIVE_SDK:?}
build=${SK_NATIVE_BUILD_DIR:?}
if [[ "$lane" == asan-ubsan ]]; then
    [[ "${SK_NATIVE_EXTRA_CXX_FLAGS:-}" == *-fsanitize=address,undefined* ]]
    [[ "${SK_NATIVE_EXTRA_CXX_FLAGS:-}" == *-fno-sanitize=vptr* ]]
    [[ "${SK_NATIVE_EXTRA_LINK_FLAGS:-}" == *-fsanitize=address,undefined* ]]
fi
mkdir -p "$sdk/lib" "$sdk/include" "$build"
printf '{"hardening_failpoints":true,"sanitizer":"%s"}\n' "$lane" > "$sdk/manifest.json"
: > "$sdk/lib/libsearchkernel_vespa.so.2"
: > "$sdk/include/searchkernel_vespa.h"
printf '%s\n' "$lane" >> "${M11_TEST_BUILD_LOG:?}"
SH
chmod +x "$fixture/core/native/vespa/build-m11-sanitizer-sdk.sh"
chmod +x "$fixture/core/native/vespa/build-sdk.sh"

cat > "$fixture/tools/bin/python3" <<'SH'
#!/usr/bin/env bash
exit 0
SH

cat > "$fixture/tools/bin/cargo" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
if [[ -n "${SK_NATIVE_SDK:-}" ]]; then
    echo "Cargo must use the normal native SDK resolver for the sanitizer driver build" >&2
    exit 1
fi
mkdir -p "${CARGO_TARGET_DIR:?}/debug"
cat > "$CARGO_TARGET_DIR/debug/searchkernel-m11-driver" <<'DRIVER'
#!/usr/bin/env bash
case "${LD_LIBRARY_PATH:-}" in
    */sanitizer/asan-ubsan/sdk/lib*) ;;
    *) echo "sanitizer adapter is not first in runtime library lookup" >&2; exit 1 ;;
esac
exit 0
DRIVER
chmod +x "$CARGO_TARGET_DIR/debug/searchkernel-m11-driver"
SH

real_cc=$(command -v cc)
cat > "$test_root/preload.c" <<'C'
void searchkernel_test_preload(void) {}
C
"$real_cc" -shared -fPIC "$test_root/preload.c" -o "$test_root/libpreload.so"
export M11_TEST_REAL_CC="$real_cc"
export M11_TEST_PRELOAD="$test_root/libpreload.so"

cat > "$fixture/tools/bin/cc" <<'SH'
#!/usr/bin/env bash
case "${1:-}" in
    -print-file-name=libasan.so.8|-print-file-name=libtsan.so.2)
        printf '%s\n' "${M11_TEST_PRELOAD:?}"
        ;;
    *)
        exec "${M11_TEST_REAL_CC:?}" "$@"
        ;;
esac
SH

cat > "$fixture/tools/bin/ldd" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
library_dir=${LD_LIBRARY_PATH%%:*}
printf 'libsearchkernel_vespa.so.2 => %s/libsearchkernel_vespa.so.2 (0x0)\n' "$library_dir"
SH

cat > "$fixture/tools/bin/c++" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
output=
while (($#)); do
    if [[ "$1" == -o ]]; then
        output=$2
        shift 2
    else
        shift
    fi
done
[[ -n "$output" ]]
cat > "$output" <<'BIN'
#!/usr/bin/env bash
exit 0
BIN
chmod +x "$output"
SH

chmod +x "$fixture/tools/bin/python3" "$fixture/tools/bin/cargo" \
    "$fixture/tools/bin/cc" "$fixture/tools/bin/ldd" "$fixture/tools/bin/c++"

printf 'halt_on_error=1\n' > "$fixture/reference/vespa/m11/sanitizer/asan.options"
printf 'halt_on_error=1\n' > "$fixture/reference/vespa/m11/sanitizer/ubsan.options"
: > "$fixture/reference/vespa/m11/sanitizer/lsan.supp"
: > "$fixture/reference/vespa/m11/sanitizer/tsan.supp"

build_log="$test_root/build.log"
: > "$build_log"
export M11_TEST_BUILD_LOG="$build_log"
export PATH="$fixture/tools/bin:$PATH"
export CXX="$fixture/tools/bin/c++"
export M11_SANITIZER_LANES=asan-ubsan
export SK_NATIVE_SDK="$test_root/invalid-explicit-sdk"

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
