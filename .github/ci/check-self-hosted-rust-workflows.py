#!/usr/bin/env python3
"""Enforce zero shared mutable toolchain state on self-hosted CI workers."""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(".")
WORKFLOWS = ROOT / ".github" / "workflows"
IMAGE_RUNNER = WORKFLOWS / "ci-image-runner.yml"

FORBIDDEN_IN_WORKFLOWS = (
    (re.compile(r"ensure-runner-rust\.sh"), "host Rust repair helper"),
    (re.compile(r"Swatinem/rust-cache@"), "shared/persistent Cargo build cache"),
    (re.compile(r"dtolnay/rust-toolchain@"), "job-local host Rust installation"),
    (re.compile(r"actions/setup-node@"), "host Node toolcache mutation"),
    (re.compile(r"actions/setup-python@"), "host Python toolcache mutation"),
    (re.compile(r"ruby/setup-ruby@"), "host Ruby toolcache mutation"),
    (
        re.compile(r"\brustup\s+(?:toolchain|default|update|component|self)\b"),
        "runtime rustup mutation",
    ),
    (
        re.compile(r"\bsudo\s+(?:-n\s+)?(?:apt-get|dnf|yum)\s+(?:install|update)\b"),
        "runtime host package mutation",
    ),
)

HOST_BUILD_PATTERNS = (
    re.compile(r"(?m)^\s*(?:run:\s*)?(?:cargo|rustc|rustfmt)\b"),
    re.compile(r"(?m)^\s*(?:run:\s*)?npm\s+(?:ci|install|test|run)\b"),
    re.compile(r"(?m)^\s*(?:run:\s*)?(?:python\d*(?:\.\d+)?|\$CI_PYTHON).*\s+-m\s+pip\s+install\b"),
    re.compile(r"(?m)^\s*(?:run:\s*)?\./dev\s+(?:test|verify|lint)\b"),
)


def line_for(text: str, offset: int) -> int:
    return text.count("\n", 0, offset) + 1


def main() -> int:
    failures: list[str] = []

    if not IMAGE_RUNNER.is_file():
        failures.append(f"{IMAGE_RUNNER}: immutable CI image runner is missing")
    else:
        runner = IMAGE_RUNNER.read_text(encoding="utf-8")
        required = (
            "docker.io/superkelvint/searchkernel-ci:latest",
            'docker volume create "$SOURCE_VOLUME"',
            'docker volume create "$TARGET_VOLUME"',
            '-v "$SOURCE_VOLUME:/work"',
            '-v "$TARGET_VOLUME:/cargo-target"',
            "CARGO_TARGET_DIR=/cargo-target",
            "zero shared mutable worker state",
        )
        for needle in required:
            if needle not in runner:
                failures.append(f"{IMAGE_RUNNER}: missing isolation contract {needle!r}")

        forbidden_runner = (
            "searchkernel-ci-cargo-registry",
            "searchkernel-ci-cargo-git",
            "searchkernel-native-ci-lock",
            "/native-lock",
            ':/root/.cargo/registry',
            ':/root/.cargo/git',
            "flock 9",
        )
        for needle in forbidden_runner:
            if needle in runner:
                failures.append(
                    f"{IMAGE_RUNNER}: shared mutable worker state is forbidden: {needle}"
                )

    for path in sorted(WORKFLOWS.glob("*.y*ml")):
        text = path.read_text(encoding="utf-8")

        for pattern, description in FORBIDDEN_IN_WORKFLOWS:
            for match in pattern.finditer(text):
                failures.append(
                    f"{path}:{line_for(text, match.start())}: forbidden {description}"
                )

        if path == IMAGE_RUNNER:
            continue

        # Orchestration-only self-hosted jobs may still use immutable host tools
        # such as git/bash/python. Compilation, package installation, language
        # package managers, and product verification must execute in the image
        # runner so one worker can never mutate another worker's environment.
        if "self-hosted" in text and "ci-image-runner.yml" not in text:
            for pattern in HOST_BUILD_PATTERNS:
                match = pattern.search(text)
                if match:
                    failures.append(
                        f"{path}:{line_for(text, match.start())}: "
                        "self-hosted product execution must use ci-image-runner.yml"
                    )

    if failures:
        print("Self-hosted CI isolation policy violations:", file=sys.stderr)
        for failure in failures:
            print(f"  - {failure}", file=sys.stderr)
        print(
            "\nWorkers may share immutable Docker image layers only. "
            "Rust/Node/Ruby/Python package state, Cargo outputs, native caches, "
            "and job workspaces must remain job-private.",
            file=sys.stderr,
        )
        return 1

    print("Self-hosted zero-shared-state CI policy: PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
