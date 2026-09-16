#!/usr/bin/env python3
"""C7 Python NativeTransport structural anti-shortcut checker.

Generated files/directories are excluded. The checker is intentionally simple:
it guards architectural ownership, while behavioral probes guard FFI behavior.
"""

from __future__ import annotations

import argparse
import ast
import json
import re
from pathlib import Path

FORBIDDEN_METHODS = {
    "insert",
    "upsert",
    "patch",
    "delete",
    "get",
    "search",
    "analyze",
    "describe",
    "inspect_query",
    "facet",
    "facets",
    "ranking",
    "filter",
}

FORBIDDEN_SEMANTIC_PATTERNS = [
    r"\bBM25\b",
    r"\bRankingPolicy\b",
    r"\bFilterExpr\b",
    r"\bFacetRequest\b",
    r"\bResolved(?:Search|Field|Index|Request)\b",
    r"\bto_ir\b",
    r"searchkernel[-_]vespa",
    r"JSON\s*[-=]>\s*IR",
    r"portable[_ ]error.*map",
]

# Handwritten NativeTransport should treat canonical FlatBuffers envelopes as
# opaque bytes. Generated client code may of course import FlatBuffers.
FORBIDDEN_TRANSPORT_PATTERNS = [
    r"flatbuffers\.(?:Builder|Table)",
    r"GetRootAs[A-Za-z0-9_]*\s*\(",
    r"(?:parse|decode|deserialize).*(?:flatbuffer|request|response)",
]


def is_generated(path: Path) -> bool:
    parts = {p.lower() for p in path.parts}
    return bool(parts & {"generated", "gen", "vendor", "target", "__pycache__", ".venv", "venv"})


def check_python(path: Path, text: str, problems: list[str]):
    try:
        tree = ast.parse(text, filename=str(path))
    except SyntaxError as exc:
        problems.append(f"{path}: invalid Python: {exc}")
        return

    for node in ast.walk(tree):
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
            if node.name.lower() in FORBIDDEN_METHODS:
                problems.append(
                    f"{path}:{node.lineno}: operation-specific transport method {node.name!r}"
                )


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("paths", nargs="+", type=Path)
    args = ap.parse_args()
    problems: list[str] = []
    checked = 0

    for root in args.paths:
        paths = [root] if root.is_file() else list(root.rglob("*"))
        for path in paths:
            if not path.is_file() or is_generated(path) or path.suffix != ".py":
                continue
            checked += 1
            text = path.read_text(errors="replace")
            check_python(path, text, problems)
            for pattern in FORBIDDEN_SEMANTIC_PATTERNS + FORBIDDEN_TRANSPORT_PATTERNS:
                for match in re.finditer(pattern, text, re.IGNORECASE | re.DOTALL):
                    line = text.count("\n", 0, match.start()) + 1
                    problems.append(
                        f"{path}:{line}: semantic/protocol logic forbidden in handwritten "
                        f"NativeTransport ({pattern})"
                    )

    if problems:
        print(json.dumps({"status": "FAIL", "problems": problems}, indent=2))
        raise SystemExit(1)
    if checked == 0:
        raise SystemExit("no handwritten Python source files were checked")
    print(json.dumps({"status": "PASS", "checked_files": checked}, indent=2))


if __name__ == "__main__":
    main()
