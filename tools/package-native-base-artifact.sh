#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
OUT_DIR=${1:-$ROOT/target/native-base-artifacts}
BUILD_ENV=${SEARCHKERNEL_BUILD_ENV:-host}
TARGET=${SK_NATIVE_TARGET:-$(rustc -vV | sed -n 's/^host: //p')}
PINNED_VESPA_COMMIT=$(python3 - "$ROOT/core/native/vespa/manifest.json" <<'PY'
import json
import sys
with open(sys.argv[1], encoding="utf-8") as stream:
    print(json.load(stream)["vespa_commit"])
PY
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
cleanup() {
    local status=$?
    trap - EXIT INT TERM
    if ! bash "$ROOT/tools/remove-readonly-tree.sh" "$STAGE" && (( status == 0 )); then
        status=1
    fi
    exit "$status"
}
trap cleanup EXIT
BUNDLE=$STAGE/searchkernel-vespa-base
mkdir -p "$BUNDLE/base" "$BUNDLE/toolchain/upstream/vespa" "$BUNDLE/toolchain/vespa-deps/include" "$BUNDLE/toolchain/vespa-deps/lib64"
cp -aL "$BASE/." "$BUNDLE/base/"
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
python3 "$ROOT/tools/bundle-native-runtime-closure.py" \
    "$BUNDLE/base/lib" "${runtime_search_args[@]}"
python3 "$ROOT/tools/check-native-runtime-closure.py" "$BUNDLE/base/lib" >&2

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
python3 - "$LINK_FILE" "$VESPA_ROOT" "$BUNDLE/toolchain/upstream/vespa" <<'PY'
import pathlib
import shlex
import shutil
import sys

link_file = pathlib.Path(sys.argv[1]).resolve()
vespa_root = pathlib.Path(sys.argv[2]).resolve()
destination_root = pathlib.Path(sys.argv[3])

tokens = shlex.split(link_file.read_text(encoding="utf-8"))
for token in tokens:
    if not token.endswith(".a"):
        continue
    if pathlib.Path(token).name == "libsearchcore_test.a":
        continue

    source = pathlib.Path(token)
    if not source.is_absolute():
        # CMake writes link.txt below CMakeFiles/<target>.dir, but evaluates
        # relative link arguments from the target's binary directory.
        source = link_file.parent.parent.parent / source
    source = source.resolve()

    try:
        relative = source.relative_to(vespa_root)
    except ValueError as exc:
        raise SystemExit(
            f"adapter link references static archive outside Vespa root: {token}"
        ) from exc

    if not source.is_file():
        raise SystemExit(f"adapter link static archive is missing: {source}")

    destination = destination_root / relative
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, destination)
PY

# Configured metadata may contain absolute producer-machine paths. Normalize
# those paths to the canonical placeholders already understood by build-sdk.sh
# so the packaged toolchain remains relocatable on a fresh machine.
python3 - "$BUNDLE/toolchain/upstream/vespa" "$VESPA_ROOT" "$DEPS" <<'PY'
import pathlib
import re
import sys

root = pathlib.Path(sys.argv[1])
vespa_root = sys.argv[2].rstrip("/")
deps_root = sys.argv[3].rstrip("/")

def normalize_absolute_root(text: str, source: str, destination: str) -> str:
    # CMake link metadata contains both absolute producer paths and legitimate
    # relative paths with a component named "vespa", for example
    # ../../../vespa/searchcore/.... A blind string replacement corrupts the
    # latter when the producer checkout itself is mounted at /vespa.
    for prefix in ("-I", "-L", "-isystem"):
        text = text.replace(prefix + source, prefix + destination)

    # Rewrite only a true absolute-path occurrence. Relative path components
    # are preceded by "." or "/", both intentionally excluded here.
    pattern = re.compile(
        r"(?<![A-Za-z0-9_./-])" + re.escape(source) + r"(?=/|$)"
    )
    return pattern.sub(destination, text)

for name in ("flags.make", "link.txt"):
    for path in root.rglob(name):
        text = path.read_text(encoding="utf-8")
        text = normalize_absolute_root(text, vespa_root, "/work/vespa")
        text = normalize_absolute_root(text, deps_root, "/opt/vespa-deps")
        path.write_text(text, encoding="utf-8")
PY

python3 - "$BUNDLE/manifest.json" "$BUNDLE/toolchain/upstream/vespa/.searchkernel-vespa-artifact.json" "$BUILD_ENV" "$TARGET" "$PINNED_VESPA_COMMIT" <<'PY'
import json
import sys
bundle_path, marker_path, build_env, target, commit = sys.argv[1:]
data = {
    "format_version": 2,
    "build_environment": build_env,
    "native_target": target,
    "vespa_commit": commit,
}
for path in (bundle_path, marker_path):
    with open(path, "w", encoding="utf-8") as stream:
        json.dump(data, stream, indent=2, sort_keys=True)
        stream.write("\n")
PY

ASSET="searchkernel-vespa-base-${BUILD_ENV}-${TARGET}-${PINNED_VESPA_COMMIT}.tar.gz"
ARCHIVE=$OUT_DIR/$ASSET
rm -f "$ARCHIVE" "$ARCHIVE.sha256"
(
    cd "$STAGE"
    tar --sort=name --mtime='UTC 1970-01-01' --owner=0 --group=0 --numeric-owner -czf "$ARCHIVE" searchkernel-vespa-base
)
sha256sum "$ARCHIVE" > "$ARCHIVE.sha256"
printf '%s\n%s\n' "$ARCHIVE" "$ARCHIVE.sha256"
