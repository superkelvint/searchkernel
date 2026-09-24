#!/usr/bin/env bash
set -euo pipefail

###############################################################################
# SearchKernel native searchkerneld builder
#
# Run from anywhere inside a SearchKernel checkout.
#
# HOST responsibilities:
#   - Docker
#   - gh, already authenticated
#   - download private SearchKernel native-base artifacts
#
# CONTAINER responsibilities:
#   - Vespa AlmaLinux build environment
#   - CMake and build utilities
#   - Rust
#   - FlatBuffers 25.12.19
#   - native SDK installation
#   - searchkerneld build
#
# The container is persistent. We deliberately DO NOT use --rm.
#
# Therefore:
#   - dnf packages are installed once
#   - FlatBuffers is installed once
#   - Cargo/rustup/native SDK caches persist
#
# GitHub credentials are NEVER passed into the container.
###############################################################################

REPO="superkelvint/searchkernel"

IMAGE="${SEARCHKERNEL_NATIVE_IMAGE:-docker.io/vespaengine/vespa-dev-almalinux-8:latest}"
CONTAINER_NAME="${SEARCHKERNEL_NATIVE_CONTAINER:-searchkernel-native}"

BUILD_ENV="${SEARCHKERNEL_BUILD_ENV:-ci}"
TARGET="${SEARCHKERNEL_NATIVE_TARGET:-x86_64-unknown-linux-gnu}"
CONTAINER_CARGO_TARGET_DIR="${SEARCHKERNEL_NATIVE_CARGO_TARGET_DIR:-/work/target/actions-runner-alma}"

CARGO_VOLUME="${SEARCHKERNEL_CARGO_VOLUME:-searchkernel-cargo}"
RUSTUP_VOLUME="${SEARCHKERNEL_RUSTUP_VOLUME:-searchkernel-rustup}"
NATIVE_CACHE_VOLUME="${SEARCHKERNEL_NATIVE_CACHE_VOLUME:-searchkernel-native-cache}"

HOST_ARTIFACT_CACHE="${SEARCHKERNEL_ARTIFACT_CACHE:-$HOME/.cache/searchkernel/native-artifacts}"

FLATBUFFERS_VERSION="25.12.19"

###############################################################################
# Helpers
###############################################################################

die() {
    echo "ERROR: $*" >&2
    exit 1
}

section() {
    echo
    echo "==> $*"
}

###############################################################################
# Find SearchKernel repository
###############################################################################

SCRIPT_DIR="$(
    cd "$(dirname "${BASH_SOURCE[0]}")" &&
    pwd -P
)"

if REPO_ROOT="$(git -C "$SCRIPT_DIR" rev-parse --show-toplevel 2>/dev/null)"; then
    :
elif REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null)"; then
    :
else
    die "this script must be run from inside a SearchKernel checkout"
fi

REPO_ROOT="$(cd "$REPO_ROOT" && pwd -P)"

[[ -f "$REPO_ROOT/Cargo.toml" ]] ||
    die "Cargo.toml not found at repository root"

[[ -x "$REPO_ROOT/dev" ]] ||
    die "./dev is missing or not executable"

###############################################################################
# Host prerequisites
###############################################################################

section "Checking host"

for cmd in docker gh python3 sha256sum; do
    command -v "$cmd" >/dev/null 2>&1 ||
        die "host is missing required command: $cmd"
done

docker info >/dev/null 2>&1 ||
    die "docker is not usable by the current user"

gh api "repos/$REPO" >/dev/null 2>&1 ||
    die "host gh authentication cannot access $REPO"

echo "Repository:"
echo "  $REPO_ROOT"

echo "Container:"
echo "  $CONTAINER_NAME"

echo "Image:"
echo "  $IMAGE"

echo "Private GitHub access OK"

###############################################################################
# Read pinned Vespa identity from SearchKernel
###############################################################################

section "Reading pinned native-base identity"

MANIFEST="$REPO_ROOT/core/native/vespa/manifest.json"

[[ -f "$MANIFEST" ]] ||
    die "missing native manifest: $MANIFEST"

VESPA_COMMIT="$(
    python3 - "$MANIFEST" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as stream:
    print(json.load(stream)["vespa_commit"])
PY
)"

[[ -n "$VESPA_COMMIT" ]] ||
    die "could not determine pinned Vespa commit"

mkdir -p "$HOST_ARTIFACT_CACHE"
HOST_ARTIFACT_CACHE="$(cd "$HOST_ARTIFACT_CACHE" && pwd -P)"

section "Fetching newest published native base"

ARCHIVE="$(
    SEARCHKERNEL_ARTIFACT_CACHE="$HOST_ARTIFACT_CACHE" \
    SK_NATIVE_TARGET="$TARGET" \
        "$REPO_ROOT/tools/fetch-native-base.sh" "$BUILD_ENV"
)"

[[ -s "$ARCHIVE" ]] ||
    die "native-base fetcher returned an invalid artifact: $ARCHIVE"

ARCHIVE_NAME="$(basename "$ARCHIVE")"
CHECKSUM="${ARCHIVE}.sha256"
CHECKSUM_NAME="$(basename "$CHECKSUM")"

[[ -s "$CHECKSUM" ]] ||
    die "native-base checksum is missing: $CHECKSUM"

echo "Build environment: $BUILD_ENV"
echo "Target:            $TARGET"
echo "Vespa commit:      $VESPA_COMMIT"
echo "Native base:       $ARCHIVE_NAME"

###############################################################################
# Persistent Docker volumes
###############################################################################

section "Ensuring persistent Docker volumes"

for volume in \
    "$CARGO_VOLUME" \
    "$RUSTUP_VOLUME" \
    "$NATIVE_CACHE_VOLUME"
do
    if docker volume inspect "$volume" >/dev/null 2>&1; then
        echo "Reusing $volume"
    else
        docker volume create "$volume" >/dev/null
        echo "Created $volume"
    fi
done

###############################################################################
# Persistent native build container
###############################################################################

if docker container inspect "$CONTAINER_NAME" >/dev/null 2>&1; then
    section "Reusing persistent native container"

    EXISTING_WORK_MOUNT="$(
        docker inspect \
            --format '{{range .Mounts}}{{if eq .Destination "/work"}}{{.Source}}{{end}}{{end}}' \
            "$CONTAINER_NAME"
    )"

    if [[ "$EXISTING_WORK_MOUNT" != "$REPO_ROOT" ]]; then
        cat >&2 <<EOF

ERROR: existing container '$CONTAINER_NAME' belongs to another checkout.

Existing /work:
  $EXISTING_WORK_MOUNT

Current checkout:
  $REPO_ROOT

Remove it once with:

  docker rm -f $CONTAINER_NAME

Then rerun this script.

EOF
        exit 1
    fi

    RUNNING="$(
        docker inspect \
            --format '{{.State.Running}}' \
            "$CONTAINER_NAME"
    )"

    if [[ "$RUNNING" != "true" ]]; then
        docker start "$CONTAINER_NAME" >/dev/null
    fi

else
    section "Creating persistent native container"

    docker run \
        -d \
        --name "$CONTAINER_NAME" \
        --privileged \
        --pids-limit=-1 \
        -v "$REPO_ROOT:/work" \
        -v "$HOST_ARTIFACT_CACHE:/native-artifacts:ro" \
        -v "$CARGO_VOLUME:/root/.cargo" \
        -v "$RUSTUP_VOLUME:/root/.rustup" \
        -v "$NATIVE_CACHE_VOLUME:/root/.cache/searchkernel" \
        -e "SEARCHKERNEL_BUILD_ENV=$BUILD_ENV" \
        -e "CARGO_TARGET_DIR=$CONTAINER_CARGO_TARGET_DIR" \
        -e "PATH=/root/.cargo/bin:/opt/rh/gcc-toolset-15/root/usr/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin" \
        -e "LD_LIBRARY_PATH=/opt/rh/gcc-toolset-15/root/usr/lib64:/opt/rh/gcc-toolset-15/root/usr/lib" \
        -w /work \
        "$IMAGE" \
        /bin/bash -lc 'exec sleep infinity' \
        >/dev/null

    echo "Created $CONTAINER_NAME"
fi

###############################################################################
# Container setup + native build
###############################################################################

section "Preparing container and building searchkerneld"

docker exec \
    -e "SK_NATIVE_ARCHIVE_NAME=$ARCHIVE_NAME" \
    -e "SEARCHKERNEL_BUILD_ENV=$BUILD_ENV" \
    -e "CARGO_TARGET_DIR=$CONTAINER_CARGO_TARGET_DIR" \
    "$CONTAINER_NAME" \
    /bin/bash -lc '
set -euo pipefail

say() {
    echo
    echo "==> $*"
}

die() {
    echo "ERROR: $*" >&2
    exit 1
}

cd /work

git config --global --add safe.directory /work >/dev/null 2>&1 || true

export PATH="/root/.cargo/bin:/opt/rh/gcc-toolset-15/root/usr/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"

export LD_LIBRARY_PATH="/opt/rh/gcc-toolset-15/root/usr/lib64:/opt/rh/gcc-toolset-15/root/usr/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"

export SEARCHKERNEL_BUILD_ENV="${SEARCHKERNEL_BUILD_ENV:-ci}"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/work/target/actions-runner-alma}"

unset VESPA_ROOT || true
unset SK_NATIVE_SDK || true

###############################################################################
# OS prerequisites -- install only if actually missing
###############################################################################

say "Checking container OS prerequisites"

NEED_DNF=0

for cmd in \
    cmake \
    curl \
    file \
    git \
    make \
    python3 \
    sha256sum \
    tar
do
    if ! command -v "$cmd" >/dev/null 2>&1; then
        NEED_DNF=1
        break
    fi
done

if [[ "$NEED_DNF" == "0" ]] && { ! command -v clang >/dev/null 2>&1 || ! python3 -c 'import clang.cindex' >/dev/null 2>&1; }; then
    NEED_DNF=1
fi

if [[ "$NEED_DNF" == "1" ]]; then
    say "Installing missing container OS prerequisites once"

    dnf install -y \
        clang \
        cmake \
        curl \
        file \
        git \
        gzip \
        make \
        python3 \
        python3-clang \
        tar \
        which

    dnf clean all
else
    echo "Container OS prerequisites already installed"
fi

for cmd in \
    cmake \
    curl \
    file \
    git \
    make \
    python3 \
    sha256sum \
    tar
do
    command -v "$cmd" >/dev/null 2>&1 ||
        die "required container command is unavailable: $cmd"
done

command -v clang >/dev/null 2>&1 ||
    die "clang frontend is unavailable"
python3 -c 'import clang.cindex' >/dev/null 2>&1 ||
    die "python3 clang.cindex bindings are unavailable"

###############################################################################
# Vespa compiler
###############################################################################

say "Checking Vespa C++ toolchain"

command -v c++ >/dev/null 2>&1 ||
    die "C++ compiler unavailable"

c++ --version | head -n 1

cat >/tmp/searchkernel-cxx-probe.cc <<'"'"'EOF'"'"'
#include <format>
#include <string>

int main() {
    auto result = std::format("{}", 42);
    return result == "42" ? 0 : 1;
}
EOF

c++ \
    -std=gnu++23 \
    /tmp/searchkernel-cxx-probe.cc \
    -o /tmp/searchkernel-cxx-probe

/tmp/searchkernel-cxx-probe

rm -f \
    /tmp/searchkernel-cxx-probe.cc \
    /tmp/searchkernel-cxx-probe

echo "C++23 + std::format OK"

###############################################################################
# Rust
###############################################################################

say "Checking Rust"

if ! command -v rustup >/dev/null 2>&1; then
    echo "Installing Rust once"

    curl \
        --proto "=https" \
        --tlsv1.2 \
        --silent \
        --show-error \
        --fail \
        https://sh.rustup.rs \
        | sh -s -- -y --profile minimal
fi

export PATH="/root/.cargo/bin:$PATH"

if ! rustup toolchain list | grep -q "^stable"; then
    rustup toolchain install stable --profile minimal
fi

rustup default stable >/dev/null
rustup component add rustfmt clippy >/dev/null

rustc --version
cargo --version
rustfmt --version
cargo clippy --version

###############################################################################
# FlatBuffers
###############################################################################

FLATBUFFERS_VERSION="25.12.19"
FLATBUFFERS_PREFIX="/usr/local"

flatbuffers_ok() {
    [[ -x "$FLATBUFFERS_PREFIX/bin/flatc" ]] || return 1

    [[ "$(
        "$FLATBUFFERS_PREFIX/bin/flatc" --version 2>/dev/null || true
    )" == "flatc version $FLATBUFFERS_VERSION" ]] || return 1

    [[ -f "$FLATBUFFERS_PREFIX/include/flatbuffers/base.h" ]] || return 1
    [[ -f "$FLATBUFFERS_PREFIX/include/flatbuffers/idl.h" ]] || return 1
    [[ -f "$FLATBUFFERS_PREFIX/lib/libflatbuffers.a" ]] || return 1
}

if flatbuffers_ok; then
    say "Reusing FlatBuffers $FLATBUFFERS_VERSION"
else
    say "Installing FlatBuffers $FLATBUFFERS_VERSION once"

    FB_TMP="$(mktemp -d /tmp/searchkernel-flatbuffers.XXXXXX)"

    trap '\''rm -rf "$FB_TMP"'\'' EXIT

    curl \
        --fail \
        --location \
        --silent \
        --show-error \
        "https://github.com/google/flatbuffers/archive/refs/tags/v${FLATBUFFERS_VERSION}.tar.gz" \
        -o "$FB_TMP/flatbuffers.tar.gz"

    mkdir -p "$FB_TMP/source"

    tar \
        -xzf "$FB_TMP/flatbuffers.tar.gz" \
        --strip-components=1 \
        -C "$FB_TMP/source"

    cmake \
        -S "$FB_TMP/source" \
        -B "$FB_TMP/build" \
        -DCMAKE_BUILD_TYPE=Release \
        -DCMAKE_INSTALL_PREFIX="$FLATBUFFERS_PREFIX" \
        -DCMAKE_INSTALL_LIBDIR=lib \
        -DCMAKE_POSITION_INDEPENDENT_CODE=ON \
        -DFLATBUFFERS_BUILD_TESTS=OFF

    cmake \
        --build "$FB_TMP/build" \
        --parallel "$(nproc)"

    cmake --install "$FB_TMP/build"

    rm -rf "$FB_TMP"
    trap - EXIT

    flatbuffers_ok ||
        die "FlatBuffers installation failed validation"
fi

flatc --version

###############################################################################
# Native Vespa base
###############################################################################

say "Refreshing SearchKernel native base"

ARTIFACT="/native-artifacts/${SK_NATIVE_ARCHIVE_NAME}"

[[ -f "$ARTIFACT" ]] ||
    die "host-provided native archive is missing: $ARTIFACT"

[[ -f "${ARTIFACT}.sha256" ]] ||
    die "host-provided checksum is missing: ${ARTIFACT}.sha256"

export SK_NATIVE_BASE_ARTIFACT_URL="file://${ARTIFACT}"

# Always run ensure during provisioning. It compares the installed artifact
# generation with the host-selected release and atomically refreshes the shared
# native cache only when a newer generation is available.
./dev native-sdk ensure

SDK_PATH="$(./dev native-sdk path)"

[[ -n "$SDK_PATH" ]] ||
    die "native SDK resolver returned an empty path"

[[ -d "$SDK_PATH" ]] ||
    die "native SDK does not exist: $SDK_PATH"

[[ -f "$SDK_PATH/manifest.json" ]] ||
    die "native SDK manifest is missing"

echo
echo "Native SDK:"
echo "  $SDK_PATH"

###############################################################################
# Node / TypeScript
###############################################################################

say "Checking Node.js / TypeScript"

node_ok() {
    command -v node >/dev/null 2>&1 || return 1
    command -v npm >/dev/null 2>&1 || return 1

    local version major minor

    version="$(node -p "process.versions.node")"

    IFS=. read -r major minor _ <<< "$version"

    (( major > 20 || (major == 20 && minor >= 19) ))
}

if ! node_ok; then
    say "Installing Node.js 20"

    dnf module reset -y nodejs || true
    dnf module enable -y nodejs:20
    dnf install -y nodejs npm

    node_ok ||
        die "Node.js >=20.19 is required"
fi

echo "Node:"
node --version
npm --version

# npm ci is only necessary when node_modules is absent.
if [[ ! -x /work/clients/typescript/node_modules/.bin/tsc ]]; then
    say "Installing TypeScript dependencies"

    npm ci --prefix /work/clients/typescript
else
    echo "TypeScript dependencies already installed"
fi


###############################################################################
# Python client dependencies
###############################################################################

say "Checking Python client dependencies"

PYTHON_DEPS_OK=0

if python3 /work/tools/check-python-dependencies.py >/dev/null 2>&1; then
    PYTHON_DEPS_OK=1
fi

if [[ "$PYTHON_DEPS_OK" == "0" ]]; then
    say "Installing Python client dependencies"

    python3 -m pip install \
        --upgrade \
        --disable-pip-version-check \
        -e /work/clients/python

    python3 /work/tools/check-python-dependencies.py ||
        die "Python dependency installation did not satisfy SearchKernel"
else
    echo "Python dependencies already satisfied"
fi


###############################################################################
# PHP
###############################################################################

say "Checking PHP"

php_ok() {
    command -v php >/dev/null 2>&1 || return 1

    php -r "exit(version_compare(PHP_VERSION, \"8.1.0\", \">=\") ? 0 : 1);"
}

if php_ok; then
    echo "PHP already installed:"
    php --version | head -n 1
else
    say "Installing PHP >= 8.1"

    dnf install -y \
        epel-release \
        https://rpms.remirepo.net/enterprise/remi-release-8.rpm

    dnf module reset -y php
    dnf module enable -y php:remi-8.3

    dnf install -y php-cli

    php_ok ||
        die "PHP >= 8.1 unavailable after installation"

    php --version | head -n 1
fi

###############################################################################
# Native verifier prerequisites
###############################################################################

say "Checking native verifier prerequisites"

if ! command -v rg >/dev/null 2>&1; then
    dnf install -y epel-release
    dnf install -y ripgrep
fi
if ! command -v nm >/dev/null 2>&1 || ! command -v readelf >/dev/null 2>&1; then
    dnf install -y binutils
fi

for cmd in rg nm readelf ldd; do
    command -v "$cmd" >/dev/null 2>&1 ||
        die "native verifier prerequisite is unavailable: $cmd"
done

###############################################################################
# Doctor
###############################################################################

say "Running SearchKernel doctor"

unset VESPA_ROOT || true
unset SK_NATIVE_SDK || true

./dev doctor

###############################################################################
# searchkerneld
###############################################################################

say "Building native searchkerneld"

cargo build \
    -p searchkerneld \
    --features native

BINARY="$CARGO_TARGET_DIR/debug/searchkerneld"

[[ -x "$BINARY" ]] ||
    die "searchkerneld binary not produced: $BINARY"

###############################################################################
# Clean-runner native acceptance
###############################################################################

say "Running clean-runner native verifier acceptance"

PINNED_VESPA_COMMIT="$(python3 -c "import json; print(json.load(open(\"core/native/vespa/manifest.json\", encoding=\"utf-8\"))[\"vespa_commit\"])")"
CACHE_NAMESPACE="${SDK_PATH%/objects/$PINNED_VESPA_COMMIT}"
PUBLISHED_VESPA_ROOT="$CACHE_NAMESPACE/toolchains/$PINNED_VESPA_COMMIT/upstream/vespa"

[[ -f "$PUBLISHED_VESPA_ROOT/.searchkernel-vespa-artifact.json" ]] ||
    die "published native-base toolchain provenance is missing: $PUBLISHED_VESPA_ROOT"

# Force the acceptance run through the published toolchain even if the checkout
# mounted at /work happens to contain a a developer-configured upstream/vespa
# tree. live_oracle_enabled() must therefore select frozen-oracle replay.
export VESPA_ROOT="$PUBLISHED_VESPA_ROOT"
unset SK_NATIVE_SDK || true
unset SEARCHKERNEL_VERIFY_LIVE_ORACLE || true

./dev verify m14
./dev verify product.rust-real-engine
./dev verify integration

say "Running PHP real-engine E2E"

SEARCHKERNEL_LOCAL_HOST="$BINARY" \
    bash clients/php/examples/run_searchkernel_client_api_e2e.sh

###############################################################################
# Result
###############################################################################

say "Build and native verification succeeded"

ls -lh "$BINARY"
file "$BINARY"

echo
echo "============================================================"
echo "SearchKernel native searchkerneld + verifier acceptance SUCCESS"
echo "============================================================"
echo
echo "Binary:"
echo "  $BINARY"
echo
echo "Native SDK:"
echo "  $SDK_PATH"
echo
'

###############################################################################
# Host summary
###############################################################################

section "Done"

echo "searchkerneld:"
case "$CONTAINER_CARGO_TARGET_DIR" in
    /work/*)
        HOST_BINARY="$REPO_ROOT/${CONTAINER_CARGO_TARGET_DIR#/work/}/debug/searchkerneld"
        ;;
    *)
        HOST_BINARY="$CONTAINER_CARGO_TARGET_DIR/debug/searchkerneld"
        ;;
esac

echo "  $HOST_BINARY"

echo
echo "Persistent container:"
echo "  $CONTAINER_NAME"

echo
echo "Enter it manually with:"
echo "  docker exec -it $CONTAINER_NAME /bin/bash"

echo
echo "Destroy/rebuild the native environment with:"
echo "  docker rm -f $CONTAINER_NAME"