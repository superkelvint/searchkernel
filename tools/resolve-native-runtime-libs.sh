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

runtime_dirs_from_adapter_lib() {
    local adapter_lib=$1
    local base_sdk=$2
    local adapter_dir

    [[ -e "$adapter_lib" || -L "$adapter_lib" ]] || {
        echo "native runtime adapter does not exist: $adapter_lib" >&2
        return 1
    }
    adapter_dir=$(cd -- "$(dirname -- "$adapter_lib")" && pwd -P)
    normalize_runtime_dirs "$adapter_dir:$base_sdk/lib"
}

runtime_dirs_from_executable() {
    local executable=$1
    local origin encoded entry raw= normalized
    local -a entries

    [[ -n "$executable" && -e "$executable" ]] || return 2
    command -v readelf >/dev/null 2>&1 || return 2
    readelf -h "$executable" >/dev/null 2>&1 || return 2

    origin=$(cd -- "$(dirname -- "$executable")" && pwd -P)
    while IFS= read -r encoded; do
        [[ -n "$encoded" ]] || continue
        IFS=: read -r -a entries <<<"$encoded"
        for entry in "${entries[@]}"; do
            entry=${entry//\$ORIGIN/$origin}
            entry=${entry//\${ORIGIN}/$origin}
            [[ -n "$entry" ]] || continue
            if [[ -z "$raw" ]]; then
                raw=$entry
            else
                raw="$raw:$entry"
            fi
        done
    done < <(
        readelf -d "$executable" 2>/dev/null |
            sed -n 's/.*Library \(rpath\|runpath\): \[\(.*\)\]/\2/p'
    )

    [[ -n "$raw" ]] || return 2
    if normalized=$(normalize_runtime_dirs "$raw" 2>/dev/null); then
        printf '%s\n' "$normalized"
        return 0
    fi
    return 2
}

if [[ -n "${SEARCHKERNEL_RUNTIME_LIB_DIRS:-}" ]]; then
    normalize_runtime_dirs "$SEARCHKERNEL_RUNTIME_LIB_DIRS"
    exit 0
fi

executable=${1:-}

if [[ -n "$executable" ]]; then
    if runtime_from_executable=$(runtime_dirs_from_executable "$executable"); then
        printf '%s\n' "$runtime_from_executable"
        exit 0
    fi
fi

adapter_lib=${SEARCHKERNEL_RUNTIME_ADAPTER_LIB:-${NATIVE_ADAPTER_LIB:-}}
if [[ -n "$adapter_lib" ]]; then
    base_sdk=${SEARCHKERNEL_RUNTIME_BASE_SDK:-}
    if [[ -z "$base_sdk" ]]; then
        base_sdk=$(bash "$ROOT/tools/resolve-native-sdk.sh" path) || {
            echo "exact native adapter is known but its pinned runtime base is unavailable: $adapter_lib" >&2
            exit 1
        }
    fi
    runtime_dirs_from_adapter_lib "$adapter_lib" "$base_sdk"
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

if [[ -n "$executable" ]]; then
    echo "cannot determine exact native runtime adapter for $executable" >&2
else
    echo "cannot determine exact native runtime adapter" >&2
fi
echo "provide SEARCHKERNEL_RUNTIME_LIB_DIRS, SEARCHKERNEL_RUNTIME_ADAPTER_LIB (or NATIVE_ADAPTER_LIB), SEARCHKERNEL_VERIFIER_SDK, or SK_NATIVE_SDK" >&2
echo "the runtime launcher will not synthesize a replacement adapter at exec time" >&2
exit 1
