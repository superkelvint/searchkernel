#!/usr/bin/env bash
set -euo pipefail

# Build and publish the thin Search Kernel CI leaf image.
#
# Reusable inputs are published independently:
#   superkelvint/searchkernel-ci-toolchain:<toolchain-id>
#   superkelvint/searchkernel-ci-vespa:<vespa-commit>-<bundle-id>
#
# The final image inherits only the toolchain. Vespa is mounted from its own
# immutable image during build-time validation and is mounted separately by CI
# runners at runtime. No Vespa payload is copied or pushed into searchkernel-ci.
#
# Preferred usage:
#   ./dev image publish [tag]

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
ci_image_login

TAG="${1:-}"
if [[ -z "$TAG" ]]; then
    TAG="$(git rev-parse --short=12 HEAD)"
fi
[[ "$TAG" =~ ^[A-Za-z0-9_][A-Za-z0-9_.-]{0,127}$ ]] ||
    ci_image_die "invalid Docker tag: $TAG"
[[ "$TAG" != base-* ]] ||
    ci_image_die "Docker tags beginning with 'base-' are reserved"
[[ "$TAG" != latest ]] ||
    ci_image_die "Docker tag 'latest' is reserved for canonical promotion; omit the tag and publish from main"

[[ "$PUSH_LATEST" == 0 || "$PUSH_LATEST" == 1 ]] ||
    ci_image_die "SEARCHKERNEL_CI_PUSH_LATEST must be 0 or 1"

if [[ "$PUSH_LATEST" == 1 ]]; then
    ci_image_require_main_checkout "$ROOT"
fi

echo
echo "==> Resolving immutable toolchain base"
PINNED_TOOLCHAIN_BASE="$(ci_image_pull_and_resolve_digest "$TOOLCHAIN_BASE_IMAGE")"

TOOLCHAIN_ID="$(ci_image_toolchain_id "$ROOT" "$PINNED_TOOLCHAIN_BASE")"
VESPA_COMMIT="$(ci_image_vespa_commit "$ROOT")"
VESPA_BUNDLE_ID="$(ci_image_vespa_bundle_id "$ROOT")"

TOOLCHAIN_REF="$TOOLCHAIN_IMAGE:$TOOLCHAIN_ID"
VESPA_REF="$VESPA_IMAGE:$VESPA_COMMIT-$VESPA_BUNDLE_ID"

if ! ci_image_pull_if_available "$TOOLCHAIN_REF"; then
    cat >&2 <<EOF

ERROR: required CI toolchain image is not published:

  $TOOLCHAIN_REF

Publish it once with:

  ./dev image toolchain publish

EOF
    exit 1
fi
ci_image_require_label "$TOOLCHAIN_REF" io.searchkernel.toolchain-id "$TOOLCHAIN_ID"
ci_image_require_label "$TOOLCHAIN_REF" io.searchkernel.toolchain-base "$PINNED_TOOLCHAIN_BASE"
PINNED_TOOLCHAIN_REF="$(ci_image_resolve_local_digest "$TOOLCHAIN_REF")"

if ! ci_image_pull_if_available "$VESPA_REF"; then
    cat >&2 <<EOF

ERROR: required Vespa bundle image is not published:

  $VESPA_REF

Publish it once with:

  ./dev image vespa publish

EOF
    exit 1
fi
ci_image_require_label "$VESPA_REF" io.searchkernel.vespa-commit "$VESPA_COMMIT"
ci_image_require_label "$VESPA_REF" io.searchkernel.vespa-bundle-id "$VESPA_BUNDLE_ID"
PINNED_VESPA_REF="$(ci_image_resolve_local_digest "$VESPA_REF")"

CI_ENVIRONMENT_ID="$(ci_image_environment_id     "$ROOT/docker/searchkernel-ci/Dockerfile"     "$PINNED_TOOLCHAIN_REF"     "$PINNED_VESPA_REF")"

echo
echo "CI image inputs:"
echo "  toolchain:   $PINNED_TOOLCHAIN_REF"
echo "  Vespa:       $PINNED_VESPA_REF"
echo "  environment: $CI_ENVIRONMENT_ID"

TMP="$(mktemp -d "${TMPDIR:-/tmp}/searchkernel-ci-image.XXXXXX")"
cleanup() {
    rm -rf "$TMP"
}
trap cleanup EXIT INT TERM

SOURCE_CONTEXT="$TMP/source"
mkdir -p "$SOURCE_CONTEXT"
git archive --format=tar HEAD | tar -xf - -C "$SOURCE_CONTEXT"
[[ -f "$SOURCE_CONTEXT/Cargo.toml" ]] ||
    ci_image_die "source context is missing Cargo.toml"

IMAGE_REF="$IMAGE:$TAG"
REVISION="$(git rev-parse HEAD)"

echo
echo "==> Building thin CI leaf $IMAGE_REF"
leaf_build_args=(
    --build-arg "TOOLCHAIN_IMAGE=$PINNED_TOOLCHAIN_REF"
    --build-arg "VESPA_IMAGE=$PINNED_VESPA_REF"
    --build-arg "TOOLCHAIN_ID=$TOOLCHAIN_ID"
    --build-arg "VESPA_COMMIT=$VESPA_COMMIT"
    --build-arg "VESPA_BUNDLE_ID=$VESPA_BUNDLE_ID"
    --build-arg "CI_ENVIRONMENT_ID=$CI_ENVIRONMENT_ID"
    --build-arg "BUILDKIT_CACHE_MOUNT_NS=searchkernel-ci-publish-$CI_ENVIRONMENT_ID"
    --label "org.opencontainers.image.source=https://github.com/superkelvint/searchkernel"
    --label "org.opencontainers.image.revision=$REVISION"
    --label "io.searchkernel.environment-id=$CI_ENVIRONMENT_ID"
    --label "io.searchkernel.toolchain-id=$TOOLCHAIN_ID"
    --label "io.searchkernel.toolchain-image-digest=$PINNED_TOOLCHAIN_REF"
    --label "io.searchkernel.vespa-commit=$VESPA_COMMIT"
    --label "io.searchkernel.vespa-bundle-id=$VESPA_BUNDLE_ID"
    --label "io.searchkernel.vespa-image-digest=$PINNED_VESPA_REF"
    -f "$ROOT/docker/searchkernel-ci/Dockerfile"
    -t "$IMAGE_REF"
)
DOCKER_BUILDKIT=1 docker build "${leaf_build_args[@]}" "$SOURCE_CONTEXT"

ci_image_require_label "$IMAGE_REF" io.searchkernel.environment-id "$CI_ENVIRONMENT_ID"
ci_image_require_label "$IMAGE_REF" io.searchkernel.toolchain-id "$TOOLCHAIN_ID"
ci_image_require_label "$IMAGE_REF" io.searchkernel.toolchain-image-digest "$PINNED_TOOLCHAIN_REF"
ci_image_require_label "$IMAGE_REF" io.searchkernel.vespa-commit "$VESPA_COMMIT"
ci_image_require_label "$IMAGE_REF" io.searchkernel.vespa-bundle-id "$VESPA_BUNDLE_ID"
ci_image_require_label "$IMAGE_REF" io.searchkernel.vespa-image-digest "$PINNED_VESPA_REF"

echo
echo "==> Verifying final image is genuinely thin"
docker run --rm "$IMAGE_REF" /bin/bash -lc '
set -euo pipefail
test -f /opt/searchkernel/native-validation-pass
if find /opt/searchkernel/native-sdk-cache -mindepth 1 -print -quit 2>/dev/null | grep -q .; then
  echo "final CI image unexpectedly contains native SDK/Vespa payload" >&2
  exit 1
fi
cargo llvm-cov --version >/dev/null
rustc --version | grep -q "^rustc 1\.98\.0 "
test "$(flatc --version)" = "flatc version 25.12.19"
"$PYTHON" -c "import flatbuffers, grpc, google.protobuf"
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
echo
echo "Vespa remains external and is NOT republished into $IMAGE."
