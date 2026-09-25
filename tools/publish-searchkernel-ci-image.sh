#!/usr/bin/env bash
set -euo pipefail

# Build and publish the thin Search Kernel CI leaf image.
#
# Reusable inputs are published independently:
#   superkelvint/searchkernel-ci-toolchain:<toolchain-id>
#   superkelvint/searchkernel-ci-vespa:<vespa-commit>-<bundle-id>
#
# Composition always consumes immutable repository digests, never mutable tags.
# A composite base is cached by the exact toolchain image digest, Vespa image
# digest, and composition recipe. Ordinary source publishes then validate only
# the current Search Kernel source revision on top of that immutable base.
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
ci_image_require_clean_checkout "$ROOT"

docker build --help 2>/dev/null | grep -q -- '--build-context' ||
    ci_image_die "Docker BuildKit with --build-context support is required"

TAG="${1:-}"
if [[ -z "$TAG" ]]; then
    TAG="$(git rev-parse --short=12 HEAD)"
fi
[[ "$TAG" =~ ^[A-Za-z0-9_][A-Za-z0-9_.-]{0,127}$ ]] ||
    ci_image_die "invalid Docker tag: $TAG"
[[ "$TAG" != base-* ]] ||
    ci_image_die "Docker tags beginning with 'base-' are reserved for composite CI bases"
[[ "$TAG" != latest ]] ||
    ci_image_die "Docker tag 'latest' is reserved for canonical promotion; omit the tag and publish from current main"

[[ "$PUSH_LATEST" == 0 || "$PUSH_LATEST" == 1 ]] ||
    ci_image_die "SEARCHKERNEL_CI_PUSH_LATEST must be 0 or 1"

if [[ "$PUSH_LATEST" == 1 ]]; then
    ci_image_require_main_checkout "$ROOT"
fi

ci_image_login

echo
echo "==> Resolving immutable toolchain base"
PINNED_TOOLCHAIN_BASE="$(ci_image_pull_and_resolve_digest "$TOOLCHAIN_BASE_IMAGE")"

TOOLCHAIN_ID="$(ci_image_toolchain_id "$ROOT/docker/searchkernel-ci-toolchain" "$PINNED_TOOLCHAIN_BASE")"
VESPA_COMMIT="$(ci_image_vespa_commit "$ROOT")"
VESPA_BUNDLE_ID="$(ci_image_vespa_bundle_id "$ROOT")"

TOOLCHAIN_REF="$TOOLCHAIN_IMAGE:$TOOLCHAIN_ID"
VESPA_REF="$VESPA_IMAGE:$VESPA_COMMIT-$VESPA_BUNDLE_ID"

echo
echo "CI image inputs:"
echo "  toolchain tag:  $TOOLCHAIN_REF"
echo "  toolchain base: $PINNED_TOOLCHAIN_BASE"
echo "  Vespa tag:      $VESPA_REF"

if ! ci_image_pull_if_available "$TOOLCHAIN_REF"; then
    cat >&2 <<EOF

ERROR: required CI toolchain image could not be pulled:

  $TOOLCHAIN_REF

The toolchain recipe or its resolved upstream base changed, or registry access
failed. If this is a new toolchain, publish it once with:

  ./dev image toolchain publish

EOF
    exit 1
fi
ci_image_require_label "$TOOLCHAIN_REF" io.searchkernel.toolchain-id "$TOOLCHAIN_ID"
ci_image_require_label "$TOOLCHAIN_REF" io.searchkernel.toolchain-base "$PINNED_TOOLCHAIN_BASE"
PINNED_TOOLCHAIN_REF="$(ci_image_resolve_local_digest "$TOOLCHAIN_REF")"

if ! ci_image_pull_if_available "$VESPA_REF"; then
    cat >&2 <<EOF

ERROR: required Vespa bundle image could not be pulled:

  $VESPA_REF

The pinned Vespa bundle may be unpublished, or registry access may have failed.
Publish a new bundle once with:

  ./dev image vespa publish

EOF
    exit 1
fi
ci_image_require_label "$VESPA_REF" io.searchkernel.vespa-commit "$VESPA_COMMIT"
ci_image_require_label "$VESPA_REF" io.searchkernel.vespa-bundle-id "$VESPA_BUNDLE_ID"
PINNED_VESPA_REF="$(ci_image_resolve_local_digest "$VESPA_REF")"

COMPOSITE_ID="$(ci_image_composite_id     "$ROOT/docker/searchkernel-ci/base.Dockerfile"     "$PINNED_TOOLCHAIN_REF"     "$PINNED_VESPA_REF")"
COMPOSITE_REF="$IMAGE:base-$COMPOSITE_ID"

echo "  toolchain:      $PINNED_TOOLCHAIN_REF"
echo "  Vespa:          $PINNED_VESPA_REF"
echo "  composite:      $COMPOSITE_REF"

if ci_image_pull_if_available "$COMPOSITE_REF"; then
    ci_image_require_label "$COMPOSITE_REF" io.searchkernel.composite-id "$COMPOSITE_ID"
    ci_image_require_label "$COMPOSITE_REF" io.searchkernel.toolchain-id "$TOOLCHAIN_ID"
    ci_image_require_label "$COMPOSITE_REF" io.searchkernel.vespa-commit "$VESPA_COMMIT"
    ci_image_require_label "$COMPOSITE_REF" io.searchkernel.vespa-bundle-id "$VESPA_BUNDLE_ID"
    ci_image_require_label "$COMPOSITE_REF" io.searchkernel.toolchain-image-digest "$PINNED_TOOLCHAIN_REF"
    ci_image_require_label "$COMPOSITE_REF" io.searchkernel.vespa-image-digest "$PINNED_VESPA_REF"
    echo "Reusing published composite base."
else
    echo
    echo "==> Composing compatible toolchain + Vespa base"

    base_build_args=(
        --build-arg "TOOLCHAIN_IMAGE=$PINNED_TOOLCHAIN_REF"
        --build-arg "VESPA_IMAGE=$PINNED_VESPA_REF"
        --build-arg "TOOLCHAIN_ID=$TOOLCHAIN_ID"
        --build-arg "VESPA_COMMIT=$VESPA_COMMIT"
        --build-arg "VESPA_BUNDLE_ID=$VESPA_BUNDLE_ID"
        --label "org.opencontainers.image.source=https://github.com/superkelvint/searchkernel"
        --label "io.searchkernel.composite-id=$COMPOSITE_ID"
        --label "io.searchkernel.toolchain-id=$TOOLCHAIN_ID"
        --label "io.searchkernel.vespa-commit=$VESPA_COMMIT"
        --label "io.searchkernel.vespa-bundle-id=$VESPA_BUNDLE_ID"
        --label "io.searchkernel.toolchain-image-digest=$PINNED_TOOLCHAIN_REF"
        --label "io.searchkernel.vespa-image-digest=$PINNED_VESPA_REF"
        -f "$ROOT/docker/searchkernel-ci/base.Dockerfile"
        -t "$COMPOSITE_REF"
    )
    DOCKER_BUILDKIT=1 docker build "${base_build_args[@]}" "$ROOT/docker/searchkernel-ci"

    ci_image_require_label "$COMPOSITE_REF" io.searchkernel.composite-id "$COMPOSITE_ID"
    ci_image_require_label "$COMPOSITE_REF" io.searchkernel.toolchain-id "$TOOLCHAIN_ID"
    ci_image_require_label "$COMPOSITE_REF" io.searchkernel.vespa-commit "$VESPA_COMMIT"
    ci_image_require_label "$COMPOSITE_REF" io.searchkernel.vespa-bundle-id "$VESPA_BUNDLE_ID"
    ci_image_require_label "$COMPOSITE_REF" io.searchkernel.toolchain-image-digest "$PINNED_TOOLCHAIN_REF"
    ci_image_require_label "$COMPOSITE_REF" io.searchkernel.vespa-image-digest "$PINNED_VESPA_REF"

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

# Resolve the registry digest after the push/pull boundary and use that exact
# immutable image as the leaf base. This closes the tag-mutation race between
# validation and the following Docker build.
PINNED_COMPOSITE_REF="$(ci_image_pull_and_resolve_digest "$COMPOSITE_REF")"
ci_image_require_label "$PINNED_COMPOSITE_REF" io.searchkernel.composite-id "$COMPOSITE_ID"
ci_image_require_label "$PINNED_COMPOSITE_REF" io.searchkernel.toolchain-id "$TOOLCHAIN_ID"
ci_image_require_label "$PINNED_COMPOSITE_REF" io.searchkernel.vespa-commit "$VESPA_COMMIT"
ci_image_require_label "$PINNED_COMPOSITE_REF" io.searchkernel.vespa-bundle-id "$VESPA_BUNDLE_ID"
ci_image_require_label "$PINNED_COMPOSITE_REF" io.searchkernel.toolchain-image-digest "$PINNED_TOOLCHAIN_REF"
ci_image_require_label "$PINNED_COMPOSITE_REF" io.searchkernel.vespa-image-digest "$PINNED_VESPA_REF"

TMP="$(mktemp -d "${TMPDIR:-/tmp}/searchkernel-ci-image.XXXXXX")"
cleanup() {
    rm -rf "$TMP"
}
trap cleanup EXIT INT TERM

echo
echo "==> Preparing clean Search Kernel source context"

SOURCE_CONTEXT="$TMP/source"
mkdir -p "$SOURCE_CONTEXT"

# Use exactly HEAD. ci_image_require_clean_checkout above prevents a publish
# from mixing committed source with uncommitted image recipes.
git archive --format=tar HEAD | tar -xf - -C "$SOURCE_CONTEXT"

[[ -f "$SOURCE_CONTEXT/Cargo.toml" ]] ||
    ci_image_die "source context is missing Cargo.toml"

IMAGE_REF="$IMAGE:$TAG"
REVISION="$(git rev-parse HEAD)"

echo
echo "==> Building thin CI leaf $IMAGE_REF"
echo "Revision:         $REVISION"
echo "Toolchain id:     $TOOLCHAIN_ID"
echo "Vespa commit:     $VESPA_COMMIT"
echo "Vespa bundle id:  $VESPA_BUNDLE_ID"
echo "Composite digest: $PINNED_COMPOSITE_REF"

leaf_build_args=(
    --build-context "searchkernel-source=$SOURCE_CONTEXT"
    --build-arg "CI_BASE_IMAGE=$PINNED_COMPOSITE_REF"
    --build-arg "TOOLCHAIN_ID=$TOOLCHAIN_ID"
    --build-arg "VESPA_COMMIT=$VESPA_COMMIT"
    --build-arg "VESPA_BUNDLE_ID=$VESPA_BUNDLE_ID"
    --build-arg "BUILDKIT_CACHE_MOUNT_NS=searchkernel-ci-publish-$COMPOSITE_ID"
    --label "org.opencontainers.image.source=https://github.com/superkelvint/searchkernel"
    --label "org.opencontainers.image.revision=$REVISION"
    --label "io.searchkernel.composite-id=$COMPOSITE_ID"
    --label "io.searchkernel.composite-image-digest=$PINNED_COMPOSITE_REF"
    --label "io.searchkernel.toolchain-id=$TOOLCHAIN_ID"
    --label "io.searchkernel.vespa-commit=$VESPA_COMMIT"
    --label "io.searchkernel.vespa-bundle-id=$VESPA_BUNDLE_ID"
    -f "$ROOT/docker/searchkernel-ci/Dockerfile"
    -t "$IMAGE_REF"
)
DOCKER_BUILDKIT=1 docker build "${leaf_build_args[@]}" "$ROOT/docker/searchkernel-ci"

echo
echo "==> Verifying final image capabilities"

ci_image_require_label "$IMAGE_REF" io.searchkernel.composite-id "$COMPOSITE_ID"
ci_image_require_label "$IMAGE_REF" io.searchkernel.composite-image-digest "$PINNED_COMPOSITE_REF"
ci_image_require_label "$IMAGE_REF" io.searchkernel.toolchain-id "$TOOLCHAIN_ID"
ci_image_require_label "$IMAGE_REF" io.searchkernel.vespa-commit "$VESPA_COMMIT"
ci_image_require_label "$IMAGE_REF" io.searchkernel.vespa-bundle-id "$VESPA_BUNDLE_ID"

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
