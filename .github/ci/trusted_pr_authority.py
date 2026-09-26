#!/usr/bin/env python3
"""Fail closed when a PR changes code that defines its own verification authority."""
from __future__ import annotations
import argparse
from pathlib import PurePosixPath

PROTECTED_PREFIXES = (
    ".github/ci/", ".github/workflows/", "verification/", "reference/vespa/", "tools/",
    ".moon/",
)
PROTECTED_FILES = {"dev", "moon.yml", "workspace.yml", ".prototools"}


def protected(path: str) -> bool:
    path = PurePosixPath(path).as_posix(); path = path[2:] if path.startswith("./") else path
    return path in PROTECTED_FILES or any(path.startswith(prefix) for prefix in PROTECTED_PREFIXES)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--changed-paths", required=True)
    args = parser.parse_args()
    paths = [line.strip() for line in open(args.changed_paths, encoding="utf-8") if line.strip()]
    blocked = sorted(path for path in paths if protected(path))
    if blocked:
        print("::error title=Trusted verification authority changed::This PR changes merge-authorization machinery. Candidate verification cannot authorize that transition; land it through the explicit trusted transition/review path, then verify subsequent PRs with the new base authority.")
        for path in blocked:
            print(f"  {path}")
        return 3
    print("Trusted verification authority is unchanged by the candidate.")
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
