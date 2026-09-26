#!/usr/bin/env python3
"""Classify whether changed paths require the tooling invariant lane."""

from __future__ import annotations

import fnmatch
import sys
from collections.abc import Iterable

TOOLING_PATTERNS = (
    "tools/**",
    ".github/ci/**",
)


def matching_rule(path: str) -> str | None:
    normalized = path.strip().replace("\\", "/")
    if not normalized:
        return None
    for pattern in TOOLING_PATTERNS:
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
        print(f"tooling invariant required: {path} (rule {rule})", file=sys.stderr)
    print("true" if matches else "false")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
