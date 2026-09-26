#!/usr/bin/env bash
# Shared runtime setup for real client E2E lanes.
#
# Source this file, then call prepare_client_e2e_runtime [prevalidated-sdk].
# The function resolves the exact Cargo-built native adapter from Cargo build
# metadata and exports the daemon and runtime library paths consumed by clients.

CLIENT_E2E_RUNTIME_ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)

prepare_client_e2e_runtime() {
    local prevalidated_native_sdk=${1:-0}
    local root cargo python target_root host adapter_lib cargo_messages

    root=$CLIENT_E2E_RUNTIME_ROOT

    target_root=${CARGO_TARGET_DIR:-"$root/target/host"}
    case "$target_root" in
        /*) ;;
        *) target_root="$root/$target_root" ;;
    esac

    host=${SEARCHKERNEL_LOCAL_HOST:-"$target_root/debug/searchkerneld"}
    adapter_lib=${SEARCHKERNEL_SDK_LIB:-}

    if [[ -x "$host" &&
          -n "$adapter_lib" &&
          -f "$adapter_lib/libsearchkernel_vespa.so.2" ]]; then
        export SEARCHKERNEL_LOCAL_HOST="$host"
        export SEARCHKERNEL_SDK_LIB="$adapter_lib"
        export LD_LIBRARY_PATH="$adapter_lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
        return 0
    fi

    cargo=$(bash "$root/tools/resolve-rust.sh" cargo)
    python=$(bash "$root/tools/resolve-python.sh")

    if [[ "$prevalidated_native_sdk" != 1 ]]; then
        "$root/dev" native-sdk ensure
    fi

    cargo_messages=$(mktemp "${TMPDIR:-/tmp}/searchkernel-client-e2e-cargo.XXXXXX")
    if ! "$cargo" build -p searchkerneld --features native \
        --message-format=json-render-diagnostics | tee "$cargo_messages"; then
        rm -f -- "$cargo_messages"
        return 1
    fi

    if ! adapter_lib=$(
        "$python" "$root/.github/ci/resolve-native-adapter-lib.py" \
            "$cargo_messages" "$target_root"
    ); then
        rm -f -- "$cargo_messages"
        return 1
    fi
    rm -f -- "$cargo_messages"

    host=${SEARCHKERNEL_LOCAL_HOST:-"$target_root/debug/searchkerneld"}
    [[ -x "$host" ]] || {
        echo "client E2E host binary missing after build: $host" >&2
        return 1
    }
    [[ -f "$adapter_lib/libsearchkernel_vespa.so.2" ]] || {
        echo "client E2E native adapter runtime missing: $adapter_lib/libsearchkernel_vespa.so.2" >&2
        return 1
    }

    export SEARCHKERNEL_LOCAL_HOST="$host"
    export SEARCHKERNEL_SDK_LIB="$adapter_lib"
    export LD_LIBRARY_PATH="$adapter_lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
}
