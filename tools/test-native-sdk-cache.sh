#!/usr/bin/env bash
set -euo pipefail
trap 'echo "native SDK cache regression failed at line $LINENO: $BASH_COMMAND" >&2' ERR

source_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)

# Fail directly on resolver syntax damage instead of allowing later behavioral
# assertions to obscure a malformed shell function.
bash -n "$source_root/tools/resolve-native-sdk.sh"
bash -n "$source_root/tools/install-native-base-artifact.sh"
bash -n "$source_root/tools/package-native-base-artifact.sh"
bash -n "$source_root/tools/publish-native-base.sh"
bash -n "$source_root/tools/build-native-base-release.sh"
bash -n "$source_root/tools/build-native-base-adapter.sh"
bash -n "$source_root/tools/verify-native-base-archive.sh"
bash -n "$source_root/tools/fetch-native-base.sh"
bash -n "$source_root/tools/test-native-base-artifact.sh"
bash -n "$source_root/tools/test-fetch-native-base.sh"
bash -n "$source_root/tools/test-native-base-publish-policy.sh"

# Docker-backed native provisioning must consume the digest-addressed Vespa
# bundle already mounted by run-in-ci-image.sh. It must never consult the
# parallel GitHub native-base release stream and replace that mounted authority.
python3 - "$source_root/tools/resolve-native-sdk.sh" <<'PY'
import pathlib
import sys

text = pathlib.Path(sys.argv[1]).read_text(encoding="utf-8")
if "canonical_mounted_install()" not in text:
    raise SystemExit("native SDK resolver is missing the canonical mounted-image boundary")
if 'SEARCHKERNEL_IN_CI_IMAGE' not in text or '/opt/searchkernel/native-validation-pass' not in text:
    raise SystemExit("mounted-image detection must recognize the canonical CI-image execution path")
valid = text.index("if valid_install; then")
mounted = text.index("if canonical_mounted_install; then", valid)
freshness = text.index("if published_install_is_current; then", valid)
if not (valid < mounted < freshness):
    raise SystemExit(
        "mounted canonical Vespa image must short-circuit before legacy release freshness checks"
    )
PY

bash "$source_root/tools/test-hermetic-env.sh"
python3 "$source_root/tools/test_check_native_base_release_selection.py"
python3 "$source_root/tools/test_check_native_runtime_closure.py"
python3 "$source_root/tools/test_bundle_native_runtime_closure.py"
grep -Fq 'bundle-native-runtime-closure.py' "$source_root/tools/package-native-base-artifact.sh"
grep -Fq 'check-native-runtime-closure.py' "$source_root/tools/package-native-base-artifact.sh"
grep -Fq '"$BUNDLE/base/lib" --require-provenance' "$source_root/tools/package-native-base-artifact.sh"
grep -Fq 'git/ref/heads/main' "$source_root/tools/publish-native-base.sh"
grep -Fq 'tools/install-native-base-artifact.sh' "$source_root/tools/verify-native-base-archive.sh"
grep -Fq 'check-native-runtime-closure.py' "$source_root/tools/build-native-base-release.sh"
grep -Fq 'bundle-native-runtime-closure.py' "$source_root/tools/build-native-base-release.sh"
grep -Fq -- '--extra-elf "$STAGED_ADAPTER/sdk/lib/libsearchkernel_vespa.so.2"' "$source_root/tools/build-native-base-release.sh"
grep -Fq '"$BASE/lib" "$OUTPUT/sdk/lib/libsearchkernel_vespa.so.2"' "$source_root/tools/build-native-base-adapter.sh"
grep -Fq 'verify-native-base-archive.sh' "$source_root/tools/build-native-base-release.sh"
grep -Fq 'verify-native-base-archive.sh' "$source_root/tools/publish-native-base.sh"
grep -Fq '"searchkernel_commit": args.searchkernel_commit' "$source_root/tools/write-native-base-manifest.py"
grep -Fq 'section "Revalidating canonical publication source"' "$source_root/tools/publish-native-base.sh"
grep -Fq -- '--draft' "$source_root/tools/publish-native-base.sh"
grep -Fq -- '--target "$SOURCE_COMMIT"' "$source_root/tools/publish-native-base.sh"
python3 - "$source_root/tools/publish-native-base.sh" <<'PY'
import pathlib
import sys

text = pathlib.Path(sys.argv[1]).read_text(encoding="utf-8")
draft_upload = text.index('gh release create "$TAG"')
remote_verify = text.index('section "Verifying uploaded draft bytes $TAG"')
publish_verified = text.index('section "Publishing verified native-base drafts"')
if not (draft_upload < remote_verify < publish_verified):
    raise SystemExit(
        "native-base release must stay draft through remote verification and publish only afterward"
    )
PY
python3 - "$source_root/tools/publish-native-base.sh" <<'PY'
import pathlib
import sys

text = pathlib.Path(sys.argv[1]).read_text(encoding="utf-8")
section = text.index('section "Revalidating canonical publication source"')
preflight = text.index('section "Publication preflight"', section)
between = text[section:preflight]
if "require_canonical_main" not in between:
    raise SystemExit("publisher must revalidate canonical main before release preflight")
PY
bash -n "$source_root/reference/vespa/common/verify-common.sh"
bash -n "$source_root/reference/vespa/common/test-published-provenance.sh"
bash "$source_root/tools/test-native-base-artifact.sh"
bash "$source_root/tools/test-fetch-native-base.sh"
bash "$source_root/tools/test-native-base-publish-policy.sh"
bash "$source_root/reference/vespa/common/test-published-provenance.sh"

# The resolver executes the SDK builder directly. Git mode is therefore part of
# the build contract: a content-identical 0644 checkout fails before any native
# provisioning can begin.
if [[ ! -x "$source_root/core/native/vespa/build-sdk.sh" ]]; then
    echo 'native SDK builder must be tracked executable' >&2
    exit 1
fi

# Consumer Cargo builds automatically provision and validate the pinned base.
grep -q 'resolve_shared_native_sdk(&manifest_dir, &target, "ensure")' \
    "$source_root/core/crates/searchkernel-vespa-bindings/build.rs"
if grep -Fq 'resolve_shared_native_sdk(&manifest_dir, &target, "path")' \
    "$source_root/core/crates/searchkernel-vespa-bindings/build.rs"; then
    echo 'Consumer Cargo builds must not require manual native-base provisioning' >&2
    exit 1
fi
grep -q 'do not set SK_NATIVE_SDK' \
    "$source_root/core/crates/searchkernel-vespa-bindings/build.rs"
grep -q '.searchkernel-native-base-artifact.json' \
    "$source_root/core/crates/searchkernel-vespa-bindings/build.rs"

verify_common="$source_root/reference/vespa/common/verify-common.sh"
if grep -Fq 'core/native/sdk/linux-x86_64' "$verify_common"; then
    echo 'Vespa verifier must not require a clone-local packaged SDK' >&2
    exit 1
fi
grep -Fq 'tools/resolve-native-sdk.sh" path' "$verify_common"
grep -Fq 'tools/resolve-native-sdk.sh" vespa-root' "$verify_common"
grep -Fq 'SK_NATIVE_SDK_WAS_EXPLICIT=0' "$verify_common"
grep -Fq 'unset SK_NATIVE_SDK' "$verify_common"
# Workspace binding checks use the source header; probe SDK paths must not leak
# into normal Cargo builds.
grep -Fq 'SK_NATIVE_ABI_HEADER="$REPO_ROOT/core/native/vespa/searchkernel_vespa.h"' "$verify_common"
# The synthesized native-probe adapter must remain a shell-local view. Ordinary
# SearchReply probes consume $SDK; exporting it through SK_NATIVE_SDK would
# make Cargo mistake it for a full explicit SDK.
searchreply_verify="$source_root/reference/vespa/searchreply/verify-searchreply.sh"
if grep -Fq 'SK_NATIVE_SDK="$SDK"' "$searchreply_verify"; then
    echo 'SearchReply must not export the synthesized native-probe SDK to Cargo' >&2
    exit 1
fi
grep -Fq -- '-I"$SDK/include"' "$searchreply_verify"
grep -Fq 'LD_LIBRARY_PATH="$SDK/lib' "$searchreply_verify"
fixture=$(mktemp -d "${TMPDIR:-/tmp}/searchkernel-sdk-cache.XXXXXX")
cleanup_fixture() {
    chmod -R u+w "$fixture" 2>/dev/null || true
    rm -rf "$fixture"
}
trap cleanup_fixture EXIT

repo="$fixture/repo"
mkdir -p "$repo/tools" "$repo/core/native/vespa" "$repo/upstream/vespa" "$repo/vespa-deps/lib64"
cp "$source_root/tools/resolve-native-sdk.sh" \
   "$source_root/tools/resolve-python.sh" \
   "$source_root/tools/resolve-rust.sh" \
   "$repo/tools/"
cat >"$repo/core/native/vespa/searchkernel_vespa.h" <<'EOF'
#define SK_NATIVE_ABI_VERSION 1U
EOF
printf '{"native_abi_version":1,"vespa_commit":"fixture-vespa-commit"}\n' \
    >"$repo/core/native/vespa/manifest.json"
echo 'adapter-v1' >"$repo/core/native/vespa/adapter.cc"
echo 'vespa-source' >"$repo/upstream/vespa/tracked.txt"
printf 'dependency-v1\n' >"$repo/vespa-deps/lib64/libsearchkernel_fixture.so"

cat >"$repo/tools/fake-sdk-builder.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
sdk=$SK_NATIVE_SDK
mkdir -p "$sdk/include" "$sdk/lib" "$sdk/share" "$SK_NATIVE_BUILD_DIR"
[[ "${SK_NATIVE_BASE_ONLY:-0}" == 1 ]] || {
    echo "cache fixture expected base-only builder mode" >&2
    exit 1
}
if [[ -n "${VESPA_LIBRARY_ROOT:-}" ]]; then
    find "$VESPA_LIBRARY_ROOT" -maxdepth 1 -name 'libvespa*.so*' -exec cp {} "$sdk/lib/" \;
else
    cat >"$SK_NATIVE_BUILD_DIR/cache_fixture_base.c" <<'C'
int searchkernel_cache_fixture_base(void) { return 1; }
C
    cc -shared -fPIC "$SK_NATIVE_BUILD_DIR/cache_fixture_base.c" \
        -Wl,-soname,libvespalib.so \
        -o "$sdk/lib/libvespalib.so"
fi
pinned=$(python3 - "$root/core/native/vespa/manifest.json" <<'PY'
import json
import sys
with open(sys.argv[1], encoding="utf-8") as stream:
    print(json.load(stream)["vespa_commit"])
PY
)
printf '{"kind":"vespa-base","vespa_commit":"%s"}\n' "$pinned" >"$sdk/manifest.json"
common=$(git -C "$root" rev-parse --git-common-dir)
[[ "$common" = /* ]] || common="$root/$common"
printf 'build\n' >>"$common/sdk-cache-build-count"
EOF
chmod +x "$repo/tools/"*.sh

git -C "$repo" init -q
git -C "$repo" config user.email test@example.com
git -C "$repo" config user.name 'SDK Cache Test'
git -C "$repo" add .
git -C "$repo" commit -qm 'fixture'

configured="$repo/upstream/vespa/searchcore/src/tests/proton/documentdb/CMakeFiles/searchcore_embedded_crud_spike_app.dir"
mkdir -p "$configured"
printf 'link\n' >"$configured/link.txt"
printf 'CXX_INCLUDES = \n' >"$configured/flags.make"

# Reproduce the post-#234 migration state: the configured Vespa tree still has
# CMake metadata but no built libvespa*.so files, while the pre-split full SDK
# already contains a self-contained Vespa runtime closure.
legacy_sdk="$repo/core/native/sdk/linux-x86_64"
mkdir -p "$legacy_sdk/lib"
legacy_dependency_source="$fixture/legacy-vespa-defaults.c"
cat >"$legacy_dependency_source" <<'C'
int searchkernel_cache_fixture_dependency(void) { return 3; }
C
cc -shared -fPIC "$legacy_dependency_source" \
    -Wl,-soname,libvespadefaults.so \
    -o "$legacy_sdk/lib/libvespadefaults.so"

legacy_source="$fixture/legacy-vespa.c"
cat >"$legacy_source" <<'C'
extern int searchkernel_cache_fixture_dependency(void);
int searchkernel_cache_fixture_base(void) {
    return searchkernel_cache_fixture_dependency();
}
C
cc -shared -fPIC "$legacy_source" \
    -L"$legacy_sdk/lib" -Wl,--no-as-needed -lvespadefaults \
    -Wl,-soname,libvespalib.so \
    -o "$legacy_sdk/lib/libvespalib.so"

# The full-SDK seed deliberately has no RPATH on libvespalib.so. Once both
# libraries are flattened into the immutable base, closure validation must
# resolve libvespadefaults.so from the packaged sibling directory rather than
# from the producer build tree or host.
if readelf -d "$legacy_sdk/lib/libvespalib.so" | grep -Eq '(RPATH|RUNPATH)'; then
    echo 'cache fixture unexpectedly encoded an RPATH/RUNPATH' >&2
    exit 1
fi

printf '{"native_abi_version":1,"vespa_commit":"fixture-vespa-commit"}\n' \
    >"$legacy_sdk/manifest.json"
cat >"$legacy_sdk/native-build-fingerprint-v2.txt" <<'EOF'
fingerprint-version=3
sha256=fixture-full-sdk
build.environment	cache-regression
cargo.target	x86_64-unknown-linux-gnu
fingerprint.scope	full
EOF

untrusted_library_root="$fixture/untrusted-vespa"
mkdir -p "$untrusted_library_root"
cat >"$fixture/untrusted-vespa.c" <<'C'
int searchkernel_untrusted_fixture(void) { return 99; }
C
cc -shared -fPIC "$fixture/untrusted-vespa.c" \
    -Wl,-soname,libvespalib.so \
    -o "$untrusted_library_root/libvespalib.so"

worktree="$fixture/worktree"
git -C "$repo" worktree add -q "$worktree" -b cache-test
echo 'unrelated rust-side change' >"$worktree/unrelated.txt"
git -C "$worktree" add unrelated.txt
git -C "$worktree" commit -qm 'unrelated branch change'

hermetic_home="$fixture/hermetic-home"
hermetic_tmp="$fixture/hermetic-tmp"
mkdir -p "$hermetic_home" "$hermetic_tmp"

run_resolver() {
    local root=$1
    local mode=${2:-ensure}
    local build_env=${3:-cache-regression}
    local native_target=${4:-x86_64-unknown-linux-gnu}
    local execution_path=${PATH:-/usr/local/bin:/usr/bin:/bin}
    (
        cd "$root"
        bash "$source_root/tools/run-hermetic-env.sh" \
            "PATH=$execution_path" \
            "HOME=$hermetic_home" \
            "TMPDIR=$hermetic_tmp" \
            "LANG=C" \
            "LC_ALL=C" \
            "SEARCHKERNEL_BUILD_ENV=$build_env" \
            "SK_NATIVE_TARGET=$native_target" \
            "XDG_CACHE_HOME=$fixture/machine-cache-home" \
            "SK_NATIVE_DISABLE_PUBLISHED_BASE=1" \
            "SK_NATIVE_CACHE_BUILDER=$root/tools/fake-sdk-builder.sh" \
            "VESPA_LIBRARY_ROOT=$untrusted_library_root" \
            -- bash "$root/tools/resolve-native-sdk.sh" "$mode"
    )
}

# Poison the parent process with representative explicit native/Vespa overrides.
# Every resolver invocation below must behave identically because the synthetic
# fixture receives only the inputs declared above.
export SEARCHKERNEL_AMBIENT_SENTINEL=must-not-leak
export SK_NATIVE_SDK="$fixture/ambient-sdk"
export SK_NATIVE_CACHE_DIR="$fixture/ambient-cache"
export SK_NATIVE_BASE_ARTIFACT_URL="file://$fixture/ambient-native-base.tar.gz"
export SK_NATIVE_BASE_ARTIFACT_GENERATION=999
export VESPA_ROOT="$fixture/ambient-vespa-root"
export VESPA_DEPS="$fixture/ambient-vespa-deps"
export VESPA_DEPS_LIB_DIR="$fixture/ambient-vespa-deps/lib"
export VESPA_EXTRA_LIB_DIR="$fixture/ambient-vespa-extra"
export VESPA_AWS_LIB_DIR="$fixture/ambient-vespa-aws"

common=$(git -C "$repo" rev-parse --git-common-dir)
[[ "$common" = /* ]] || common="$repo/$common"

if run_resolver "$repo" path >"$fixture/cold-path.out" 2>"$fixture/cold-path.err"; then
    echo 'native SDK cache path lookup unexpectedly succeeded on a cold cache' >&2
    exit 1
fi
[[ ! -e "$common/sdk-cache-build-count" ]]
grep -q 'native Vespa base is not installed for checkout fixture-vespa-commit' "$fixture/cold-path.err"

cp "$legacy_sdk/manifest.json" "$fixture/legacy-manifest.good"
printf '{"native_abi_version":1,"vespa_commit":"stale-vespa-commit"}\n' \
    >"$legacy_sdk/manifest.json"
if run_resolver "$repo" >"$fixture/stale.out" 2>"$fixture/stale.err"; then
    echo 'native SDK cache unexpectedly accepted stale legacy SDK provenance' >&2
    exit 1
fi
grep -q 'no compatible full SDK was available' "$fixture/stale.err"
[[ ! -e "$common/sdk-cache-build-count" ]]
mv "$fixture/legacy-manifest.good" "$legacy_sdk/manifest.json"

if run_resolver "$repo" ensure other-environment >"$fixture/legacy-env.out" 2>"$fixture/legacy-env.err"; then
    echo 'native SDK cache unexpectedly accepted a legacy SDK from another build environment' >&2
    exit 1
fi
grep -q 'no compatible full SDK was available' "$fixture/legacy-env.err"

if run_resolver "$repo" ensure cache-regression aarch64-unknown-linux-gnu >"$fixture/legacy-target.out" 2>"$fixture/legacy-target.err"; then
    echo 'native SDK cache unexpectedly accepted a legacy SDK from another native target' >&2
    exit 1
fi
grep -q 'no compatible full SDK was available' "$fixture/legacy-target.err"
[[ ! -e "$common/sdk-cache-build-count" ]]

# A configured upstream/vespa checkout with no built DSOs must be able to
# bootstrap itself once. This is the cold-machine path that previously stopped
# with "shared libraries are missing" even though the configured Vespa tree was
# present.
fake_bin="$fixture/fake-bin"
mkdir -p "$fake_bin"
cat >"$fake_bin/cmake" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
[[ "$#" -ge 6 ]]
[[ "$1" == "--build" ]]
vespa_root=$2
[[ "$3" == "--target" ]]
[[ "$4" == "searchcore_embedded_crud_spike_app" ]]
[[ "$5" == "--parallel" ]]
[[ "$6" =~ ^[0-9]+$ ]]
cat >"$vespa_root/cache_fixture_vespa.c" <<'C'
int searchkernel_cache_fixture_base(void) { return 7; }
C
cc -shared -fPIC "$vespa_root/cache_fixture_vespa.c" \
    -Wl,-soname,libvespalib.so \
    -o "$vespa_root/libvespalib.so"
printf 'build\n' >>"$vespa_root/cmake-build-count"
EOF
chmod +x "$fake_bin/cmake"

mv "$legacy_sdk" "$fixture/legacy-sdk.saved"
touch "$repo/upstream/vespa/CMakeCache.txt"
if ! source_built=$(PATH="$fake_bin:$PATH" run_resolver "$repo" ensure source-build 2>"$fixture/source-build.err"); then
    echo 'native SDK cache failed to bootstrap from configured upstream/vespa:' >&2
    cat "$fixture/source-build.err" >&2
    exit 1
fi
grep -q "building configured target 'searchcore_embedded_crud_spike_app' once" "$fixture/source-build.err"
[[ "$source_built" == *"/searchkernel/native-sdk-cache/source-build/x86_64-unknown-linux-gnu/objects/fixture-vespa-commit" ]]
[[ -s "$source_built/lib/libvespalib.so" ]]
[[ $(wc -l <"$repo/upstream/vespa/cmake-build-count") -eq 1 ]]

source_warm=$(PATH="$fake_bin:$PATH" run_resolver "$repo" path source-build)
[[ "$source_warm" == "$source_built" ]]
[[ $(wc -l <"$repo/upstream/vespa/cmake-build-count") -eq 1 ]]

rm -f "$repo/upstream/vespa/libvespalib.so" \
    "$repo/upstream/vespa/cache_fixture_vespa.c" \
    "$repo/upstream/vespa/cmake-build-count" \
    "$repo/upstream/vespa/CMakeCache.txt"
mv "$fixture/legacy-sdk.saved" "$legacy_sdk"
rm -f "$common/sdk-cache-build-count"

if ! first=$(run_resolver "$repo" 2>"$fixture/bootstrap.err"); then
    echo 'native SDK cache bootstrap failed:' >&2
    cat "$fixture/bootstrap.err" >&2
    exit 1
fi
[[ -d "$first" ]]
grep -q 'using Vespa shared libraries from compatible full SDK' "$fixture/bootstrap.err"
[[ -s "$first/lib/libvespalib.so" ]]
[[ ! -e "$first/native-build-fingerprint-v2.txt" ]]
grep -Eq '"kind"[[:space:]]*:[[:space:]]*"vespa-base"' "$first/manifest.json"
grep -Eq '"vespa_commit"[[:space:]]*:[[:space:]]*"fixture-vespa-commit"' "$first/manifest.json"
[[ "$first" == *"/searchkernel/native-sdk-cache/cache-regression/x86_64-unknown-linux-gnu/objects/fixture-vespa-commit" ]]
[[ ! -e "$first/lib/libsearchkernel_vespa.so" ]]
[[ ! -e "$first/include/searchkernel_vespa.h" ]]
[[ $(wc -l <"$common/sdk-cache-build-count") -eq 1 ]]

warm_path=$(run_resolver "$repo" path)
[[ "$warm_path" == "$first" ]]
[[ $(wc -l <"$common/sdk-cache-build-count") -eq 1 ]]

# A genuinely independent clone has a different Git common directory and none
# of the untracked configured Vespa outputs or pre-split SDK used to bootstrap
# the base. It must still resolve the already-installed machine base solely
# from the pinned Vespa checkout identity within the same compatibility namespace.
fresh_clone="$fixture/fresh-clone"
git clone -q "$repo" "$fresh_clone"
fresh_path=$(run_resolver "$fresh_clone" path 2>"$fixture/fresh-clone.err")
[[ "$fresh_path" == "$first" ]]
fresh_vespa_root=$(run_resolver "$fresh_clone" vespa-root)
[[ "$fresh_vespa_root" == "$repo/upstream/vespa" ]]
[[ $(wc -l <"$common/sdk-cache-build-count") -eq 1 ]]

if run_resolver "$fresh_clone" path other-environment >"$fixture/other-env.out" 2>"$fixture/other-env.err"; then
    echo 'native SDK cache unexpectedly reused a base across build environments' >&2
    exit 1
fi
grep -q 'native Vespa base is not installed for checkout fixture-vespa-commit' "$fixture/other-env.err"

if run_resolver "$fresh_clone" path cache-regression aarch64-unknown-linux-gnu >"$fixture/other-target.out" 2>"$fixture/other-target.err"; then
    echo 'native SDK cache unexpectedly reused a base across native targets' >&2
    exit 1
fi
grep -q 'native Vespa base is not installed for checkout fixture-vespa-commit' "$fixture/other-target.err"
[[ $(wc -l <"$common/sdk-cache-build-count") -eq 1 ]]

cp "$fresh_clone/core/native/vespa/manifest.json" "$fixture/fresh-manifest.saved"
printf '{"native_abi_version":1,"vespa_commit":"fixture-vespa-next"}\n' \
    >"$fresh_clone/core/native/vespa/manifest.json"
if run_resolver "$fresh_clone" path >"$fixture/new-pin.out" 2>"$fixture/new-pin.err"; then
    echo 'native SDK cache unexpectedly reused a base across different Vespa checkout hashes' >&2
    exit 1
fi
grep -q 'native Vespa base is not installed for checkout fixture-vespa-next' "$fixture/new-pin.err"
mv "$fixture/fresh-manifest.saved" "$fresh_clone/core/native/vespa/manifest.json"

second=$(run_resolver "$worktree")
[[ "$second" == "$first" ]]
[[ $(wc -l <"$common/sdk-cache-build-count") -eq 1 ]]

printf 'adapter-v2\n' >>"$worktree/core/native/vespa/adapter.cc"
third=$(run_resolver "$worktree")
[[ "$third" == "$first" ]]
[[ $(wc -l <"$common/sdk-cache-build-count") -eq 1 ]]
[[ ! -e "$third/lib/libsearchkernel_vespa.so" ]]
[[ ! -e "$third/include/searchkernel_vespa.h" ]]
[[ -z "$(find "$third" -perm /222 -print -quit)" ]]

printf 'legacy-adapter-bytes-changed\n' >"$legacy_sdk/lib/libsearchkernel_vespa.so"
same_after_legacy_adapter=$(run_resolver "$worktree")
[[ "$same_after_legacy_adapter" == "$third" ]]
[[ $(wc -l <"$common/sdk-cache-build-count") -eq 1 ]]

printf 'unrelated-task-dependency\n' >"$worktree/vespa-deps/lib64/libsearchkernel_fixture.so"
same_after_unrelated_deps=$(run_resolver "$worktree")
[[ "$same_after_unrelated_deps" == "$third" ]]
[[ $(wc -l <"$common/sdk-cache-build-count") -eq 1 ]]

cat >"$legacy_source" <<'C'
int searchkernel_cache_fixture_base(void) { return 2; }
C
cc -shared -fPIC "$legacy_source" \
    -Wl,-soname,libvespalib.so \
    -o "$legacy_sdk/lib/libvespalib.so"
same_after_source_bytes_change=$(run_resolver "$worktree")
[[ "$same_after_source_bytes_change" == "$third" ]]
[[ $(wc -l <"$common/sdk-cache-build-count") -eq 1 ]]

printf 'dependency-v2-with-different-size\n' >"$repo/vespa-deps/lib64/libsearchkernel_fixture.so"
same_after_dependency_bytes_change=$(run_resolver "$worktree")
[[ "$same_after_dependency_bytes_change" == "$third" ]]
[[ $(wc -l <"$common/sdk-cache-build-count") -eq 1 ]]

chmod u+w "$third/lib"
rm "$third/lib/libvespalib.so"
chmod a-w "$third/lib"
healed=$(run_resolver "$worktree" 2>"$fixture/incomplete.err")
[[ "$healed" == "$third" ]]
[[ -s "$third/lib/libvespalib.so" ]]
[[ $(wc -l <"$common/sdk-cache-build-count") -eq 2 ]]
grep -q 'quarantining and rebuilding under the checkout lock' "$fixture/incomplete.err"

chmod u+w "$third/lib" "$third/lib/libvespalib.so"
truncate -s 1 "$third/lib/libvespalib.so"
chmod a-w "$third/lib/libvespalib.so" "$third/lib"
corrupted_healed=$(run_resolver "$worktree" 2>"$fixture/corrupt.err")
[[ "$corrupted_healed" == "$third" ]]
[[ -s "$third/lib/libvespalib.so" ]]
[[ $(wc -l <"$common/sdk-cache-build-count") -eq 3 ]]
grep -q 'quarantining and rebuilding under the checkout lock' "$fixture/corrupt.err"

chmod u+w "$third/lib"
rm "$third/lib/libvespalib.so"
chmod a-w "$third/lib"
before=$(wc -l <"$common/sdk-cache-build-count")
run_resolver "$worktree" >"$fixture/race-a.out" 2>"$fixture/race-a.err" &
pid_a=$!
run_resolver "$worktree" >"$fixture/race-b.out" 2>"$fixture/race-b.err" &
pid_b=$!
wait "$pid_a"
wait "$pid_b"
[[ "$(cat "$fixture/race-a.out")" == "$third" ]]
[[ "$(cat "$fixture/race-b.out")" == "$third" ]]
after=$(wc -l <"$common/sdk-cache-build-count")
[[ $((after - before)) -eq 1 ]]
[[ -s "$third/lib/libvespalib.so" ]]

echo 'native SDK cache regression PASS'
