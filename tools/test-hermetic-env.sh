#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
RUNNER="$ROOT/tools/run-hermetic-env.sh"

bash -n "$RUNNER"

tmp=$(mktemp -d "${TMPDIR:-/tmp}/searchkernel-hermetic-env.XXXXXX")
trap 'rm -rf -- "$tmp"' EXIT
mkdir -p "$tmp/home" "$tmp/tmp"

export SEARCHKERNEL_AMBIENT_SENTINEL=must-not-leak
export SK_NATIVE_SDK=/ambient/poison/sdk
export SK_NATIVE_CACHE_DIR=/ambient/poison/cache
export SK_NATIVE_BASE_ARTIFACT_URL=file:///ambient/poison.tar.gz
export SK_NATIVE_BASE_ARTIFACT_GENERATION=999
export VESPA_ROOT=/ambient/poison/vespa
export VESPA_DEPS=/ambient/poison/deps
export VESPA_DEPS_LIB_DIR=/ambient/poison/deps/lib
export VESPA_EXTRA_LIB_DIR=/ambient/poison/extra
export VESPA_AWS_LIB_DIR=/ambient/poison/aws

bash "$RUNNER" \
    "PATH=$PATH" \
    "HOME=$tmp/home" \
    "TMPDIR=$tmp/tmp" \
    "LANG=C" \
    "LC_ALL=C" \
    "SEARCHKERNEL_EXPLICIT_SENTINEL=allowed" \
    -- bash -c '
        set -euo pipefail
        [[ "$SEARCHKERNEL_EXPLICIT_SENTINEL" == allowed ]]
        for name in \
            SEARCHKERNEL_AMBIENT_SENTINEL \
            SK_NATIVE_SDK \
            SK_NATIVE_CACHE_DIR \
            SK_NATIVE_BASE_ARTIFACT_URL \
            SK_NATIVE_BASE_ARTIFACT_GENERATION \
            VESPA_ROOT \
            VESPA_DEPS \
            VESPA_DEPS_LIB_DIR \
            VESPA_EXTRA_LIB_DIR \
            VESPA_AWS_LIB_DIR; do
            if [[ -v "$name" ]]; then
                echo "ambient variable leaked through hermetic runner: $name" >&2
                exit 1
            fi
        done
    '

if bash "$RUNNER" NOT_AN_ASSIGNMENT -- true >/dev/null 2>&1; then
    echo "hermetic runner unexpectedly accepted a non-assignment input" >&2
    exit 1
fi

poison_path="$tmp/poison-path"
mkdir -p "$poison_path"
cat >"$poison_path/env" <<'EOF'
#!/usr/bin/env bash
echo "poison env executable was selected" >&2
exit 99
EOF
chmod +x "$poison_path/env"

/bin/bash "$RUNNER" \
    "PATH=/usr/bin:/bin" \
    "HOME=$tmp/home" \
    "TMPDIR=$tmp/tmp" \
    -- /bin/bash -c '[[ "$PATH" == /usr/bin:/bin ]]'

echo "hermetic test environment regression PASS"
