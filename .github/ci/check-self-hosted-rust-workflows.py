#!/usr/bin/env python3
"""Enforce job-private CI state with one narrowly bounded compiler-cache exception."""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(".")
WORKFLOWS = ROOT / ".github" / "workflows"
IMAGE_RUNNER = WORKFLOWS / "ci-image-runner.yml"
SCCACHE_HELPER = ROOT / "tools" / "ensure-ci-sccache.sh"

FORBIDDEN_IN_WORKFLOWS = (
    (re.compile(r"ensure-runner-rust\.sh"), "host Rust repair helper"),
    (re.compile(r"Swatinem/rust-cache@"), "shared/persistent Cargo build cache"),
    (re.compile(r"actions/cache@"), "unreviewed shared workflow cache"),
    (
        re.compile(r"\bmoon\s+(?:run|ci|check|task)\b"),
        "task-result execution via Moon without proven exact-head provenance closure",
    ),
    (
        re.compile(r"\bcargo\s+nextest\s+archive\b|\bnextest\s+archive\b|--archive-file(?:=|\s)|--archive-format(?:=|\s)"),
        "archive reuse via nextest without proven exact-head provenance closure",
    ),
    (re.compile(r"(?<!s)\bccache\b"), "unreviewed compiler cache"),
    (re.compile(r"\bcachepot\b"), "unreviewed compiler cache"),
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

        # The sole permitted mutable cross-job build cache is a version-pinned
        # sccache compiler-output service. It is optional during rollout: the
        # fully job-private model remains valid. If a candidate opts into
        # sccache, fail closed unless it uses the reviewed helper contract and
        # keeps the actual compiler inside the exact-head job container.
        uses_sccache = "sccache" in runner.lower()
        if uses_sccache:
            required_sccache_runner = (
                "ensure-ci-sccache.sh",
                '-v "$SCCACHE_BINARY_VOLUME:/opt/searchkernel/sccache:ro"',
                '-v "$SCCACHE_SOCKET_VOLUME:/sccache:ro"',
                "RUSTC_WRAPPER=/opt/searchkernel/sccache/sccache",
                "SCCACHE_SERVER_UDS=/sccache/sccache.sock",
                "SCCACHE_CLIENT_SIDE=1",
                "SCCACHE_IGNORE_SERVER_IO_ERROR=1",
            )
            for needle in required_sccache_runner:
                if needle not in runner:
                    failures.append(
                        f"{IMAGE_RUNNER}: incomplete bounded sccache contract: {needle!r}"
                    )

            for needle in ("SCCACHE_DIR=", "SCCACHE_CACHE_SIZE="):
                if needle in runner:
                    failures.append(
                        f"{IMAGE_RUNNER}: sccache cache storage must not be mounted/configured in job containers: {needle}"
                    )

            if not SCCACHE_HELPER.is_file():
                failures.append(
                    f"{SCCACHE_HELPER}: bounded sccache helper is required by the image runner"
                )
            else:
                helper = SCCACHE_HELPER.read_text(encoding="utf-8")
                required_helper = (
                    "SCCACHE_VERSION=0.17.0",
                    "SCCACHE_SHA256=67c4a96dd237c1f518f6b36083f270f9976d516f1e57fce891755ea782e50006",
                    'TOOLCHAIN_ID=$(docker image inspect',
                    'io.searchkernel.toolchain-id',
                    'CACHE_NAMESPACE=$TOOLCHAIN_ID',
                    'BINARY_VOLUME="searchkernel-ci-sccache-bin-v$SAFE_VERSION"',
                    'CACHE_VOLUME="searchkernel-ci-sccache-cache-v$SAFE_VERSION-$CACHE_NAMESPACE"',
                    'SOCKET_VOLUME="searchkernel-ci-sccache-socket-v$SAFE_VERSION-$CACHE_NAMESPACE"',
                    "SCCACHE_CACHE_SIZE=20G",
                    "SCCACHE_SERVER_UDS=/sccache/sccache.sock",
                    "SCCACHE_START_SERVER=1",
                    "SCCACHE_NO_DAEMON=1",
                    "flock 9",
                )
                for needle in required_helper:
                    if needle not in helper:
                        failures.append(
                            f"{SCCACHE_HELPER}: missing bounded sccache contract {needle!r}"
                        )
                for needle in ("/cargo-target", ':/work', ':/root/.cargo/registry', ':/root/.cargo/git'):
                    if needle in helper:
                        failures.append(
                            f"{SCCACHE_HELPER}: compiler-cache helper must not share job build/package state: {needle}"
                        )
                if "--start-server" in helper:
                    failures.append(
                        f"{SCCACHE_HELPER}: service container must run the sccache server directly in foreground mode"
                    )

    # Task-result caching through Moon is intentionally not an evidence authority yet.
    # Fail closed if task configuration is introduced before a reviewed
    # provenance closure can bind cache results to the exact candidate and
    # every semantic external input. sccache remains a compiler-output-only
    # exception and can never stand in for executed tests.
    moon_root = ROOT / ".moon"
    if moon_root.exists():
        for path in sorted(moon_root.rglob("*.y*ml")):
            failures.append(
                f"{path}: task-result configuration through Moon is forbidden for acceptance evidence "
                "until exact-head provenance closure is implemented"
            )

    # Workflows may delegate acceptance commands to repository scripts. Scan
    # executable/config text under the repository as well, otherwise a workflow
    # can hide forbidden result reuse behind `tools/foo.sh`.
    delegated_roots = [ROOT / "tools", ROOT / ".github" / "ci", ROOT / "verification"]
    delegated_suffixes = {".sh", ".py", ".toml", ".yml", ".yaml"}
    for delegated_root in delegated_roots:
        if not delegated_root.exists():
            continue
        for path in sorted(p for p in delegated_root.rglob("*") if p.is_file() and p.suffix in delegated_suffixes):
            candidate_checker = (
                ROOT / ".github" / "ci" / "check-self-hosted-rust-workflows.py"
            )
            if path.resolve() == candidate_checker.resolve() or path.name.startswith("test_"):
                continue
            text = path.read_text(encoding="utf-8", errors="replace")
            for pattern, description in FORBIDDEN_IN_WORKFLOWS[3:5]:
                for match in pattern.finditer(text):
                    failures.append(f"{path}:{line_for(text, match.start())}: forbidden delegated {description}")

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
            "\nWorkers may share immutable Docker image layers and only the "
            "reviewed version-pinned sccache compiler-output cache. Rust/Node/"
            "Ruby/Python package state, Cargo target directories, native caches, "
            "source snapshots, and job workspaces must remain job-private.",
            file=sys.stderr,
        )
        return 1

    print("Self-hosted isolated-state CI policy: PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
