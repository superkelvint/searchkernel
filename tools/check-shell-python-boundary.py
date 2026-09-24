#!/usr/bin/env python3
"""Reject Python source embedded directly in tracked shell scripts."""

from __future__ import annotations

import pathlib
import re
import subprocess
import sys


PYTHON_COMMAND = r'(?:python(?:3(?:\.[0-9]+)?)?|"?\$\{?[A-Za-z0-9_]*PYTHON\}?"?)'
INLINE_C = re.compile(PYTHON_COMMAND + r"\s+-c(?:\s|$)")
HEREDOC = re.compile(PYTHON_COMMAND + r"[^#\n]*<<-?\s*['\"]?[A-Za-z_][A-Za-z0-9_]*['\"]?")


def tracked_shell_files() -> list[pathlib.Path]:
    completed = subprocess.run(
        ["git", "ls-files", "-z", "--", "*.sh"],
        check=True,
        stdout=subprocess.PIPE,
    )
    return [pathlib.Path(raw.decode()) for raw in completed.stdout.split(b"\0") if raw]


def violations(path: pathlib.Path) -> list[tuple[int, str]]:
    found: list[tuple[int, str]] = []
    for line_number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        stripped = line.lstrip()
        if stripped.startswith("#"):
            continue
        if INLINE_C.search(line) or HEREDOC.search(line):
            found.append((line_number, line.strip()))
    return found


def main() -> int:
    bad = False
    for path in tracked_shell_files():
        for line_number, line in violations(path):
            bad = True
            print(
                f"{path}:{line_number}: embedded Python is forbidden in shell scripts: {line}",
                file=sys.stderr,
            )
    if bad:
        print(
            "Move Python logic into a named .py file and let shell only orchestrate it.",
            file=sys.stderr,
        )
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
