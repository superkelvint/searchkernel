#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"

for script in \
    "$ROOT/dev" \
    "$ROOT/tools/searchkernel-ci-image-common.sh" \
    "$ROOT/tools/publish-searchkernel-ci-toolchain.sh" \
    "$ROOT/tools/publish-searchkernel-ci-vespa.sh" \
    "$ROOT/tools/publish-searchkernel-ci-image.sh"; do
    bash -n "$script"
done

python3 - \
    "$ROOT/dev" \
    "$ROOT/docker/searchkernel-ci-toolchain/Dockerfile" \
    "$ROOT/docker/searchkernel-ci-vespa/Dockerfile" \
    "$ROOT/docker/searchkernel-ci/base.Dockerfile" \
    "$ROOT/docker/searchkernel-ci/Dockerfile" \
    "$ROOT/tools/searchkernel-ci-image-common.sh" \
    "$ROOT/tools/publish-searchkernel-ci-toolchain.sh" \
    "$ROOT/tools/publish-searchkernel-ci-vespa.sh" \
    "$ROOT/tools/publish-searchkernel-ci-image.sh" <<'PY'
from pathlib import Path
import sys

(
    dev_path,
    toolchain_dockerfile_path,
    vespa_dockerfile_path,
    base_dockerfile_path,
    leaf_dockerfile_path,
    common_path,
    toolchain_publish_path,
    vespa_publish_path,
    leaf_publish_path,
) = map(Path, sys.argv[1:])

dev = dev_path.read_text(encoding="utf-8")
toolchain = toolchain_dockerfile_path.read_text(encoding="utf-8")
vespa = vespa_dockerfile_path.read_text(encoding="utf-8")
base = base_dockerfile_path.read_text(encoding="utf-8")
leaf = leaf_dockerfile_path.read_text(encoding="utf-8")
common = common_path.read_text(encoding="utf-8")
toolchain_publish = toolchain_publish_path.read_text(encoding="utf-8")
vespa_publish = vespa_publish_path.read_text(encoding="utf-8")
leaf_publish = leaf_publish_path.read_text(encoding="utf-8")

for fragment in (
    "./dev image publish",
    "./dev image toolchain publish",
    "./dev image vespa publish",
    "publish-searchkernel-ci-toolchain.sh",
    "publish-searchkernel-ci-vespa.sh",
):
    if fragment not in dev:
        raise SystemExit(f"dev image publishing surface is missing {fragment!r}")

for fragment in (
    "ARG BASE_IMAGE=",
    "ARG NODE_VERSION=",
    "ARG RUBY_VERSION=",
    "ARG RUST_TOOLCHAIN=",
    "ARG FLATBUFFERS_VERSION=",
    "llvm-tools-preview",
    "cargo-llvm-cov",
    "dnf install",
):
    if fragment not in toolchain:
        raise SystemExit(f"toolchain image is missing {fragment!r}")

for forbidden in (
    "ARG VESPA_COMMIT",
    "searchkernel-native-base",
    "searchkernel-native-toolchain",
    "prepare-searchkernel-ci-native-context",
):
    if forbidden in toolchain:
        raise SystemExit(f"toolchain image improperly depends on Vespa: {forbidden!r}")

for fragment in (
    "FROM scratch",
    "ARG VESPA_COMMIT",
    "ARG VESPA_BUNDLE_ID",
    "searchkernel-native-base",
    "searchkernel-native-toolchain",
    "io.searchkernel.vespa-commit",
    "io.searchkernel.vespa-bundle-id",
    "ci_image_require_clean_checkout",
):
    if fragment not in vespa:
        raise SystemExit(f"Vespa image is missing {fragment!r}")

for forbidden in ("dnf install", "RUST_TOOLCHAIN", "RUBY_VERSION", "NODE_VERSION"):
    if forbidden in vespa:
        raise SystemExit(f"Vespa image improperly owns language tooling: {forbidden!r}")

for fragment in (
    "ARG TOOLCHAIN_IMAGE",
    "ARG VESPA_IMAGE",
    "FROM ${VESPA_IMAGE} AS vespa",
    "FROM ${TOOLCHAIN_IMAGE}",
    "COPY --link --from=vespa",
    "io.searchkernel.toolchain-id",
    "io.searchkernel.vespa-commit",
    "io.searchkernel.vespa-bundle-id",
    "Composition-time compatibility gate",
    "adapter_static_link_closure",
    "cargo llvm-cov --version",
):
    if fragment not in base:
        raise SystemExit(f"composite CI base is missing {fragment!r}")

for fragment in (
    "ARG CI_BASE_IMAGE",
    "FROM ${CI_BASE_IMAGE} AS runtime",
    "ARG VESPA_BUNDLE_ID",
    "io.searchkernel.vespa-bundle-id",
    "COPY --from=searchkernel-source",
    "searchkernel-ci-publish-target",
    "sharing=locked",
    "/opt/searchkernel/native-validation-pass",
):
    if fragment not in leaf:
        raise SystemExit(f"thin CI leaf is missing {fragment!r}")

for forbidden in (
    "dnf install",
    "searchkernel-native-base",
    "searchkernel-native-toolchain",
    "vespaengine/vespa-dev-almalinux",
):
    if forbidden in leaf:
        raise SystemExit(f"thin CI leaf regressed into rebuilding a base input: {forbidden!r}")

if "searchkernel-ci-toolchain-v2" not in common or "rglob" not in common:
    raise SystemExit("toolchain identity must hash the complete recipe and immutable base digest")
if "ci_image_resolve_local_digest" not in common or "ci_image_pull_and_resolve_digest" not in common:
    raise SystemExit("split-image publishing has no canonical tag-to-digest resolver")
if "ci_image_require_clean_checkout" not in common:
    raise SystemExit("publishers do not share a clean-checkout gate")
if "ci_image_require_remote_main" not in common:
    raise SystemExit("canonical :latest publication has no remote-main gate")
if "searchkernel-ci-vespa-bundle-v1" not in common:
    raise SystemExit("Vespa bundle identity must include the packaging/closure recipe")
if 'root / "tools/prepare-searchkernel-ci-native-context.sh"' not in common:
    raise SystemExit("Vespa bundle identity does not include native-context packaging")

for fragment in (
    "superkelvint/searchkernel-ci-toolchain",
    "io.searchkernel.toolchain-id",
    "BASE_IMAGE=",
    "PINNED_BASE_IMAGE",
    "io.searchkernel.toolchain-base",
    "ci_image_require_clean_checkout",
):
    if fragment not in toolchain_publish:
        raise SystemExit(f"toolchain publisher is missing {fragment!r}")

if "prepare-searchkernel-ci-native-context.sh" in toolchain_publish:
    raise SystemExit("toolchain publishing must not package or rebuild Vespa")

for fragment in (
    "superkelvint/searchkernel-ci-vespa",
    "prepare-searchkernel-ci-native-context.sh",
    "ci_image_vespa_bundle_id",
    'IMAGE_REF="$IMAGE:$VESPA_COMMIT-$VESPA_BUNDLE_ID"',
    "io.searchkernel.vespa-commit",
    "io.searchkernel.vespa-bundle-id",
):
    if fragment not in vespa_publish:
        raise SystemExit(f"Vespa publisher is missing {fragment!r}")

for forbidden in ("searchkernel-ci-toolchain", "TOOLCHAIN_ID"):
    if forbidden in vespa_publish:
        raise SystemExit(f"Vespa publishing must be independent of CI language tooling: {forbidden!r}")

for fragment in (
    "superkelvint/searchkernel-ci-toolchain",
    "superkelvint/searchkernel-ci-vespa",
    'VESPA_REF="$VESPA_IMAGE:$VESPA_COMMIT-$VESPA_BUNDLE_ID"',
    'COMPOSITE_REF="$IMAGE:base-$COMPOSITE_ID"',
    "ci_image_vespa_bundle_id",
    "ci_image_composite_id",
    "PINNED_TOOLCHAIN_REF",
    "PINNED_VESPA_REF",
    "PINNED_COMPOSITE_REF",
    "io.searchkernel.composite-id",
    "io.searchkernel.toolchain-image-digest",
    "io.searchkernel.vespa-image-digest",
    "io.searchkernel.composite-image-digest",
    "BUILDKIT_CACHE_MOUNT_NS",
    "ci_image_require_clean_checkout",
    "ci_image_require_remote_main",
    "tags beginning with 'base-' are reserved",
    "io.searchkernel.vespa-bundle-id",
    "./dev image toolchain publish",
    "./dev image vespa publish",
    "Reusing published composite base.",
    "CI_BASE_IMAGE=$COMPOSITE_REF",
):
    if fragment not in leaf_publish:
        raise SystemExit(f"final image publisher is missing split-image behavior {fragment!r}")

for forbidden in (
    "prepare-searchkernel-ci-native-context.sh",
    "SEARCHKERNEL_CI_FULL_SDK",
    "SEARCHKERNEL_CI_VESPA_ROOT",
):
    if forbidden in leaf_publish:
        raise SystemExit(
            f"ordinary final-image publishing must not rebuild/repackage Vespa: {forbidden!r}"
        )

print("split CI image publishing contract: PASS")
PY

python3 "$ROOT/tools/test_ci_image_identity.py"
