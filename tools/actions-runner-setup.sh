#!/usr/bin/env bash
set -euo pipefail

# Provision the persistent SearchKernel native build container used on self-hosted
# Actions runners. Re-running this script is the supported update path: it
# discovers the newest published CI native-base generation, refreshes the
# container's checkout-keyed native cache when necessary, and otherwise reuses
# all persistent toolchain/cache volumes.

CONTAINER_NAME="${SEARCHKERNEL_NATIVE_CONTAINER:-searchkernel-native}"
IMAGE="${SEARCHKERNEL_NATIVE_IMAGE:-docker.io/vespaengine/vespa-dev-almalinux-8:latest}"
BUILD_ENV="ci"
NATIVE_TARGET="${SK_NATIVE_TARGET:-x86_64-unknown-linux-gnu}"
CARGO_VOLUME="${SEARCHKERNEL_CARGO_VOLUME:-searchkernel-cargo}"
RUSTUP_VOLUME="${SEARCHKERNEL_RUSTUP_VOLUME:-searchkernel-rustup}"
NATIVE_CACHE_VOLUME="${SEARCHKERNEL_NATIVE_CACHE_VOLUME:-searchkernel-native-cache}"

fail() {
    echo "ERROR: $*" >&2
    exit 1
}

section() {
    echo
    echo "==> $*"
}

ROOT="$(git rev-parse --show-toplevel 2>/dev/null)" ||
    fail "run this from inside a SearchKernel checkout"
ROOT="$(cd "$ROOT" && pwd -P)"
cd "$ROOT"

for cmd in docker gh git python3 sha256sum; do
    command -v "$cmd" >/dev/null 2>&1 || fail "missing required host command: $cmd"
done

gh api repos/superkelvint/searchkernel >/dev/null 2>&1 ||
    fail "gh authentication cannot access superkelvint/searchkernel"

docker info >/dev/null 2>&1 ||
    fail "Docker is not available to the current user"

ARTIFACT_CACHE="${SEARCHKERNEL_ARTIFACT_CACHE:-${XDG_CACHE_HOME:-$HOME/.cache}/searchkernel/native-artifacts}"
mkdir -p "$ARTIFACT_CACHE"
ARTIFACT_CACHE="$(cd "$ARTIFACT_CACHE" && pwd -P)"

section "Fetching newest published CI native base"
ARTIFACT="$({
    SEARCHKERNEL_ARTIFACT_CACHE="$ARTIFACT_CACHE" \
    SK_NATIVE_TARGET="$NATIVE_TARGET" \
        tools/fetch-native-base.sh "$BUILD_ENV"
})"
[[ -s "$ARTIFACT" ]] || fail "native-base fetcher returned an invalid artifact: $ARTIFACT"
ARTIFACT_NAME="$(basename "$ARTIFACT")"

section "Preparing persistent container"
docker pull "$IMAGE"
for volume in "$CARGO_VOLUME" "$RUSTUP_VOLUME" "$NATIVE_CACHE_VOLUME"; do
    docker volume inspect "$volume" >/dev/null 2>&1 || docker volume create "$volume" >/dev/null
done

container_matches_checkout() {
    local mounted
    mounted="$(docker inspect -f '{{range .Mounts}}{{if eq .Destination "/work"}}{{.Source}}{{end}}{{end}}' "$CONTAINER_NAME" 2>/dev/null || true)"
    [[ "$mounted" == "$ROOT" ]]
}

container_matches_artifact_cache() {
    local mounted
    mounted="$(docker inspect -f '{{range .Mounts}}{{if eq .Destination "/native-artifacts"}}{{.Source}}{{end}}{{end}}' "$CONTAINER_NAME" 2>/dev/null || true)"
    [[ "$mounted" == "$ARTIFACT_CACHE" ]]
}

if docker container inspect "$CONTAINER_NAME" >/dev/null 2>&1; then
    if ! container_matches_checkout || ! container_matches_artifact_cache; then
        echo "Existing $CONTAINER_NAME uses different bind mounts; recreating it." >&2
        docker rm -f "$CONTAINER_NAME" >/dev/null
    fi
fi

if ! docker container inspect "$CONTAINER_NAME" >/dev/null 2>&1; then
    docker run -d \
        --name "$CONTAINER_NAME" \
        --hostname "$CONTAINER_NAME" \
        --workdir /work \
        --volume "$ROOT:/work" \
        --volume "$ARTIFACT_CACHE:/native-artifacts:ro" \
        --volume "$CARGO_VOLUME:/root/.cargo" \
        --volume "$RUSTUP_VOLUME:/root/.rustup" \
        --volume "$NATIVE_CACHE_VOLUME:/root/.cache/searchkernel" \
        "$IMAGE" \
        /bin/bash -lc 'while :; do sleep 3600; done' >/dev/null
elif [[ "$(docker inspect -f '{{.State.Running}}' "$CONTAINER_NAME")" != "true" ]]; then
    docker start "$CONTAINER_NAME" >/dev/null
fi

section "Provisioning container toolchain"
docker exec "$CONTAINER_NAME" /bin/bash -lc '
set -euo pipefail

if command -v dnf >/dev/null 2>&1; then
    dnf install -y \
        cmake \
        curl \
        git \
        libxml2-devel \
        make \
        ninja-build \
        pkgconf-pkg-config \
        python3 \
        python3-pip \
        tar \
        zstd >/dev/null
fi

if [[ -f /opt/rh/gcc-toolset-15/enable ]]; then
    source /opt/rh/gcc-toolset-15/enable
fi

c++ --version
cat > /tmp/searchkernel-cxx23.cc <<'CPP'
#include <format>
#include <string>
int main() {
    std::string value = std::format("{}", 42);
    return value == "42" ? 0 : 1;
}
CPP
c++ -std=c++23 /tmp/searchkernel-cxx23.cc -o /tmp/searchkernel-cxx23
/tmp/searchkernel-cxx23
rm -f /tmp/searchkernel-cxx23.cc /tmp/searchkernel-cxx23

if ! command -v rustup >/dev/null 2>&1; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
fi
source /root/.cargo/env
rustup toolchain install stable --profile minimal --no-self-update
rustup default stable

git config --global --add safe.directory /work

cd /work
SEARCHKERNEL_BUILD_ENV=ci ./dev install flatbuffers
'

section "Installing newest native base into persistent runner cache"
docker exec \
    -e SEARCHKERNEL_BUILD_ENV="$BUILD_ENV" \
    -e SK_NATIVE_TARGET="$NATIVE_TARGET" \
    -e SK_NATIVE_BASE_ARTIFACT_URL="file:///native-artifacts/$ARTIFACT_NAME" \
    -e CARGO_TARGET_DIR=/work/target/ci \
    "$CONTAINER_NAME" \
    /bin/bash -lc '
set -euo pipefail
if [[ -f /opt/rh/gcc-toolset-15/enable ]]; then
    source /opt/rh/gcc-toolset-15/enable
fi
source /root/.cargo/env
cd /work
./dev native-sdk ensure >/dev/null
./dev native-sdk path
'

section "Runner native environment ready"
echo "Container:      $CONTAINER_NAME"
echo "Image:          $IMAGE"
echo "Checkout:       $ROOT -> /work"
echo "Native base:    $ARTIFACT_NAME"
echo "Artifact cache: $ARTIFACT_CACHE -> /native-artifacts (read-only)"
echo
echo "Build searchkerneld with:"
echo "  docker exec $CONTAINER_NAME /bin/bash -lc 'source /opt/rh/gcc-toolset-15/enable 2>/dev/null || true; source /root/.cargo/env; cd /work; SEARCHKERNEL_BUILD_ENV=ci CARGO_TARGET_DIR=target/ci cargo build -p searchkerneld --features native'"
