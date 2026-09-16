#!/usr/bin/env python3
"""C7 structural anti-shortcut checker.

This intentionally errs on the conservative side. Generated files/directories
are excluded; only handwritten native-transport source should be passed here.
"""

import argparse
import ast
import json
import re
from pathlib import Path

FORBIDDEN_METHODS = {
    "insert", "upsert", "patch", "delete", "get", "search", "analyze",
    "describe", "inspect_query", "facet", "facets", "ranking", "filter",
}

FORBIDDEN_SEMANTIC_WORDS = [
    r"\bBM25\b",
    r"\bRankingPolicy\b",
    r"\bFilterExpr\b",
    r"\bFacetRequest\b",
    r"\bResolved(?:Search|Field|Index|Request)\b",
    r"\bto_ir\b",
    r"searchkernel[-_]vespa",
    r"JSON\s*[-=]>\s*IR",
]


def check_python(path: Path, text: str, problems: list[str]):
    try:
        tree = ast.parse(text, filename=str(path))
    except SyntaxError as exc:
        problems.append(f"{path}: invalid Python: {exc}")
        return
    for node in ast.walk(tree):
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)) and node.name.lower() in FORBIDDEN_METHODS:
            problems.append(f"{path}:{node.lineno}: operation-specific transport method {node.name!r}")


def check_go(path: Path, text: str, problems: list[str]):
    for m in re.finditer(r"\bfunc\s*(?:\([^)]*\)\s*)?([A-Za-z_][A-Za-z0-9_]*)\s*\(", text):
        name = m.group(1).lower()
        if name in FORBIDDEN_METHODS:
            line = text.count("\n", 0, m.start()) + 1
            problems.append(f"{path}:{line}: operation-specific transport method {m.group(1)!r}")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("paths", nargs="+", type=Path)
    args = ap.parse_args()
    problems: list[str] = []

    for root in args.paths:
        paths = [root] if root.is_file() else list(root.rglob("*"))
        for path in paths:
            if not path.is_file():
                continue
            rel = str(path).lower()
            if any(part in rel for part in ("generated", "gen/", "vendor", "target/", "__pycache__")):
                continue
            if path.suffix not in {".py", ".go", ".rs", ".c", ".h"}:
                continue
            text = path.read_text(errors="replace")
            if path.suffix == ".py":
                check_python(path, text, problems)
            elif path.suffix == ".go":
                check_go(path, text, problems)
            for pattern in FORBIDDEN_SEMANTIC_WORDS:
                for m in re.finditer(pattern, text, re.IGNORECASE):
                    line = text.count("\n", 0, m.start()) + 1
                    problems.append(f"{path}:{line}: semantic logic/import forbidden in NativeTransport ({pattern})")

    if problems:
        print(json.dumps({"status": "FAIL", "problems": problems}, indent=2))
        raise SystemExit(1)
    print(json.dumps({"status": "PASS", "checked": [str(p) for p in args.paths]}, indent=2))


if __name__ == "__main__":
    main()
