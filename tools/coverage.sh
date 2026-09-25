#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$ROOT"

CARGO=${CARGO:-cargo}
OUTPUT_DIR=${SEARCHKERNEL_COVERAGE_DIR:-"$ROOT/coverage"}
IGNORE_RE='(^|/)(target|generated|upstream|vendor|node_modules|tests?|benches|examples|reference)(/|$)|(^|/)core/crates/searchkernel-testkit(/|$)'

command -v "$CARGO" >/dev/null 2>&1 || {
    echo "error: cargo is required for coverage" >&2
    exit 1
}

if ! "$CARGO" llvm-cov --version >/dev/null 2>&1; then
    echo "error: cargo-llvm-cov is not available in this build environment" >&2
    echo "Search Kernel coverage runs in the immutable CI image; publish an image containing cargo-llvm-cov." >&2
    exit 1
fi

rm -rf -- "$OUTPUT_DIR"
mkdir -p "$OUTPUT_DIR"

echo "==> coverage: cleaning prior LLVM profiles"
"$CARGO" llvm-cov clean --workspace

echo "==> coverage: running Rust workspace tests (all targets/features)"
"$CARGO" llvm-cov \
    --workspace \
    --all-targets \
    --all-features \
    --no-report

echo "==> coverage: writing machine-readable and human-readable reports"
"$CARGO" llvm-cov report \
    --ignore-filename-regex "$IGNORE_RE" \
    --json \
    --output-path "$OUTPUT_DIR/coverage.json"
"$CARGO" llvm-cov report \
    --ignore-filename-regex "$IGNORE_RE" \
    --json \
    --summary-only \
    --output-path "$OUTPUT_DIR/summary.json"
"$CARGO" llvm-cov report \
    --ignore-filename-regex "$IGNORE_RE" \
    --lcov \
    --output-path "$OUTPUT_DIR/lcov.info"
"$CARGO" llvm-cov report \
    --ignore-filename-regex "$IGNORE_RE" \
    --text \
    --output-path "$OUTPUT_DIR/summary.txt"
"$CARGO" llvm-cov report \
    --ignore-filename-regex "$IGNORE_RE" \
    --html \
    --output-dir "$OUTPUT_DIR/html"

head_sha=$(git rev-parse HEAD)
tool_version=$("$CARGO" llvm-cov --version | head -n 1)
python3 - "$OUTPUT_DIR/metadata.json" "$head_sha" "$tool_version" "$IGNORE_RE" <<'PY'
import datetime
import json
import sys

path, head, tool_version, ignore_re = sys.argv[1:]
payload = {
    "schema_version": 1,
    "git_head": head,
    "generated_at_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
    "tool": tool_version,
    "test_scope": "cargo llvm-cov --workspace --all-targets --all-features",
    "report_scope": "Rust production sources; generated/test/reference/testkit paths excluded from reports",
    "ignore_filename_regex": ignore_re,
    "native_cpp_coverage": False,
    "native_cpp_note": (
        "Vespa/SearchKernel C++ is not instrumented by this Rust coverage run. "
        "Use native/real-engine verifiers for native behavior."
    ),
}
with open(path, "w", encoding="utf-8") as stream:
    json.dump(payload, stream, indent=2, sort_keys=True)
    stream.write("\n")
PY

cat > "$OUTPUT_DIR/README.txt" <<'EOF'
Search Kernel coverage evidence

This report measures Rust production code reached by:
  cargo llvm-cov --workspace --all-targets --all-features

Generated code, test/reference sources, examples/benches, upstream/vendor code,
and searchkernel-testkit are excluded from the reported denominator.

This does NOT instrument the linked Vespa/SearchKernel C++ native implementation.
A high Rust coverage percentage does not replace native/real-engine verification.
See metadata.json for the exact Git HEAD and tool version.
EOF

cat "$OUTPUT_DIR/summary.txt"
echo
echo "Coverage reports: $OUTPUT_DIR"
