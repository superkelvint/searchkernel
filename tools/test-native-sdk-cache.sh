#!/usr/bin/env bash
set -euo pipefail

source_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
fixture=$(mktemp -d "${TMPDIR:-/tmp}/searchkernel-sdk-cache.XXXXXX")
cleanup_fixture() {
    # Published cache objects are deliberately non-writable. Restore owner
    # write permission only inside this disposable fixture before removal.
    chmod -R u+w "$fixture" 2>/dev/null || true
    rm -rf "$fixture"
}
trap cleanup_fixture EXIT

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
mkdir -p "$sdk/include" "$sdk/lib" "$sdk/share" "$SK_NATIVE_BUILD_DIR"
cp "$root/core/native/vespa/searchkernel_vespa.h" "$sdk/include/"
cat >"$SK_NATIVE_BUILD_DIR/cache_fixture_dep.c" <<'C'
int searchkernel_cache_fixture_dep(void) { return 1; }
C
cat >"$SK_NATIVE_BUILD_DIR/cache_fixture_adapter.c" <<'C'
extern int searchkernel_cache_fixture_dep(void);
int sk_native_abi_version(void) { return searchkernel_cache_fixture_dep(); }
C
cc -shared -fPIC "$SK_NATIVE_BUILD_DIR/cache_fixture_dep.c" \
    -Wl,-soname,libsearchkernel_cache_fixture_dep.so \
    -o "$sdk/lib/libsearchkernel_cache_fixture_dep.so"
cc -shared -fPIC "$SK_NATIVE_BUILD_DIR/cache_fixture_adapter.c" \
    -L"$sdk/lib" -Wl,-rpath,'$ORIGIN' \
    -lsearchkernel_cache_fixture_dep \
    -o "$sdk/lib/libsearchkernel_vespa.so"
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

# Give the second worktree a different parent-repository HEAD without changing
# upstream/vespa. Reuse must be based on the Vespa identity, not branch HEAD.
echo 'unrelated rust-side change' >"$worktree/unrelated.txt"
git -C "$worktree" add unrelated.txt
git -C "$worktree" commit -qm 'unrelated branch change'

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

# A published object whose required runtime dependency disappears must be
# quarantined and rebuilt automatically under the per-digest lock. Normal
# consumers never mutate the published object in place and need no cache surgery.
chmod u+w "$third/lib"
rm "$third/lib/libsearchkernel_cache_fixture_dep.so"
chmod a-w "$third/lib"
healed=$(run_resolver "$worktree" 2>"$fixture/incomplete.err")
[[ "$healed" == "$third" ]]
[[ -f "$third/lib/libsearchkernel_cache_fixture_dep.so" ]]
[[ $(wc -l <"$common/sdk-cache-build-count") -eq 3 ]]
grep -q 'quarantining and rebuilding under the cache lock' "$fixture/incomplete.err"
[[ -n "$(find "$(dirname "$third")/../quarantine" -maxdepth 1 -type d -name "$(basename "$third").*" -print -quit 2>/dev/null)" ]]
echo 'native SDK cache: incomplete published runtime closure self-healed automatically'

echo 'native SDK cache regression PASS'
