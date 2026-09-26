#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"

for script in \
    "$ROOT/dev" \
    "$ROOT/tools/searchkernel-ci-image-common.sh" \
    "$ROOT/tools/publish-searchkernel-ci-toolchain.sh" \
    "$ROOT/tools/publish-searchkernel-ci-vespa.sh" \
    "$ROOT/tools/publish-searchkernel-ci-image.sh" \
    "$ROOT/tools/ensure-ci-vespa-volume.sh" \
    "$ROOT/tools/ensure-ci-sccache.sh" \
    "$ROOT/tools/run-in-ci-image.sh" \
    "$ROOT/tools/generate-typescript-grpc-web.sh"; do
    bash -n "$script"
done

python3 - \
    "$ROOT/docker/searchkernel-ci-toolchain/Dockerfile" \
    "$ROOT/docker/searchkernel-ci-vespa/Dockerfile" \
    "$ROOT/docker/searchkernel-ci/Dockerfile" \
    "$ROOT/tools/searchkernel-ci-image-common.sh" \
    "$ROOT/tools/publish-searchkernel-ci-toolchain.sh" \
    "$ROOT/tools/publish-searchkernel-ci-vespa.sh" \
    "$ROOT/tools/publish-searchkernel-ci-image.sh" \
    "$ROOT/tools/ensure-ci-vespa-volume.sh" \
    "$ROOT/tools/ensure-ci-sccache.sh" \
    "$ROOT/tools/run-in-ci-image.sh" \
    "$ROOT/tools/generate-typescript-grpc-web.sh" \
    "$ROOT/.github/workflows/ci-image-runner.yml" <<'PY'
from pathlib import Path
import sys

(
    toolchain_path,
    vespa_path,
    leaf_path,
    common_path,
    toolchain_publish_path,
    vespa_publish_path,
    leaf_publish_path,
    ensure_volume_path,
    ensure_sccache_path,
    local_runner_path,
    typescript_generator_path,
    workflow_path,
) = map(Path, sys.argv[1:])

toolchain = toolchain_path.read_text(encoding="utf-8")
vespa = vespa_path.read_text(encoding="utf-8")
leaf = leaf_path.read_text(encoding="utf-8")
common = common_path.read_text(encoding="utf-8")
toolchain_publish = toolchain_publish_path.read_text(encoding="utf-8")
vespa_publish = vespa_publish_path.read_text(encoding="utf-8")
leaf_publish = leaf_publish_path.read_text(encoding="utf-8")
ensure_volume = ensure_volume_path.read_text(encoding="utf-8")
ensure_sccache = ensure_sccache_path.read_text(encoding="utf-8")
local_runner = local_runner_path.read_text(encoding="utf-8")
typescript_generator = typescript_generator_path.read_text(encoding="utf-8")
workflow = workflow_path.read_text(encoding="utf-8")

for fragment in (
    "ARG BASE_IMAGE=",
    "ARG RUST_TOOLCHAIN=",
    "ARG FLATBUFFERS_VERSION=",
    "ARG SCCACHE_VERSION=0.17.0",
    "ARG SCCACHE_SHA256=67c4a96dd237c1f518f6b36083f270f9976d516f1e57fce891755ea782e50006",
    "sccache-v${SCCACHE_VERSION}-x86_64-unknown-linux-musl.tar.gz",
    "command -v sccache",
    "cargo-llvm-cov",
    "SEARCHKERNEL_TYPESCRIPT_NODE_MODULES",
    "COPY clients/typescript/package.json clients/typescript/package-lock.json",
    "npm ci --prefix /opt/searchkernel/typescript",
):
    if fragment not in toolchain:
        raise SystemExit(f"toolchain image is missing {fragment!r}")

for forbidden in ("ARG VESPA_COMMIT", "searchkernel-native-base", "searchkernel-native-toolchain"):
    if forbidden in toolchain:
        raise SystemExit(f"toolchain image improperly depends on Vespa: {forbidden!r}")

for fragment in (
    "FROM scratch",
    "ARG VESPA_COMMIT",
    "ARG VESPA_BUNDLE_ID",
    "searchkernel-native-base",
    "searchkernel-native-toolchain",
):
    if fragment not in vespa:
        raise SystemExit(f"Vespa image is missing {fragment!r}")

for fragment in (
    "ARG TOOLCHAIN_IMAGE",
    "ARG VESPA_IMAGE",
    "FROM ${VESPA_IMAGE} AS vespa",
    "FROM ${TOOLCHAIN_IMAGE} AS runtime",
    "RUN --mount=from=vespa",
    "FROM runtime AS final",
    "io.searchkernel.environment-id",
    "io.searchkernel.vespa-bundle-id",
    "/opt/searchkernel/native-validation-pass",
):
    if fragment not in leaf:
        raise SystemExit(f"thin CI leaf is missing {fragment!r}")

for forbidden in (
    "COPY --from=vespa",
    "COPY --link --from=vespa",
    "CI_BASE_IMAGE",
    "FROM ${TOOLCHAIN_IMAGE}\n\nCOPY",
):
    if forbidden in leaf:
        raise SystemExit(f"CI leaf republishes Vespa payload: {forbidden!r}")

for fragment in (
    "searchkernel-ci-toolchain-v3",
    "searchkernel-ci-vespa-bundle-v1",
    "searchkernel-ci-vespa-producer-v1",
    "searchkernel-ci-environment-v1",
    "ci_image_environment_id",
    "ci_image_pull_and_resolve_digest",
):
    if fragment not in common:
        raise SystemExit(f"shared image identity helpers are missing {fragment!r}")

for fragment in (
    "superkelvint/searchkernel-ci-toolchain",
    "PINNED_BASE_IMAGE",
    "io.searchkernel.toolchain-base",
    'ci_image_toolchain_id "$ROOT" "$PINNED_BASE_IMAGE"',
    "clients/typescript/package-lock.json",
    'docker build "${toolchain_build_args[@]}" "$BUILD_CONTEXT"',
):
    if fragment not in toolchain_publish:
        raise SystemExit(f"toolchain publisher is missing {fragment!r}")

for fragment in (
    "SEARCHKERNEL_TYPESCRIPT_NODE_MODULES",
    "/opt/searchkernel/typescript/node_modules",
    'ln -s "$toolchain_node_modules" "$client/node_modules"',
    "SEARCHKERNEL_IN_CI_IMAGE",
    'ci --prefix "$client" --no-audit --no-fund',
):
    if fragment not in typescript_generator:
        raise SystemExit(
            f"TypeScript generator does not consume the image-owned dependency store: missing {fragment!r}"
        )

for fragment in (
    "superkelvint/searchkernel-ci-vespa",
    "VESPA_PRODUCER_ID",
    "io.searchkernel.vespa-producer-id",
):
    if fragment not in vespa_publish:
        raise SystemExit(f"Vespa publisher is missing {fragment!r}")

for fragment in (
    "PINNED_TOOLCHAIN_REF",
    "PINNED_VESPA_REF",
    "CI_ENVIRONMENT_ID",
    "ci_image_environment_id",
    'TOOLCHAIN_IMAGE=$PINNED_TOOLCHAIN_REF',
    'VESPA_IMAGE=$PINNED_VESPA_REF',
    "io.searchkernel.vespa-image-digest",
    "Vespa remains external and is NOT republished",
    'ci_image_toolchain_id "$ROOT" "$PINNED_TOOLCHAIN_BASE"',
):
    if fragment not in leaf_publish:
        raise SystemExit(f"leaf publisher is missing external-Vespa behavior {fragment!r}")

for forbidden in (
    "base.Dockerfile",
    "COMPOSITE_REF",
    "COMPOSITE_ID",
    "ci_image_composite_id",
    "COPY --link",
):
    if forbidden in leaf_publish:
        raise SystemExit(f"leaf publisher still has composite-image behavior: {forbidden!r}")

for fragment in (
    'VOLUME="searchkernel-ci-vespa-',
    "docker export",
    "/opt/searchkernel/native-sdk-cache",
    ".searchkernel-vespa-image-digest",
    "flock 9",
):
    if fragment not in ensure_volume:
        raise SystemExit(f"Vespa volume materializer is missing {fragment!r}")

for fragment in (
    'SERVER="searchkernel-ci-sccache-v$SAFE_VERSION"',
    'BINARY_VOLUME="searchkernel-ci-sccache-bin-v$SAFE_VERSION"',
    'CACHE_VOLUME="searchkernel-ci-sccache-cache-v$SAFE_VERSION"',
    'SOCKET_VOLUME="searchkernel-ci-sccache-socket-v$SAFE_VERSION"',
    "SCCACHE_VERSION=0.17.0",
    "SCCACHE_SHA256=67c4a96dd237c1f518f6b36083f270f9976d516f1e57fce891755ea782e50006",
    "SCCACHE_NO_DAEMON=1",
    "SCCACHE_CACHE_SIZE=20G",
    "flock 9",
    "sccache --show-stats",
):
    if fragment not in ensure_sccache:
        raise SystemExit(f"sccache server helper is missing {fragment!r}")

for surface, text in (("local runner", local_runner), ("CI workflow", workflow)):
    for fragment in (
        "ensure-ci-vespa-volume.sh",
        "io.searchkernel.vespa-image-digest",
        "/opt/searchkernel/native-sdk-cache:ro",
        "ensure-ci-sccache.sh",
        "RUSTC_WRAPPER=/opt/searchkernel/sccache/sccache",
        "/opt/searchkernel/sccache:ro",
        "SCCACHE_SERVER_UDS=/sccache/sccache.sock",
        "SCCACHE_CLIENT_SIDE=1",
    ):
        if fragment not in text:
            raise SystemExit(f"{surface} does not externalize Vespa correctly: missing {fragment!r}")

print("external Vespa CI image publishing contract: PASS")
PY

python3 "$ROOT/tools/test_ci_image_identity.py"
