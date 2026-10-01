#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
PYTHON_BIN=$(bash "$ROOT/tools/resolve-python.sh")
RUSTC_BIN=$(bash "$ROOT/tools/resolve-rust.sh" rustc)
if [[ $# -ne 2 ]]; then
    echo "usage: $0 <output-dir> <staging-dir>" >&2
    exit 2
fi
OUT_DIR=$1
STAGE=$2
BUILD_ENV=${SEARCHKERNEL_BUILD_ENV:-host}
TARGET=${SK_NATIVE_TARGET:-$("$RUSTC_BIN" -vV | sed -n 's/^host: //p')}
ARTIFACT_GENERATION=${SK_NATIVE_BASE_GENERATION:-1}
SOURCE_COMMIT=${SK_NATIVE_BASE_SOURCE_COMMIT:-$(git -C "$ROOT" rev-parse HEAD)}
PINNED_VESPA_COMMIT=$("$PYTHON_BIN" - "$ROOT/core/native/vespa/manifest.json" <<'PY'
import json
import sys
with open(sys.argv[1], encoding="utf-8") as stream:
    print(json.load(stream)["vespa_commit"])
PY
)
[[ -n "$TARGET" ]] || { echo "could not determine native target" >&2; exit 1; }
[[ "$ARTIFACT_GENERATION" =~ ^[1-9][0-9]*$ ]] || { echo "invalid native-base generation: $ARTIFACT_GENERATION" >&2; exit 1; }
[[ "$SOURCE_COMMIT" =~ ^[0-9a-fA-F]{40}$ ]] || { echo "invalid Search Kernel source commit: $SOURCE_COMMIT" >&2; exit 1; }

BASE=$(bash "$ROOT/tools/resolve-native-sdk.sh" path)
VESPA_ROOT=$(bash "$ROOT/tools/resolve-native-sdk.sh" vespa-root)
FLAGS_FILE=$VESPA_ROOT/searchcore/src/tests/proton/documentdb/CMakeFiles/searchcore_embedded_crud_spike_app.dir/flags.make
LINK_FILE=$VESPA_ROOT/searchcore/src/tests/proton/documentdb/CMakeFiles/searchcore_embedded_crud_spike_app.dir/link.txt
[[ -f "$FLAGS_FILE" && -f "$LINK_FILE" ]] || { echo "configured Vespa metadata is missing" >&2; exit 1; }

VESPA_MARKER="$VESPA_ROOT/.searchkernel-vespa-artifact.json"
VESPA_FROM_MARKER=0
if [[ -f "$VESPA_MARKER" ]]; then
    "$PYTHON_BIN" - "$VESPA_MARKER" "$BUILD_ENV" "$TARGET" "$PINNED_VESPA_COMMIT" <<'PY'
import json
import sys

path, build_env, target, commit = sys.argv[1:]
with open(path, encoding="utf-8") as stream:
    marker = json.load(stream)
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
PY
    VESPA_FROM_MARKER=1
else
    VESPA_TOP=$(git -C "$VESPA_ROOT" rev-parse --show-toplevel 2>/dev/null || true)
    [[ -n "$VESPA_TOP" && "$(cd -- "$VESPA_TOP" && pwd -P)" == "$(cd -- "$VESPA_ROOT" && pwd -P)" ]] || {
        echo "standalone pinned Vespa checkout or verified published toolchain is required to package a published base" >&2
        exit 1
    }
    ACTUAL_VESPA_COMMIT=$(git -C "$VESPA_ROOT" rev-parse HEAD)
    [[ "$ACTUAL_VESPA_COMMIT" == "$PINNED_VESPA_COMMIT" ]] || {
        echo "cannot publish Vespa base from $ACTUAL_VESPA_COMMIT; expected $PINNED_VESPA_COMMIT" >&2
        exit 1
    }
    git -C "$VESPA_ROOT" diff --quiet --ignore-submodules -- &&
    git -C "$VESPA_ROOT" diff --cached --quiet --ignore-submodules -- || {
        echo "cannot publish Vespa base from a dirty Vespa checkout" >&2
        exit 1
    }
fi

RAW_INCLUDES=$(sed -n 's/^CXX_INCLUDES = //p' "$FLAGS_FILE")
CONFIGURED_DEPS=$(printf '%s\n' "$RAW_INCLUDES" | sed -n 's#.*-isystem \([^ ]*\)/include.*#\1#p')
VESPA_WORKTREE_ROOT=
case "$VESPA_ROOT" in
    */upstream/vespa) VESPA_WORKTREE_ROOT=$(cd -- "$VESPA_ROOT/../.." && pwd) ;;
esac
if [[ -n "${VESPA_DEPS:-}" ]]; then
    DEPS=$VESPA_DEPS
elif [[ -n "$VESPA_WORKTREE_ROOT" && -d "$VESPA_WORKTREE_ROOT/vespa-deps" ]]; then
    DEPS=$VESPA_WORKTREE_ROOT/vespa-deps
elif [[ -d "$ROOT/vespa-deps" ]]; then
    DEPS=$ROOT/vespa-deps
else
    DEPS=$CONFIGURED_DEPS
fi
[[ -d "$DEPS/include" ]] || { echo "dependency headers are unavailable at ${DEPS:-unknown}" >&2; exit 1; }

mkdir -p "$OUT_DIR"
[[ ! -e "$STAGE" ]] || { echo "native-base staging directory already exists: $STAGE" >&2; exit 1; }
mkdir -p "$STAGE"
cleanup() {
    local status=$?
    trap - EXIT INT TERM
    if (( status != 0 )); then
        bash "$ROOT/tools/remove-readonly-tree.sh" "$STAGE" || true
    fi
    exit "$status"
}
trap cleanup EXIT
BUNDLE=$STAGE/searchkernel-vespa-base
mkdir -p "$BUNDLE/base" "$BUNDLE/toolchain/upstream/vespa" "$BUNDLE/toolchain/vespa-deps/include" "$BUNDLE/toolchain/vespa-deps/lib64"
# Keep the cache's relative linker aliases as symlinks to their exact SONAME
# payloads. Dereferencing here turns aliases into duplicate ELF files; the
# runtime pruner then correctly drops those duplicates because only the
# SONAME path is needed at load time, which would leave the archive unable to
# rebuild its adapter from -lNAME inputs.
cp -a "$BASE/." "$BUNDLE/base/"
# The installed cache is immutable; this copy is the mutable packaging stage.
bash "$ROOT/tools/make-writable-tree.sh" "$BUNDLE/base"

# The immutable base builder intentionally owns Vespa DSOs, but the producer
# host may satisfy ordinary transitive dependencies (for example libxml2) from
# its system/dependency installation. A published base must carry every
# non-baseline DT_NEEDED dependency itself so clean consumers do not inherit
# accidental producer-host state.
runtime_search_args=()
for runtime_dir in "$DEPS/lib64" "$DEPS/lib"; do
    if [[ -d "$runtime_dir" ]]; then
        runtime_search_args+=(--search-dir "$runtime_dir")
    fi
done
runtime_source_args=(
    --source-root "native-base=$BASE"
    --source-root "vespa-build=$VESPA_ROOT"
    --source-root "vespa-deps=$DEPS"
    --source-root "system=/usr"
)
"$PYTHON_BIN" "$ROOT/tools/bundle-native-runtime-closure.py" \
    "$BUNDLE/base/lib" "${runtime_search_args[@]}" "${runtime_source_args[@]}"
"$PYTHON_BIN" "$ROOT/tools/check-native-runtime-closure.py" "$BUNDLE/base/lib" --require-provenance >&2

# Package the immutable Vespa source/header surface plus generated header-like
# files and the exact compile/link metadata used by the adapter build. Heavy
# native objects and libraries stay in the base lib directory instead. A
# published toolchain is already a filtered immutable header/config snapshot, so
# it has no Git index to enumerate.
if [[ "$VESPA_FROM_MARKER" != 1 ]]; then
    (
        cd "$VESPA_ROOT"
        git ls-files -z -- \
            '*.h' '*.hh' '*.hpp' '*.hxx' '*.ipp' '*.inl' '*.inc' '*.def' '*.tcc' |
            tar --null -T - -chf -
    ) | tar -C "$BUNDLE/toolchain/upstream/vespa" -xf -
fi
(
    cd "$VESPA_ROOT"
    find . -type f \( \
        -name '*.h' -o -name '*.hh' -o -name '*.hpp' -o -name '*.hxx' -o \
        -name '*.ipp' -o -name '*.inl' -o -name '*.inc' -o -name '*.def' -o -name '*.tcc' -o -name 'flags.make' -o -name 'link.txt' -o -name '.searchkernel-vespa-artifact.json' \
    \) -print0 | tar --null -T - -chf -
) | tar -C "$BUNDLE/toolchain/upstream/vespa" -xf -
cp -aL "$DEPS/include/." "$BUNDLE/toolchain/vespa-deps/include/"
STATIC_ROOT_ARGS=(--producer-root "/work/vespa=$VESPA_ROOT")
if [[ -n "$CONFIGURED_DEPS" ]]; then
    STATIC_ROOT_ARGS+=(--producer-root "$CONFIGURED_DEPS=$DEPS")
fi
"$PYTHON_BIN" "$ROOT/tools/copy-native-base-static-link-inputs.py" \
    "$LINK_FILE" "$VESPA_ROOT" "$DEPS" \
    "$BUNDLE/toolchain/upstream/vespa" "$BUNDLE/toolchain/vespa-deps" \
    "${STATIC_ROOT_ARGS[@]}"

# Configured metadata may contain absolute producer-machine paths. Normalize
# those paths to the canonical placeholders already understood by build-sdk.sh
# so the packaged toolchain remains relocatable on a fresh machine.
"$PYTHON_BIN" "$ROOT/tools/normalize-native-base-metadata.py" \
    "$BUNDLE/toolchain/upstream/vespa" "$VESPA_ROOT" "$DEPS"
"$PYTHON_BIN" "$ROOT/tools/write-native-base-manifest.py" \
    "$BUNDLE/manifest.json" \
    "$BUNDLE/toolchain/upstream/vespa/.searchkernel-vespa-artifact.json" \
    "$BUNDLE/base/lib/.searchkernel-runtime-provenance.json" \
    "$BUILD_ENV" "$TARGET" "$PINNED_VESPA_COMMIT" \
    "$ARTIFACT_GENERATION" "$SOURCE_COMMIT"

printf '%s\n' "$BUNDLE"
