#!/usr/bin/env bash
set -euo pipefail

source_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
fixture=$(mktemp -d "${TMPDIR:-/tmp}/searchkernel-sdk-cache.XXXXXX")
trap 'rm -rf "$fixture"' EXIT

repo="$fixture/repo"
mkdir -p "$repo/tools" "$repo/core/native/vespa" "$repo/upstream/vespa"
cp "$source_root/tools/resolve-native-sdk.sh" "$repo/tools/"
cp "$source_root/tools/native-build-fingerprint.py" "$repo/tools/"
cat >"$repo/core/native/vespa/searchkernel_vespa.h" <<'EOF'
#define SK_NATIVE_ABI_VERSION 1U
EOF
echo 'adapter-v1' >"$repo/core/native/vespa/adapter.cc"
echo 'vespa-source' >"$repo/upstream/vespa/tracked.txt"

cat >"$repo/tools/fake-sdk-builder.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
sdk=$SK_NATIVE_SDK
mkdir -p "$sdk/include" "$sdk/lib" "$sdk/share"
cp "$root/core/native/vespa/searchkernel_vespa.h" "$sdk/include/"
printf 'fake-sdk\n' >"$sdk/lib/libsearchkernel_vespa.so"
printf '{}\n' >"$sdk/manifest.json"
python3 "$root/tools/native-build-fingerprint.py" \
    --root "$root" \
    --vespa-root "$VESPA_ROOT" \
    --target "$SK_NATIVE_TARGET" \
    --output "$sdk/native-build-fingerprint-v2.txt"
common=$(git -C "$root" rev-parse --git-common-dir)
[[ "$common" = /* ]] || common="$root/$common"
printf 'build\n' >>"$common/sdk-cache-build-count"
EOF
chmod +x "$repo/tools/"*.sh "$repo/tools/native-build-fingerprint.py"

git -C "$repo" init -q
git -C "$repo" config user.email test@example.com
git -C "$repo" config user.name 'SDK Cache Test'
git -C "$repo" add .
git -C "$repo" commit -qm 'fixture'

configured="$repo/upstream/vespa/searchcore/src/tests/proton/documentdb/CMakeFiles/searchcore_embedded_crud_spike_app.dir"
mkdir -p "$configured"
printf 'link\n' >"$configured/link.txt"
printf 'CXX_INCLUDES = \n' >"$configured/flags.make"

worktree="$fixture/worktree"
git -C "$repo" worktree add -q "$worktree" -b cache-test

run_resolver() {
    local root=$1
    (
        cd "$root"
        SEARCHKERNEL_BUILD_ENV=cache-regression \
        SK_NATIVE_TARGET=x86_64-unknown-linux-gnu \
        SK_NATIVE_CACHE_BUILDER="$root/tools/fake-sdk-builder.sh" \
            bash "$root/tools/resolve-native-sdk.sh" ensure
    )
}

first=$(run_resolver "$repo")
[[ -d "$first" ]]
common=$(git -C "$repo" rev-parse --git-common-dir)
[[ "$common" = /* ]] || common="$repo/$common"
[[ $(wc -l <"$common/sdk-cache-build-count") -eq 1 ]]

second=$(run_resolver "$worktree")
[[ "$second" == "$first" ]]
[[ $(wc -l <"$common/sdk-cache-build-count") -eq 1 ]]
echo 'native SDK cache: worktree without Vespa outputs reused sibling-built immutable object'

printf 'adapter-v2\n' >>"$worktree/core/native/vespa/adapter.cc"
third=$(run_resolver "$worktree")
[[ "$third" != "$first" ]]
[[ -d "$third" ]]
[[ $(wc -l <"$common/sdk-cache-build-count") -eq 2 ]]
[[ -z "$(find "$first" -perm /222 -print -quit)" ]]
[[ -z "$(find "$third" -perm /222 -print -quit)" ]]
echo 'native SDK cache: native input change selected a distinct read-only object'

echo 'native SDK cache regression PASS'
