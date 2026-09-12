#!/usr/bin/env bash
set -euo pipefail

ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
BUILD="${SK_FFI_SANITIZER_DIR:-$ROOT/target/ffi-sanitizer}"
REFERENCE="$ROOT/reference/ffi-conformance"

cd "$ROOT"
mkdir -p "$BUILD"

# Rust's sanitizer support is currently exposed as an unstable compiler
# option. Prefer an installed nightly toolchain, but keep this usable on the
# stable-only development images used by the repository by enabling that
# compiler option through RUSTC_BOOTSTRAP.
if command -v rustup >/dev/null 2>&1 && rustup run nightly rustc --version >/dev/null 2>&1; then
    RUST_CARGO=(cargo +nightly)
    RUST_ENV=()
else
    RUST_CARGO=(cargo)
    RUST_ENV=(RUSTC_BOOTSTRAP=1)
fi

run_cargo() {
    local target_dir="$1"
    local rustflags="$2"
    shift 2
    env "${RUST_ENV[@]}" \
        RUSTFLAGS="$rustflags" \
        CARGO_TARGET_DIR="$target_dir" \
        "${RUST_CARGO[@]}" "$@"
}

build_ffi() {
    local target_dir="$1"
    local rustflags="$2"
    run_cargo "$target_dir" "$rustflags" \
        build -p searchkernel-ffi --no-default-features
}

run_callers() {
    local name="$1"
    local target_dir="$2"
    local compiler_sanitizers="$3"
    local caller_dir="$BUILD/$name"
    local library_dir="$target_dir/debug"

    mkdir -p "$caller_dir"

    cc -std=c11 -O1 -g -Wall -Wextra -Werror -pedantic \
        -fno-omit-frame-pointer "$compiler_sanitizers" \
        -I"$REFERENCE/include" \
        "$REFERENCE/c/abi_smoke.c" \
        -L"$library_dir" -lsearchkernel_ffi \
        -Wl,-rpath,"$library_dir" \
        -o "$caller_dir/abi-smoke"

    c++ -std=c++17 -O1 -g -Wall -Wextra -Werror -pedantic \
        -fno-omit-frame-pointer "$compiler_sanitizers" \
        -I"$REFERENCE/include" \
        "$REFERENCE/cpp/header_smoke.cc" \
        -L"$library_dir" -lsearchkernel_ffi \
        -Wl,-rpath,"$library_dir" \
        -o "$caller_dir/header-smoke"

    ASAN_OPTIONS=detect_leaks=1:halt_on_error=1:abort_on_error=1 \
        UBSAN_OPTIONS=halt_on_error=1:print_stacktrace=1 \
        "$caller_dir/abi-smoke"
    ASAN_OPTIONS=detect_leaks=1:halt_on_error=1:abort_on_error=1 \
        UBSAN_OPTIONS=halt_on_error=1:print_stacktrace=1 \
        "$caller_dir/header-smoke"
}

ADDRESS_TARGET="$BUILD/cargo-address"
build_ffi "$ADDRESS_TARGET" "-Zsanitizer=address"
run_callers address "$ADDRESS_TARGET" "-fsanitize=address,undefined"

# The Rust compiler has no UBSan sanitizer mode. Keep the Rust library
# uninstrumented for this pass and exercise the complete exported C ABI from
# UBSan-instrumented C and C++ callers.
UNDEFINED_TARGET="$BUILD/cargo-undefined"
build_ffi "$UNDEFINED_TARGET" ""
run_callers undefined "$UNDEFINED_TARGET" "-fsanitize=undefined"

run_cargo "$ADDRESS_TARGET" "-Zsanitizer=address" \
    test --manifest-path "$REFERENCE/rust/Cargo.toml" --all-targets --all-features

echo "FFI ASan/UBSan PASS"
