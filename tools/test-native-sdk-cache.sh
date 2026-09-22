#!/usr/bin/env bash
set -euo pipefail

source_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
fixture=$(mktemp -d "${TMPDIR:-/tmp}/searchkernel-sdk-cache.XXXXXX")
cleanup_fixture() {
    chmod -R u+w "$fixture" 2>/dev/null || true
    rm -rf "$fixture"
}
trap cleanup_fixture EXIT

repo="$fixture/repo"
mkdir -p "$repo/tools" "$repo/core/native/vespa" "$repo/upstream/vespa" "$repo/vespa-deps/lib64"
cp "$source_root/tools/resolve-native-sdk.sh" "$repo/tools/"
cp "$source_root/tools/native-build-fingerprint.py" "$repo/tools/"
cat >"$repo/core/native/vespa/searchkernel_vespa.h" <<'EOF'
#define SK_NATIVE_ABI_VERSION 1U
EOF
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
cat >"$SK_NATIVE_BUILD_DIR/cache_fixture_base.c" <<'C'
int searchkernel_cache_fixture_base(void) { return 1; }
C
cc -shared -fPIC "$SK_NATIVE_BUILD_DIR/cache_fixture_base.c" \
    -Wl,-soname,libvespalib.so \
    -o "$sdk/lib/libvespalib.so"
printf '{"kind":"vespa-base"}\n' >"$sdk/manifest.json"
python3 "$root/tools/native-build-fingerprint.py" \
    --root "$root" \
    --vespa-root "$VESPA_ROOT" \
    --target "$SK_NATIVE_TARGET" \
    --scope base \
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
printf 'vespa-library-bytes-v1\n' >"$repo/upstream/vespa/cache-fixture.so"

worktree="$fixture/worktree"
git -C "$repo" worktree add -q "$worktree" -b cache-test
echo 'unrelated rust-side change' >"$worktree/unrelated.txt"
git -C "$worktree" add unrelated.txt
git -C "$worktree" commit -qm 'unrelated branch change'

run_resolver() {
    local root=$1
    local mode=${2:-ensure}
    (
        cd "$root"
        SEARCHKERNEL_BUILD_ENV=cache-regression \
        SK_NATIVE_TARGET=x86_64-unknown-linux-gnu \
        SK_NATIVE_CACHE_BUILDER="$root/tools/fake-sdk-builder.sh" \
            bash "$root/tools/resolve-native-sdk.sh" "$mode"
    )
}

common=$(git -C "$repo" rev-parse --git-common-dir)
[[ "$common" = /* ]] || common="$repo/$common"

if run_resolver "$repo" path >"$fixture/cold-path.out" 2>"$fixture/cold-path.err"; then
    echo 'native SDK cache path lookup unexpectedly succeeded on a cold cache' >&2
    exit 1
fi
[[ ! -e "$common/sdk-cache-build-count" ]]
grep -q 'native SDK cache miss' "$fixture/cold-path.err"

first=$(run_resolver "$repo")
[[ -d "$first" ]]
grep -q '^fingerprint-version=3$' "$first/native-build-fingerprint-v2.txt"
grep -q '^fingerprint.scope[[:space:]]*base$' "$first/native-build-fingerprint-v2.txt"
grep -Eq '"kind"[[:space:]]*:[[:space:]]*"vespa-base"' "$first/manifest.json"
[[ ! -e "$first/lib/libsearchkernel_vespa.so" ]]
[[ ! -e "$first/include/searchkernel_vespa.h" ]]
[[ $(wc -l <"$common/sdk-cache-build-count") -eq 1 ]]

warm_path=$(run_resolver "$repo" path)
[[ "$warm_path" == "$first" ]]
[[ $(wc -l <"$common/sdk-cache-build-count") -eq 1 ]]

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

printf 'unrelated-task-dependency\n' >"$worktree/vespa-deps/lib64/libsearchkernel_fixture.so"
same_after_unrelated_deps=$(run_resolver "$worktree")
[[ "$same_after_unrelated_deps" == "$third" ]]
[[ $(wc -l <"$common/sdk-cache-build-count") -eq 1 ]]

printf 'vespa-library-bytes-v2-changed\n' >"$repo/upstream/vespa/cache-fixture.so"
vespa_changed=$(run_resolver "$worktree")
[[ "$vespa_changed" != "$third" ]]
[[ -d "$vespa_changed" ]]
[[ $(wc -l <"$common/sdk-cache-build-count") -eq 2 ]]

printf 'dependency-v2-with-different-size\n' >"$repo/vespa-deps/lib64/libsearchkernel_fixture.so"
fourth=$(run_resolver "$worktree")
[[ "$fourth" != "$vespa_changed" ]]
[[ -d "$fourth" ]]
[[ $(wc -l <"$common/sdk-cache-build-count") -eq 3 ]]

chmod u+w "$fourth/lib"
rm "$fourth/lib/libvespalib.so"
chmod a-w "$fourth/lib"
healed=$(run_resolver "$worktree" 2>"$fixture/incomplete.err")
[[ "$healed" == "$fourth" ]]
[[ -s "$fourth/lib/libvespalib.so" ]]
[[ $(wc -l <"$common/sdk-cache-build-count") -eq 4 ]]
grep -q 'quarantining and rebuilding under the cache lock' "$fixture/incomplete.err"

chmod u+w "$fourth/lib"
truncate -s 1 "$fourth/lib/libvespalib.so"
chmod a-w "$fourth/lib"
corrupted_healed=$(run_resolver "$worktree" 2>"$fixture/corrupt.err")
[[ "$corrupted_healed" == "$fourth" ]]
[[ -s "$fourth/lib/libvespalib.so" ]]
[[ $(wc -l <"$common/sdk-cache-build-count") -eq 5 ]]
grep -q 'quarantining and rebuilding under the cache lock' "$fixture/corrupt.err"

chmod u+w "$fourth/lib"
rm "$fourth/lib/libvespalib.so"
chmod a-w "$fourth/lib"
before=$(wc -l <"$common/sdk-cache-build-count")
run_resolver "$worktree" >"$fixture/race-a.out" 2>"$fixture/race-a.err" &
pid_a=$!
run_resolver "$worktree" >"$fixture/race-b.out" 2>"$fixture/race-b.err" &
pid_b=$!
wait "$pid_a"
wait "$pid_b"
[[ "$(cat "$fixture/race-a.out")" == "$fourth" ]]
[[ "$(cat "$fixture/race-b.out")" == "$fourth" ]]
after=$(wc -l <"$common/sdk-cache-build-count")
[[ $((after - before)) -eq 1 ]]
[[ -s "$fourth/lib/libvespalib.so" ]]

echo 'native SDK cache regression PASS'
