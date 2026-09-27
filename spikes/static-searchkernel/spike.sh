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
SDK=$(bash "$ROOT/tools/resolve-native-sdk.sh" path)
VESPA=$(bash "$ROOT/tools/resolve-native-sdk.sh" vespa-root)

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
        echo "target=$(bash "$ROOT/tools/resolve-rust.sh" rustc -vV | sed -n 's/^host: //p')"
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
    ldd "$artifact" >"$BUILD/results/$name/ldd.txt" || true
}

build_smoke() {
    local name=$1 rustflags=${2:-}
    local target="$BUILD/$name/target"
    mkdir -p "$target"
    if [[ -n "$rustflags" ]]; then
        CARGO_TARGET_DIR="$target" RUSTFLAGS="$rustflags" "$CARGO" build --release --manifest-path "$SMOKE/Cargo.toml" || return
    else
        CARGO_TARGET_DIR="$target" "$CARGO" build --release --manifest-path "$SMOKE/Cargo.toml" || return
    fi
    printf '%s\n' "$target/release/static-searchkernel-smoke"
}

stage_runtime() {
    local name=$1 binary=$2 adapter=$3 with_closure=$4
    local runtime="$BUILD/$name/runtime"
    rm -rf "$runtime"
    mkdir -p "$runtime/.searchkernel-native"
    cp "$binary" "$runtime/static-searchkernel-smoke"
    cp "$adapter" "$runtime/.searchkernel-native/libsearchkernel_vespa.so.2"
    ln "$runtime/.searchkernel-native/libsearchkernel_vespa.so.2" "$runtime/.searchkernel-native/libsearchkernel_vespa.so"
    if command -v patchelf >/dev/null 2>&1; then
        patchelf --force-rpath --set-rpath '$ORIGIN' "$runtime/.searchkernel-native/libsearchkernel_vespa.so.2"
    fi
    if [[ "$with_closure" == 1 ]]; then
        "$PYTHON" "$ROOT/tools/bundle-native-runtime-closure.py" \
            "$runtime/.searchkernel-native" --search-dir "$SDK/lib"
    fi
    printf '%s\n' "$runtime"
}

run_clean() {
    local name=$1 runtime=$2 expected=$3
    local out="$BUILD/results/$name/clean-runtime.txt"
    if readelf -d "$runtime/.searchkernel-native/libsearchkernel_vespa.so.2" | grep -Eq '(RPATH|RUNPATH).*(/home/|native-sdk-cache|searchkern2)'; then
        echo "INVALID: adapter retains a development-machine runtime path" >"$out"
        return 2
    fi
    set +e
    (cd / && clean_env "$runtime/static-searchkernel-smoke") >"$out" 2>&1
    local status=$?
    set -e
    if [[ "$expected" == pass && $status -ne 0 ]]; then
        echo "FAIL (exit=$status)" >>"$out"; return 1
    fi
    if [[ "$expected" == fail && $status -eq 0 ]]; then
        echo "unexpected PASS" >>"$out"; return 1
    fi
    echo "exit=$status" >>"$out"
}

candidate_c() {
    record_environment
    local binary adapter runtime
    binary=$(build_smoke candidate-c)
    adapter="$BUILD/candidate-c/target/release/.searchkernel-native/libsearchkernel_vespa.so.2"
    [[ -f "$adapter" ]] || { echo "normal Cargo build did not stage adapter" >&2; exit 1; }
    runtime=$(stage_runtime candidate-c "$binary" "$adapter" 1)
    capture candidate-c "$runtime/static-searchkernel-smoke"
    capture candidate-c-runtime "$runtime/.searchkernel-native/libsearchkernel_vespa.so.2"
    timed "$BUILD/results/candidate-c/smoke-time.txt" \
        env -u LD_LIBRARY_PATH -u SK_NATIVE_SDK -u SK_NATIVE_ABI_HEADER \
        -u SK_NATIVE_CACHE_DIR -u SEARCHKERNEL_PACKAGED_SUPPORT_ROOT \
        -u SEARCHKERNEL_SDK_LIB "$runtime/static-searchkernel-smoke"
    run_clean candidate-c "$runtime" pass || true
}

candidate_b() {
    record_environment
    local native="$BUILD/candidate-b/native" builddir="$BUILD/candidate-b/native-build"
    mkdir -p "$BUILD/candidate-b" "$BUILD/results/candidate-b"
    timed "$BUILD/results/candidate-b/link-time.txt" env \
        VESPA_ROOT="$VESPA" SK_NATIVE_SDK="$native" SK_NATIVE_BUILD_DIR="$builddir" \
        SK_NATIVE_REGENERATE_BINDINGS=0 \
        SK_NATIVE_SKIP_BINDINGS_CHECK=1 SK_NATIVE_SKIP_ABI_AUTHORITIES=1 \
        bash "$ROOT/core/native/vespa/build-sdk.sh"
    local binary="$BUILD/candidate-c/target/release/static-searchkernel-smoke"
    [[ -x "$binary" ]] || binary=$(build_smoke candidate-b)
    local adapter="$native/lib/libsearchkernel_vespa.so.2" runtime
    runtime=$(stage_runtime candidate-b "$binary" "$adapter" 0)
    capture candidate-b "$runtime/.searchkernel-native/libsearchkernel_vespa.so.2"
    # A one-library distribution is the actual Candidate B proposition. It is
    # expected to fail if the candidate still needs Vespa/private DSOs.
    run_clean candidate-b "$runtime" fail || true
    runtime=$(stage_runtime candidate-b-closure "$binary" "$adapter" 1)
    capture candidate-b-closure "$runtime/.searchkernel-native/libsearchkernel_vespa.so.2"
    run_clean candidate-b-closure "$runtime" pass
}

candidate_a() {
    record_environment
    # Current public bindings explicitly request dylib=searchkernel_vespa.
    # This command is an executable-level static-link attempt against the
    # supported consumer path, not a production binding rewrite.
    local binary
    set +e
    binary=$(build_smoke candidate-a '-C target-feature=+crt-static')
    local status=$?
    set -e
    mkdir -p "$BUILD/results/candidate-a"
    if [[ $status -ne 0 || ! -x "${binary:-}" ]]; then
        echo "static Rust executable link failed before smoke; see Cargo output above" >"$BUILD/results/candidate-a/result.txt"
        return 0
    fi
    capture candidate-a "$binary"
    readelf -d "$binary" | grep -F 'libsearchkernel_vespa' >"$BUILD/results/candidate-a/adapter-needed.txt" || true
    # A second independently linked target quantifies whether the native
    # closure is duplicated once executable-level absorption becomes possible.
    CARGO_TARGET_DIR="$BUILD/candidate-a/target-2" RUSTFLAGS='-C target-feature=+crt-static' \
        "$CARGO" test --release --manifest-path "$SMOKE/Cargo.toml" --no-run \
        >"$BUILD/results/candidate-a/second-target.txt" 2>&1 || true
}

case "${1:-}" in
    candidate-a) candidate_a ;;
    candidate-b) candidate_b ;;
    candidate-c) candidate_c ;;
    all) candidate_c; candidate_a; candidate_b ;;
    clean) rm -rf "$BUILD" ;;
    *) usage ;;
esac
