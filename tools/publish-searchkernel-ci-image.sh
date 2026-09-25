#!/usr/bin/env bash
set -euo pipefail

# Build and publish the thin Search Kernel CI leaf image.
#
# Reusable inputs are published independently:
#   superkelvint/searchkernel-ci-toolchain:<toolchain-id>
#   superkelvint/searchkernel-ci-vespa:<vespa-commit>
#
# This script automatically reuses (or creates once) a composite base tag in
# superkelvint/searchkernel-ci and then validates only the current Search Kernel
# source revision on top of it.
#
# Preferred usage:
#   ./dev image publish [tag]
#
# Dependency publishing:
#   ./dev image toolchain publish   # language/compiler/runtime changes
#   ./dev image vespa publish       # pinned Vespa/native bundle changes

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd "$ROOT"

source "$ROOT/tools/searchkernel-ci-image-common.sh"

IMAGE="${SEARCHKERNEL_CI_IMAGE:-superkelvint/searchkernel-ci}"
TOOLCHAIN_IMAGE="${SEARCHKERNEL_CI_TOOLCHAIN_IMAGE:-superkelvint/searchkernel-ci-toolchain}"
TOOLCHAIN_BASE_IMAGE="${SEARCHKERNEL_CI_TOOLCHAIN_BASE_IMAGE:-docker.io/vespaengine/vespa-dev-almalinux-8:latest}"
VESPA_IMAGE="${SEARCHKERNEL_CI_VESPA_IMAGE:-superkelvint/searchkernel-ci-vespa}"
PUSH_LATEST="${SEARCHKERNEL_CI_PUSH_LATEST:-0}"

ci_image_require_commands docker git python3 tar
ci_image_require_docker

docker build --help 2>/dev/null | grep -q -- '--build-context' ||
    ci_image_die "Docker BuildKit with --build-context support is required"

TAG="${1:-}"
if [[ -z "$TAG" ]]; then
    TAG="$(git rev-parse --short=12 HEAD)"
fi
[[ "$TAG" =~ ^[A-Za-z0-9_][A-Za-z0-9_.-]{0,127}$ ]] ||
    ci_image_die "invalid Docker tag: $TAG"

[[ "$PUSH_LATEST" == 0 || "$PUSH_LATEST" == 1 ]] ||
    ci_image_die "SEARCHKERNEL_CI_PUSH_LATEST must be 0 or 1"

ci_image_login

TOOLCHAIN_ID="$(ci_image_toolchain_id "$ROOT/docker/searchkernel-ci-toolchain" "$TOOLCHAIN_BASE_IMAGE")"
VESPA_COMMIT="$(ci_image_vespa_commit "$ROOT")"
COMPOSITE_ID="$(ci_image_composite_id "$ROOT/docker/searchkernel-ci/base.Dockerfile" "$TOOLCHAIN_ID" "$VESPA_COMMIT")"

TOOLCHAIN_REF="$TOOLCHAIN_IMAGE:$TOOLCHAIN_ID"
VESPA_REF="$VESPA_IMAGE:$VESPA_COMMIT"
COMPOSITE_TAG="base-$COMPOSITE_ID"
COMPOSITE_REF="$IMAGE:$COMPOSITE_TAG"

echo
echo "CI image inputs:"
echo "  toolchain: $TOOLCHAIN_REF"
echo "  Vespa:     $VESPA_REF"
echo "  base:      $COMPOSITE_REF"

if ci_image_pull_if_available "$COMPOSITE_REF"; then
    ci_image_require_label "$COMPOSITE_REF" io.searchkernel.toolchain-id "$TOOLCHAIN_ID"
    ci_image_require_label "$COMPOSITE_REF" io.searchkernel.vespa-commit "$VESPA_COMMIT"
    echo "Reusing published composite base."
else
    echo
    echo "==> Composite base is missing; resolving reusable inputs"

    if ! ci_image_pull_if_available "$TOOLCHAIN_REF"; then
        cat >&2 <<EOF

ERROR: required CI toolchain image is not published:

  $TOOLCHAIN_REF

The toolchain recipe changed (for example a client language/runtime/compiler
was added or bumped). Publish it once with:

  ./dev image toolchain publish

EOF
        exit 1
    fi
    ci_image_require_label "$TOOLCHAIN_REF" io.searchkernel.toolchain-id "$TOOLCHAIN_ID"

    if ! ci_image_pull_if_available "$VESPA_REF"; then
        cat >&2 <<EOF

ERROR: required Vespa bundle image is not published:

  $VESPA_REF

Publish the pinned Vespa/native bundle once with:

  ./dev image vespa publish

EOF
        exit 1
    fi
    ci_image_require_label "$VESPA_REF" io.searchkernel.vespa-commit "$VESPA_COMMIT"

    echo
    echo "==> Composing compatible toolchain + Vespa base"

    DOCKER_BUILDKIT=1 docker build         --build-arg "TOOLCHAIN_IMAGE=$TOOLCHAIN_REF"         --build-arg "VESPA_IMAGE=$VESPA_REF"         --build-arg "TOOLCHAIN_ID=$TOOLCHAIN_ID"         --build-arg "VESPA_COMMIT=$VESPA_COMMIT"         --label "org.opencontainers.image.source=https://github.com/superkelvint/searchkernel"         --label "io.searchkernel.composite-id=$COMPOSITE_ID"         --label "io.searchkernel.toolchain-id=$TOOLCHAIN_ID"         --label "io.searchkernel.vespa-commit=$VESPA_COMMIT"         -f "$ROOT/docker/searchkernel-ci/base.Dockerfile"         -t "$COMPOSITE_REF"         "$ROOT/docker/searchkernel-ci"

    ci_image_require_label "$COMPOSITE_REF" io.searchkernel.toolchain-id "$TOOLCHAIN_ID"
    ci_image_require_label "$COMPOSITE_REF" io.searchkernel.vespa-commit "$VESPA_COMMIT"

    docker run --rm "$COMPOSITE_REF" /bin/bash -lc '
      set -euo pipefail
      cache=/opt/searchkernel/native-sdk-cache/ci/x86_64-unknown-linux-gnu
      test -s "$cache/objects/'"$VESPA_COMMIT"'/lib/libvespalib.so"
      test -f "$cache/toolchains/'"$VESPA_COMMIT"'/upstream/vespa/.searchkernel-vespa-artifact.json"
      for cmd in cargo rustc rustfmt cargo-clippy c++ flatc python3 curl rg gh php ruby composer node npm nm readelf; do
        command -v "$cmd" >/dev/null
      done
    '

    echo
    echo "==> Pushing reusable composite base"
    docker push "$COMPOSITE_REF"
fi

TMP="$(mktemp -d "${TMPDIR:-/tmp}/searchkernel-ci-image.XXXXXX")"
cleanup() {
    rm -rf "$TMP"
}
trap cleanup EXIT INT TERM

echo
echo "==> Preparing clean Search Kernel source context"

SOURCE_CONTEXT="$TMP/source"
mkdir -p "$SOURCE_CONTEXT"

# Use exactly HEAD: do not accidentally publish an image whose validation was
# performed against uncommitted working-tree state.
git archive --format=tar HEAD | tar -xf - -C "$SOURCE_CONTEXT"

[[ -f "$SOURCE_CONTEXT/Cargo.toml" ]] ||
    ci_image_die "source context is missing Cargo.toml"

IMAGE_REF="$IMAGE:$TAG"
REVISION="$(git rev-parse HEAD)"

echo
echo "==> Building thin CI leaf $IMAGE_REF"
echo "Revision:     $REVISION"
echo "Toolchain id: $TOOLCHAIN_ID"
echo "Vespa commit: $VESPA_COMMIT"

DOCKER_BUILDKIT=1 docker build     --build-context "searchkernel-source=$SOURCE_CONTEXT"     --build-arg "CI_BASE_IMAGE=$COMPOSITE_REF"     --build-arg "TOOLCHAIN_ID=$TOOLCHAIN_ID"     --build-arg "VESPA_COMMIT=$VESPA_COMMIT"     --label "org.opencontainers.image.source=https://github.com/superkelvint/searchkernel"     --label "org.opencontainers.image.revision=$REVISION"     --label "io.searchkernel.toolchain-id=$TOOLCHAIN_ID"     --label "io.searchkernel.vespa-commit=$VESPA_COMMIT"     -f "$ROOT/docker/searchkernel-ci/Dockerfile"     -t "$IMAGE_REF"     "$ROOT/docker/searchkernel-ci"

echo
echo "==> Verifying final image capabilities"

ci_image_require_label "$IMAGE_REF" io.searchkernel.toolchain-id "$TOOLCHAIN_ID"
ci_image_require_label "$IMAGE_REF" io.searchkernel.vespa-commit "$VESPA_COMMIT"

docker run --rm "$IMAGE_REF" /bin/bash -lc '
set -euo pipefail
test -f /opt/searchkernel/native-validation-pass
node -e '"'"'const [M,m]=process.versions.node.split(".").map(Number); if (M < 20 || (M === 20 && m < 19)) process.exit(1)'"'"'
ruby -e '"'"'abort if Gem::Version.new(RUBY_VERSION) < Gem::Version.new("3.3.0")'"'"'
php -r '"'"'exit(version_compare(PHP_VERSION, "8.1.0", ">=") ? 0 : 1);'"'"'
python=$PYTHON
"$python" -c '"'"'import flatbuffers, grpc, google.protobuf'"'"'
test "$(flatc --version)" = "flatc version 25.12.19"
rg --version >/dev/null
gh --version >/dev/null
rustc --version
cargo --version
cargo llvm-cov --version
node --version
ruby --version
php --version | head -n 1
"$python" --version
flatc --version
'

echo
echo "==> Pushing $IMAGE_REF"
docker push "$IMAGE_REF"

if [[ "$PUSH_LATEST" == 1 ]]; then
    echo
    echo "==> Pushing $IMAGE:latest"
    docker tag "$IMAGE_REF" "$IMAGE:latest"
    docker push "$IMAGE:latest"
fi

echo
echo "Published:"
echo "  $IMAGE_REF"
if [[ "$PUSH_LATEST" == 1 ]]; then
    echo "  $IMAGE:latest"
fi
