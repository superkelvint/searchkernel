#!/usr/bin/env python3
"""Reject acceptance-fixture inputs embedded in production source."""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path


def fixture_values(value, keys, key=None):
    if isinstance(value, dict):
        for child_key, child in value.items():
            yield from fixture_values(child, keys, child_key)
    elif isinstance(value, list):
        for child in value:
            yield from fixture_values(child, keys, key)
    elif isinstance(value, str) and key in keys and len(value) >= 4:
        yield value


def load(path: Path):
    if path.suffix == ".jsonl":
        return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line.strip()]
    return [json.loads(path.read_text(encoding="utf-8"))]


def source_files(path: Path):
    if path.is_file():
        yield path
        return
    for child in path.rglob("*"):
        relative_parts = child.relative_to(path).parts
        if child.is_file() and not any(part in {"target", "__pycache__", "test", "tests"}
                                       for part in relative_parts[:-1]):
            # The Vespa-compatible stemmer dictionary legitimately contains
            # individual English fixture terms such as "brown".
            if child.name == "data.rs":
                continue
            if child.suffix in {".rs", ".c", ".cc", ".cpp", ".h", ".hh", ".hpp"}:
                yield child


def main() -> int:
    args = sys.argv[1:]
    keys = {"id", "input", "text", "query", "doc"}
    if args[:1] == ["--keys"]:
        if len(args) < 2:
            raise SystemExit("--keys requires a comma-separated key list")
        keys = set(args[1].split(","))
        args = args[2:]
    if len(args) < 2:
        raise SystemExit(f"usage: {sys.argv[0]} [--keys KEY,...] SCENARIO SOURCE...")
    scenario = Path(args[0])
    needles = sorted({needle for root in load(scenario) for needle in fixture_values(root, keys)})
    for source_root in map(Path, args[1:]):
        for source in source_files(source_root):
            contents = source.read_text(encoding="utf-8", errors="replace")
            if source.suffix == ".rs":
                # Unit tests may intentionally use the same semantic words as
                # an acceptance scenario. The scan is for production literals;
                # Rust keeps conventional unit tests behind this cfg boundary.
                contents = re.split(r"(?m)^\s*#\[cfg\(test\)\]", contents, maxsplit=1)[0]
            for needle in needles:
                if any(char.isspace() for char in needle):
                    found = needle in contents
                else:
                    found = bool(re.search(rf"([\"']){re.escape(needle)}\1", contents))
                if found:
                    raise SystemExit(f"fixture literal {needle!r} appears in production source {source}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
