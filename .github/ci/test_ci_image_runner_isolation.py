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
            "Execute lane with zero shared mutable worker state",
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
            "Native verification policy",
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
