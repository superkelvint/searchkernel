#!/usr/bin/env python3
"""Enforce one canonical owner for PR evidence and one image execution authority."""

from __future__ import annotations

import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
WORKFLOWS = ROOT / ".github" / "workflows"
CANONICAL = WORKFLOWS / "behavioral-verification.yml"
EXECUTOR = WORKFLOWS / "ci-image-runner.yml"
RETIRED = WORKFLOWS / "unit-tests.yml"

EXPECTED_COMMANDS = {
    "Rust": "cargo test --workspace --all-targets --no-default-features",
    "Python": '"$PYTHON" -m unittest discover -s clients/python/tests -p \'test_*.py\'',
    "TypeScript": "npm --prefix clients/typescript test",
}


def owners_of(fragment: str) -> list[Path]:
    owners: list[Path] = []
    for path in sorted(WORKFLOWS.glob("*.yml")):
        if fragment in path.read_text(encoding="utf-8"):
            owners.append(path)
    return owners


def main() -> int:
    errors: list[str] = []

    if RETIRED.exists():
        errors.append(
            f"{RETIRED.relative_to(ROOT)} must stay retired; broad evidence belongs "
            "to the canonical invariant workflow"
        )

    for path, label in ((CANONICAL, "canonical CI workflow"), (EXECUTOR, "CI image runner")):
        if not path.exists():
            errors.append(f"missing {label}: {path.relative_to(ROOT)}")

    if errors:
        print_errors(errors)
        return 1

    canonical = CANONICAL.read_text(encoding="utf-8")
    executor = EXECUTOR.read_text(encoding="utf-8")

    # Behavioral CI owns selection/gating. It delegates execution rather than
    # embedding compiler/package-manager commands in host jobs.
    required_delegations = (
        ("Rust", "target: rust-non-native"),
        ("Python", "target: python"),
        ("TypeScript", "target: typescript"),
        ("Ruby", "target: ruby"),
        ("Tooling", "target: tooling"),
        ("Native", "target: native-full"),
    )
    for language, target in required_delegations:
        if f"name: {language} invariant" not in canonical and language not in {"Tooling", "Native"}:
            errors.append(f"canonical workflow is missing {language} invariant lane")
        if target not in canonical:
            errors.append(f"canonical workflow does not delegate {language} via {target!r}")

    for fragment in (
        "name: Plan invariant lanes",
        "name: CI and build-tooling invariant",
        "name: Invariant gate",
        "uses: ./.github/workflows/ci-image-runner.yml",
        "check-ci-evidence-ownership.py",
    ):
        if fragment not in canonical:
            errors.append(f"canonical workflow is missing {fragment!r}")

    # The command implementations themselves live in one reusable workflow.
    # This keeps command drift and host-side duplicate execution out of PR CI.
    for language, command in EXPECTED_COMMANDS.items():
        owners = owners_of(command)
        if owners != [EXECUTOR]:
            rendered = ", ".join(str(path.relative_to(ROOT)) for path in owners) or "none"
            errors.append(
                f"{language} baseline command must be owned only by "
                f"{EXECUTOR.relative_to(ROOT)}; found {rendered}"
            )

    for forbidden in (
        "ensure-runner-rust.sh",
        "Swatinem/rust-cache@",
        "CARGO_TARGET_DIR: target/ci-non-native",
        'workspaces: ". -> target/ci-non-native"',
        "shared-key: non-native",
    ):
        if forbidden in canonical:
            errors.append(
                f"canonical PR workflow must not own mutable worker state: {forbidden!r}"
            )

    if "zero shared mutable worker state" not in executor:
        errors.append("CI image runner must declare the zero-shared-state execution contract")

    if errors:
        print_errors(errors)
        return 1

    print(
        "CI evidence ownership PASS: behavioral CI selects/gates lanes; "
        "ci-image-runner is the sole Rust/Python/TypeScript execution authority."
    )
    return 0


def print_errors(errors: list[str]) -> None:
    for error in errors:
        print(f"CI evidence ownership error: {error}", file=sys.stderr)


if __name__ == "__main__":
    raise SystemExit(main())
