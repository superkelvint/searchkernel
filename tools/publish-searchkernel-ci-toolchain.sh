#!/usr/bin/env bash
set -euo pipefail

# Publish the reusable language/compiler/tooling layer.
#
# Usage:
#   ./dev image toolchain publish
#   tools/publish-searchkernel-ci-toolchain.sh
#
# The immutable tag is a content hash of docker/searchkernel-ci-toolchain/.
# Adding a client language or changing a compiler/runtime changes that identity
# without changing the Vespa bundle identity.

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd "$ROOT"

source "$ROOT/tools/searchkernel-ci-image-common.sh"

IMAGE="${SEARCHKERNEL_CI_TOOLCHAIN_IMAGE:-superkelvint/searchkernel-ci-toolchain}"
BASE_IMAGE="${SEARCHKERNEL_CI_TOOLCHAIN_BASE_IMAGE:-docker.io/vespaengine/vespa-dev-almalinux-8:latest}"
RECIPE="$ROOT/docker/searchkernel-ci-toolchain"

ci_image_require_commands docker git python3
ci_image_require_docker
ci_image_login

TOOLCHAIN_ID="$(ci_image_toolchain_id "$RECIPE" "$BASE_IMAGE")"
IMAGE_REF="$IMAGE:$TOOLCHAIN_ID"

if ci_image_pull_if_available "$IMAGE_REF"; then
    ci_image_require_label "$IMAGE_REF" io.searchkernel.toolchain-id "$TOOLCHAIN_ID"
    echo
    echo "Toolchain image already published:"
    echo "  $IMAGE_REF"
    exit 0
fi

echo
echo "==> Pinning toolchain base image"
docker pull "$BASE_IMAGE" >/dev/null
PINNED_BASE_IMAGE="$(
    docker image inspect \
        --format '{{if .RepoDigests}}{{index .RepoDigests 0}}{{end}}' \
        "$BASE_IMAGE"
)"
[[ -n "$PINNED_BASE_IMAGE" ]] ||
    ci_image_die "could not resolve an immutable digest for $BASE_IMAGE"

REVISION="$(git rev-parse HEAD)"

echo
echo "==> Building reusable CI toolchain"
echo "Toolchain id: $TOOLCHAIN_ID"
echo "Base image:   $PINNED_BASE_IMAGE"
echo "Revision:     $REVISION"

toolchain_build_args=(
    --build-arg "BASE_IMAGE=$PINNED_BASE_IMAGE"
    --label "org.opencontainers.image.source=https://github.com/superkelvint/searchkernel"
    --label "org.opencontainers.image.revision=$REVISION"
    --label "io.searchkernel.toolchain-id=$TOOLCHAIN_ID"
    --label "io.searchkernel.toolchain-base=$PINNED_BASE_IMAGE"
    -f "$RECIPE/Dockerfile"
    -t "$IMAGE_REF"
)
DOCKER_BUILDKIT=1 docker build "${toolchain_build_args[@]}" "$RECIPE"

ci_image_require_label "$IMAGE_REF" io.searchkernel.toolchain-id "$TOOLCHAIN_ID"

docker run --rm "$IMAGE_REF" /bin/bash -lc '
set -euo pipefail
for cmd in cargo rustc rustfmt cargo-clippy c++ flatc python3 curl rg gh php ruby composer node npm nm readelf; do
  command -v "$cmd" >/dev/null
done
cargo llvm-cov --version
rustc --version | grep -q "^rustc 1\.98\.0 "
test "$(flatc --version)" = "flatc version 25.12.19"
node -e '"'"'const [M,m]=process.versions.node.split(".").map(Number); if (M < 20 || (M === 20 && m < 19)) process.exit(1)'"'"'
ruby -e '"'"'abort if Gem::Version.new(RUBY_VERSION) < Gem::Version.new("3.3.0")'"'"'
/usr/bin/python3 -c "import clang.cindex"
"$PYTHON" -c "import flatbuffers, grpc, google.protobuf"
'

echo
echo "==> Pushing $IMAGE_REF"
docker push "$IMAGE_REF"

echo
echo "Published:"
echo "  $IMAGE_REF"
