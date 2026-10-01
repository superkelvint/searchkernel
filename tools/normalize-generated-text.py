#!/usr/bin/env python3
"""Normalize whitespace emitted by FlatBuffers source generators."""

from __future__ import annotations

import sys
from pathlib import Path


def main() -> None:
    if len(sys.argv) != 2:
        raise SystemExit("usage: normalize-generated-text.py GENERATED_DIR")
    root = Path(sys.argv[1])
    for suffix in (".pyi", ".ts"):
        for path in root.rglob(f"*{suffix}"):
            source = path.read_text(encoding="utf-8")
            normalized = "\n".join(line.rstrip() for line in source.splitlines()).rstrip("\n") + "\n"
            if source != normalized:
                path.write_text(normalized, encoding="utf-8")


if __name__ == "__main__":
    main()
