#!/usr/bin/env bash
set -euo pipefail

REPO="${SEARCHKERNEL_GITHUB_REPO:-superkelvint/searchkernel}"
MODE="${1:-all}"
ARTIFACT_FORMAT_VERSION=2

case "$MODE" in
    ci) BUILD_ENVS=(ci) ;;
    host) BUILD_ENVS=(host) ;;
    all) BUILD_ENVS=(host ci) ;;
    *) echo "usage: $0 [ci|host|all]" >&2; exit 2 ;;
esac

die() { echo "ERROR: $*" >&2; exit 1; }
section() { echo; echo "==> $*"; }

ROOT="$(git rev-parse --show-toplevel 2>/dev/null)" || die "run this from inside a SearchKernel checkout"
ROOT="$(cd "$ROOT" && pwd -P)"
cd "$ROOT"

for cmd in cargo cmake c++ find gh git ldd python3 readelf rustc sha256sum tar; do
    command -v "$cmd" >/dev/null 2>&1 || die "missing required command: $cmd"
done
[[ -x ./dev ]] || die "./dev is missing or not executable"
gh api "repos/$REPO" >/dev/null 2>&1 || die "gh authentication cannot access $REPO"

SOURCE_COMMIT="$(git rev-parse HEAD)"

require_canonical_main() {
    local remote_main
    remote_main="$(gh api "repos/$REPO/git/ref/heads/main" --jq '.object.sha')"
    [[ "$SOURCE_COMMIT" == "$remote_main" ]] ||
        die "publisher checkout is $SOURCE_COMMIT, but canonical main is $remote_main; update main before publishing"
    [[ -z "$(git status --porcelain --untracked-files=normal)" ]] ||
        die "publisher checkout has uncommitted or untracked files; publish only from a clean canonical main"
}

require_canonical_main

PINNED_VESPA_COMMIT="$(python3 - <<'PY'
import json
with open("core/native/vespa/manifest.json", encoding="utf-8") as stream:
    print(json.load(stream)["vespa_commit"])
PY
)"
[[ "$PINNED_VESPA_COMMIT" =~ ^[0-9a-fA-F]{40}$ ]] || die "invalid pinned Vespa commit: $PINNED_VESPA_COMMIT"
TARGET="${SK_NATIVE_TARGET:-$(rustc -vV | sed -n 's/^host: //p')}"
[[ -n "$TARGET" ]] || die "could not determine Rust target"

MARKER_REL="searchcore/src/tests/proton/documentdb/CMakeFiles/searchcore_embedded_crud_spike_app.dir/link.txt"
FLAGS_REL="searchcore/src/tests/proton/documentdb/CMakeFiles/searchcore_embedded_crud_spike_app.dir/flags.make"

candidate_valid() {
    local candidate="$1" top actual
    [[ -d "$candidate" && -f "$candidate/CMakeCache.txt" && -f "$candidate/$MARKER_REL" && -f "$candidate/$FLAGS_REL" ]] || return 1
    top="$(git -C "$candidate" rev-parse --show-toplevel 2>/dev/null || true)"
    [[ -n "$top" && "$(cd "$top" && pwd -P)" == "$(cd "$candidate" && pwd -P)" ]] || return 1
    actual="$(git -C "$candidate" rev-parse HEAD)"
    [[ "$actual" == "$PINNED_VESPA_COMMIT" ]] || return 1
    git -C "$candidate" diff --quiet --ignore-submodules -- || return 1
    git -C "$candidate" diff --cached --quiet --ignore-submodules -- || return 1
}

section "Finding configured pinned Vespa build"
REAL_VESPA_ROOT=""
if [[ -n "${VESPA_ROOT:-}" ]] && candidate_valid "$VESPA_ROOT"; then
    REAL_VESPA_ROOT="$(cd "$VESPA_ROOT" && pwd -P)"
elif candidate_valid "$ROOT/upstream/vespa"; then
    REAL_VESPA_ROOT="$(cd "$ROOT/upstream/vespa" && pwd -P)"
else
    while IFS= read -r cache; do
        candidate="$(dirname "$cache")"
        case "$candidate" in */upstream/vespa) ;; *) continue ;; esac
        if candidate_valid "$candidate"; then
            REAL_VESPA_ROOT="$(cd "$candidate" && pwd -P)"
            break
        fi
    done < <(find "$HOME" -maxdepth 8 -type f -name CMakeCache.txt -path '*/upstream/vespa/CMakeCache.txt' -print 2>/dev/null)
fi
[[ -n "$REAL_VESPA_ROOT" ]] || die "no clean configured standalone Vespa checkout at $PINNED_VESPA_COMMIT found"

REAL_LINK_FILE="$REAL_VESPA_ROOT/$MARKER_REL"
REAL_FLAGS_FILE="$REAL_VESPA_ROOT/$FLAGS_REL"
RAW_INCLUDES="$(sed -n 's/^CXX_INCLUDES = //p' "$REAL_FLAGS_FILE")"
CONFIGURED_DEPS="$(printf '%s\n' "$RAW_INCLUDES" | sed -n 's#.*-isystem \([^ ]*\)/include.*#\1#p')"
WORKTREE_ROOT=""
case "$REAL_VESPA_ROOT" in */upstream/vespa) WORKTREE_ROOT="$(cd "$REAL_VESPA_ROOT/../.." && pwd -P)" ;; esac
if [[ -n "${VESPA_DEPS:-}" ]]; then
    REAL_DEPS="$VESPA_DEPS"
elif [[ -n "$WORKTREE_ROOT" && -d "$WORKTREE_ROOT/vespa-deps" ]]; then
    REAL_DEPS="$WORKTREE_ROOT/vespa-deps"
elif [[ -d "$ROOT/vespa-deps" ]]; then
    REAL_DEPS="$ROOT/vespa-deps"
else
    REAL_DEPS="$CONFIGURED_DEPS"
fi
[[ -n "$REAL_DEPS" && -d "$REAL_DEPS/include" ]] || die "Vespa dependency headers unavailable at ${REAL_DEPS:-unknown}"
REAL_DEPS="$(cd "$REAL_DEPS" && pwd -P)"

section "Determining next native-base generation"
ARTIFACT_GENERATION="$(
    gh api --paginate "repos/${REPO}/releases?per_page=100" --jq '.[].tag_name' |
    python3 -c '
import re, sys
target, commit = sys.argv[1:]
pat = re.compile(r"^vespa-base-v([0-9]+)-(?:host|ci)-" + re.escape(target) + "-" + re.escape(commit) + r"$")
versions = []
for raw in sys.stdin:
    m = pat.match(raw.strip())
    if m:
        versions.append(int(m.group(1)))
print(max(versions, default=0) + 1)
' "$TARGET" "$PINNED_VESPA_COMMIT"
)"
[[ "$ARTIFACT_GENERATION" =~ ^[1-9][0-9]*$ ]] || die "failed to determine next artifact generation"
echo "Publishing generation v${ARTIFACT_GENERATION}"

TMP="$(mktemp -d "/tmp/searchkernel-native-base-v${ARTIFACT_GENERATION}.XXXXXX")"
cleanup() {
    local status=$?
    trap - EXIT INT TERM
    if [[ "${KEEP_NATIVE_BASE_TMP:-0}" == 1 ]]; then
        echo "Keeping $TMP"
    elif ! bash "$ROOT/tools/remove-readonly-tree.sh" "$TMP" && (( status == 0 )); then
        status=1
    fi
    exit "$status"
}
trap cleanup EXIT

declare -A ARCHIVES CHECKSUMS TAGS ASSETS

augment_static_closure() {
    local bundle="$1"
    local packaged_vespa="$bundle/toolchain/upstream/vespa"
    local packaged_deps="$bundle/toolchain/vespa-deps"
    python3 - "$REAL_LINK_FILE" "$REAL_VESPA_ROOT" "$REAL_DEPS" "$packaged_vespa" "$packaged_deps" <<'PY'
import pathlib, shlex, shutil, sys
link_file = pathlib.Path(sys.argv[1]).resolve()
vespa_root = pathlib.Path(sys.argv[2]).resolve()
deps_root = pathlib.Path(sys.argv[3]).resolve()
packaged_vespa = pathlib.Path(sys.argv[4])
packaged_deps = pathlib.Path(sys.argv[5])
link_cwd = link_file.parent.parent.parent
seen = set()
count = 0
for token in shlex.split(link_file.read_text(encoding="utf-8")):
    if not token.endswith(".a") or pathlib.Path(token).name == "libsearchcore_test.a":
        continue
    source = pathlib.Path(token)
    if not source.is_absolute():
        source = link_cwd / source
    source = source.resolve(strict=True)
    if str(source) in seen:
        continue
    seen.add(str(source))
    try:
        rel = source.relative_to(vespa_root)
        dest = packaged_vespa / rel
    except ValueError:
        try:
            rel = source.relative_to(deps_root)
            dest = packaged_deps / rel
        except ValueError as exc:
            raise SystemExit(f"adapter static archive is outside declared roots: {source}") from exc
    dest.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, dest)
    count += 1
if count == 0:
    raise SystemExit("canonical adapter link command contained no production static archives")
print(f"packaged {count} production static archives")
PY
}

stamp_generation() {
    local bundle="$1" build_env="$2"
    python3 - "$bundle/manifest.json" "$bundle/toolchain/upstream/vespa/.searchkernel-vespa-artifact.json" "$ARTIFACT_FORMAT_VERSION" "$ARTIFACT_GENERATION" "$build_env" "$TARGET" "$PINNED_VESPA_COMMIT" "$SOURCE_COMMIT" <<'PY'
import json, sys
bundle, marker, expected_format, generation, env, target, commit, source_commit = sys.argv[1:]
expected_format = int(expected_format)
generation = int(generation)
for path in (bundle, marker):
    with open(path, encoding="utf-8") as stream:
        data = json.load(stream)
    if data.get("format_version") != expected_format:
        raise SystemExit(
            f"refusing to publish stale native-base schema from {path}: "
            f"expected format_version {expected_format}, found {data.get('format_version')!r}"
        )
    if data.get("build_environment") != env or data.get("native_target") != target or data.get("vespa_commit") != commit:
        raise SystemExit(f"artifact metadata identity mismatch in {path}")
    data["artifact_generation"] = generation
    data["adapter_static_link_closure"] = True
    data["searchkernel_commit"] = source_commit
    with open(path, "w", encoding="utf-8") as stream:
        json.dump(data, stream, indent=2, sort_keys=True)
        stream.write("\n")
PY
}

verify_bundle() {
    local bundle="$1" build_env="$2" label="$3"
    local base="$bundle/base" vespa="$bundle/toolchain/upstream/vespa" deps="$bundle/toolchain/vespa-deps"
    [[ -d "$base/lib" && -f "$vespa/$MARKER_REL" && -f "$vespa/$FLAGS_REL" && -d "$deps/include" ]] || die "$label: incomplete bundle"
    mkdir -p "$deps/lib64"
    python3 - "$bundle/manifest.json" "$vespa/.searchkernel-vespa-artifact.json" "$ARTIFACT_FORMAT_VERSION" "$ARTIFACT_GENERATION" "$build_env" "$TARGET" "$PINNED_VESPA_COMMIT" "$SOURCE_COMMIT" <<'PY'
import json
import sys

bundle_path, marker_path, expected_format, generation, build_env, target, commit, source_commit = sys.argv[1:]
expected_format = int(expected_format)
generation = int(generation)
for path in (bundle_path, marker_path):
    with open(path, encoding="utf-8") as stream:
        data = json.load(stream)
    expected = {
        "format_version": expected_format,
        "artifact_generation": generation,
        "build_environment": build_env,
        "native_target": target,
        "vespa_commit": commit,
        "adapter_static_link_closure": True,
        "searchkernel_commit": source_commit,
    }
    for key, value in expected.items():
        if data.get(key) != value:
            raise SystemExit(
                f"published metadata mismatch in {path}: "
                f"{key} expected {value!r}, found {data.get(key)!r}"
            )
PY
    python3 "$ROOT/tools/check-native-runtime-closure.py" "$base/lib"
    python3 - "$vespa/$MARKER_REL" "$vespa" "$deps" <<'PY'
import pathlib, shlex, sys
link_file = pathlib.Path(sys.argv[1])
vespa = pathlib.Path(sys.argv[2])
deps = pathlib.Path(sys.argv[3])
text = link_file.read_text(encoding="utf-8").replace("/work/vespa", str(vespa)).replace("/opt/vespa-deps", str(deps))
cwd = link_file.parent.parent.parent
missing = []
for token in shlex.split(text):
    if not token.endswith(".a") or pathlib.Path(token).name == "libsearchcore_test.a":
        continue
    path = pathlib.Path(token)
    if not path.is_absolute():
        path = cwd / path
    if not path.exists():
        missing.append(str(path))
if missing:
    raise SystemExit("packaged static-link closure is incomplete:\n  " + "\n  ".join(missing))
PY
    local verify_root="$TMP/verify-${build_env}-${label}"
    rm -rf "$verify_root"
    mkdir -p "$verify_root"
    env -u LD_PRELOAD \
        SEARCHKERNEL_BUILD_ENV="$build_env" \
        SK_NATIVE_TARGET="$TARGET" \
        SK_NATIVE_ADAPTER_ONLY=1 \
        SK_NATIVE_BASE_SDK="$base" \
        SK_NATIVE_SDK="$verify_root/sdk" \
        SK_NATIVE_BUILD_DIR="$verify_root/build" \
        SK_NATIVE_REGENERATE_BINDINGS=0 \
        SK_NATIVE_SKIP_BINDINGS_CHECK=1 \
        VESPA_ROOT="$vespa" \
        VESPA_DEPS="$deps" \
        VESPA_DEPS_INCLUDE_DIR="$deps/include" \
        VESPA_DEPS_LIB_DIR="$deps/lib64" \
        VESPA_EXTRA_LIB_DIR="$base/lib" \
        bash core/native/vespa/build-sdk.sh
    [[ -s "$verify_root/sdk/lib/libsearchkernel_vespa.so" ]] || die "$label: adapter build produced no library"

    if [[ "$label" == staged ]]; then
        local runtime_search_args=()
        local runtime_dir
        for runtime_dir in "$REAL_DEPS/lib64" "$REAL_DEPS/lib"; do
            if [[ -d "$runtime_dir" ]]; then
                runtime_search_args+=(--search-dir "$runtime_dir")
            fi
        done
        python3 "$ROOT/tools/bundle-native-runtime-closure.py" \
            "$base/lib" \
            --extra-elf "$verify_root/sdk/lib/libsearchkernel_vespa.so.2" \
            "${runtime_search_args[@]}"
    fi
    python3 "$ROOT/tools/check-native-runtime-closure.py" \
        "$base/lib" "$verify_root/sdk/lib/libsearchkernel_vespa.so.2"

    local output
    output="$(env -u LD_PRELOAD LD_LIBRARY_PATH="$base/lib" ldd "$verify_root/sdk/lib/libsearchkernel_vespa.so")"
    printf '%s\n' "$output"
    ! printf '%s\n' "$output" | grep -q 'not found' || die "$label: unresolved runtime dependencies"
}
verify_archive_consumer() {
    local archive="$1" build_env="$2" label="$3"
    local cache="$TMP/consumer-cache-${build_env}-${label}"
    local verify_root="$TMP/consumer-adapter-${build_env}-${label}"
    local vespa base deps output

    rm -rf "$cache" "$verify_root"
    mkdir -p "$cache" "$verify_root"

    vespa="$(bash "$ROOT/tools/install-native-base-artifact.sh" \
        "file://$archive" "$cache" "$build_env" "$TARGET" "$PINNED_VESPA_COMMIT")"
    base="$cache/objects/$PINNED_VESPA_COMMIT"
    deps="$cache/toolchains/$PINNED_VESPA_COMMIT/vespa-deps"

    [[ "$vespa" == "$cache/toolchains/$PINNED_VESPA_COMMIT/upstream/vespa" ]] ||
        die "$label: installer returned unexpected Vespa toolchain path: $vespa"
    [[ -d "$base/lib" && -d "$deps/include" ]] ||
        die "$label: installer did not publish complete consumer cache"

    mkdir -p "$deps/lib64"
    env -u LD_PRELOAD \
        SEARCHKERNEL_BUILD_ENV="$build_env" \
        SK_NATIVE_TARGET="$TARGET" \
        SK_NATIVE_ADAPTER_ONLY=1 \
        SK_NATIVE_BASE_SDK="$base" \
        SK_NATIVE_SDK="$verify_root/sdk" \
        SK_NATIVE_BUILD_DIR="$verify_root/build" \
        SK_NATIVE_REGENERATE_BINDINGS=0 \
        SK_NATIVE_SKIP_BINDINGS_CHECK=1 \
        VESPA_ROOT="$vespa" \
        VESPA_DEPS="$deps" \
        VESPA_DEPS_INCLUDE_DIR="$deps/include" \
        VESPA_DEPS_LIB_DIR="$deps/lib64" \
        VESPA_EXTRA_LIB_DIR="$base/lib" \
        bash "$ROOT/core/native/vespa/build-sdk.sh"

    [[ -s "$verify_root/sdk/lib/libsearchkernel_vespa.so" ]] ||
        die "$label: consumer-path adapter build produced no library"
    python3 "$ROOT/tools/check-native-runtime-closure.py" \
        "$base/lib" "$verify_root/sdk/lib/libsearchkernel_vespa.so.2"
    output="$(env -u LD_PRELOAD LD_LIBRARY_PATH="$base/lib" \
        ldd "$verify_root/sdk/lib/libsearchkernel_vespa.so")"
    printf '%s\n' "$output"
    ! printf '%s\n' "$output" | grep -q 'not found' ||
        die "$label: consumer-path adapter has unresolved runtime dependencies"
}


provision_native_base() {
    local build_env="$1"

    if [[ "$build_env" == host ]]; then
        env -u SK_NATIVE_SDK \
            SEARCHKERNEL_DEV_USE_CI_IMAGE=0 \
            SK_NATIVE_DISABLE_PUBLISHED_BASE=1 \
            ./dev native-sdk ensure
        return
    fi

    env -u SK_NATIVE_SDK \
        SK_NATIVE_DISABLE_PUBLISHED_BASE=1 \
        ./dev native-sdk ensure
}

for BUILD_ENV in "${BUILD_ENVS[@]}"; do
    section "Building $BUILD_ENV artifact"
    export SEARCHKERNEL_BUILD_ENV="$BUILD_ENV" SK_NATIVE_TARGET="$TARGET" VESPA_ROOT="$REAL_VESPA_ROOT" VESPA_DEPS="$REAL_DEPS"
    provision_native_base "$BUILD_ENV"
    ORIGINAL_OUT="$TMP/original-$BUILD_ENV"
    mkdir -p "$ORIGINAL_OUT"
    mapfile -t ORIGINAL_FILES < <(
        env -u SK_NATIVE_SDK \
            bash tools/package-native-base-artifact.sh "$ORIGINAL_OUT"
    )
    [[ ${#ORIGINAL_FILES[@]} -ge 2 && -s "${ORIGINAL_FILES[0]}" ]] || die "canonical packager did not emit an archive"
    STAGE="$TMP/stage-$BUILD_ENV"
    mkdir -p "$STAGE"
    tar -xzf "${ORIGINAL_FILES[0]}" -C "$STAGE"
    BUNDLE="$STAGE/searchkernel-vespa-base"
    augment_static_closure "$BUNDLE"
    stamp_generation "$BUNDLE" "$BUILD_ENV"
    verify_bundle "$BUNDLE" "$BUILD_ENV" staged
    TAG="vespa-base-v${ARTIFACT_GENERATION}-${BUILD_ENV}-${TARGET}-${PINNED_VESPA_COMMIT}"
    ASSET="searchkernel-vespa-base-v${ARTIFACT_GENERATION}-${BUILD_ENV}-${TARGET}-${PINNED_VESPA_COMMIT}.tar.gz"
    OUT="$TMP/artifacts-$BUILD_ENV"
    mkdir -p "$OUT"
    ARCHIVE="$OUT/$ASSET"
    CHECKSUM="$ARCHIVE.sha256"
    (cd "$STAGE" && tar --sort=name --mtime='UTC 1970-01-01' --owner=0 --group=0 --numeric-owner -czf "$ARCHIVE" searchkernel-vespa-base)
    (cd "$OUT" && sha256sum "$ASSET" > "$ASSET.sha256")
    FINAL="$TMP/final-$BUILD_ENV"
    mkdir -p "$FINAL"
    tar -xzf "$ARCHIVE" -C "$FINAL"
    verify_bundle "$FINAL/searchkernel-vespa-base" "$BUILD_ENV" archive
    verify_archive_consumer "$ARCHIVE" "$BUILD_ENV" archive-consumer
    ARCHIVES[$BUILD_ENV]="$ARCHIVE"
    CHECKSUMS[$BUILD_ENV]="$CHECKSUM"
    TAGS[$BUILD_ENV]="$TAG"
    ASSETS[$BUILD_ENV]="$ASSET"
done

section "Revalidating publication source"
require_canonical_main

section "Publication preflight"
for BUILD_ENV in "${BUILD_ENVS[@]}"; do
    TAG="${TAGS[$BUILD_ENV]}"
    ! gh release view "$TAG" -R "$REPO" >/dev/null 2>&1 || die "immutable release already exists: $TAG"
done

for BUILD_ENV in "${BUILD_ENVS[@]}"; do
    TAG="${TAGS[$BUILD_ENV]}"
    section "Uploading draft $TAG"
    gh release create "$TAG" "${ARCHIVES[$BUILD_ENV]}" "${CHECKSUMS[$BUILD_ENV]}" \
        -R "$REPO" \
        --draft \
        --target "$SOURCE_COMMIT" \
        --title "SearchKernel immutable Vespa base $TAG" \
        --notes "Automatically allocated native-base generation v${ARTIFACT_GENERATION} for SearchKernel ${SOURCE_COMMIT} and pinned Vespa ${PINNED_VESPA_COMMIT}."
    [[ "$(gh release view "$TAG" -R "$REPO" --json isDraft --jq '.isDraft')" == "true" ]] ||
        die "$TAG: release became consumer-visible before verification"
done

for BUILD_ENV in "${BUILD_ENVS[@]}"; do
    TAG="${TAGS[$BUILD_ENV]}"
    ASSET="${ASSETS[$BUILD_ENV]}"
    section "Remote draft verification: $TAG"
    REMOTE="$TMP/remote-$BUILD_ENV"
    mkdir -p "$REMOTE"
    gh release download "$TAG" -R "$REPO" -D "$REMOTE" -p "$ASSET" -p "$ASSET.sha256"
    EXPECTED="$(awk 'NR == 1 {print $1}' "$REMOTE/$ASSET.sha256")"
    ACTUAL="$(sha256sum "$REMOTE/$ASSET" | awk '{print $1}')"
    LOCAL="$(sha256sum "${ARCHIVES[$BUILD_ENV]}" | awk '{print $1}')"
    [[ "$EXPECTED" == "$ACTUAL" && "$LOCAL" == "$ACTUAL" ]] || die "$TAG: uploaded bytes failed checksum verification"
    EXTRACT="$TMP/remote-extract-$BUILD_ENV"
    mkdir -p "$EXTRACT"
    tar -xzf "$REMOTE/$ASSET" -C "$EXTRACT"
    verify_bundle "$EXTRACT/searchkernel-vespa-base" "$BUILD_ENV" remote-draft
    verify_archive_consumer "$REMOTE/$ASSET" "$BUILD_ENV" remote-draft-consumer
    echo "Verified draft $TAG ($ACTUAL)"
done

section "Publishing verified native-base releases"
require_canonical_main
for BUILD_ENV in "${BUILD_ENVS[@]}"; do
    TAG="${TAGS[$BUILD_ENV]}"
    gh release edit "$TAG" -R "$REPO" --draft=false
    [[ "$(gh release view "$TAG" -R "$REPO" --json isDraft --jq '.isDraft')" == "false" ]] ||
        die "$TAG: verified release did not publish"
done

echo
echo "Published native-base generation v${ARTIFACT_GENERATION}:"
printf '  %s\n' "${TAGS[@]}"
