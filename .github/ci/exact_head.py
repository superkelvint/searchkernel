#!/usr/bin/env python3
"""Exact-head and change-aware behavioral verification helpers for CI."""

from __future__ import annotations

import argparse
import fnmatch
import subprocess
import sys
from pathlib import Path
from typing import Iterable

LANES = (
    "rust",
    "python",
    "typescript",
    "php",
    "native",
    "protocol",
    "api_conformance",
    "verification",
)

LANE_PATTERNS: dict[str, tuple[str, ...]] = {
    "rust": (
        "Cargo.toml",
        "Cargo.lock",
        ".cargo/**",
        "rust-toolchain*",
        "api/**",
        "core/**",
        "searchkerneld/**",
        "dev",
    ),
    "python": (
        "api/**",
        "clients/python/**",
        "reference/client-contracts/**",
        "tools/check-client-contracts.py",
        "tools/test_check_client_contracts.py",
        "tools/check-python-*.py",
        "tools/test_python_*.py",
        "tools/generate-python-*.sh",
    ),
    "typescript": (
        "api/**",
        "clients/typescript/**",
        "reference/client-contracts/**",
        "tools/check-client-contracts.py",
        "tools/test_check_client_contracts.py",
        "tools/generate-typescript-*.sh",
    ),
    "php": (
        "clients/php/**",
        "reference/client-contracts/**",
        "tools/check-client-contracts.py",
        "tools/test_check_client_contracts.py",
    ),
    "native": (
        ".githooks/pre-commit",
        "core/native/**",
        "core/crates/searchkernel-vespa-bindings/**",
        "core/crates/searchkernel-vespa-engine/**",
        "core/crates/searchkernel-ir-adapter/**",
        "core/crates/searchkernel/Cargo.toml",
        "core/crates/searchkernel/src/index/native.rs",
        "core/crates/searchkernel/src/index/manifest.rs",
        "core/crates/searchkernel/src/portable_definition.rs",
        "searchkerneld/Cargo.toml",
        "searchkerneld/src/main.rs",
        "reference/vespa/**",
        "tools/check-vespa-bindings.sh",
        "tools/check-native-*.sh",
        "tools/test-native-*.sh",
        "tools/native-*.py",
        "tools/resolve-native-sdk.sh",
    ),
    "protocol": (
        "api/**",
        "core/crates/searchkernel-protocol/src/generated/**",
        "core/crates/searchkernel-protocol/bfbs/**",
        "searchkerneld/src/generated/**",
        "clients/python/searchkernel/_wire_generated/**",
        "clients/python/searchkernel/rpc/**",
        "clients/typescript/src/generated/**",
        "tools/rpc-codegen/**",
        "tools/generate-searchkernel-protocol.sh",
        "tools/generate-flatbuffers-bfbs.sh",
        "tools/generate-python-flatbuffers.sh",
        "tools/generate-python-grpc.sh",
        "tools/generate-searchkernel-rpc.sh",
        "tools/generate-typescript-flatbuffers.sh",
        "tools/generate-typescript-grpc-web.sh",
        "tools/check-flatbuffers-bfbs.sh",
        "tools/check-searchkernel-protocol-generated.sh",
        "tools/check-searchkernel-rpc.sh",
        "tools/check-python-flatbuffers-generated.sh",
        "tools/check-python-grpc.sh",
        "tools/check-typescript-generated.sh",
        "tools/patch-python-flatbuffers-object-api.py",
    ),
    "api_conformance": (
        "reference/api-conformance/**",
    ),
    "verification": (
        ".githooks/pre-commit",
        ".github/ci/exact_head.py",
        ".github/ci/test_exact_head.py",
        "dev",
    ),
}


def _git(repo: Path, *args: str) -> str:
    completed = subprocess.run(
        ["git", *args],
        cwd=repo,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        check=False,
    )
    if completed.returncode != 0:
        raise RuntimeError(completed.stdout.strip() or "git command failed")
    return completed.stdout.strip()


def assert_exact_head(expected: str, repo: Path) -> int:
    expected = expected.strip().lower()
    if not expected:
        print("::error title=Exact-head verification::Expected head SHA is empty.")
        return 2
    try:
        actual = _git(repo, "rev-parse", "HEAD").lower()
    except RuntimeError as error:
        print(f"::error title=Exact-head verification::{error}")
        return 2

    if actual != expected:
        print(
            "::error title=Stale verification checkout::"
            f"Expected exact PR head {expected}, checked out {actual}. "
            "Verification from a different SHA cannot authorize this head."
        )
        return 1

    print(f"Verified exact PR head {actual}.")
    return 0


def _matches(path: str, patterns: Iterable[str]) -> bool:
    return any(fnmatch.fnmatchcase(path, pattern) for pattern in patterns)


def classify(paths: Iterable[str]) -> dict[str, bool]:
    normalized = [path.strip().replace("\\", "/") for path in paths if path.strip()]
    return {
        lane: any(_matches(path, LANE_PATTERNS[lane]) for path in normalized)
        for lane in LANES
    }


def _read_changed_paths() -> list[str]:
    raw = sys.stdin.buffer.read()
    chunks = raw.split(b"\0") if b"\0" in raw else raw.splitlines()
    return [chunk.decode("utf-8", errors="strict") for chunk in chunks if chunk]


def write_github_output(path: Path, selected: dict[str, bool]) -> None:
    with path.open("a", encoding="utf-8") as output:
        for lane in LANES:
            output.write(f"{lane}={'true' if selected[lane] else 'false'}\n")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(dest="command", required=True)

    head = subparsers.add_parser("assert-head")
    head.add_argument("--expected", required=True)
    head.add_argument("--repo", type=Path, default=Path.cwd())

    changes = subparsers.add_parser("classify")
    changes.add_argument("--github-output", type=Path)
    return parser


def main() -> int:
    args = build_parser().parse_args()
    if args.command == "assert-head":
        return assert_exact_head(args.expected, args.repo)

    changed_paths = _read_changed_paths()
    selected = classify(changed_paths)
    if args.github_output is not None:
        write_github_output(args.github_output, selected)
    print("Changed paths:")
    for changed_path in changed_paths:
        print(f"  {changed_path}")
    print("Behavioral lanes:")
    for lane in LANES:
        print(f"  {lane}: {'required' if selected[lane] else 'not required'}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
