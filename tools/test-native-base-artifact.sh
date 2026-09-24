#!/usr/bin/env bash
set -euo pipefail
trap 'echo "native base artifact regression failed at line $LINENO: $BASH_COMMAND" >&2' ERR

source_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
bash -n "$source_root/tools/install-native-base-artifact.sh"
bash -n "$source_root/tools/package-native-base-artifact.sh"
bash -n "$source_root/tools/resolve-native-sdk.sh"

fixture=$(mktemp -d "${TMPDIR:-/tmp}/searchkernel-native-base-artifact.XXXXXX")
cleanup() {
    chmod -R u+w "$fixture" 2>/dev/null || true
    rm -rf "$fixture"
}
trap cleanup EXIT

repo="$fixture/repo"
mkdir -p "$repo/tools" "$repo/core/native/vespa"
cp "$source_root/tools/resolve-native-sdk.sh" "$repo/tools/"
cp "$source_root/tools/install-native-base-artifact.sh" "$repo/tools/"
chmod +x "$repo/tools/"*.sh
printf '{"vespa_commit":"fixture-vespa-commit"}\n' >"$repo/core/native/vespa/manifest.json"

git -C "$repo" init -q
git -C "$repo" config user.email test@example.com
git -C "$repo" config user.name 'Native Artifact Test'
git -C "$repo" add .
git -C "$repo" commit -qm 'fixture'

clone="$fixture/fresh-clone"
git clone -q "$repo" "$clone"
[[ ! -e "$clone/upstream/vespa" ]]
[[ ! -e "$clone/core/native/sdk" ]]

bundle_parent="$fixture/published"
bundle="$bundle_parent/searchkernel-vespa-base"
vespa_toolchain="$bundle/toolchain/upstream/vespa"
configured="$vespa_toolchain/searchcore/src/tests/proton/documentdb/CMakeFiles/searchcore_embedded_crud_spike_app.dir"
mkdir -p "$bundle/base/lib" "$configured" "$bundle/toolchain/vespa-deps/include" "$bundle/toolchain/vespa-deps/lib64"
cat >"$fixture/libvespa.c" <<'C'
int searchkernel_published_fixture(void) { return 1; }
C
cc -shared -fPIC "$fixture/libvespa.c" -Wl,-soname,libvespalib.so -o "$bundle/base/lib/libvespalib.so"
ln -s libvespalib.so "$bundle/base/lib/libvespalib.so.fixture"
ln "$bundle/base/lib/libvespalib.so" "$bundle/base/lib/libvespalib.so.hardlink"
printf '{"kind":"vespa-base","vespa_commit":"fixture-vespa-commit"}\n' >"$bundle/base/manifest.json"
printf 'generation-2\n' >"$bundle/base/generation.txt"
printf 'CXX_INCLUDES = -I/work/vespa -isystem /fixture-deps/include\n' >"$configured/flags.make"
static_dir="$vespa_toolchain/searchcore/src/vespa/searchcore/proton/server"
mkdir -p "$static_dir"
printf 'fixture-static\n' >"$static_dir/libsearchcore_server.a"
printf 'c++ CMakeFiles/searchcore_embedded_crud_spike_app.dir/embedded_crud_spike.cpp.o ../../../vespa/searchcore/proton/server/libsearchcore_server.a -o searchcore_embedded_crud_spike_app\n' >"$configured/link.txt"
printf '#define FIXTURE_DEP 1\n' >"$bundle/toolchain/vespa-deps/include/fixture.h"
python3 "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/test-native-base-artifact-python-1.py" "$bundle/manifest.json" "$vespa_toolchain/.searchkernel-vespa-artifact.json"
archive="$fixture/searchkernel-vespa-base-v2-artifact-regression-x86_64-unknown-linux-gnu-fixture-vespa-commit.tar.gz"
(
    cd "$bundle_parent"
    tar -czf "$archive" searchkernel-vespa-base
)
sha256sum "$archive" >"$archive.sha256"
artifact_url="file://$archive"

# Schema-1 is not accepted generically. Compatibility requires the explicit
# static-link-closure attestation and the installer still verifies that closure.
python3 "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/test-native-base-artifact-python-2.py" "$bundle/manifest.json" "$vespa_toolchain/.searchkernel-vespa-artifact.json"
bad_legacy="$fixture/searchkernel-vespa-base-v2-no-closure-artifact-regression-x86_64-unknown-linux-gnu-fixture-vespa-commit.tar.gz"
(
    cd "$bundle_parent"
    tar -czf "$bad_legacy" searchkernel-vespa-base
)
sha256sum "$bad_legacy" >"$bad_legacy.sha256"
python3 "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/test-native-base-artifact-python-3.py" "$bundle/manifest.json" "$vespa_toolchain/.searchkernel-vespa-artifact.json"

# Schema-1 compatibility is deliberately pinned to the canonical generation 2.
# A later generation must use the current schema even if it carries the closure
# attestation.
python3 "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/test-native-base-artifact-python-4.py" "$bundle/manifest.json" "$vespa_toolchain/.searchkernel-vespa-artifact.json"
bad_legacy_generation="$fixture/searchkernel-vespa-base-v3-legacy-schema-artifact-regression-x86_64-unknown-linux-gnu-fixture-vespa-commit.tar.gz"
(
    cd "$bundle_parent"
    tar -czf "$bad_legacy_generation" searchkernel-vespa-base
)
sha256sum "$bad_legacy_generation" >"$bad_legacy_generation.sha256"
python3 "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/test-native-base-artifact-python-5.py" "$bundle/manifest.json" "$vespa_toolchain/.searchkernel-vespa-artifact.json"

cache_home="$fixture/cache-home"

run_resolver() {
    local mode=${1:-ensure}
    local build_env=${2:-artifact-regression}
    local native_target=${3:-x86_64-unknown-linux-gnu}
    local url=${4:-$artifact_url}
    (
        cd "$clone"
        SEARCHKERNEL_BUILD_ENV="$build_env" \
        SK_NATIVE_TARGET="$native_target" \
        XDG_CACHE_HOME="$cache_home" \
        SK_NATIVE_BASE_ARTIFACT_URL="$url" \
        SK_NATIVE_CACHE_BUILDER=/bin/false \
            bash "$clone/tools/resolve-native-sdk.sh" "$mode"
    )
}

# A schema-1 artifact without the closure attestation must still be rejected.
if run_resolver ensure artifact-regression x86_64-unknown-linux-gnu "file://$bad_legacy" >"$fixture/bad-legacy.out" 2>"$fixture/bad-legacy.err"; then
    echo 'unattested schema-1 artifact unexpectedly installed' >&2
    exit 1
fi
grep -q 'unsupported published Vespa base format' "$fixture/bad-legacy.err"
[[ ! -e "$cache_home/searchkernel/native-sdk-cache/artifact-regression/x86_64-unknown-linux-gnu/objects/fixture-vespa-commit" ]]
[[ ! -e "$cache_home/searchkernel/native-sdk-cache/artifact-regression/x86_64-unknown-linux-gnu/toolchains/fixture-vespa-commit" ]]
rm -rf "$cache_home"

# A schema-1 artifact from any generation other than the canonical v2 must be
# rejected even if it claims complete static-link closure.
if run_resolver ensure artifact-regression x86_64-unknown-linux-gnu "file://$bad_legacy_generation" >"$fixture/bad-legacy-generation.out" 2>"$fixture/bad-legacy-generation.err"; then
    echo 'schema-1 non-v2 artifact unexpectedly installed' >&2
    exit 1
fi
grep -q 'unsupported published Vespa base format' "$fixture/bad-legacy-generation.err"
[[ ! -e "$cache_home/searchkernel/native-sdk-cache/artifact-regression/x86_64-unknown-linux-gnu/objects/fixture-vespa-commit" ]]
[[ ! -e "$cache_home/searchkernel/native-sdk-cache/artifact-regression/x86_64-unknown-linux-gnu/toolchains/fixture-vespa-commit" ]]
rm -rf "$cache_home"

# Read-only lookup must not download or mutate the published prerequisite.
if run_resolver path >"$fixture/cold-path.out" 2>"$fixture/cold-path.err"; then
    echo 'cold path lookup unexpectedly installed a published base' >&2
    exit 1
fi
[[ ! -e "$cache_home/searchkernel/native-sdk-cache/artifact-regression" ]]

# Reproduce a persistent runner that already has the original schema-1 cache.
# Its installed toolchain marker predates artifact_generation and closure
# metadata. ensure must recognize it as generation 1 via the existing
# format_version fallback, classify it as stale, and replace it with the
# host-selected published artifact without requiring cache deletion.
cache_root="$cache_home/searchkernel/native-sdk-cache/artifact-regression/x86_64-unknown-linux-gnu"
stale_object="$cache_root/objects/fixture-vespa-commit"
stale_toolchain="$cache_root/toolchains/fixture-vespa-commit"
mkdir -p "$stale_object" "$stale_toolchain" "$cache_root/checkouts"
cp -a "$bundle/base/." "$stale_object/"
cp -a "$bundle/toolchain/." "$stale_toolchain/"
python3 "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/test-native-base-artifact-python-6.py" "$stale_toolchain/upstream/vespa/.searchkernel-vespa-artifact.json"
ln -s "$stale_toolchain/upstream/vespa" "$cache_root/checkouts/fixture-vespa-commit"
chmod -R a-w "$stale_object" "$stale_toolchain"

if ! after=$(run_resolver ensure 2>"$fixture/install.err"); then
    cat "$fixture/install.err" >&2
    exit 1
fi
expected="$cache_home/searchkernel/native-sdk-cache/artifact-regression/x86_64-unknown-linux-gnu/objects/fixture-vespa-commit"
[[ "$after" == "$expected" ]]
[[ -s "$after/lib/libvespalib.so" ]]
[[ -L "$after/lib/libvespalib.so.fixture" ]]
[[ "$(readlink "$after/lib/libvespalib.so.fixture")" == "libvespalib.so" ]]
[[ -f "$after/lib/libvespalib.so.hardlink" ]]
[[ "$(stat -c %i "$after/lib/libvespalib.so")" == "$(stat -c %i "$after/lib/libvespalib.so.hardlink")" ]]
[[ ! -e "$after/lib/libsearchkernel_vespa.so" ]]
[[ -z "$(find "$after" ! -type l -perm /222 -print -quit)" ]]
[[ "$(cat "$after/generation.txt")" == "generation-2" ]]
python3 "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/test-native-base-artifact-python-7.py" "$after/.searchkernel-native-base-artifact.json"
grep -q 'published native-base cache is older than the desired generation; refresh required' "$fixture/install.err"
grep -q 'refreshing published native-base cache to generation v2' "$fixture/install.err"
grep -q 'installed published Vespa base generation v2' "$fixture/install.err"

warm=$(run_resolver path)
[[ "$warm" == "$after" ]]
resolved_root=$(run_resolver vespa-root)
[[ "$resolved_root" == "$cache_home/searchkernel/native-sdk-cache/artifact-regression/x86_64-unknown-linux-gnu/toolchains/fixture-vespa-commit/upstream/vespa" ]]
[[ -f "$resolved_root/.searchkernel-vespa-artifact.json" ]]
[[ -f "$resolved_root/searchcore/src/tests/proton/documentdb/CMakeFiles/searchcore_embedded_crud_spike_app.dir/flags.make" ]]
[[ -f "$resolved_root/searchcore/src/vespa/searchcore/proton/server/libsearchcore_server.a" ]]

# A newer published generation for the same Vespa pin must replace the shared
# object/toolchain atomically at the same cache path. No manual cache deletion
# is allowed, and read-only "path" lookups remain offline.
printf 'generation-3\n' >"$bundle/base/generation.txt"
python3 "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/test-native-base-artifact-python-8.py" "$bundle/manifest.json" "$vespa_toolchain/.searchkernel-vespa-artifact.json"
archive_v3="$fixture/searchkernel-vespa-base-v3-artifact-regression-x86_64-unknown-linux-gnu-fixture-vespa-commit.tar.gz"
(
    cd "$bundle_parent"
    tar -czf "$archive_v3" searchkernel-vespa-base
)
sha256sum "$archive_v3" >"$archive_v3.sha256"

refreshed=$(run_resolver ensure artifact-regression x86_64-unknown-linux-gnu "file://$archive_v3" 2>"$fixture/refresh.err")
[[ "$refreshed" == "$after" ]]
[[ "$(cat "$refreshed/generation.txt")" == "generation-3" ]]
grep -q 'refreshing published native-base cache to generation v3' "$fixture/refresh.err"
grep -q 'installed published Vespa base generation v3' "$fixture/refresh.err"
python3 "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/test-native-base-artifact-python-9.py" "$refreshed/.searchkernel-native-base-artifact.json" "$resolved_root/.searchkernel-vespa-artifact.json"

# A stale override must never downgrade an already newer installed generation.
not_downgraded=$(run_resolver ensure artifact-regression x86_64-unknown-linux-gnu "$artifact_url")
[[ "$not_downgraded" == "$after" ]]
[[ "$(cat "$not_downgraded/generation.txt")" == "generation-3" ]]

if run_resolver ensure other-environment x86_64-unknown-linux-gnu >"$fixture/wrong-env.out" 2>"$fixture/wrong-env.err"; then
    echo 'published base unexpectedly crossed build environments' >&2
    exit 1
fi
grep -q 'build_environment mismatch' "$fixture/wrong-env.err"

if run_resolver ensure artifact-regression aarch64-unknown-linux-gnu >"$fixture/wrong-target.out" 2>"$fixture/wrong-target.err"; then
    echo 'published base unexpectedly crossed native targets' >&2
    exit 1
fi
grep -q 'native_target mismatch' "$fixture/wrong-target.err"

cp "$clone/core/native/vespa/manifest.json" "$fixture/manifest.saved"
printf '{"vespa_commit":"fixture-vespa-next"}\n' >"$clone/core/native/vespa/manifest.json"
if run_resolver ensure artifact-regression x86_64-unknown-linux-gnu >"$fixture/wrong-pin.out" 2>"$fixture/wrong-pin.err"; then
    echo 'published base unexpectedly crossed Vespa pins' >&2
    exit 1
fi
grep -q 'vespa_commit mismatch' "$fixture/wrong-pin.err"
mv "$fixture/manifest.saved" "$clone/core/native/vespa/manifest.json"

corrupt="$fixture/corrupt.tar.gz"
cp "$archive" "$corrupt"
printf 'corrupt\n' >>"$corrupt"
cp "$archive.sha256" "$corrupt.sha256"
if run_resolver ensure corrupt-environment x86_64-unknown-linux-gnu "file://$corrupt" >"$fixture/corrupt.out" 2>"$fixture/corrupt.err"; then
    echo 'corrupt published base unexpectedly installed' >&2
    exit 1
fi
grep -q 'checksum mismatch' "$fixture/corrupt.err"

truncated="$fixture/truncated.tar.gz"
head -c 32 "$archive" >"$truncated"
sha256sum "$truncated" >"$truncated.sha256"
if run_resolver ensure truncated-environment x86_64-unknown-linux-gnu "file://$truncated" >"$fixture/truncated.out" 2>"$fixture/truncated.err"; then
    echo 'truncated published base unexpectedly installed' >&2
    exit 1
fi

missing="$fixture/missing.tar.gz"
if run_resolver ensure missing-environment x86_64-unknown-linux-gnu "file://$missing" >"$fixture/missing.out" 2>"$fixture/missing.err"; then
    echo 'missing published base unexpectedly installed' >&2
    exit 1
fi
grep -q 'published Vespa base is unavailable' "$fixture/missing.err"
grep -q 'no configured Vespa build outputs found' "$fixture/missing.err"

echo 'native base artifact regression PASS'