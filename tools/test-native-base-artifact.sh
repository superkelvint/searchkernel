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
printf '{"kind":"vespa-base","vespa_commit":"fixture-vespa-commit"}\n' >"$bundle/base/manifest.json"
printf 'CXX_INCLUDES = -I/work/vespa -isystem /fixture-deps/include\n' >"$configured/flags.make"
printf 'c++ CMakeFiles/searchcore_embedded_crud_spike_app.dir/embedded_crud_spike.cpp.o -o searchcore_embedded_crud_spike_app\n' >"$configured/link.txt"
printf '#define FIXTURE_DEP 1\n' >"$bundle/toolchain/vespa-deps/include/fixture.h"
python3 - "$bundle/manifest.json" "$vespa_toolchain/.searchkernel-vespa-artifact.json" <<'PY'
import json
import sys
payload = {
    "format_version": 1,
    "build_environment": "artifact-regression",
    "native_target": "x86_64-unknown-linux-gnu",
    "vespa_commit": "fixture-vespa-commit",
}
for path in sys.argv[1:]:
    with open(path, "w", encoding="utf-8") as stream:
        json.dump(payload, stream, sort_keys=True)
        stream.write("\n")
PY
archive="$fixture/searchkernel-vespa-base.tar.gz"
(
    cd "$bundle_parent"
    tar -czf "$archive" searchkernel-vespa-base
)
sha256sum "$archive" >"$archive.sha256"
artifact_url="file://$archive"
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

# Read-only lookup must not download or mutate the published prerequisite.
if run_resolver path >"$fixture/cold-path.out" 2>"$fixture/cold-path.err"; then
    echo 'cold path lookup unexpectedly installed a published base' >&2
    exit 1
fi
[[ ! -e "$cache_home/searchkernel/native-sdk-cache/artifact-regression" ]]

if ! after=$(run_resolver ensure 2>"$fixture/install.err"); then
    cat "$fixture/install.err" >&2
    exit 1
fi
expected="$cache_home/searchkernel/native-sdk-cache/artifact-regression/x86_64-unknown-linux-gnu/objects/fixture-vespa-commit"
[[ "$after" == "$expected" ]]
[[ -s "$after/lib/libvespalib.so" ]]
[[ ! -e "$after/lib/libsearchkernel_vespa.so" ]]
[[ -z "$(find "$after" -perm /222 -print -quit)" ]]
grep -q 'installed published Vespa base' "$fixture/install.err"

warm=$(run_resolver path)
[[ "$warm" == "$after" ]]
resolved_root=$(run_resolver vespa-root)
[[ "$resolved_root" == "$cache_home/searchkernel/native-sdk-cache/artifact-regression/x86_64-unknown-linux-gnu/toolchains/fixture-vespa-commit/upstream/vespa" ]]
[[ -f "$resolved_root/.searchkernel-vespa-artifact.json" ]]
[[ -f "$resolved_root/searchcore/src/tests/proton/documentdb/CMakeFiles/searchcore_embedded_crud_spike_app.dir/flags.make" ]]

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


# Private GitHub release assets must use the workflow token. A fake curl keeps
# this regression hermetic while asserting that the installer supplies auth.
fake_bin="$fixture/fake-bin"
mkdir -p "$fake_bin"
cat >"$fake_bin/curl" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
auth=false
output=
url=
while (($#)); do
    case "$1" in
        -H)
            [[ "$2" == "Authorization: Bearer \${EXPECTED_GITHUB_TOKEN:?}" ]] && auth=true
            shift 2
            ;;
        -o)
            output=$2
            shift 2
            ;;
        -*)
            shift
            ;;
        *)
            url=$1
            shift
            ;;
    esac
done
[[ "$auth" == true ]] || { echo "missing GitHub release auth header" >&2; exit 22; }
[[ -n "$output" && -n "$url" ]]
case "$url" in
    *.sha256) cp "\${FIXTURE_ARCHIVE:?}.sha256" "$output" ;;
    *) cp "\${FIXTURE_ARCHIVE:?}" "$output" ;;
esac
SH
chmod +x "$fake_bin/curl"

private_cache="$fixture/private-cache"
private_url="https://github.com/superkelvint/searchkernel/releases/download/fixture/private-base.tar.gz"
PATH="$fake_bin:$PATH" \
EXPECTED_GITHUB_TOKEN=fixture-token \
FIXTURE_ARCHIVE="$archive" \
GITHUB_TOKEN=fixture-token \
    bash "$clone/tools/install-native-base-artifact.sh" \
        "$private_url" \
        "$private_cache" \
        artifact-regression \
        x86_64-unknown-linux-gnu \
        fixture-vespa-commit >/dev/null
[[ -s "$private_cache/objects/fixture-vespa-commit/lib/libvespalib.so" ]]

echo 'native base artifact regression PASS'