#!/usr/bin/env bash
# Build and exercise the isolated static-native runtime candidates. Generated
# artifacts and diagnostic output stay under build/ and are intentionally not
# part of the production build graph.
set -euo pipefail

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
SPIKE=$ROOT/spikes/static-searchkernel
BUILD=$SPIKE/build
SMOKE=$SPIKE/smoke
PYTHON=$(bash "$ROOT/tools/resolve-python.sh")
CARGO=$(bash "$ROOT/tools/resolve-rust.sh" cargo)
RUSTC=$(bash "$ROOT/tools/resolve-rust.sh" rustc)
SDK=$(bash "$ROOT/tools/resolve-native-sdk.sh" path)
VESPA=$(bash "$ROOT/tools/resolve-native-sdk.sh" vespa-root)
STATIC_PLAN=$BUILD/static-plan

usage() {
    echo "usage: $0 {candidate-a|candidate-b|candidate-c|all|clean}" >&2
    exit 2
}

clean_env() {
    env -u LD_LIBRARY_PATH -u SK_NATIVE_SDK -u SK_NATIVE_ABI_HEADER \
        -u SK_NATIVE_CACHE_DIR -u SEARCHKERNEL_PACKAGED_SUPPORT_ROOT \
        -u SEARCHKERNEL_SDK_LIB "$@"
}

timed() {
    local output=$1
    shift
    { time -p "$@"; } 2>"$output"
}

record_environment() {
    mkdir -p "$BUILD/results"
    {
        echo "searchkernel_sha=$(git -C "$ROOT" rev-parse HEAD)"
        echo "vespa_sha=$(git -C "$VESPA" rev-parse HEAD)"
        echo "target=$("$RUSTC" -vV | sed -n 's/^host: //p')"
        uname -a
        "$CARGO" --version
        c++ --version | head -1
        ld --version | head -1
        echo "sdk=$SDK"
    } >"$BUILD/results/environment.txt"
}

capture() {
    local name=$1 artifact=$2
    mkdir -p "$BUILD/results/$name"
    stat -c '%n %s' "$artifact" >"$BUILD/results/$name/size.txt"
    readelf -d "$artifact" >"$BUILD/results/$name/readelf-d.txt" || true
    readelf -Ws "$artifact" >"$BUILD/results/$name/readelf-ws.txt" || true
    ldd "$artifact" >"$BUILD/results/$name/ldd.txt" || true
}

record_payload() {
    local name=$1 runtime=$2
    mkdir -p "$BUILD/results/$name"
    du -sb "$runtime" >"$BUILD/results/$name/payload-bytes.txt"
    find "$runtime" -type f | wc -l >"$BUILD/results/$name/file-count.txt"
    find "$runtime" -type f \( -name '*.so' -o -name '*.so.*' \) | wc -l \
        >"$BUILD/results/$name/dso-file-count.txt"
}

build_smoke() {
    local name=$1
    local target="$BUILD/$name/target"
    mkdir -p "$target"
    CARGO_TARGET_DIR="$target" "$CARGO" build --release --manifest-path "$SMOKE/Cargo.toml"
    printf '%s\n' "$target/release/static-searchkernel-smoke"
}

stage_runtime() {
    local name=$1 binary=$2 adapter=$3 with_closure=$4
    local runtime="$BUILD/$name/runtime"
    rm -rf "$runtime"
    mkdir -p "$runtime/.searchkernel-native"
    cp "$binary" "$runtime/static-searchkernel-smoke"
    cp "$adapter" "$runtime/.searchkernel-native/libsearchkernel_vespa.so.2"
    ln "$runtime/.searchkernel-native/libsearchkernel_vespa.so.2" \
        "$runtime/.searchkernel-native/libsearchkernel_vespa.so"
    if command -v patchelf >/dev/null 2>&1; then
        patchelf --force-rpath --set-rpath '$ORIGIN' \
            "$runtime/.searchkernel-native/libsearchkernel_vespa.so.2"
    fi
    if [[ "$with_closure" == 1 ]]; then
        "$PYTHON" "$ROOT/tools/bundle-native-runtime-closure.py" \
            "$runtime/.searchkernel-native" --search-dir "$SDK/lib" \
            --extra-elf "$runtime/.searchkernel-native/libsearchkernel_vespa.so.2"
    fi
    printf '%s\n' "$runtime"
}

stage_executable_runtime() {
    local name=$1 binary=$2 with_closure=$3
    local runtime="$BUILD/$name/runtime"
    rm -rf "$runtime"
    mkdir -p "$runtime/.searchkernel-native"
    cp "$binary" "$runtime/static-searchkernel-smoke"
    if [[ "$with_closure" == 1 ]]; then
        "$PYTHON" "$ROOT/tools/bundle-native-runtime-closure.py" \
            "$runtime/.searchkernel-native" --search-dir "$SDK/lib" \
            --extra-elf "$runtime/static-searchkernel-smoke"
    fi
    printf '%s\n' "$runtime"
}

run_clean() {
    local name=$1 runtime=$2 expected=$3 elf=$4
    local out="$BUILD/results/$name/clean-runtime.txt"
    mkdir -p "$(dirname "$out")"
    if readelf -d "$elf" | grep -Eq '(RPATH|RUNPATH).*(/home/|native-sdk-cache|searchkern2|/work/)'; then
        echo "INVALID: artifact retains a development-machine runtime path" >"$out"
        return 2
    fi
    set +e
    (cd / && clean_env "$runtime/static-searchkernel-smoke") >"$out" 2>&1
    local status=$?
    set -e
    if [[ "$expected" == pass && $status -ne 0 ]]; then
        echo "FAIL (exit=$status)" >>"$out"
        return 1
    fi
    if [[ "$expected" == fail && $status -eq 0 ]]; then
        echo "unexpected PASS" >>"$out"
        return 1
    fi
    if [[ "$expected" == observe ]]; then
        if [[ $status -eq 0 ]]; then
            echo "PASS" >>"$out"
        else
            echo "FAIL (exit=$status)" >>"$out"
        fi
        return 0
    fi
    echo "exit=$status" >>"$out"
}

prepare_static_plan() {
    if [[ -f "$STATIC_PLAN/results/plan.json" ]]; then
        return
    fi

    record_environment
    rm -rf "$STATIC_PLAN"
    mkdir -p "$STATIC_PLAN/results" "$STATIC_PLAN/candidate-b"

    local native="$STATIC_PLAN/baseline-native"
    local builddir="$STATIC_PLAN/baseline-build"
    timed "$STATIC_PLAN/results/baseline-build-time.txt" env \
        VESPA_ROOT="$VESPA" SK_NATIVE_SDK="$native" SK_NATIVE_BUILD_DIR="$builddir" \
        SK_NATIVE_REGENERATE_BINDINGS=0 \
        SK_NATIVE_SKIP_BINDINGS_CHECK=1 SK_NATIVE_SKIP_ABI_AUTHORITIES=1 \
        bash "$ROOT/core/native/vespa/build-sdk.sh"

    local object="$builddir/searchkernel_vespa.o"
    if [[ ! -f "$object" ]]; then
        echo "baseline native build did not leave adapter object: $object" >&2
        return 1
    fi

    set +e
    "$PYTHON" "$SPIKE/scripts/staticize-link.py" \
        --vespa-root "$VESPA" \
        --sdk "$SDK" \
        --object "$object" \
        --shared-output "$STATIC_PLAN/candidate-b/libsearchkernel_vespa.so.2" \
        --version-script "$ROOT/core/native/vespa/searchkernel_vespa.map" \
        --results "$STATIC_PLAN/results"
    local status=$?
    set -e
    printf '%s\n' "$status" >"$STATIC_PLAN/results/candidate-b-link-status.txt"
}

candidate_c() {
    record_environment
    local binary adapter runtime
    binary=$(build_smoke candidate-c)
    adapter="$BUILD/candidate-c/target/release/.searchkernel-native/libsearchkernel_vespa.so.2"
    [[ -f "$adapter" ]] || {
        echo "normal Cargo build did not stage adapter" >&2
        return 1
    }
    runtime=$(stage_runtime candidate-c "$binary" "$adapter" 1)
    capture candidate-c "$runtime/static-searchkernel-smoke"
    capture candidate-c-runtime "$runtime/.searchkernel-native/libsearchkernel_vespa.so.2"
    record_payload candidate-c "$runtime"
    timed "$BUILD/results/candidate-c/smoke-time.txt" \
        env -u LD_LIBRARY_PATH -u SK_NATIVE_SDK -u SK_NATIVE_ABI_HEADER \
        -u SK_NATIVE_CACHE_DIR -u SEARCHKERNEL_PACKAGED_SUPPORT_ROOT \
        -u SEARCHKERNEL_SDK_LIB "$runtime/static-searchkernel-smoke"
    run_clean candidate-c "$runtime" observe \
        "$runtime/.searchkernel-native/libsearchkernel_vespa.so.2" || true
}

candidate_b() {
    prepare_static_plan || return 0
    local adapter="$STATIC_PLAN/candidate-b/libsearchkernel_vespa.so.2"
    mkdir -p "$BUILD/results/candidate-b"
    cp "$STATIC_PLAN/results/"* "$BUILD/results/candidate-b/" 2>/dev/null || true

    if [[ ! -f "$adapter" ]]; then
        echo "Candidate B did not link; inspect build/results/candidate-b/candidate-b-link.log" >&2
        return 1
    fi

    local binary
    binary=$(build_smoke candidate-b-consumer)

    local runtime
    runtime=$(stage_runtime candidate-b-minimal "$binary" "$adapter" 0)
    capture candidate-b "$runtime/.searchkernel-native/libsearchkernel_vespa.so.2"
    record_payload candidate-b-minimal "$runtime"
    run_clean candidate-b-minimal "$runtime" observe \
        "$runtime/.searchkernel-native/libsearchkernel_vespa.so.2" || true

    runtime=$(stage_runtime candidate-b-closure "$binary" "$adapter" 1)
    record_payload candidate-b-closure "$runtime"
    run_clean candidate-b-closure "$runtime" pass \
        "$runtime/.searchkernel-native/libsearchkernel_vespa.so.2" || true
}

make_candidate_a_workspace() {
    local workspace="$BUILD/candidate-a/workspace"
    rm -rf "$workspace"
    mkdir -p "$workspace"
    git -C "$ROOT" archive HEAD | tar -x -C "$workspace"

    printf '%s\n' 'fn main() {}' \
        >"$workspace/core/crates/searchkernel-vespa-bindings/build.rs"
    printf '%s\n' "$workspace"
}

make_linker_shim() {
    local shim="$BUILD/candidate-a/linker.sh"
    mkdir -p "$(dirname "$shim")"
    {
        echo '#!/bin/sh'
        printf 'exec %q %q "$@"\n' "$PYTHON" "$SPIKE/scripts/linker-wrapper.py"
    } >"$shim"
    chmod +x "$shim"
    printf '%s\n' "$shim"
}

candidate_a() {
    prepare_static_plan || return 0
    local plan="$STATIC_PLAN/results/executable-extra-args.json"
    [[ -f "$plan" ]] || {
        echo "Candidate A link plan was not produced" >&2
        return 1
    }

    local workspace shim target host target_key linker_var
    workspace=$(make_candidate_a_workspace)
    shim=$(make_linker_shim)
    target="$BUILD/candidate-a/target"
    host=$("$RUSTC" -vV | sed -n 's/^host: //p')
    target_key=$(printf '%s' "$host" | tr '[:lower:]-' '[:upper:]_')
    linker_var="CARGO_TARGET_${target_key}_LINKER"

    mkdir -p "$BUILD/results/candidate-a" "$target"
    set +e
    env \
        "$linker_var=$shim" \
        STATIC_SK_BASE_LINKER="${CC:-cc}" \
        STATIC_SK_LINK_ARGS_JSON="$plan" \
        STATIC_SK_FINAL_LINK_COMMAND="$BUILD/results/candidate-a/final-link-command.txt" \
        CARGO_TARGET_DIR="$target" \
        "$CARGO" build --release \
        --manifest-path "$workspace/spikes/static-searchkernel/smoke/Cargo.toml" \
        >"$BUILD/results/candidate-a/build.log" 2>&1
    local status=$?
    set -e
    printf '%s\n' "$status" >"$BUILD/results/candidate-a/build-status.txt"
    if [[ $status -ne 0 ]]; then
        echo "Candidate A final executable link failed; inspect build/results/candidate-a/build.log" >&2
        return 1
    fi

    local binary="$target/release/static-searchkernel-smoke"
    [[ -x "$binary" ]] || {
        echo "Candidate A build succeeded but smoke executable is missing" >&2
        return 1
    }
    capture candidate-a "$binary"

    local runtime
    runtime=$(stage_executable_runtime candidate-a-minimal "$binary" 0)
    record_payload candidate-a-minimal "$runtime"
    run_clean candidate-a-minimal "$runtime" observe "$runtime/static-searchkernel-smoke" || true

    runtime=$(stage_executable_runtime candidate-a-closure "$binary" 1)
    record_payload candidate-a-closure "$runtime"
    run_clean candidate-a-closure "$runtime" pass "$runtime/static-searchkernel-smoke" || true

    set +e
    env \
        "$linker_var=$shim" \
        STATIC_SK_BASE_LINKER="${CC:-cc}" \
        STATIC_SK_LINK_ARGS_JSON="$plan" \
        CARGO_TARGET_DIR="$BUILD/candidate-a/target-tests" \
        "$CARGO" test --release --no-run \
        --manifest-path "$workspace/spikes/static-searchkernel/smoke/Cargo.toml" \
        >"$BUILD/results/candidate-a/second-target.log" 2>&1
    status=$?
    set -e
    printf '%s\n' "$status" >"$BUILD/results/candidate-a/second-target-status.txt"
    find "$BUILD/candidate-a/target-tests/release" -maxdepth 2 -type f -executable \
        -name 'static_searchkernel_smoke-*' -printf '%p %s\n' 2>/dev/null \
        >"$BUILD/results/candidate-a/second-target-sizes.txt" || true
}

case "${1:-}" in
    candidate-a) candidate_a ;;
    candidate-b) candidate_b ;;
    candidate-c) candidate_c ;;
    all)
        rm -rf "$BUILD"
        candidate_c || true
        candidate_b || true
        candidate_a || true
        ;;
    clean) rm -rf "$BUILD" ;;
    *) usage ;;
esac
