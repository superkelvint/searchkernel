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
    env \
        SEARCHKERNEL_BUILD_ENV=regression \
        CXX="$1" \
        CXXFLAGS="${2:-}" \
        python3 "$root/tools/native-build-fingerprint.py" \
            --root "$root" \
            --target x86_64-unknown-linux-gnu \
            | sed -n '2p'
}

first=$(fingerprint "$temporary/compiler-a")
second=$(fingerprint "$temporary/compiler-a")
[[ "$first" == "$second" ]]
echo 'native fingerprint: unchanged compiler/configuration is stable'

changed_compiler=$(fingerprint "$temporary/compiler-b")
[[ "$first" != "$changed_compiler" ]]
echo 'native fingerprint: compiler identity changes the fingerprint'

changed_flags=$(fingerprint "$temporary/compiler-a" '-DSEARCHKERNEL_NATIVE_FINGERPRINT_REGRESSION=1')
[[ "$first" != "$changed_flags" ]]
echo 'native fingerprint: relevant flags change the fingerprint'

[[ "$(grep -o 'target/host' "$root/.cargo/config.toml" | head -1)" == target/host ]]
grep -q 'ENV CARGO_TARGET_DIR=target/devcontainer' "$root/.yolobox.Dockerfile"
grep -q 'CARGO_TARGET_DIR: target/ci' "$root/.github/workflows/abi-surface.yml"
echo 'native environments: host, devcontainer, and CI use separate Cargo targets'

echo 'native environment fingerprint regression PASS'
