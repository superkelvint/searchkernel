#!/usr/bin/env python3
"""Select non-native behavioral CI lanes from changed repository paths.

The authoritative copy is loaded from the pull request's trusted base revision.
Unknown behavioral/build/tooling paths deliberately select every lane so a new
surface cannot become untested merely because this temporary selector predates
it.
"""

from __future__ import annotations

import argparse
import fnmatch
import sys
from collections.abc import Iterable
from pathlib import Path

LANES = ("rust", "python", "typescript", "ruby")

# Portable/client-contract changes can alter every first-party facade's behavior.
ALL_LANE_PATTERNS = (
    "api/**",
    "tools/check-capabilities.py",
    "tools/check-client-contracts.py",
    "tools/test_check_capabilities.py",
    "tools/test_check_client_contracts.py",
    ".github/workflows/behavioral-verification.yml",
    ".github/ci/behavioral_lanes_required.py",
    ".github/ci/test_behavioral_lanes_required.py",
)

# Some files sit physically inside one language subtree but define or verify a
# cross-language wire contract. These rules must run before the broad per-lane
# path rules below or a path such as core/** would incorrectly hide the
# additional client lanes.
MULTI_LANE_PATTERNS: tuple[tuple[tuple[str, ...], tuple[str, ...]], ...] = (
    (
        (
            "api/rpc/searchkernel.proto",
            "core/crates/searchkernel-protocol/bfbs/**",
            "tools/generate-flatbuffers-bfbs.sh",
            "tools/check-flatbuffers-bfbs.sh",
            "tools/check-flatbuffers-cpp.sh",
            "tools/resolve-flatc.sh",
            "tools/test-flatbuffers-cpp-toolchain.sh",
        ),
        ("rust", "python", "typescript"),
    ),
    (
        ("core/crates/searchkernel-protocol/tests/ruby_json_contract.rs",),
        ("rust", "ruby"),
    ),
)

LANE_PATTERNS = {
    "rust": (
        "Cargo.toml",
        "Cargo.lock",
        ".cargo/**",
        "rust-toolchain*",
        "core/**",
        "searchkerneld/**",
        "reference/api-conformance/**",
        "reference/vespa/**",
        "tools/check-ir-boundary.sh",
        "tools/test_check_ir_boundary.py",
        "tools/check-search-semantic-entry-points.py",
        "tools/test_check_search_semantic_entry_points.py",
        "tools/run_clippy.py",
        "tools/test_run_clippy.py",
        "tools/rpc-codegen/**",
        "tools/generate-searchkernel-protocol.sh",
        "tools/generate-searchkernel-rpc.sh",
        "tools/check-searchkernel-protocol-generated.sh",
        "tools/check-searchkernel-rpc.sh",
    ),
    "python": (
        "clients/python/**",
        "tools/check-python-*.py",
        "tools/check-python-*.sh",
        "tools/test-python-*.py",
        "tools/test_python_*.py",
        "tools/run-python-*.sh",
        "tools/generate-python-*.sh",
        "tools/patch-python-*.py",
    ),
    "typescript": (
        "clients/typescript/**",
        "tools/*typescript*.sh",
        "tools/*typescript*.py",
    ),
    "ruby": (
        "clients/ruby/**",
        "tools/*ruby*.sh",
        "tools/*ruby*.py",
    ),
}

# These paths cannot affect executable behavioral lanes in this workflow.
NON_BEHAVIORAL_PATTERNS = (
    "README.md",
    "LIMITATIONS.md",
    "AGENTS.md",
    "BUILDING.md",
    "docs/**",
    "implementation-plan/**",
    "clients/*.md",
    "clients/php/**",
    "tools/check-layout.sh",
    ".gitignore",
    ".rgignore",
)


def _matches(path: str, patterns: Iterable[str]) -> str | None:
    for pattern in patterns:
        if fnmatch.fnmatchcase(path, pattern):
            return pattern
    return None


def classify(paths: Iterable[str]) -> tuple[dict[str, bool], dict[str, list[str]]]:
    selected = {lane: False for lane in LANES}
    reasons = {lane: [] for lane in LANES}
    dev_changed = False

    for raw_path in paths:
        path = raw_path.strip().replace("\\", "/")
        if not path:
            continue

        # `dev` is a shared command wrapper. When it changes alongside a
        # clearly owned product surface, keep verification scoped to those
        # already-selected lanes instead of fanning out every language. A
        # standalone `dev` change still fails closed below.
        if path == "dev":
            dev_changed = True
            continue

        if _matches(path, NON_BEHAVIORAL_PATTERNS) is not None:
            continue

        matched_multi = False
        for patterns, lanes in MULTI_LANE_PATTERNS:
            rule = _matches(path, patterns)
            if rule is None:
                continue
            matched_multi = True
            for lane in lanes:
                selected[lane] = True
                reasons[lane].append(f"{path} (shared rule {rule})")
            break
        if matched_multi:
            continue

        all_rule = _matches(path, ALL_LANE_PATTERNS)
        if all_rule is not None:
            for lane in LANES:
                selected[lane] = True
                reasons[lane].append(f"{path} (shared rule {all_rule})")
            continue

        matched = False
        for lane in LANES:
            rule = _matches(path, LANE_PATTERNS[lane])
            if rule is not None:
                matched = True
                selected[lane] = True
                reasons[lane].append(f"{path} (rule {rule})")

        if not matched:
            # Fail closed for source/build/tooling surfaces this temporary map
            # does not yet understand. Explicit documentation-only paths above
            # are the only zero-lane exemption.
            for lane in LANES:
                selected[lane] = True
                reasons[lane].append(f"{path} (unclassified: conservative fallback)")

    if dev_changed:
        already_selected = [lane for lane in LANES if selected[lane]]
        if already_selected:
            for lane in already_selected:
                reasons[lane].append(
                    "dev (supports already-selected product lane; no cross-language broadening)"
                )
        else:
            for lane in LANES:
                selected[lane] = True
                reasons[lane].append(
                    "dev (standalone shared developer-tooling change: conservative fallback)"
                )

    return selected, reasons


def _read_paths() -> list[str]:
    return [line for line in sys.stdin.read().splitlines() if line.strip()]


def write_github_output(path: Path, selected: dict[str, bool]) -> None:
    with path.open("a", encoding="utf-8") as output:
        for lane in LANES:
            output.write(f"{lane}={'true' if selected[lane] else 'false'}\n")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--github-output", type=Path)
    args = parser.parse_args()

    selected, reasons = classify(_read_paths())
    if args.github_output is not None:
        write_github_output(args.github_output, selected)

    for lane in LANES:
        state = "required" if selected[lane] else "not required"
        print(f"{lane}: {state}")
        for reason in reasons[lane]:
            print(f"  - {reason}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
