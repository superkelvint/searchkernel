#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
temporary=$(mktemp -d "${TMPDIR:-/tmp}/searchkernel-native-fingerprint.XXXXXX")
trap 'rm -rf "$temporary"' EXIT

cat >"$temporary/compiler-a" <<'EOF'
#!/bin/sh
case "${1:-}" in
    --version) echo 'Search Kernel compiler fixture A 1.0' ;;
    -dumpmachine) echo x86_64-searchkernel-fixture-linux ;;
    *) exec c++ "$@" ;;
esac
EOF
cp "$temporary/compiler-a" "$temporary/compiler-b"
sed -i 's/fixture A/fixture B/' "$temporary/compiler-b"
chmod +x "$temporary/compiler-a" "$temporary/compiler-b"

fingerprint() {
    local extra_lib_dir=${3:-}
    local deps_root=${4:-}
    local scope=${5:-full}
    local adapter_flags=${6:-}
    env \
        SEARCHKERNEL_BUILD_ENV=regression \
        CXX="$1" \
        CXXFLAGS="${2:-}" \
        SK_NATIVE_EXTRA_CXX_FLAGS="$adapter_flags" \
        VESPA_EXTRA_LIB_DIR="$extra_lib_dir" \
        VESPA_DEPS="$deps_root" \
        python3 "$root/tools/native-build-fingerprint.py" \
            --root "$root" \
            --target x86_64-unknown-linux-gnu \
            --scope "$scope" \
            | sed -n '2p'
}

first=$(fingerprint "$temporary/compiler-a")
second=$(fingerprint "$temporary/compiler-a")
[[ "$first" == "$second" ]]
echo 'native fingerprint: unchanged full-package environment is stable'

changed_compiler=$(fingerprint "$temporary/compiler-b")
[[ "$first" != "$changed_compiler" ]]
echo 'native fingerprint: full-package compiler identity changes the fingerprint'

changed_flags=$(fingerprint "$temporary/compiler-a" '-DSEARCHKERNEL_NATIVE_FINGERPRINT_REGRESSION=1')
[[ "$first" != "$changed_flags" ]]
echo 'native fingerprint: full-package compiler flags change the fingerprint'

if grep -q -- '--scope base' "$root/tools/resolve-native-sdk.sh" "$root/core/native/vespa/build-sdk.sh"; then
    echo 'immutable Vespa base must not use native-build-fingerprint.py' >&2
    exit 1
fi
grep -q 'CACHE_ROOT=\$CACHE_BASE/\$BUILD_ENV/\$TARGET' "$root/tools/resolve-native-sdk.sh"
echo 'native base identity: pinned Vespa checkout hash within build-environment/target namespace'

full_plain=$(fingerprint "$temporary/compiler-a" '' '' '' full '')
full_adapter_flags=$(fingerprint "$temporary/compiler-a" '' '' '' full '-DADAPTER_ONLY_CHANGE=1')
[[ "$full_plain" != "$full_adapter_flags" ]]
echo 'native fingerprint: full-package provenance still tracks adapter-only flags'

runtime_dir="$temporary/runtime"
mkdir -p "$runtime_dir"
printf 'llvm-runtime-a\n' >"$runtime_dir/libLLVM.so.21.1"
runtime_a=$(fingerprint "$temporary/compiler-a" '' "$runtime_dir")
printf 'llvm-runtime-b\n' >"$runtime_dir/libLLVM.so.21.1"
runtime_b=$(fingerprint "$temporary/compiler-a" '' "$runtime_dir")
[[ "$runtime_a" != "$runtime_b" ]]
echo 'native fingerprint: selected runtime library content changes the fingerprint'

deps_root="$temporary/deps"
external_deps="$temporary/external-deps"
mkdir -p "$deps_root/lib64" "$external_deps"
printf 'dependency-target-a\n' >"$external_deps/libfixture.so.1"
ln -s "$external_deps/libfixture.so.1" "$deps_root/lib64/libfixture.so"
dependency_a=$(fingerprint "$temporary/compiler-a" '' "$runtime_dir" "$deps_root")
printf 'dependency-target-b\n' >"$external_deps/libfixture.so.1"
dependency_b=$(fingerprint "$temporary/compiler-a" '' "$runtime_dir" "$deps_root")
[[ "$dependency_a" != "$dependency_b" ]]
echo 'native fingerprint: dependency symlink payload contents change the fingerprint'

# The packaged SDK preserves alias topology, so retargeting a symlink must
# invalidate the cache even when the replacement payload bytes are identical.
cp "$external_deps/libfixture.so.1" "$external_deps/libfixture.so.2"
ln -sfn "$external_deps/libfixture.so.2" "$deps_root/lib64/libfixture.so"
dependency_retargeted=$(fingerprint "$temporary/compiler-a" '' "$runtime_dir" "$deps_root")
[[ "$dependency_b" != "$dependency_retargeted" ]]
echo 'native fingerprint: dependency symlink target changes the fingerprint'

python3 "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/test-native-environment-fingerprint-python-1.py" "$root/tools/native-build-fingerprint.py" "$temporary/packaging-root"
echo 'native fingerprint: SDK packaging policy changes the fingerprint'

[[ "$(grep -o 'target/host' "$root/.cargo/config.toml" | head -1)" == target/host ]]
grep -q 'ENV CARGO_TARGET_DIR=target/devcontainer' "$root/.yolobox.Dockerfile"
grep -q 'CARGO_TARGET_DIR: target/ci' "$root/.github/workflows/abi-surface.yml"
echo 'native environments: host, devcontainer, and CI use separate Cargo targets'

echo 'native environment fingerprint regression PASS'
