#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)

normalize_runtime_dirs() {
    local raw=$1
    local dir resolved joined=
    local adapter_found=0
    local -a dirs

    IFS=: read -r -a dirs <<<"$raw"
    (("${#dirs[@]}" > 0)) || {
        echo "native runtime library path is empty" >&2
        return 1
    }

    for dir in "${dirs[@]}"; do
        [[ -n "$dir" ]] || {
            echo "native runtime library path contains an empty element" >&2
            return 1
        }
        [[ -d "$dir" ]] || {
            echo "native runtime library directory does not exist: $dir" >&2
            return 1
        }
        resolved=$(cd -- "$dir" && pwd -P)
        if [[ -e "$resolved/libsearchkernel_vespa.so.2" ]]; then
            adapter_found=1
        fi
        if [[ -z "$joined" ]]; then
            joined=$resolved
        else
            joined="$joined:$resolved"
        fi
    done

    [[ "$adapter_found" == 1 ]] || {
        echo "native runtime closure does not contain libsearchkernel_vespa.so.2: $joined" >&2
        return 1
    }
    printf '%s\n' "$joined"
}

runtime_dirs_from_sdk_pair() {
    local adapter_sdk=$1
    local base_sdk=$2
    normalize_runtime_dirs "$adapter_sdk/lib:$base_sdk/lib"
}

if [[ -n "${SEARCHKERNEL_RUNTIME_LIB_DIRS:-}" ]]; then
    normalize_runtime_dirs "$SEARCHKERNEL_RUNTIME_LIB_DIRS"
    exit 0
fi

if [[ -n "${SEARCHKERNEL_VERIFIER_SDK:-}" ]]; then
    base_sdk=${SEARCHKERNEL_VERIFIER_BASE_SDK:-}
    if [[ -z "$base_sdk" ]]; then
        base_sdk=$(bash "$ROOT/tools/resolve-native-sdk.sh" path)
    fi
    runtime_dirs_from_sdk_pair "$SEARCHKERNEL_VERIFIER_SDK" "$base_sdk"
    exit 0
fi

if [[ -n "${SEARCHKERNEL_SDK_LIB:-}" ]]; then
    base_sdk=$(bash "$ROOT/tools/resolve-native-sdk.sh" path)
    normalize_runtime_dirs "$SEARCHKERNEL_SDK_LIB:$base_sdk/lib"
    exit 0
fi

if [[ -n "${SK_NATIVE_SDK:-}" ]]; then
    sdk=$(cd -- "$SK_NATIVE_SDK" && pwd -P)
    if [[ -e "$sdk/lib/libvespalib.so" ]]; then
        normalize_runtime_dirs "$sdk/lib"
    else
        base_sdk=${SEARCHKERNEL_RUNTIME_BASE_SDK:-$(bash "$ROOT/tools/resolve-native-sdk.sh" path)}
        runtime_dirs_from_sdk_pair "$sdk" "$base_sdk"
    fi
    exit 0
fi

target_root=${CARGO_TARGET_DIR:-"$ROOT/target/${SEARCHKERNEL_BUILD_ENV:-host}"}
case "$target_root" in
    /*) ;;
    *) target_root="$ROOT/$target_root" ;;
esac

base_sdk=$(bash "$ROOT/tools/resolve-native-sdk.sh" ensure)
vespa_root=$(bash "$ROOT/tools/resolve-native-sdk.sh" vespa-root)
runtime_sdk=${SEARCHKERNEL_RUNTIME_SDK:-"$target_root/native-runtime-sdk"}
runtime_build=${SEARCHKERNEL_RUNTIME_SDK_BUILD:-"$target_root/native-runtime-sdk-build"}

mkdir -p "$target_root"
command -v flock >/dev/null 2>&1 || {
    echo "canonical native runtime preparation requires flock" >&2
    exit 1
}
exec 9>"$target_root/native-runtime-sdk.lock"
flock -w 300 9 || {
    echo "timed out waiting for canonical native runtime preparation" >&2
    exit 1
}

SK_NATIVE_SDK="$runtime_sdk" \
SK_NATIVE_BUILD_DIR="$runtime_build" \
SK_NATIVE_BASE_SDK="$base_sdk" \
SK_NATIVE_ADAPTER_ONLY=1 \
SK_NATIVE_REGENERATE_BINDINGS=0 \
SK_NATIVE_SKIP_BINDINGS_CHECK=1 \
VESPA_ROOT="$vespa_root" \
    bash "$ROOT/core/native/vespa/build-sdk.sh" >/dev/null

runtime_dirs_from_sdk_pair "$runtime_sdk" "$base_sdk"
