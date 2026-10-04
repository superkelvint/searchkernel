#!/usr/bin/env bash
set -euo pipefail

# Publish the reusable language/compiler/tooling layer.
#
# Usage:
#   ./dev image toolchain publish
#   bash tools/publish-searchkernel-ci-toolchain.sh
#
# The immutable tag is a content hash of docker/searchkernel-ci-toolchain/ plus
# the TypeScript package manifest/lockfile and the resolved digest of the
# upstream base image. Adding a client language, changing pinned client tooling,
# changing a compiler/runtime, or changing the upstream base bytes therefore
# creates a new toolchain identity without changing the Vespa bundle identity.

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd "$ROOT"

source "$ROOT/tools/searchkernel-ci-image-common.sh"

IMAGE="${SEARCHKERNEL_CI_TOOLCHAIN_IMAGE:-superkelvint/searchkernel-ci-toolchain}"
BASE_IMAGE="${SEARCHKERNEL_CI_TOOLCHAIN_BASE_IMAGE:-docker.io/vespaengine/vespa-dev-almalinux-8:latest}"
RECIPE="$ROOT/docker/searchkernel-ci-toolchain"
TYPESCRIPT_PACKAGE="$ROOT/clients/typescript/package.json"
TYPESCRIPT_LOCK="$ROOT/clients/typescript/package-lock.json"

ci_image_require_commands docker git python3
ci_image_require_docker
ci_image_require_clean_checkout "$ROOT"
ci_image_login

echo
echo "==> Resolving immutable toolchain base image"
PINNED_BASE_IMAGE="$(ci_image_pull_and_resolve_digest "$BASE_IMAGE")"

TOOLCHAIN_ID="$(ci_image_toolchain_id "$ROOT" "$PINNED_BASE_IMAGE")"
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

BUILD_CONTEXT="$(mktemp -d "${TMPDIR:-/tmp}/searchkernel-ci-toolchain.XXXXXX")"
trap 'rm -rf -- "$BUILD_CONTEXT"' EXIT
mkdir -p "$BUILD_CONTEXT/clients/typescript"
cp "$RECIPE/Dockerfile" "$BUILD_CONTEXT/Dockerfile"
cp "$TYPESCRIPT_PACKAGE" "$BUILD_CONTEXT/clients/typescript/package.json"
cp "$TYPESCRIPT_LOCK" "$BUILD_CONTEXT/clients/typescript/package-lock.json"
mkdir -p "$BUILD_CONTEXT/integrations/instantsearch"
cp "$ROOT/integrations/instantsearch/package.json" "$BUILD_CONTEXT/integrations/instantsearch/package.json"
cp "$ROOT/integrations/instantsearch/package-lock.json" "$BUILD_CONTEXT/integrations/instantsearch/package-lock.json"

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
    -f "$BUILD_CONTEXT/Dockerfile"
    -t "$IMAGE_REF"
)
DOCKER_BUILDKIT=1 docker build "${toolchain_build_args[@]}" "$BUILD_CONTEXT"

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
test -x "$SEARCHKERNEL_TYPESCRIPT_NODE_MODULES/grpc-tools/bin/protoc"
test -x "$SEARCHKERNEL_TYPESCRIPT_NODE_MODULES/grpc-tools/bin/grpc_node_plugin"
test -x "$SEARCHKERNEL_TYPESCRIPT_NODE_MODULES/protoc-gen-grpc-web/bin/protoc-gen-grpc-web"
test -x "$SEARCHKERNEL_TYPESCRIPT_NODE_MODULES/.bin/tsc"
test -x "$SEARCHKERNEL_TYPESCRIPT_NODE_MODULES/.bin/vitest"
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
