#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET="${SK_NATIVE_COVERAGE_DIR:-$ROOT/target/native-adapter-coverage}"
SDK="$TARGET/sdk"
NATIVE_BUILD="$TARGET/native-build"
CARGO_TARGET="$TARGET/cargo-target"
RUN="$TARGET/run"
CXX="${CXX:-c++}"

rm -rf "$TARGET"
mkdir -p "$TARGET" "$RUN"

echo "building coverage SDK"
SK_NATIVE_SDK="$SDK" \
SK_NATIVE_BUILD_DIR="$NATIVE_BUILD" \
SK_NATIVE_ENABLE_FAILPOINTS=1 \
SK_NATIVE_COVERAGE=1 \
SK_NATIVE_REGENERATE_BINDINGS=0 \
    "$ROOT/core/native/vespa/build-sdk.sh"

echo "building M11 native driver against coverage SDK"
export CARGO_TARGET_DIR="$CARGO_TARGET"
export SK_NATIVE_SDK="$SDK"
cargo build --quiet --manifest-path "$ROOT/reference/vespa/m11/rust-driver/Cargo.toml"
DRIVER="$CARGO_TARGET/debug/searchkernel-m11-driver"
CONFIG="$RUN/config"
LD_LIBRARY_PATH="$SDK/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
    "$DRIVER" compile "$CONFIG"

TEST="$TARGET/native_adapter_coverage_test"
echo "building native adapter coverage test"
$CXX -std=gnu++23 -O0 -g -Wall -Wextra -Werror -pthread --coverage \
    -DSK_ENABLE_HARDENING_FAILPOINTS -I"$SDK/include" \
    "$ROOT/core/native/vespa/native_adapter_coverage_test.cpp" \
    -L"$SDK/lib" -Wl,-rpath,"$SDK/lib" -lsearchkernel_vespa \
    --coverage -o "$TEST"

echo "running direct native ABI coverage test"
LD_LIBRARY_PATH="$SDK/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
    "$TEST" "$RUN/direct-data" "$CONFIG"

echo "running existing native Rust integration tests against coverage SDK"
SK_NATIVE_SDK="$SDK" \
LD_LIBRARY_PATH="$SDK/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
    cargo test --quiet --manifest-path "$ROOT/core/crates/searchkernel-vespa-engine/Cargo.toml" \
        --all-targets --all-features
SK_NATIVE_SDK="$SDK" \
LD_LIBRARY_PATH="$SDK/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
    cargo test --quiet --manifest-path "$ROOT/core/crates/searchkernel/Cargo.toml" \
        --test native_e2e --all-features

echo "running adapter through M11 lifecycle and failure scenarios"
LD_LIBRARY_PATH="$SDK/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
    "$DRIVER" lifecycle "$CONFIG" "$RUN/lifecycle"
LD_LIBRARY_PATH="$SDK/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
    "$DRIVER" malformed "$CONFIG" "$RUN/malformed"
LD_LIBRARY_PATH="$SDK/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
    "$DRIVER" failures "$CONFIG" "$RUN/failures"
LD_LIBRARY_PATH="$SDK/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
    "$DRIVER" reopen "$CONFIG" "$RUN/reopen"
LD_LIBRARY_PATH="$SDK/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
    "$DRIVER" concurrency "$CONFIG" "$RUN/concurrency"
LD_LIBRARY_PATH="$SDK/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
    "$DRIVER" advanced "$CONFIG" "$RUN/advanced"
LD_LIBRARY_PATH="$SDK/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
    "$DRIVER" adapter "$CONFIG" "$RUN/adapter"

echo "running timing instrumentation in a fresh process"
SK_ENGINE_TIMING=1 \
LD_LIBRARY_PATH="$SDK/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
    "$TEST" "$RUN/timing-data" "$CONFIG" >"$TARGET/timing.stdout" 2>"$TARGET/timing.stderr"
grep -q 'searchkernel_timing phase=' "$TARGET/timing.stderr"

REPORT="$TARGET/gcov.txt"
(
    cd "$TARGET"
    gcov -b -c "$NATIVE_BUILD/searchkernel_vespa.o"
) >"$REPORT"

read -r LINE_RATE BRANCH_RATE FUNCTION_RATE < <(
    python3 - "$REPORT" "$ROOT/core/native/vespa" <<'PY'
import pathlib
import re
import sys

report = open(sys.argv[1], encoding="utf-8").read()
sources = {path.name for path in pathlib.Path(sys.argv[2]).glob("*.cc")}
sections = re.split(r"(?m)^File ['\"]", report)[1:]
reported_sources = set()
line_rates = []
branch_rates = []
function_rates = []
for section in sections:
    name, _, body = section.partition("'\n")
    if name.rsplit("/", 1)[-1] not in sources:
        continue
    reported_sources.add(name.rsplit("/", 1)[-1])
    lines = re.search(r"Lines executed:([0-9.]+)% of ([0-9]+)", body)
    branches = re.search(r"Branches executed:([0-9.]+)% of ([0-9]+)", body)
    functions = re.search(r"Functions executed:([0-9.]+)% of ([0-9]+)", body)
    if lines:
        line_rates.append((float(lines.group(1)), int(lines.group(2))))
    if branches:
        branch_rates.append((float(branches.group(1)), int(branches.group(2))))
    if functions:
        function_rates.append((float(functions.group(1)), int(functions.group(2))))
if not line_rates:
    raise SystemExit("gcov did not report the split native adapter sources")
missing = sorted(sources - reported_sources)
if missing:
    raise SystemExit(
        "gcov did not report all native adapter translation units: "
        + ", ".join(missing)
    )
def average(values):
    if not values:
        return "unknown"
    rate, total = zip(*values)
    return f"{sum(value * count for value, count in values) / sum(total):.2f}"
print(average(line_rates), average(branch_rates), average(function_rates))
PY
)

printf 'native adapter coverage: lines=%s%% branches=%s%% functions=%s%%\n' \
    "${LINE_RATE:-unknown}" "${BRANCH_RATE:-unknown}" "${FUNCTION_RATE:-unknown}"

if [[ -z "$LINE_RATE" ]] || ! awk -v rate="$LINE_RATE" 'BEGIN { exit !(rate >= 90.0) }'; then
    echo "native adapter line coverage is below the required 90%" >&2
    exit 1
fi

echo "native adapter coverage PASS"
