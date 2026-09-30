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
CI_IMAGE_RUNNER="$ROOT/.github/ci/ci-image-runner.yml"

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
SEARCHKERNEL_NATIVE_PROBE_BASE_SDK="$SOURCE_TEST/base" \
SEARCHKERNEL_NATIVE_PROBE_VESPA_ROOT="$SOURCE_TEST/vespa" \
SEARCHKERNEL_NATIVE_PROBE_SDK="$SOURCE_TEST/sdk" \
PATH="$SOURCE_TEST/bin:$PATH" \
    /bin/bash -euo pipefail -c 'unset SK_NATIVE_SDK; source "$1"' _ "$COMMON"
[[ ! -e "$SOURCE_TEST/build-sdk-called" ]] || {
    echo "verify-common.sh ran build-sdk.sh while being sourced" >&2
    exit 1
}
if grep -F 'prepare_native_probe_sdk' "$VERIFY" >/dev/null; then
    echo "verify-through must not prepare a native-probe SDK for every Rust verifier" >&2
    exit 1
fi
grep -F 'prepare_native_probe_sdk' "$ROOT/reference/vespa/m0/verify-m0.sh" >/dev/null || {
    echo "M0 direct native probe does not explicitly prepare its native-probe SDK" >&2
    exit 1
}
for normal_rust_verifier in m13 m14; do
    if grep -F 'require_sdk_manifest' "$ROOT/reference/vespa/$normal_rust_verifier/verify-$normal_rust_verifier.sh" >/dev/null; then
        echo "$normal_rust_verifier Rust verifier still requires a private SDK manifest" >&2
        exit 1
    fi
done


# Verifier SDK caching must prove the real lifecycle: a clean first invocation
# builds and publishes cache metadata, a second independent process reuses it,
# moving the immutable base invalidates the adapter because its absolute RPATH
# changes, and a stale fingerprint rebuilds again.
CACHE_TEST="$SOURCE_TEST/cache"
mkdir -p "$CACHE_TEST/bin" "$CACHE_TEST/base/lib" "$CACHE_TEST/base-moved/lib" "$CACHE_TEST/vespa"
printf '{"kind":"vespa-base","vespa_commit":"fixture"}\n' > "$CACHE_TEST/base/manifest.json"
cp "$CACHE_TEST/base/manifest.json" "$CACHE_TEST/base-moved/manifest.json"
cat > "$CACHE_TEST/bin/bash" <<'SH'
#!/bin/bash
set -euo pipefail
if [[ "${1:-}" == */core/native/vespa/build-sdk.sh ]]; then
    printf 'build\n' >> "$SEARCHKERNEL_BUILD_SDK_MARKER"
    mkdir -p "$SK_NATIVE_SDK/include" "$SK_NATIVE_SDK/lib"
    printf '{}\n' > "$SK_NATIVE_SDK/manifest.json"
    printf '#define SK_NATIVE_ABI_VERSION 14U\n' > "$SK_NATIVE_SDK/include/searchkernel_vespa.h"
    : > "$SK_NATIVE_SDK/lib/libsearchkernel_vespa.so"

    python_bin=$(/bin/bash "$SEARCHKERNEL_TEST_REPO_ROOT/tools/resolve-python.sh")
    rustc_bin=$(/bin/bash "$SEARCHKERNEL_TEST_REPO_ROOT/tools/resolve-rust.sh" rustc)
    native_target="${SK_NATIVE_TARGET:-$("$rustc_bin" -vV | sed -n 's/^host: //p')}"
    RUSTC="$rustc_bin" "$python_bin" "$SEARCHKERNEL_TEST_REPO_ROOT/tools/native-build-fingerprint.py" \
        --root "$SEARCHKERNEL_TEST_REPO_ROOT" \
        --vespa-root "$VESPA_ROOT" \
        --base-sdk "$SK_NATIVE_BASE_SDK" \
        --target "$native_target" \
        --scope full \
        --output "$SK_NATIVE_SDK/native-build-fingerprint-v2.txt"
    exit 0
fi
exec /bin/bash "$@"
SH
chmod +x "$CACHE_TEST/bin/bash"

run_cache_prepare() {
    local base=$1
    SEARCHKERNEL_BUILD_SDK_MARKER="$CACHE_TEST/build-sdk-called" \
    SEARCHKERNEL_TEST_REPO_ROOT="$ROOT" \
    SEARCHKERNEL_NATIVE_PROBE_BASE_SDK="$base" \
    SEARCHKERNEL_NATIVE_PROBE_VESPA_ROOT="$CACHE_TEST/vespa" \
    SEARCHKERNEL_NATIVE_PROBE_SDK="$CACHE_TEST/sdk" \
    SEARCHKERNEL_NATIVE_PROBE_SDK_BUILD="$CACHE_TEST/build" \
    PATH="$CACHE_TEST/bin:$PATH" \
        /bin/bash -euo pipefail -c 'unset SK_NATIVE_SDK SEARCHKERNEL_NATIVE_PROBE_SDK_PREPARED; source "$1"; prepare_native_probe_sdk' _ "$COMMON"
}
build_count() {
    if [[ -f "$CACHE_TEST/build-sdk-called" ]]; then
        wc -l < "$CACHE_TEST/build-sdk-called" | tr -d '[:space:]'
    else
        printf '0\n'
    fi
}

run_cache_prepare "$CACHE_TEST/base"
[[ "$(build_count)" == 1 ]] || {
    echo "clean native-probe SDK did not build exactly once" >&2
    exit 1
}
[[ -f "$CACHE_TEST/sdk/native-build-fingerprint-v2.txt" ]] || {
    echo "native-probe adapter build did not publish its fingerprint" >&2
    exit 1
}

run_cache_prepare "$CACHE_TEST/base"
[[ "$(build_count)" == 1 ]] || {
    echo "matching native-probe SDK fingerprint did not reuse the built adapter" >&2
    exit 1
}

run_cache_prepare "$CACHE_TEST/base-moved"
[[ "$(build_count)" == 2 ]] || {
    echo "changed native base SDK location did not rebuild the adapter" >&2
    exit 1
}

printf 'fingerprint-version=3\nsha256=stale\n' > "$CACHE_TEST/sdk/native-build-fingerprint-v2.txt"
run_cache_prepare "$CACHE_TEST/base-moved"
[[ "$(build_count)" == 3 ]] || {
    echo "stale native-probe SDK fingerprint did not rebuild the adapter" >&2
    exit 1
}

grep -F -- '--base-sdk "$BASE_SDK"' "$ROOT/core/native/vespa/build-sdk.sh" >/dev/null || {
    echo "adapter-only SDK build does not bind its fingerprint to BASE_SDK" >&2
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
grep -F "unknown verify target: vespa.this-target-does-not-exist" <<<"$unknown_output" >/dev/null || {
    echo "unknown semantic target did not fail through the canonical registry" >&2
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

# M3 must execute its owning workspace example through normal Cargo runtime setup.
M3_RUN="$ROOT/reference/vespa/m3/run-rust.sh"
mkdir -p "$SOURCE_TEST/m3-bin" "$SOURCE_TEST/m3-target"
cat > "$SOURCE_TEST/m3-bin/cargo" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
[[ "${1:-}" == run && " $* " == *" -p searchkernel-vespa-engine "* && " $* " == *" --example searchkernel-m3-conformance-driver "* ]] || {
    echo "unexpected cargo invocation: $*" >&2
    exit 96
}
while IFS=$'\t' read -r id _; do
    [[ -n "$id" ]] || continue
    printf '{"id":"%s","terms":[]}\n' "$id"
done
SH
cat > "$SOURCE_TEST/m3-bin/python3" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
exec /usr/bin/python3 "$@"
SH
chmod +x "$SOURCE_TEST/m3-bin/cargo" "$SOURCE_TEST/m3-bin/python3"
PATH="$SOURCE_TEST/m3-bin:$PATH" \
CARGO_TARGET_DIR="$SOURCE_TEST/m3-target" \
    "$M3_RUN" document "$SOURCE_TEST/m3-output.jsonl"

# An ordinary migrated Rust runner must use Cargo's native runtime path and
# must not synthesize the direct-probe SDK just because it is a verifier.
NORMAL_RUST_SDK="$SOURCE_TEST/normal-rust/native-probe-sdk"
PATH="$SOURCE_TEST/m3-bin:$PATH" \
SEARCHKERNEL_NATIVE_PROBE_SDK="$NORMAL_RUST_SDK" \
CARGO_TARGET_DIR="$SOURCE_TEST/normal-rust-target" \
    "$M3_RUN" document "$SOURCE_TEST/normal-rust-output.jsonl"
[[ ! -e "$NORMAL_RUST_SDK" ]] || {
    echo "ordinary Rust verifier synthesized a native-probe SDK" >&2
    exit 1
}

echo "semantic verifier routing PASS"
