#!/usr/bin/env bash
set -euo pipefail

ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
test_root="$(mktemp -d)"
trap 'rm -rf "$test_root"' EXIT

fake_bin="$test_root/bin"
log_dir="$test_root/log"
mkdir -p "$fake_bin" "$log_dir"

printf '%s\n' baseline current > "$test_root/baseline.so"
printf '%s\n' current > "$test_root/current.so"

printf '%s\n' \
    '#!/bin/sh' \
    'set -eu' \
    'output=' \
    'while [ "$#" -gt 0 ]; do' \
    '    if [ "$1" = "--out-file" ]; then' \
    '        output=$2' \
    '        shift 2' \
    '    else' \
    '        printf "%s\n" "$1" > "$ABIDW_INPUT_LOG"' \
    '        shift' \
    '    fi' \
    'done' \
    'printf "%s\n" "$output" > "$ABIDW_OUTPUT_LOG"' \
    'printf "%s\n" "<abi-corpus version=\"2.1\"/>" > "$output"' \
    > "$fake_bin/abidw"
chmod +x "$fake_bin/abidw"

printf '%s\n' \
    '#!/bin/sh' \
    'set -eu' \
    'printf "%s\n" "$@" > "$ABIDIFF_LOG"' \
    'if [ "$1" != "--fail-no-debug-info" ]; then' \
    '    exit 1' \
    'fi' \
    'exit "${ABIDIFF_EXIT:-0}"' \
    > "$fake_bin/abidiff"
chmod +x "$fake_bin/abidiff"

output="$test_root/reports/current.abi.xml"
PATH="$fake_bin:$PATH" \
    ABIDW_INPUT_LOG="$log_dir/abidw-input" \
    ABIDW_OUTPUT_LOG="$log_dir/abidw-output" \
    ABIDIFF_LOG="$log_dir/abidiff" \
    "$ROOT/tools/check-native-abi.sh" \
    "$test_root/baseline.so" "$test_root/current.so" "$output"

grep -Fx -- "$test_root/current.so" "$log_dir/abidw-input" >/dev/null
grep -Fx -- "$output" "$log_dir/abidw-output" >/dev/null
grep -Fx -- '--fail-no-debug-info' "$log_dir/abidiff" >/dev/null
grep -Fx -- "$test_root/baseline.so" "$log_dir/abidiff" >/dev/null
grep -Fx -- "$test_root/current.so" "$log_dir/abidiff" >/dev/null
grep -Fx -- '<abi-corpus version="2.1"/>' "$output" >/dev/null

if PATH="$fake_bin:$PATH" \
    ABIDW_INPUT_LOG="$log_dir/abidw-input" \
    ABIDW_OUTPUT_LOG="$log_dir/abidw-output" \
    ABIDIFF_LOG="$log_dir/abidiff" \
    ABIDIFF_EXIT=8 \
    "$ROOT/tools/check-native-abi.sh" \
    "$test_root/baseline.so" "$test_root/current.so" "$output"; then
    echo "abidiff failure was swallowed" >&2
    exit 1
fi

grep -Fx -- '<abi-corpus version="2.1"/>' "$output" >/dev/null
echo "native ABI checker regression PASS"
