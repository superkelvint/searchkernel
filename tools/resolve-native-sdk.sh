#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
TARGET=${SK_NATIVE_TARGET:-$(rustc -vV | sed -n 's/^host: //p')}
BUILD_ENV=${SEARCHKERNEL_BUILD_ENV:-host}
MODE=${1:-ensure}
unset VESPA_LIBRARY_ROOT

case "$MODE" in
    ensure|path|vespa-root) ;;
    *) echo "usage: tools/resolve-native-sdk.sh [ensure|path|vespa-root]" >&2; exit 2 ;;
esac

[[ -n "$TARGET" ]] || { echo "could not determine Rust target; set SK_NATIVE_TARGET" >&2; exit 1; }

PINNED_VESPA_COMMIT=$(python3 - "$ROOT/core/native/vespa/manifest.json" <<'PY'
import json
import sys
with open(sys.argv[1], encoding="utf-8") as stream:
    print(json.load(stream)["vespa_commit"])
PY
)
[[ "$PINNED_VESPA_COMMIT" =~ ^[0-9a-fA-F]{40}$ || "$PINNED_VESPA_COMMIT" == fixture-* ]] || {
    echo "invalid pinned Vespa checkout hash '$PINNED_VESPA_COMMIT'" >&2
    exit 1
}

MACHINE_CACHE_HOME=${XDG_CACHE_HOME:-${HOME:?HOME must be set when XDG_CACHE_HOME is unset}/.cache}
CACHE_BASE=${SK_NATIVE_CACHE_DIR:-$MACHINE_CACHE_HOME/searchkernel/native-sdk-cache}
# Share the prerequisite across independent clones, but never across an
# incompatible declared build environment or native target. The pinned Vespa
# checkout remains the immutable object key inside this compatibility namespace.
CACHE_ROOT=$CACHE_BASE/$BUILD_ENV/$TARGET
OBJECT=$CACHE_ROOT/objects/$PINNED_VESPA_COMMIT
CHECKOUT_LINK=$CACHE_ROOT/checkouts/$PINNED_VESPA_COMMIT

configured_marker() {
    local vespa_root=$1
    [[ -f "$vespa_root/searchcore/src/tests/proton/documentdb/CMakeFiles/searchcore_embedded_crud_spike_app.dir/link.txt" &&
       -f "$vespa_root/searchcore/src/tests/proton/documentdb/CMakeFiles/searchcore_embedded_crud_spike_app.dir/flags.make" ]]
}

has_vespa_libraries() {
    local root=$1
    find "$root" -type f -name 'libvespa*.so*' -print -quit 2>/dev/null | grep -q .
}

PUBLISHED_BASE_RESOLVED=0
PUBLISHED_BASE_GENERATION=
PUBLISHED_BASE_URL=

published_toolchain_marker_path() {
    printf '%s\n' "$CACHE_ROOT/toolchains/$PINNED_VESPA_COMMIT/upstream/vespa/.searchkernel-vespa-artifact.json"
}

has_published_install() {
    [[ -f "$(published_toolchain_marker_path)" ]]
}

artifact_generation_from_url() {
    local url=$1
    python3 - "$url" <<'PY'
import re
import sys
from urllib.parse import urlparse

name = urlparse(sys.argv[1]).path.rsplit("/", 1)[-1]
match = re.match(r"^searchkernel-vespa-base-v([0-9]+)-.+\.tar\.gz$", name)
if not match:
    raise SystemExit(
        "SK_NATIVE_BASE_ARTIFACT_URL must use a canonical generation-bearing "
        "asset name, or SK_NATIVE_BASE_ARTIFACT_GENERATION must be set"
    )
print(match.group(1))
PY
}

latest_published_generation() {
    python3 "$ROOT/tools/native-base-release-selection.py" \
        "$BUILD_ENV" "$TARGET" "$PINNED_VESPA_COMMIT"
}
resolve_published_base_identity() {
    [[ "$PUBLISHED_BASE_RESOLVED" == 0 ]] || return 0

    local generation tag asset

    if [[ -n "${SK_NATIVE_BASE_ARTIFACT_URL:-}" ]]; then
        if [[ -n "${SK_NATIVE_BASE_ARTIFACT_GENERATION:-}" ]]; then
            generation=$SK_NATIVE_BASE_ARTIFACT_GENERATION
            [[ "$generation" =~ ^[1-9][0-9]*$ ]] || {
                echo "invalid SK_NATIVE_BASE_ARTIFACT_GENERATION '$generation'" >&2
                return 1
            }
        else
            generation=$(artifact_generation_from_url "$SK_NATIVE_BASE_ARTIFACT_URL")
        fi
        PUBLISHED_BASE_GENERATION=$generation
        PUBLISHED_BASE_URL=$SK_NATIVE_BASE_ARTIFACT_URL
        PUBLISHED_BASE_RESOLVED=1
        return 0
    fi

    generation=$(latest_published_generation)
    tag="vespa-base-v${generation}-${BUILD_ENV}-${TARGET}-${PINNED_VESPA_COMMIT}"
    asset="searchkernel-vespa-base-v${generation}-${BUILD_ENV}-${TARGET}-${PINNED_VESPA_COMMIT}.tar.gz"
    PUBLISHED_BASE_GENERATION=$generation
    PUBLISHED_BASE_URL="https://github.com/superkelvint/searchkernel/releases/download/${tag}/${asset}"
    PUBLISHED_BASE_RESOLVED=1
}

installed_published_generation() {
    local marker
    marker=$(published_toolchain_marker_path)
    [[ -f "$marker" ]] || return 1
    python3 - "$marker" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as stream:
    marker = json.load(stream)
generation = marker.get("artifact_generation")
if generation is None:
    # Migration compatibility for v1/v2 artifacts published before generation
    # became explicit metadata. Their format version matched their release
    # generation.
    generation = marker.get("format_version")
if not isinstance(generation, int) or generation < 1:
    raise SystemExit("published native-base marker has no valid artifact generation")
print(generation)
PY
}

published_install_is_current() {
    local installed
    has_published_install || return 1
    resolve_published_base_identity || return 1
    installed=$(installed_published_generation) || return 1
    (( installed >= PUBLISHED_BASE_GENERATION ))
}

verify_published_toolchain_marker() {
    local vespa_root=$1
    local marker="$vespa_root/.searchkernel-vespa-artifact.json"
    [[ -f "$marker" ]] || return 1
    python3 - "$marker" "$BUILD_ENV" "$TARGET" "$PINNED_VESPA_COMMIT" <<'PY'
import json
import sys

path, build_env, target, commit = sys.argv[1:]
try:
    with open(path, encoding="utf-8") as stream:
        marker = json.load(stream)
except (OSError, json.JSONDecodeError) as exc:
    raise SystemExit(f"invalid published Vespa toolchain marker {path}: {exc}") from exc
expected = {
    "build_environment": build_env,
    "native_target": target,
    "vespa_commit": commit,
}
for key, value in expected.items():
    if marker.get(key) != value:
        raise SystemExit(
            f"published Vespa toolchain {key} mismatch: expected {value!r}, found {marker.get(key)!r}"
        )

# Installed cache markers may come from an older artifact schema. At this
# stage only provenance identity is authoritative; generation freshness is
# evaluated separately and newly downloaded artifacts are validated by the
# installer before publication into the cache.
PY
}

build_configured_vespa_libraries() {
    local vespa_root=$1
    local jobs=1
    local target=searchcore_embedded_crud_spike_app

    [[ -f "$vespa_root/CMakeCache.txt" ]] || return 1
    command -v cmake >/dev/null 2>&1 || {
        echo "configured Vespa build at '$vespa_root' is missing shared libraries and cmake is unavailable" >&2
        return 1
    }

    if command -v nproc >/dev/null 2>&1; then
        jobs=$(nproc)
    fi

    echo "Vespa shared libraries are missing; building configured target '$target' once under $vespa_root" >&2
    cmake --build "$vespa_root" --target "$target" --parallel "$jobs" || {
        echo "failed to build configured Vespa target '$target' under '$vespa_root'" >&2
        return 1
    }

    if ! has_vespa_libraries "$vespa_root"; then
        echo "configured Vespa target '$target' completed without producing libvespa*.so* below '$vespa_root'" >&2
        return 1
    fi
}

fingerprint_field() {
    local file=$1
    local field=$2
    awk -F '\t' -v wanted="$field" '$1 == wanted { print $2; exit }' "$file"
}

compatible_full_sdk() {
    local sdk=$1
    local pinned_commit=$2
    local manifest="$sdk/manifest.json"
    local fingerprint="$sdk/native-build-fingerprint-v2.txt"

    [[ -f "$manifest" && -f "$fingerprint" && -d "$sdk/lib" ]] || return 1
    grep -Eq '"kind"[[:space:]]*:[[:space:]]*"vespa-base"' "$manifest" && return 1
    grep -Eq '"vespa_commit"[[:space:]]*:[[:space:]]*"'"$pinned_commit"'"' "$manifest" || return 1
    grep -q '^fingerprint-version=3$' "$fingerprint" || return 1
    [[ "$(fingerprint_field "$fingerprint" fingerprint.scope)" == full ]] || return 1
    [[ "$(fingerprint_field "$fingerprint" build.environment)" == "$BUILD_ENV" ]] || return 1
    [[ "$(fingerprint_field "$fingerprint" cargo.target)" == "$TARGET" ]] || return 1
    has_vespa_libraries "$sdk/lib" || return 1
}
resolve_prebuilt_vespa_library_root() {
    local vespa_root=$1
    local pinned_commit=$2
    local worktree_root candidate worktree

    if has_vespa_libraries "$vespa_root"; then
        printf '%s\n' "$vespa_root"
        return
    fi

    case "$vespa_root" in
        */upstream/vespa)
            worktree_root=$(cd -- "$vespa_root/../.." && pwd)
            for candidate in \
                "$worktree_root/core/native/sdk/linux-x86_64" \
                "$worktree_root/target/host/native-sdk/linux-x86_64" \
                "$worktree_root/target/native-sdk-package"; do
                if compatible_full_sdk "$candidate" "$pinned_commit"; then
                    echo "using Vespa shared libraries from compatible full SDK: $candidate/lib" >&2
                    printf '%s\n' "$candidate/lib"
                    return
                fi
            done
            ;;
    esac

    while IFS= read -r worktree; do
        for candidate in \
            "$worktree/core/native/sdk/linux-x86_64" \
            "$worktree/target/host/native-sdk/linux-x86_64" \
            "$worktree/target/native-sdk-package"; do
            if compatible_full_sdk "$candidate" "$pinned_commit"; then
                echo "using Vespa shared libraries from compatible sibling full SDK: $candidate/lib" >&2
                printf '%s\n' "$candidate/lib"
                return
            fi
        done
    done < <(git -C "$ROOT" worktree list --porcelain | sed -n 's/^worktree //p')
    return 1
}

resolve_vespa_library_root() {
    local vespa_root=$1
    local pinned_commit=$2
    local prebuilt

    if prebuilt=$(resolve_prebuilt_vespa_library_root "$vespa_root" "$pinned_commit"); then
        printf '%s\n' "$prebuilt"
        return
    fi

    if build_configured_vespa_libraries "$vespa_root"; then
        printf '%s\n' "$vespa_root"
        return
    fi

    echo "configured Vespa metadata was found at '$vespa_root', but its shared libraries are missing" >&2
    echo "no compatible full SDK was available and the configured Vespa target could not be built" >&2
    echo "repair the configured upstream/vespa build, then rerun './dev native-sdk ensure'" >&2
    return 1
}
vespa_identity() {
    local worktree=$1
    local vespa_root="$worktree/upstream/vespa"
    local top

    top=$(git -C "$vespa_root" rev-parse --show-toplevel 2>/dev/null || true)
    if [[ -n "$top" && "$(cd -- "$top" && pwd)" == "$(cd -- "$vespa_root" && pwd)" ]]; then
        git -C "$vespa_root" rev-parse HEAD
    else
        # upstream/vespa may be an ordinary tracked directory rather than its
        # own checkout. Compare that tree, not the parent worktree HEAD, so
        # unrelated Rust/API commits do not prevent safe SDK reuse.
        git -C "$worktree" rev-parse HEAD:upstream/vespa 2>/dev/null || true
    fi
}

verify_pinned_vespa_checkout() {
    local vespa_root=$1
    local top actual

    if [[ -f "$vespa_root/.searchkernel-vespa-artifact.json" ]]; then
        verify_published_toolchain_marker "$vespa_root"
        return
    fi

    top=$(git -C "$vespa_root" rev-parse --show-toplevel 2>/dev/null || true)
    if [[ -z "$top" ]]; then
        echo "configured Vespa toolchain has no Git or published-artifact provenance: $vespa_root" >&2
        return 1
    fi
    if [[ "$(cd -- "$top" && pwd)" != "$(cd -- "$vespa_root" && pwd)" ]]; then
        # Test/legacy layouts may track upstream/vespa inside another repository.
        # Production standalone Vespa checkouts are verified by their Git HEAD.
        return 0
    fi

    actual=$(git -C "$vespa_root" rev-parse HEAD)
    if [[ "$actual" != "$PINNED_VESPA_COMMIT" ]]; then
        echo "configured Vespa checkout is $actual, expected pinned checkout $PINNED_VESPA_COMMIT" >&2
        return 1
    fi
    if ! git -C "$vespa_root" diff --quiet --ignore-submodules -- ||
       ! git -C "$vespa_root" diff --cached --quiet --ignore-submodules --; then
        echo "configured Vespa checkout has modified tracked source at pinned checkout $PINNED_VESPA_COMMIT" >&2
        return 1
    fi
}
registered_vespa_root() {
    [[ -L "$CHECKOUT_LINK" || -d "$CHECKOUT_LINK" ]] || return 1
    local candidate
    candidate=$(readlink -f "$CHECKOUT_LINK" 2>/dev/null || true)
    [[ -n "$candidate" && -d "$candidate" ]] || return 1
    configured_marker "$candidate" || return 1
    verify_pinned_vespa_checkout "$candidate" || return 1
    printf '%s\n' "$candidate"
}

register_vespa_root() {
    local vespa_root=$1
    local resolved existing tmp
    resolved=$(cd -- "$vespa_root" && pwd -P)
    mkdir -p "$CACHE_ROOT/checkouts"

    existing=$(registered_vespa_root 2>/dev/null || true)
    if [[ "$existing" == "$resolved" ]]; then
        return
    fi

    tmp="$CACHE_ROOT/checkouts/.tmp-$PINNED_VESPA_COMMIT-$BASHPID"
    rm -f "$tmp"
    ln -s "$resolved" "$tmp"
    mv -Tf "$tmp" "$CHECKOUT_LINK"
}

resolve_vespa_root() {
    if [[ -n "${VESPA_ROOT:-}" ]]; then
        configured_marker "$VESPA_ROOT" || {
            echo "VESPA_ROOT '$VESPA_ROOT' does not contain configured Vespa build outputs" >&2
            return 1
        }
        printf '%s\n' "$VESPA_ROOT"
        return
    fi

    if configured_marker "$ROOT/upstream/vespa"; then
        printf '%s\n' "$ROOT/upstream/vespa"
        return
    fi

    local registered
    registered=$(registered_vespa_root 2>/dev/null || true)
    if [[ -n "$registered" ]]; then
        printf '%s\n' "$registered"
        return
    fi

    local wanted candidate candidate_identity
    wanted=$(vespa_identity "$ROOT")
    while IFS= read -r candidate; do
        [[ "$candidate" != "$ROOT" ]] || continue
        configured_marker "$candidate/upstream/vespa" || continue
        if [[ -n "$wanted" ]]; then
            candidate_identity=$(vespa_identity "$candidate")
            [[ "$candidate_identity" == "$wanted" ]] || continue
        fi
        printf '%s\n' "$candidate/upstream/vespa"
        return
    done < <(git -C "$ROOT" worktree list --porcelain | sed -n 's/^worktree //p')

    echo "no configured Vespa build outputs found in this or a sibling Git worktree" >&2
    echo "build/configure upstream/vespa once in any worktree, or set VESPA_ROOT" >&2
    return 1
}

if [[ "$MODE" == vespa-root ]]; then
    VESPA_BUILD_ROOT=$(resolve_vespa_root)
    verify_pinned_vespa_checkout "$VESPA_BUILD_ROOT"
    printf '%s\n' "$VESPA_BUILD_ROOT"
    exit 0
fi

validate_link_closure() {
    local sdk=$1
    local sdk_lib library output missing external line name path resolved
    local found=0

    command -v ldd >/dev/null 2>&1 || {
        echo "native SDK runtime dependency closure validation requires ldd" >&2
        return 1
    }

    sdk_lib=$(cd -- "$sdk/lib" && pwd -P)
    external=

    while IFS= read -r path; do
        resolved=$(readlink -f "$path" 2>/dev/null || true)
        case "$resolved" in
            "$sdk_lib"/*) ;;
            *) external=$(printf '%s%s => %s\n' "$external" "$(basename "$path")" "${resolved:-unresolved}") ;;
        esac
    done < <(find "$sdk_lib" -maxdepth 1 -type l -print)

    while IFS= read -r library; do
        found=1
        if ! output=$(env -u LD_PRELOAD LD_LIBRARY_PATH="$sdk_lib" ldd "$library" 2>&1); then
            echo "native Vespa base '$sdk' runtime dependency closure probe failed for '$(basename "$library")'" >&2
            printf '%s\n' "$output" >&2
            return 1
        fi

        missing=$(printf '%s\n' "$output" | sed -n '/=> not found/p')
        if [[ -n "$missing" ]]; then
            echo "native Vespa base '$sdk' has incomplete runtime dependency closure for '$(basename "$library")':" >&2
            printf '%s\n' "$missing" >&2
            return 1
        fi

        while IFS= read -r line; do
            name=$(printf '%s\n' "$line" | awk '{print $1}')
            path=$(printf '%s\n' "$line" | awk '$2 == "=>" {print $3}')
            [[ -n "$path" && "$path" != "not" ]] || continue

            if [[ -e "$sdk_lib/$name" || -L "$sdk_lib/$name" ]]; then
                resolved=$(readlink -f "$path" 2>/dev/null || true)
                case "$resolved" in
                    "$sdk_lib"/*) ;;
                    *) external=$(printf '%s%s => %s\n' "$external" "$name" "$path") ;;
                esac
                continue
            fi

            case "$name" in
                libvespa*.so*|libLLVM*.so*|libffi.so*|libxxhash.so*)
                    resolved=$(readlink -f "$path" 2>/dev/null || true)
                    case "$resolved" in
                        "$sdk_lib"/*) ;;
                        *) external=$(printf '%s%s => %s\n' "$external" "$name" "$path") ;;
                    esac
                    ;;
            esac
        done <<<"$output"
    done < <(find "$sdk_lib" -maxdepth 1 -type f -name 'libvespa*.so*' -print)

    if [[ "$found" != 1 ]]; then
        echo "native Vespa base '$sdk' does not contain Vespa shared libraries" >&2
        return 1
    fi

    if [[ -n "$external" ]]; then
        echo "native Vespa base '$sdk' resolved private runtime dependencies outside its bundled lib directory:" >&2
        printf '%s\n' "$external" >&2
        return 1
    fi
}

valid_object() {
    [[ -f "$OBJECT/manifest.json" && -d "$OBJECT/lib" ]] || return 1
    # libvespalib is the fundamental runtime DSO for the embedded adapter.
    # Merely having some other libvespa*.so in the directory is not a valid
    # base and would allow a partially deleted cache object to pass validation.
    [[ -s "$OBJECT/lib/libvespalib.so" ]] || return 1
    [[ ! -e "$OBJECT/lib/libsearchkernel_vespa.so" &&
       ! -e "$OBJECT/include/searchkernel_vespa.h" ]] || return 1
    grep -Eq '"kind"[[:space:]]*:[[:space:]]*"vespa-base"' "$OBJECT/manifest.json" || return 1
    grep -Eq '"vespa_commit"[[:space:]]*:[[:space:]]*"'"$PINNED_VESPA_COMMIT"'"' "$OBJECT/manifest.json" || return 1
    validate_link_closure "$OBJECT"
}

valid_install() {
    valid_object && registered_vespa_root >/dev/null
}
install_published_base() {
    [[ "${SK_NATIVE_DISABLE_PUBLISHED_BASE:-0}" != 1 ]] || return 10
    local installer installed status
    installer="$ROOT/tools/install-native-base-artifact.sh"
    [[ -f "$installer" ]] || {
        echo "published Vespa base installer is missing: $installer" >&2
        return 1
    }
    resolve_published_base_identity || return 1
    if installed=$(bash "$installer" "$PUBLISHED_BASE_URL" "$CACHE_ROOT" "$BUILD_ENV" "$TARGET" "$PINNED_VESPA_COMMIT"); then
        verify_pinned_vespa_checkout "$installed"
        register_vespa_root "$installed"
        valid_install || {
            echo "published Vespa base installed but failed final cache validation" >&2
            return 1
        }
        echo "installed published Vespa base generation v$PUBLISHED_BASE_GENERATION for $BUILD_ENV/$TARGET/$PINNED_VESPA_COMMIT" >&2
        return 0
    else
        status=$?
        [[ "$status" -eq 10 ]] && return 10
        return "$status"
    fi
}

if valid_install; then
    if [[ "$MODE" != ensure || "${SK_NATIVE_DISABLE_PUBLISHED_BASE:-0}" == 1 ]] || ! has_published_install; then
        printf '%s\n' "$OBJECT"
        exit 0
    fi
    if published_install_is_current; then
        printf '%s\n' "$OBJECT"
        exit 0
    fi
    echo "published native-base cache is older than the desired generation; refresh required" >&2
fi

if [[ "$MODE" == path ]]; then
    if valid_object; then
        echo "native Vespa base exists for checkout $PINNED_VESPA_COMMIT, but its configured checkout registration is missing" >&2
    else
        echo "native Vespa base is not installed for checkout $PINNED_VESPA_COMMIT" >&2
    fi
    echo "run './dev native-sdk ensure' from the one configured Vespa checkout when provisioning this machine" >&2
    exit 1
fi

mkdir -p "$CACHE_ROOT/objects" "$CACHE_ROOT/locks" "$CACHE_ROOT/build" "$CACHE_ROOT/checkouts"
LOCK=$CACHE_ROOT/locks/$PINNED_VESPA_COMMIT.lock
command -v flock >/dev/null 2>&1 || {
    echo "native SDK cache requires flock for machine-wide build serialization" >&2
    exit 1
}
exec 9>"$LOCK"
flock -w 300 9 || {
    echo "timed out waiting for native SDK cache lock '$LOCK'" >&2
    exit 1
}

TMP=$CACHE_ROOT/objects/.tmp-$PINNED_VESPA_COMMIT-$
BUILD=$CACHE_ROOT/build/$PINNED_VESPA_COMMIT-$
cleanup() {
    rm -rf "$TMP" "$BUILD"
}
trap cleanup EXIT INT TERM

if valid_install; then
    if [[ "${SK_NATIVE_DISABLE_PUBLISHED_BASE:-0}" == 1 ]] || ! has_published_install; then
        printf '%s\n' "$OBJECT"
        exit 0
    fi
    if published_install_is_current; then
        printf '%s\n' "$OBJECT"
        exit 0
    fi
    echo "refreshing published native-base cache to generation v$PUBLISHED_BASE_GENERATION" >&2
    if install_published_base; then
        printf '%s\n' "$OBJECT"
        exit 0
    fi
    status=$?
    echo "failed to refresh published native-base cache to the desired generation" >&2
    exit "$status"
fi

# Prefer an already-configured local source when it can seed the immutable base
# without rebuilding Vespa. If no prebuilt local source exists, acquire the
# canonical published base before falling back to an explicit local Vespa build.
VESPA_BUILD_ROOT=$(resolve_vespa_root 2>/dev/null || true)
VESPA_LIBRARY_ROOT=
if [[ -n "$VESPA_BUILD_ROOT" ]]; then
    verify_pinned_vespa_checkout "$VESPA_BUILD_ROOT"
    register_vespa_root "$VESPA_BUILD_ROOT"
    if valid_object; then
        printf '%s\n' "$OBJECT"
        exit 0
    fi
    VESPA_LIBRARY_ROOT=$(resolve_prebuilt_vespa_library_root "$VESPA_BUILD_ROOT" "$PINNED_VESPA_COMMIT" || true)
fi

if [[ -z "$VESPA_LIBRARY_ROOT" && "${SK_NATIVE_DISABLE_PUBLISHED_BASE:-0}" != 1 ]]; then
    if install_published_base; then
        printf '%s\n' "$OBJECT"
        exit 0
    else
        status=$?
        if [[ "$status" -ne 10 ]]; then
            exit "$status"
        fi
    fi
fi

if [[ -z "$VESPA_BUILD_ROOT" ]]; then
    VESPA_BUILD_ROOT=$(resolve_vespa_root)
    verify_pinned_vespa_checkout "$VESPA_BUILD_ROOT"
    register_vespa_root "$VESPA_BUILD_ROOT"
fi
if [[ -z "$VESPA_LIBRARY_ROOT" ]]; then
    VESPA_LIBRARY_ROOT=$(resolve_vespa_library_root "$VESPA_BUILD_ROOT" "$PINNED_VESPA_COMMIT")
fi
if [[ -e "$OBJECT" ]]; then
    QUARANTINE_ROOT="$CACHE_ROOT/quarantine"
    mkdir -p "$QUARANTINE_ROOT"
    QUARANTINED="$QUARANTINE_ROOT/$PINNED_VESPA_COMMIT.$(date +%s).$BASHPID"
    echo "native SDK cache object '$OBJECT' failed validation; quarantining and rebuilding under the checkout lock" >&2
    # Published objects are mode-locked read-only. Once validation has failed
    # and this digest lock is held, make only the object directory itself
    # owner-writable so it can be retired from the published namespace. Do not
    # modify any packaged contents in place.
    chmod u+w "$OBJECT"
    mv "$OBJECT" "$QUARANTINED"
fi

rm -rf "$TMP" "$BUILD"
BUILDER=${SK_NATIVE_CACHE_BUILDER:-$ROOT/core/native/vespa/build-sdk.sh}
VESPA_ROOT="$VESPA_BUILD_ROOT" \
VESPA_LIBRARY_ROOT="$VESPA_LIBRARY_ROOT" \
SK_NATIVE_SDK="$TMP" \
SK_NATIVE_BUILD_DIR="$BUILD" \
SK_NATIVE_TARGET="$TARGET" \
SK_NATIVE_REGENERATE_BINDINGS=0 \
SK_NATIVE_SKIP_BINDINGS_CHECK=1 \
SK_NATIVE_BASE_ONLY=1 \
    "$BUILDER"

grep -Eq '"kind"[[:space:]]*:[[:space:]]*"vespa-base"' "$TMP/manifest.json" || {
    echo "built native package is not a Vespa base" >&2
    exit 1
}
grep -Eq '"vespa_commit"[[:space:]]*:[[:space:]]*"'"$PINNED_VESPA_COMMIT"'"' "$TMP/manifest.json" || {
    echo "built Vespa base does not match checkout $PINNED_VESPA_COMMIT" >&2
    exit 1
}

validate_link_closure "$TMP" || {
    echo "refusing to publish native SDK with incomplete runtime dependency closure" >&2
    exit 1
}

# Cache objects are immutable after publication. Build into private staging and
# publish only into the checkout-hash key; no process ever builds
# directly inside a published object. Keep the staging tree writable until it
# has been renamed because this filesystem rejects renames of read-only trees.
mv "$TMP" "$OBJECT"
chmod -R a-w "$OBJECT"
printf '%s\n' "$OBJECT"
