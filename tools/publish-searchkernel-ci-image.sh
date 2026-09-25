#!/usr/bin/env bash
set -euo pipefail

# Build and publish the immutable Search Kernel CI image to Docker Hub.
#
# The image includes:
#   - pinned toolchains and language runtimes;
#   - the validated Vespa/runtime base;
#   - the configured Vespa headers/static link metadata needed to rebuild the
#     Search Kernel adapter in each checkout.
#
# This path deliberately does NOT use the published native-base GitHub Release
# flow. It prepares Docker build contexts directly from the existing full SDK
# and matching configured Vespa toolchain.
#
# Preferred usage:
#   ./dev image publish [tag]
#
# Low-level usage:
#   tools/publish-searchkernel-ci-image.sh [tag]
#
# Authentication:
#   docker login
# or:
#   DOCKERHUB_TOKEN=... DOCKERHUB_USERNAME=superkelvint \
#     tools/publish-searchkernel-ci-image.sh [tag]

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd "$ROOT"

IMAGE="${SEARCHKERNEL_CI_IMAGE:-superkelvint/searchkernel-ci}"
BASE_IMAGE="${SEARCHKERNEL_CI_BASE_IMAGE:-docker.io/vespaengine/vespa-dev-almalinux-8:latest}"
PUSH_LATEST="${SEARCHKERNEL_CI_PUSH_LATEST:-0}"
TARGET="x86_64-unknown-linux-gnu"
BUILD_ENV="ci"

die() {
    echo "ERROR: $*" >&2
    exit 1
}

for cmd in docker git python3 tar; do
    command -v "$cmd" >/dev/null 2>&1 || die "missing required command: $cmd"
done

docker info >/dev/null 2>&1 || die "Docker is not usable by the current user"

docker build --help 2>/dev/null | grep -q -- '--build-context' ||
    die "Docker BuildKit with --build-context support is required"

TAG="${1:-}"
if [[ -z "$TAG" ]]; then
    TAG="$(git rev-parse --short=12 HEAD)"
fi
[[ "$TAG" =~ ^[A-Za-z0-9_][A-Za-z0-9_.-]{0,127}$ ]] ||
    die "invalid Docker tag: $TAG"

[[ "$PUSH_LATEST" == 0 || "$PUSH_LATEST" == 1 ]] ||
    die "SEARCHKERNEL_CI_PUSH_LATEST must be 0 or 1"

VESPA_COMMIT="$(
    python3 - "$ROOT/core/native/vespa/manifest.json" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as stream:
    print(json.load(stream)["vespa_commit"])
PY
)"
[[ "$VESPA_COMMIT" =~ ^[0-9a-f]{40}$ ]] ||
    die "invalid pinned Vespa commit: $VESPA_COMMIT"

# Capture the actual local native inputs before sanitizing ambient build
# variables. The common AlmaLinux developer shell exports both of these.
FULL_SDK="${SEARCHKERNEL_CI_FULL_SDK:-${SK_NATIVE_SDK:-$ROOT/core/native/sdk/linux-x86_64}}"
LOCAL_VESPA_ROOT="${SEARCHKERNEL_CI_VESPA_ROOT:-${VESPA_ROOT:-$ROOT/upstream/vespa}}"

[[ -d "$FULL_SDK" ]] ||
    die "full native SDK not found: $FULL_SDK"
[[ -d "$LOCAL_VESPA_ROOT" ]] ||
    die "configured Vespa root not found: $LOCAL_VESPA_ROOT"

if [[ -n "${DOCKERHUB_TOKEN:-}" ]]; then
    DOCKERHUB_USERNAME="${DOCKERHUB_USERNAME:-superkelvint}"
    printf '%s' "$DOCKERHUB_TOKEN" |
        docker login --username "$DOCKERHUB_USERNAME" --password-stdin
else
    echo "No DOCKERHUB_TOKEN supplied; using existing Docker CLI credentials."
    echo "If push authentication fails, run: docker login"
fi

TMP="$(mktemp -d "${TMPDIR:-/tmp}/searchkernel-ci-image.XXXXXX")"
cleanup() {
    rm -rf "$TMP"
}
trap cleanup EXIT INT TERM

echo
echo "==> Preparing native Docker contexts from the full SDK"
echo "Full SDK:   $FULL_SDK"
echo "Vespa root: $LOCAL_VESPA_ROOT"

mapfile -t native_contexts < <(
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
    die "native context preparation did not produce a valid base"
[[ -d "$NATIVE_TOOLCHAIN" &&
   -f "$NATIVE_TOOLCHAIN/upstream/vespa/.searchkernel-vespa-artifact.json" ]] ||
    die "native context preparation did not produce a valid toolchain"

echo
echo "==> Preparing clean Search Kernel source context"

SOURCE_CONTEXT="$TMP/source"
mkdir -p "$SOURCE_CONTEXT"

# Use exactly HEAD: do not accidentally publish an image whose validation was
# performed against uncommitted working-tree state.
git archive --format=tar HEAD | tar -xf - -C "$SOURCE_CONTEXT"

[[ -f "$SOURCE_CONTEXT/Cargo.toml" ]] ||
    die "source context is missing Cargo.toml"

echo
echo "==> Pinning Docker base image"

docker pull "$BASE_IMAGE" >/dev/null
PINNED_BASE_IMAGE="$(
    docker image inspect \
        --format '{{if .RepoDigests}}{{index .RepoDigests 0}}{{end}}' \
        "$BASE_IMAGE"
)"
[[ -n "$PINNED_BASE_IMAGE" ]] ||
    die "could not resolve an immutable digest for $BASE_IMAGE"

IMAGE_REF="$IMAGE:$TAG"
REVISION="$(git rev-parse HEAD)"

echo
echo "==> Building $IMAGE_REF"
echo "Base image:   $PINNED_BASE_IMAGE"
echo "Revision:     $REVISION"
echo "Vespa commit: $VESPA_COMMIT"

DOCKER_BUILDKIT=1 docker build \
    --build-context "searchkernel-native-base=$NATIVE_BASE" \
    --build-context "searchkernel-native-toolchain=$NATIVE_TOOLCHAIN" \
    --build-context "searchkernel-source=$SOURCE_CONTEXT" \
    --build-arg "BASE_IMAGE=$PINNED_BASE_IMAGE" \
    --build-arg "VESPA_COMMIT=$VESPA_COMMIT" \
    --label "org.opencontainers.image.source=https://github.com/superkelvint/searchkernel" \
    --label "org.opencontainers.image.revision=$REVISION" \
    --label "io.searchkernel.vespa-commit=$VESPA_COMMIT" \
    -f "$ROOT/docker/searchkernel-ci/Dockerfile" \
    -t "$IMAGE_REF" \
    "$ROOT/docker/searchkernel-ci"

echo
echo "==> Verifying final image capabilities"

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
node --version
ruby --version
php --version | head -n 1
"$python" --version
flatc --version
rg --version | head -n 1
gh --version | head -n 1
'

echo
echo "==> Pushing $IMAGE_REF"

if ! docker push "$IMAGE_REF"; then
    cat >&2 <<EOF

Docker push failed.

If this is an authentication error, authenticate once with:

  docker login

Or provide a Docker Hub personal access token:

  DOCKERHUB_USERNAME=superkelvint \
  DOCKERHUB_TOKEN=... \
  ./dev image publish $TAG

EOF
    exit 1
fi

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
