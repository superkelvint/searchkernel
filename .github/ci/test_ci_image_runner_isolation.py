#!/usr/bin/env python3
from __future__ import annotations

import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
RUNNER = ROOT / ".github" / "workflows" / "ci-image-runner.yml"
PERSISTENCE = ROOT / ".github" / "workflows" / "persistence-compatibility.yml"


class CiImageRunnerIsolationTests(unittest.TestCase):
    def test_host_checkout_is_job_private_from_first_git_operation(self) -> None:
        source = RUNNER.read_text(encoding="utf-8")

        allocation = source.split("- name: Allocate job-private checkout path", 1)[1].split(
            "- name: Check out requested revision", 1
        )[0]
        self.assertIn("id: checkout", allocation)
        self.assertIn("GITHUB_RUN_ID", allocation)
        self.assertIn("GITHUB_RUN_ATTEMPT", allocation)
        self.assertIn("CI_TARGET", allocation)
        self.assertIn('^ [A-Za-z0-9_.-]'.replace(" ", ""), allocation)
        self.assertIn('echo "relative=$relative" >> "$GITHUB_OUTPUT"', allocation)
        self.assertIn('echo "absolute=$absolute" >> "$GITHUB_OUTPUT"', allocation)

        checkout = source.split("- name: Check out requested revision", 1)[1].split(
            "- name: Resolve exact revision", 1
        )[0]
        self.assertIn("uses: actions/checkout@v4", checkout)
        self.assertIn("path: ${{ steps.checkout.outputs.relative }}", checkout)

        for step_name in (
            "Resolve exact revision",
            "Pull immutable CI image",
            "Create job-private Docker state",
            "Execute lane with isolated source and target state",
        ):
            section = source.split(f"- name: {step_name}", 1)[1].split("\n      - name:", 1)[0]
            self.assertIn(
                "working-directory: ${{ steps.checkout.outputs.absolute }}",
                section,
                msg=f"{step_name} must run from the job-private checkout",
            )

        cleanup = source.split("- name: Remove job-private checkout", 1)[1]
        self.assertIn("if: always() && steps.checkout.outputs.absolute != ''", cleanup)
        self.assertIn("CHECKOUT_DIR: ${{ steps.checkout.outputs.absolute }}", cleanup)
        self.assertIn('rm -rf -- "$CHECKOUT_DIR"', cleanup)

        self.assertNotIn(".git/refs", source)
        self.assertNotIn("*.lock", source)

    def test_shared_sccache_does_not_share_source_or_target_state(self) -> None:
        source = RUNNER.read_text(encoding="utf-8")

        self.assertIn("ensure-ci-sccache.sh", source)
        self.assertIn('RUSTC_WRAPPER=/opt/searchkernel/sccache/sccache', source)
        self.assertIn('-v "$SCCACHE_BINARY_VOLUME:/opt/searchkernel/sccache:ro"', source)
        self.assertIn('SCCACHE_SERVER_UDS=/sccache/sccache.sock', source)
        self.assertIn('SCCACHE_CLIENT_SIDE=1', source)
        self.assertIn('-v "$SCCACHE_SOCKET_VOLUME:/sccache:ro"', source)

        self.assertIn('TARGET_VOLUME="searchkernel-ci-target-', source)
        self.assertIn('-v "$TARGET_VOLUME:/cargo-target"', source)
        self.assertIn('docker volume rm -f "$SOURCE_VOLUME" "$TARGET_VOLUME"', source)
        self.assertNotIn('searchkernel-ci-target-cache', source)
        self.assertNotIn(':/root/.cargo/registry', source)
        self.assertNotIn(':/root/.cargo/git', source)


    def test_native_full_bootstraps_split_sdk_and_runs_product_integration(self) -> None:
        source = RUNNER.read_text(encoding="utf-8")
        helper = source.split("prepare_native_full_verifier_sdk() {", 1)[1].split("\n          }", 1)[0]
        lane = source.split("            native-full)", 1)[1].split("            client-e2e)", 1)[0]

        self.assertIn("tools/resolve-native-sdk.sh vespa-root", helper)
        self.assertIn("SEARCHKERNEL_VERIFIER_BASE_SDK=\"$native_base\"", helper)
        self.assertIn("prepare_verifier_sdk", helper)
        self.assertNotIn("core/native/sdk/linux-x86_64", helper)
        self.assertIn("prepare_native_full_verifier_sdk", lane)
        self.assertIn("./dev verify product.integration --baseline-already-proved", lane)

        common = (ROOT / "reference/vespa/common/verify-common.sh").read_text(encoding="utf-8")
        self.assertNotIn("core/native/sdk/linux-x86_64", common)
        self.assertIn('tools/resolve-native-sdk.sh" path', common)
        self.assertIn("SK_NATIVE_ADAPTER_ONLY=1", common)

        integration = (ROOT / "reference/vespa/i/verify-i.sh").read_text(encoding="utf-8")
        self.assertIn('verify-through.sh" m13', integration)
        self.assertIn('verify-through.sh" phase10', integration)

    def test_persistence_checkout_is_job_private(self) -> None:
        source = PERSISTENCE.read_text(encoding="utf-8")

        allocation = source.split("- name: Allocate job-private checkout path", 1)[1].split(
            "- name: Check out exact revision", 1
        )[0]
        self.assertIn("GITHUB_RUN_ID", allocation)
        self.assertIn("GITHUB_RUN_ATTEMPT", allocation)
        self.assertIn('echo "relative=$relative" >> "$GITHUB_OUTPUT"', allocation)
        self.assertIn('echo "absolute=$absolute" >> "$GITHUB_OUTPUT"', allocation)

        checkout = source.split("- name: Check out exact revision", 1)[1].split(
            "- name: Select runner Python", 1
        )[0]
        self.assertIn("path: ${{ steps.checkout.outputs.relative }}", checkout)
        self.assertIn("persist-credentials: false", checkout)

        for step_name in (
            "Select runner Python",
            "Verify exact pull-request head",
            "Verify frozen fixture structure and provenance",
            "Enforce append-only historical fixtures",
            "Verify release persistence contract",
        ):
            section = source.split(f"- name: {step_name}", 1)[1].split("\n      - name:", 1)[0]
            self.assertIn(
                "working-directory: ${{ steps.checkout.outputs.absolute }}",
                section,
                msg=f"{step_name} must run from the persistence job-private checkout",
            )

        cleanup = source.split("- name: Remove job-private checkout", 1)[1]
        self.assertIn("if: always() && steps.checkout.outputs.absolute != ''", cleanup)
        self.assertIn("CHECKOUT_DIR: ${{ steps.checkout.outputs.absolute }}", cleanup)
        self.assertIn('rm -rf -- "$CHECKOUT_DIR"', cleanup)
        self.assertNotIn(".git/refs", source)
        self.assertNotIn("*.lock", source)


if __name__ == "__main__":
    unittest.main()
