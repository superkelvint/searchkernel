#!/usr/bin/env python3
"""Guard the Python SDK against reintroducing a parallel schema codec."""

from pathlib import Path
import ast
import sys


ROOT = Path(__file__).resolve().parents[1]
PACKAGE = ROOT / "clients/python/searchkernel"


def main() -> int:
    retired = [
        PACKAGE / "wire.py",
        PACKAGE / "legacy_json.py",
        PACKAGE / "generated/models.py",
    ]
    present = [path.relative_to(ROOT) for path in retired if path.exists()]
    if present:
        print("retired handwritten Python schema files exist:", *present, file=sys.stderr)
        return 1

    forbidden = {"searchkernel.wire", "searchkernel.legacy_json", "searchkernel.generated.models"}
    for path in PACKAGE.rglob("*.py"):
        if "generated" in path.parts and "_wire" in path.parts:
            continue
        tree = ast.parse(path.read_text(), filename=str(path))
        for node in ast.walk(tree):
            module = None
            if isinstance(node, ast.Import):
                module = next((alias.name for alias in node.names if alias.name in forbidden), None)
            elif isinstance(node, ast.ImportFrom):
                module = node.module
            if module in forbidden:
                print(f"retired Python schema module imported by {path.relative_to(ROOT)}", file=sys.stderr)
                return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
