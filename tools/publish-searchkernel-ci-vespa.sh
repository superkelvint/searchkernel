#!/usr/bin/env bash
set -euo pipefail

# Publish the reusable Vespa runtime/configured toolchain bundle for the pinned
# commit. This artifact is independent of Search Kernel's language/compiler
# image and is rebuilt only when the Vespa bundle itself changes.
#
# Usage:
#   ./dev image vespa publish
#   bash tools/publish-searchkernel-ci-vespa.sh

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd "$ROOT"

source "$ROOT/tools/searchkernel-ci-image-common.sh"

IMAGE="${SEARCHKERNEL_CI_VESPA_IMAGE:-superkelvint/searchkernel-ci-vespa}"
TARGET="x86_64-unknown-linux-gnu"
BUILD_ENV="ci"

ci_image_require_commands docker git python3 tar
ci_image_require_docker
ci_image_require_clean_checkout "$ROOT"
ci_image_login

VESPA_COMMIT="$(ci_image_vespa_commit "$ROOT")"
VESPA_BUNDLE_ID="$(ci_image_vespa_bundle_id "$ROOT")"
IMAGE_REF="$IMAGE:$VESPA_COMMIT-$VESPA_BUNDLE_ID"

if ci_image_pull_if_available "$IMAGE_REF"; then
    ci_image_require_label "$IMAGE_REF" io.searchkernel.vespa-commit "$VESPA_COMMIT"
    ci_image_require_label "$IMAGE_REF" io.searchkernel.vespa-bundle-id "$VESPA_BUNDLE_ID"
    echo
    echo "Vespa image already published:"
    echo "  $IMAGE_REF"
    exit 0
fi

# Capture the actual local native inputs before sanitizing ambient build
# variables. The common AlmaLinux developer shell exports both of these.
FULL_SDK="${SEARCHKERNEL_CI_FULL_SDK:-${SK_NATIVE_SDK:-$ROOT/core/native/sdk/linux-x86_64}}"
LOCAL_VESPA_ROOT="${SEARCHKERNEL_CI_VESPA_ROOT:-${VESPA_ROOT:-$ROOT/upstream/vespa}}"

[[ -d "$FULL_SDK" ]] ||
    ci_image_die "full native SDK not found: $FULL_SDK"
[[ -d "$LOCAL_VESPA_ROOT" ]] ||
    ci_image_die "configured Vespa root not found: $LOCAL_VESPA_ROOT"

TMP="$(mktemp -d "${TMPDIR:-/tmp}/searchkernel-ci-vespa.XXXXXX")"
cleanup() {
    rm -rf "$TMP"
}
trap cleanup EXIT INT TERM

echo
echo "==> Preparing Vespa Docker contexts"
echo "Vespa commit:    $VESPA_COMMIT"
echo "Vespa bundle id: $VESPA_BUNDLE_ID"
echo "Full SDK:     $FULL_SDK"
echo "Vespa root:   $LOCAL_VESPA_ROOT"

mapfile -t native_contexts < <(
    env \
        SEARCHKERNEL_BUILD_ENV="$BUILD_ENV" \
        SK_NATIVE_TARGET="$TARGET" \
        bash "$ROOT/tools/prepare-searchkernel-ci-native-context.sh" \
            "$FULL_SDK" \
            "$LOCAL_VESPA_ROOT" \
            "$TMP/native-context" \
            "$BUILD_ENV" \
            "$TARGET"
)

NATIVE_BASE="${native_contexts[${#native_contexts[@]}-2]:-}"
NATIVE_TOOLCHAIN="${native_contexts[${#native_contexts[@]}-1]:-}"

[[ -d "$NATIVE_BASE" && -s "$NATIVE_BASE/lib/libvespalib.so" ]] ||
    ci_image_die "native context preparation did not produce a valid base"
[[ -d "$NATIVE_TOOLCHAIN" &&
   -f "$NATIVE_TOOLCHAIN/upstream/vespa/.searchkernel-vespa-artifact.json" ]] ||
    ci_image_die "native context preparation did not produce a valid toolchain"

REVISION="$(git rev-parse HEAD)"

echo
echo "==> Building reusable Vespa image"

vespa_build_args=(
    --build-context "searchkernel-native-base=$NATIVE_BASE"
    --build-context "searchkernel-native-toolchain=$NATIVE_TOOLCHAIN"
    --build-arg "VESPA_COMMIT=$VESPA_COMMIT"
    --build-arg "VESPA_BUNDLE_ID=$VESPA_BUNDLE_ID"
    --label "org.opencontainers.image.source=https://github.com/superkelvint/searchkernel"
    --label "org.opencontainers.image.revision=$REVISION"
    --label "io.searchkernel.vespa-commit=$VESPA_COMMIT"
    --label "io.searchkernel.vespa-bundle-id=$VESPA_BUNDLE_ID"
    --label "io.searchkernel.native-target=$TARGET"
    -f "$ROOT/docker/searchkernel-ci-vespa/Dockerfile"
    -t "$IMAGE_REF"
)
DOCKER_BUILDKIT=1 docker build "${vespa_build_args[@]}" "$ROOT/docker/searchkernel-ci-vespa"

ci_image_require_label "$IMAGE_REF" io.searchkernel.vespa-commit "$VESPA_COMMIT"
ci_image_require_label "$IMAGE_REF" io.searchkernel.vespa-bundle-id "$VESPA_BUNDLE_ID"

echo
echo "==> Pushing $IMAGE_REF"
docker push "$IMAGE_REF"

PINNED_IMAGE_REF="$(ci_image_pull_and_resolve_digest "$IMAGE_REF")"

echo
echo "Published:"
echo "  $IMAGE_REF"
echo "  $PINNED_IMAGE_REF"
