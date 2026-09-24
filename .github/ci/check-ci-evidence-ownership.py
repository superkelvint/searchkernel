#!/usr/bin/env python3
"""Fail if PR CI owns the same broad Rust evidence in more than one lane."""

from __future__ import annotations

import re
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
WORKFLOWS = ROOT / ".github" / "workflows"
CANONICAL = WORKFLOWS / "behavioral-verification.yml"
RETIRED = WORKFLOWS / "unit-tests.yml"
EXPECTED_RUST_COMMAND = "cargo test --workspace --all-targets --no-default-features"
EXPECTED_PYTHON_COMMAND = "$CI_PYTHON -m unittest discover -s clients/python/tests -p 'test_*.py'"
EXPECTED_TYPESCRIPT_COMMAND = "npm --prefix clients/typescript test"


def is_pull_request_workflow(text: str) -> bool:
    header = text.split("\njobs:", 1)[0]
    return "pull_request" in header


def workspace_test_commands(text: str) -> list[str]:
    commands: list[str] = []
    for line in text.splitlines():
        stripped = line.strip()
        if "cargo test --workspace" in stripped:
            commands.append(stripped.removeprefix("run:").strip())
    return commands


def exact_command_owners(command: str) -> list[Path]:
    owners: list[Path] = []
    for path in sorted(WORKFLOWS.glob("*.yml")):
        text = path.read_text(encoding="utf-8")
        if not is_pull_request_workflow(text):
            continue
        if command in text:
            owners.append(path)
    return owners


def require_one_language_owner(
    errors: list[str],
    language: str,
    command: str,
) -> None:
    owners = exact_command_owners(command)
    if owners != [CANONICAL]:
        rendered = ", ".join(str(path.relative_to(ROOT)) for path in owners) or "none"
        errors.append(
            f"{language} PR baseline must have exactly one owner, "
            f"{CANONICAL.relative_to(ROOT)}; found {rendered}"
        )


def main() -> int:
    errors: list[str] = []

    if RETIRED.exists():
        errors.append(
            f"{RETIRED.relative_to(ROOT)} must stay retired; broad Rust evidence belongs to the canonical invariant lane"
        )

    if not CANONICAL.exists():
        errors.append(f"missing canonical CI workflow: {CANONICAL.relative_to(ROOT)}")
        print_errors(errors)
        return 1

    owners: list[tuple[Path, str]] = []
    for path in sorted(WORKFLOWS.glob("*.yml")):
        text = path.read_text(encoding="utf-8")
        if not is_pull_request_workflow(text):
            continue
        for command in workspace_test_commands(text):
            owners.append((path, command))

    expected_owner = CANONICAL.relative_to(ROOT)
    if len(owners) != 1:
        rendered = ", ".join(
            f"{path.relative_to(ROOT)}: {command}" for path, command in owners
        ) or "none"
        errors.append(
            "expected exactly one broad PR Rust workspace-test owner; found "
            f"{len(owners)} ({rendered})"
        )
    elif owners[0][0] != CANONICAL or owners[0][1] != EXPECTED_RUST_COMMAND:
        path, command = owners[0]
        errors.append(
            "canonical PR Rust evidence must be "
            f"{expected_owner}: {EXPECTED_RUST_COMMAND!r}; found "
            f"{path.relative_to(ROOT)}: {command!r}"
        )

    require_one_language_owner(errors, "Python", EXPECTED_PYTHON_COMMAND)
    require_one_language_owner(errors, "TypeScript", EXPECTED_TYPESCRIPT_COMMAND)

    canonical = CANONICAL.read_text(encoding="utf-8")
    try:
        rust = canonical.split("\n  rust:\n", 1)[1].split("\n  python:\n", 1)[0]
    except IndexError:
        errors.append("canonical workflow must contain adjacent rust and python invariant jobs")
        rust = ""

    required_rust_fragments = (
        "name: Rust invariant",
        "CARGO_TARGET_DIR: target/ci-non-native",
        'workspaces: ". -> target/ci-non-native"',
        "shared-key: non-native",
        f"run: {EXPECTED_RUST_COMMAND}",
        "run: reference/api-conformance/verify.sh",
    )
    for fragment in required_rust_fragments:
        if fragment not in rust:
            errors.append(f"canonical Rust invariant is missing {fragment!r}")

    required_workflow_fragments = (
        "name: Plan invariant lanes",
        "name: Python invariant",
        "name: TypeScript invariant",
        "name: CI and build-tooling invariant",
        "name: Invariant gate",
        "check-ci-evidence-ownership.py",
    )
    for fragment in required_workflow_fragments:
        if fragment not in canonical:
            errors.append(f"canonical workflow is missing {fragment!r}")

    if re.search(
        r"cargo test --workspace\s+--no-default-features(?:\s|$)",
        canonical,
    ):
        errors.append(
            "the narrower duplicate workspace test command must not return; "
            "the canonical Rust lane owns the --all-targets baseline"
        )

    if errors:
        print_errors(errors)
        return 1

    print(
        "CI evidence ownership PASS: one canonical PR baseline per Rust/Python/TypeScript; "
        "portable conformance reuses the Rust build; tooling evidence is separate."
    )
    return 0


def print_errors(errors: list[str]) -> None:
    for error in errors:
        print(f"CI evidence ownership error: {error}", file=sys.stderr)


if __name__ == "__main__":
    raise SystemExit(main())
