#!/usr/bin/env bash
set -euo pipefail
trap 'echo "native SDK cache regression failed at line $LINENO: $BASH_COMMAND" >&2' ERR

source_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)

# Cargo must not bootstrap or repair the immutable Vespa base. The default
# build-script path is read-only; mutation belongs to ./dev native-sdk ensure.
grep -q 'resolve_shared_native_sdk(&manifest_dir, &target, "path")' \
    "$source_root/core/crates/searchkernel-vespa-bindings/build.rs"
if grep -Eq 'resolve_shared_native_sdk\([^\n]*"ensure"\)' \
    "$source_root/core/crates/searchkernel-vespa-bindings/build.rs"; then
    echo 'Cargo build.rs must not invoke the mutating shared-base ensure path' >&2
    exit 1
fi
grep -q "./dev native-sdk ensure" \
    "$source_root/core/crates/searchkernel-vespa-bindings/build.rs"
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

# Reproduce the post-#234 migration state: the configured Vespa tree still has
# CMake metadata but no built libvespa*.so files, while the pre-split full SDK
# already contains a self-contained Vespa runtime closure.
legacy_sdk="$repo/core/native/sdk/linux-x86_64"
mkdir -p "$legacy_sdk/lib"
legacy_source="$fixture/legacy-vespa.c"
cat >"$legacy_source" <<'C'
int searchkernel_cache_fixture_base(void) { return 1; }
C
cc -shared -fPIC "$legacy_source" \
    -Wl,-soname,libvespalib.so \
    -o "$legacy_sdk/lib/libvespalib.so"

refresh_legacy_fingerprint() {
    VESPA_LIBRARY_ROOT="$legacy_sdk/lib" \
    SEARCHKERNEL_BUILD_ENV=cache-regression \
    python3 "$repo/tools/native-build-fingerprint.py" \
        --root "$repo" \
        --vespa-root "$repo/upstream/vespa" \
        --target x86_64-unknown-linux-gnu \
        --scope full \
        --output "$legacy_sdk/native-build-fingerprint-v2.txt"
    printf '{"native_abi_version":1,"vespa_commit":"fixture-vespa-commit"}\n' \
        >"$legacy_sdk/manifest.json"
}
refresh_legacy_fingerprint

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

run_resolver() {
    local root=$1
    local mode=${2:-ensure}
    (
        cd "$root"
        SEARCHKERNEL_BUILD_ENV=cache-regression \
        SK_NATIVE_TARGET=x86_64-unknown-linux-gnu \
        SK_NATIVE_CACHE_BUILDER="$root/tools/fake-sdk-builder.sh" \
        VESPA_LIBRARY_ROOT="$untrusted_library_root" \
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

if ! first=$(run_resolver "$repo" 2>"$fixture/bootstrap.err"); then
    echo 'native SDK cache bootstrap failed:' >&2
    cat "$fixture/bootstrap.err" >&2
    exit 1
fi
[[ -d "$first" ]]
grep -q 'using Vespa shared libraries from compatible full SDK' "$fixture/bootstrap.err"
[[ -s "$first/lib/libvespalib.so" ]]
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
refresh_legacy_fingerprint
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
