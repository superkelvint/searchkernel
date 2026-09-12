#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
SDK=${SK_NATIVE_SDK:-$ROOT/native/sdk/linux-x86_64}
BUILD=${SK_M0_PROBE_COVERAGE_DIR:-$ROOT/target/m0-probe-coverage}
PROBE=$ROOT/reference/vespa/m0/native-probe/probe.c

mkdir -p "$BUILD"
rm -f "$BUILD"/*.gcda "$BUILD"/*.gcov "$BUILD"/probe-real "$BUILD"/probe-fake \
    "$BUILD"/probe.o "$BUILD"/m0-probe-fake.o

CFLAGS="-std=c11 -O0 -g -Wall -Wextra -Werror --coverage"

cc $CFLAGS -I"$SDK/include" -c "$PROBE" -o "$BUILD/probe.o"
cc $CFLAGS -I"$SDK/include" -c "$ROOT/tools/m0-probe-fake.c" \
    -o "$BUILD/m0-probe-fake.o"

cc $CFLAGS "$BUILD/probe.o" "$BUILD/m0-probe-fake.o" \
    -o "$BUILD/probe-fake"
cc $CFLAGS "$BUILD/probe.o" -L"$SDK/lib" -lsearchkernel_vespa \
    -Wl,-rpath,"$SDK/lib" -o "$BUILD/probe-real"

expect_exit() {
    expected=$1
    shift
    output=$BUILD/last-output
    set +e
    "$@" >"$output" 2>&1
    actual=$?
    set -e
    if [ "$actual" -ne "$expected" ]; then
        echo "expected exit $expected, got $actual: $*" >&2
        sed -n '1,80p' "$output" >&2
        exit 1
    fi
}

CONFIG=$SDK/share/embedded_crud_cfg

# Exercise the actual M0 acceptance path with the packaged native library.
mkdir -p "$BUILD/real-data"
LD_LIBRARY_PATH="$SDK/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
    expect_exit 0 "$BUILD/probe-real" "$BUILD/real-data" "$CONFIG"

# Exercise probe-only failure branches with deterministic ABI doubles.
expect_exit 2 "$BUILD/probe-fake"
SK_M0_FAKE_MODE=abi-mismatch expect_exit 1 "$BUILD/probe-fake" \
    "$BUILD/fake-data" "$CONFIG"
SK_M0_FAKE_MODE=open-error expect_exit 1 "$BUILD/probe-fake" \
    "$BUILD/fake-data" "$CONFIG"
SK_M0_FAKE_MODE=open-null-message expect_exit 1 "$BUILD/probe-fake" \
    "$BUILD/fake-data" "$CONFIG"
SK_M0_FAKE_MODE=open-null-engine expect_exit 1 "$BUILD/probe-fake" \
    "$BUILD/fake-data" "$CONFIG"
SK_M0_FAKE_MODE=second-open-error expect_exit 1 "$BUILD/probe-fake" \
    "$BUILD/fake-data" "$CONFIG"
SK_M0_FAKE_MODE=success expect_exit 0 "$BUILD/probe-fake" \
    "$BUILD/fake-data" "$CONFIG"

(cd "$BUILD" && gcov -b probe.o > gcov.txt)
cat "$BUILD/gcov.txt"

if ! grep -q 'Lines executed:100.00%' "$BUILD/gcov.txt"; then
    echo "M0 probe line coverage is below 100%" >&2
    exit 1
fi
if ! grep -q 'Branches executed:100.00%' "$BUILD/gcov.txt"; then
    echo "M0 probe branch coverage is below 100%" >&2
    exit 1
fi

echo "M0 probe coverage PASS"
