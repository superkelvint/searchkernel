#!/usr/bin/env bash
set -euo pipefail

DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$DIR/../../.." && pwd)"
VERIFY="$ROOT/reference/vespa/verify-through.sh"
DEV="$ROOT/dev"

bash -n "$VERIFY"
bash -n "$DEV"

# Sourcing verify-common must be read-only. Supply every resolved path so this
# check needs no native installation, then shadow bash to detect any accidental
# source-time build-sdk invocation.
COMMON="$ROOT/reference/vespa/common/verify-common.sh"
SOURCE_TEST="$(mktemp -d)"
trap 'rm -rf "$SOURCE_TEST"' EXIT
mkdir -p "$SOURCE_TEST/bin" "$SOURCE_TEST/base" "$SOURCE_TEST/vespa" "$SOURCE_TEST/sdk"
cat > "$SOURCE_TEST/bin/bash" <<'SH'
#!/bin/bash
if [[ "${1:-}" == */core/native/vespa/build-sdk.sh ]]; then
    : > "$SEARCHKERNEL_BUILD_SDK_MARKER"
    exit 97
fi
exec /bin/bash "$@"
SH
chmod +x "$SOURCE_TEST/bin/bash"
SEARCHKERNEL_BUILD_SDK_MARKER="$SOURCE_TEST/build-sdk-called" \
SEARCHKERNEL_VERIFIER_BASE_SDK="$SOURCE_TEST/base" \
SEARCHKERNEL_VERIFIER_VESPA_ROOT="$SOURCE_TEST/vespa" \
SEARCHKERNEL_VERIFIER_SDK="$SOURCE_TEST/sdk" \
PATH="$SOURCE_TEST/bin:$PATH" \
    /bin/bash -euo pipefail -c 'unset SK_NATIVE_SDK; source "$1"' _ "$COMMON"
[[ ! -e "$SOURCE_TEST/build-sdk-called" ]] || {
    echo "verify-common.sh ran build-sdk.sh while being sourced" >&2
    exit 1
}
grep -F 'prepare_verifier_sdk' "$VERIFY" >/dev/null || {
    echo "verify-through does not explicitly prepare the verifier SDK" >&2
    exit 1
}

while IFS= read -r wrapper; do
    bash -n "$wrapper"
done < <(find "$DIR" -type f -name verify.sh -print | sort)

list="$("$VERIFY" --list)"
for target in     vespa.grouping.yql     vespa.hardening.core     vespa.hardening.sanitizers     vespa.persistence.reopen     milestone.m14     product.integration; do
    grep -Fx "  $target" <<<"$list" >/dev/null || {
        echo "missing canonical verifier target from --list: $target" >&2
        exit 1
    }
done

# ./dev performs one-time repository hook setup on first invocation. Warm that
# setup before comparing its semantic output so a fresh clone remains deterministic.
"$DEV" verify --list >/dev/null
dev_list="$("$DEV" verify --list)"
[[ "$dev_list" == "$list" ]] || {
    echo "./dev verify --list diverges from verify-through --list" >&2
    exit 1
}

set +e
unknown_output="$("$VERIFY" vespa.this-target-does-not-exist 2>&1)"
unknown_status=$?
set -e
[[ "$unknown_status" -eq 2 ]] || {
    echo "unknown semantic target returned $unknown_status instead of 2" >&2
    exit 1
}
grep -F "unknown verifier target: vespa.this-target-does-not-exist" <<<"$unknown_output" >/dev/null || {
    echo "unknown semantic target did not fail with the expected diagnostic" >&2
    exit 1
}

grep -F 'verify_target "$@"' "$DEV" >/dev/null || {
    echo "./dev verify does not forward verifier arguments" >&2
    exit 1
}
grep -F '"${@:2}"' "$VERIFY" >/dev/null || {
    echo "verify-through does not forward verifier arguments" >&2
    exit 1
}
for milestone in 3 4 5 6 7 8 9 10 11 12 13 14; do
    script="$ROOT/reference/vespa/m$milestone/verify-m$milestone.sh"
    grep -F 'parse_verifier_args "$@"' "$script" >/dev/null || {
        echo "M$milestone does not parse verifier arguments" >&2
        exit 1
    }
done
for milestone in 4 5 6 7 8 9 10 11 12 13 14; do
    script="$ROOT/reference/vespa/m$milestone/verify-m$milestone.sh"
    grep -F -- '--skip-workspace-quality' "$script" >/dev/null || {
        echo "M$milestone cumulative child does not skip nested workspace quality" >&2
        exit 1
    }
done

grep -F 'universe_dev_step "semantic verifier $target" verify "$target" --skip-workspace-quality' "$DEV" >/dev/null
grep -F "universe_dev_step 'cumulative milestone.m14 closure' verify milestone.m14 --skip-workspace-quality" "$DEV" >/dev/null

integration="$ROOT/reference/vespa/i/verify-i.sh"
grep -F 'parse_verifier_args "$@"' "$integration" >/dev/null
for target in m13 phase10; do
    grep -F '"$ROOT/reference/vespa/verify-through.sh" '"$target"' "$@"' "$integration" >/dev/null
done

# M3 must execute the driver from Cargo's active target directory. The verifier
# exports a shared CARGO_TARGET_DIR, so crate-local target/debug is not reliable.
M3_RUN="$ROOT/reference/vespa/m3/run-rust.sh"
mkdir -p "$SOURCE_TEST/m3-bin" "$SOURCE_TEST/m3-target"
cat > "$SOURCE_TEST/m3-bin/cargo" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
[[ "${1:-}" == build ]] || {
    echo "unexpected cargo invocation: $*" >&2
    exit 96
}
mkdir -p "$CARGO_TARGET_DIR/debug"
cat > "$CARGO_TARGET_DIR/debug/searchkernel-m3-conformance-driver" <<'DRIVER'
#!/usr/bin/env bash
exit 0
DRIVER
chmod +x "$CARGO_TARGET_DIR/debug/searchkernel-m3-conformance-driver"
SH
cat > "$SOURCE_TEST/m3-bin/python3" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
expected="$CARGO_TARGET_DIR/debug/searchkernel-m3-conformance-driver"
actual="${@: -2:1}"
[[ "$actual" == "$expected" ]] || {
    echo "M3 run-rust used $actual instead of Cargo target driver $expected" >&2
    exit 95
}
SH
chmod +x "$SOURCE_TEST/m3-bin/cargo" "$SOURCE_TEST/m3-bin/python3"
PATH="$SOURCE_TEST/m3-bin:$PATH" \
CARGO_TARGET_DIR="$SOURCE_TEST/m3-target" \
    "$M3_RUN" document "$SOURCE_TEST/m3-output.jsonl"

echo "semantic verifier routing PASS"
