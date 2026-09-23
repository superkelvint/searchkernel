#!/usr/bin/env python3
"""Reject Vespa dependencies in the backend-free testkit sources."""

from __future__ import annotations

import re
from pathlib import Path
import sys


FORBIDDEN = re.compile(r"searchkernel-(?:vespa|vespa-sys)")


def files_under(path: Path) -> list[Path]:
    if path.is_file():
        return [path]
    if path.is_dir():
        return sorted(candidate for candidate in path.rglob("*") if candidate.is_file())
    raise OSError(f"path does not exist: {path}")


def main(arguments: list[str]) -> int:
    if not arguments:
        print(f"usage: {Path(sys.argv[0]).name} <file-or-directory>...", file=sys.stderr)
        return 2

    forbidden = False
    try:
        paths = [file for argument in arguments for file in files_under(Path(argument))]
        for path in paths:
            for line_number, line in enumerate(path.read_text().splitlines(), start=1):
                if FORBIDDEN.search(line):
                    print(f"{path}:{line_number}:{line}")
                    forbidden = True
    except (OSError, UnicodeError) as error:
        print(f"backend-free dependency check failed: {error}", file=sys.stderr)
        return 2

    if forbidden:
        print("searchkernel-testkit must remain backend-free", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
