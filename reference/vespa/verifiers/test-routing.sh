#!/usr/bin/env bash
set -euo pipefail

DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$DIR/../../.." && pwd)"
VERIFY="$ROOT/reference/vespa/verify-through.sh"
DEV="$ROOT/dev"
INTEGRATION="$ROOT/reference/vespa/i/verify-i.sh"
PHASE9="$ROOT/reference/vespa/phase9/verify-phase9.sh"
HNSW="$ROOT/reference/vespa/verifiers/nearest-neighbor/hnsw/verify.sh"
M14="$ROOT/reference/vespa/m14/verify-m14.sh"
CI_IMAGE_RUNNER="$ROOT/.github/workflows/ci-image-runner.yml"

bash -n "$VERIFY"
bash -n "$DEV"
bash -n "$INTEGRATION"
bash -n "$PHASE9"
bash -n "$HNSW"
bash -n "$M14"

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

grep -F 'parse_cumulative_verifier_args "$@"' "$INTEGRATION" >/dev/null
grep -F 'parse_cumulative_verifier_args "$@"' "$PHASE9" >/dev/null
grep -F -- '--baseline-already-proved|--skip-cumulative)' "$COMMON" >/dev/null || {
    echo "cumulative verifier baseline skip aliases are missing" >&2
    exit 1
}
for script in "$INTEGRATION" "$PHASE9"; do
    grep -F 'if [[ "$BASELINE_ALREADY_PROVED" != 1 ]]; then' "$script" >/dev/null || {
        echo "cumulative verifier does not guard its M13 baseline: $script" >&2
        exit 1
    }
    grep -F '"$ROOT/reference/vespa/verify-through.sh" m13 "${CHILD_VERIFIER_ARGS[@]}"' "$script" >/dev/null || {
        echo "cumulative verifier no longer proves M13 by default: $script" >&2
        exit 1
    }
done
grep -F '"$ROOT/reference/vespa/verify-through.sh" phase10 "${CHILD_VERIFIER_ARGS[@]}"' "$INTEGRATION" >/dev/null || {
    echo "integration no longer forwards verifier args to Phase 10" >&2
    exit 1
}
grep -F '"$SCRIPT_DIR/verify-entry-parity.sh"' "$INTEGRATION" >/dev/null || {
    echo "integration no longer runs the real-engine Rust-direct/gRPC parity gate" >&2
    exit 1
}

grep -F 'exec "$ROOT/reference/vespa/m14/verify-m14.sh" "$@"' "$HNSW" >/dev/null || {
    echo "HNSW semantic verifier no longer establishes the M14 cumulative baseline" >&2
    exit 1
}
grep -F '"$ROOT/reference/vespa/m13/verify-m13.sh" --skip-workspace-quality' "$M14" >/dev/null || {
    echo "M14 no longer proves M13 before native-full reuses that baseline" >&2
    exit 1
}

grep -F 'run_phase native-full-product-integration ./dev verify product.integration --baseline-already-proved' "$CI_IMAGE_RUNNER" >/dev/null || {
    echo "native-full product integration does not reuse the established cumulative baseline" >&2
    exit 1
}
grep -F 'run_phase native-full-phase9 reference/vespa/phase9/verify-phase9.sh --baseline-already-proved' "$CI_IMAGE_RUNNER" >/dev/null || {
    echo "native-full Phase 9 does not reuse the established cumulative baseline" >&2
    exit 1
}
hnsw_line="$(grep -nF 'run_phase native-full-hnsw ./dev verify vespa.nearest-neighbor.hnsw' "$CI_IMAGE_RUNNER" | cut -d: -f1)"
integration_line="$(grep -nF 'run_phase native-full-product-integration ./dev verify product.integration --baseline-already-proved' "$CI_IMAGE_RUNNER" | cut -d: -f1)"
phase9_line="$(grep -nF 'run_phase native-full-phase9 reference/vespa/phase9/verify-phase9.sh --baseline-already-proved' "$CI_IMAGE_RUNNER" | cut -d: -f1)"
[[ -n "$hnsw_line" && -n "$integration_line" && -n "$phase9_line" ]] &&
    (( hnsw_line < integration_line && hnsw_line < phase9_line )) || {
        echo "native-full skips the cumulative baseline before HNSW/M14 has proved it" >&2
        exit 1
    }

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
if [[ "${1:-}" == */.github/ci/resolve-native-adapter-lib.py ]]; then
    printf '%s\n' "$CARGO_TARGET_DIR/fake-native-adapter"
    exit 0
fi
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
