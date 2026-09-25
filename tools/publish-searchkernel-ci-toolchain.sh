#!/usr/bin/env bash
set -euo pipefail

# Publish the reusable language/compiler/tooling layer.
#
# Usage:
#   ./dev image toolchain publish
#   bash tools/publish-searchkernel-ci-toolchain.sh
#
# The immutable tag is a content hash of docker/searchkernel-ci-toolchain/ plus
# the resolved digest of its upstream base image. Adding a client language,
# changing a compiler/runtime, or changing the upstream base bytes therefore
# creates a new toolchain identity without changing the Vespa bundle identity.

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd "$ROOT"

source "$ROOT/tools/searchkernel-ci-image-common.sh"

IMAGE="${SEARCHKERNEL_CI_TOOLCHAIN_IMAGE:-superkelvint/searchkernel-ci-toolchain}"
BASE_IMAGE="${SEARCHKERNEL_CI_TOOLCHAIN_BASE_IMAGE:-docker.io/vespaengine/vespa-dev-almalinux-8:latest}"
RECIPE="$ROOT/docker/searchkernel-ci-toolchain"

ci_image_require_commands docker git python3
ci_image_require_docker
ci_image_require_clean_checkout "$ROOT"
ci_image_login

echo
echo "==> Resolving immutable toolchain base image"
PINNED_BASE_IMAGE="$(ci_image_pull_and_resolve_digest "$BASE_IMAGE")"

TOOLCHAIN_ID="$(ci_image_toolchain_id "$RECIPE" "$PINNED_BASE_IMAGE")"
IMAGE_REF="$IMAGE:$TOOLCHAIN_ID"

if ci_image_pull_if_available "$IMAGE_REF"; then
    ci_image_require_label "$IMAGE_REF" io.searchkernel.toolchain-id "$TOOLCHAIN_ID"
    ci_image_require_label "$IMAGE_REF" io.searchkernel.toolchain-base "$PINNED_BASE_IMAGE"
    echo
    echo "Toolchain image already published:"
    echo "  $IMAGE_REF"
    echo "  base: $PINNED_BASE_IMAGE"
    exit 0
fi

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
ci_image_require_label "$IMAGE_REF" io.searchkernel.toolchain-base "$PINNED_BASE_IMAGE"

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

# Pull once after push so the local image acquires the same immutable repository
# digest consumers will use during composition.
PINNED_IMAGE_REF="$(ci_image_pull_and_resolve_digest "$IMAGE_REF")"

echo
echo "Published:"
echo "  $IMAGE_REF"
echo "  $PINNED_IMAGE_REF"
