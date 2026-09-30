#!/usr/bin/env python3
"""Classify whether changed paths require exact-head Rust coverage verification.

Coverage is expensive, so select it only when the coverage execution surface
itself changes; nightly coverage remains the broad product baseline.
"""

from __future__ import annotations

import fnmatch
import sys
from collections.abc import Iterable

COVERAGE_PATTERNS = (
    "dev",
    "docker/searchkernel-ci/Dockerfile",
    "tools/coverage.sh",
    "tools/run-in-ci-image.sh",
    "tools/test-dev-ci-image.sh",
    ".github/ci/ci-image-runner.yml",
    ".github/workflows/nightly.yml",
    ".github/ci/coverage_verifier_required.py",
    ".github/ci/test_coverage_verifier_required.py",
)


def matching_rule(path: str) -> str | None:
    normalized = path.strip().replace("\\", "/")
    if not normalized:
        return None
    for pattern in COVERAGE_PATTERNS:
        if fnmatch.fnmatchcase(normalized, pattern):
            return pattern
    return None


def classify(paths: Iterable[str]) -> list[tuple[str, str]]:
    matches: list[tuple[str, str]] = []
    for raw_path in paths:
        path = raw_path.strip().replace("\\", "/")
        rule = matching_rule(path)
        if rule is not None:
            matches.append((path, rule))
    return matches


def main() -> int:
    paths = [line for line in sys.stdin.read().splitlines() if line.strip()]
    matches = classify(paths)
    for path, rule in matches:
        print(f"coverage verifier required: {path} (rule {rule})", file=sys.stderr)
    print("true" if matches else "false")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
