#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
OUT_DIR=${1:-$ROOT/target/native-base-artifacts}
BUILD_ENV=${SEARCHKERNEL_BUILD_ENV:-host}
TARGET=${SK_NATIVE_TARGET:-$(rustc -vV | sed -n 's/^host: //p')}
PINNED_VESPA_COMMIT=$(python3 "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/package-native-base-artifact-python-1.py" "$ROOT/core/native/vespa/manifest.json"
)
[[ -n "$TARGET" ]] || { echo "could not determine native target" >&2; exit 1; }

BASE=$(bash "$ROOT/tools/resolve-native-sdk.sh" path)
VESPA_ROOT=$(bash "$ROOT/tools/resolve-native-sdk.sh" vespa-root)
FLAGS_FILE=$VESPA_ROOT/searchcore/src/tests/proton/documentdb/CMakeFiles/searchcore_embedded_crud_spike_app.dir/flags.make
LINK_FILE=$VESPA_ROOT/searchcore/src/tests/proton/documentdb/CMakeFiles/searchcore_embedded_crud_spike_app.dir/link.txt
[[ -f "$FLAGS_FILE" && -f "$LINK_FILE" ]] || { echo "configured Vespa metadata is missing" >&2; exit 1; }

VESPA_TOP=$(git -C "$VESPA_ROOT" rev-parse --show-toplevel 2>/dev/null || true)
[[ -n "$VESPA_TOP" && "$(cd -- "$VESPA_TOP" && pwd -P)" == "$(cd -- "$VESPA_ROOT" && pwd -P)" ]] || {
    echo "standalone pinned Vespa checkout is required to package a published base" >&2
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
STAGE=$(mktemp -d "$OUT_DIR/.native-base.XXXXXX")
cleanup() { rm -rf "$STAGE"; }
trap cleanup EXIT INT TERM
BUNDLE=$STAGE/searchkernel-vespa-base
mkdir -p "$BUNDLE/base" "$BUNDLE/toolchain/upstream/vespa" "$BUNDLE/toolchain/vespa-deps/include" "$BUNDLE/toolchain/vespa-deps/lib64"
cp -aL "$BASE/." "$BUNDLE/base/"

# Package the immutable Vespa source/header surface plus generated header-like
# files and the exact compile/link metadata used by the adapter build. Heavy
# native objects and libraries stay in the base lib directory instead.
(
    cd "$VESPA_ROOT"
    git ls-files -z -- \
        '*.h' '*.hh' '*.hpp' '*.hxx' '*.ipp' '*.inl' '*.inc' '*.def' '*.tcc' |
        tar --null -T - -chf -
) | tar -C "$BUNDLE/toolchain/upstream/vespa" -xf -
(
    cd "$VESPA_ROOT"
    find . -type f \( \
        -name '*.h' -o -name '*.hh' -o -name '*.hpp' -o -name '*.hxx' -o \
        -name '*.ipp' -o -name '*.inl' -o -name '*.inc' -o -name '*.def' -o -name '*.tcc' -o -name 'flags.make' -o -name 'link.txt' \
    \) -print0 | tar --null -T - -chf -
) | tar -C "$BUNDLE/toolchain/upstream/vespa" -xf -
cp -aL "$DEPS/include/." "$BUNDLE/toolchain/vespa-deps/include/"

# The adapter link command also contains Vespa production static archives.
# A clean consumer has no configured Vespa build tree, so every production .a
# referenced by link.txt must travel with the published toolchain. Preserve
# each archive at its Vespa-root-relative path so the harvested relative link
# arguments continue to resolve without rewriting producer build topology.
python3 "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/package-native-base-artifact-python-2.py" "$LINK_FILE" "$VESPA_ROOT" "$BUNDLE/toolchain/upstream/vespa"

# Configured metadata may contain absolute producer-machine paths. Normalize
# those paths to the canonical placeholders already understood by build-sdk.sh
# so the packaged toolchain remains relocatable on a fresh machine.
python3 "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/package-native-base-artifact-python-3.py" "$BUNDLE/toolchain/upstream/vespa" "$VESPA_ROOT" "$DEPS"

python3 "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/package-native-base-artifact-python-4.py" "$BUNDLE/manifest.json" "$BUNDLE/toolchain/upstream/vespa/.searchkernel-vespa-artifact.json" "$BUILD_ENV" "$TARGET" "$PINNED_VESPA_COMMIT"

ASSET="searchkernel-vespa-base-${BUILD_ENV}-${TARGET}-${PINNED_VESPA_COMMIT}.tar.gz"
ARCHIVE=$OUT_DIR/$ASSET
rm -f "$ARCHIVE" "$ARCHIVE.sha256"
(
    cd "$STAGE"
    tar --sort=name --mtime='UTC 1970-01-01' --owner=0 --group=0 --numeric-owner -czf "$ARCHIVE" searchkernel-vespa-base
)
sha256sum "$ARCHIVE" > "$ARCHIVE.sha256"
printf '%s\n%s\n' "$ARCHIVE" "$ARCHIVE.sha256"
