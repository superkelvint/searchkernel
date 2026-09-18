#!/usr/bin/env python3
"""Reject positional construction of generated Python model dataclasses."""

from __future__ import annotations

import argparse
import ast
import sys
from pathlib import Path


def _is_dataclass_decorator(decorator: ast.expr) -> bool:
    if isinstance(decorator, ast.Name):
        return decorator.id == "dataclass"
    return (
        isinstance(decorator, ast.Call)
        and isinstance(decorator.func, ast.Name)
        and decorator.func.id == "dataclass"
    )


def _generated_model_names(path: Path) -> set[str]:
    tree = ast.parse(path.read_text(encoding="utf-8"), filename=str(path))
    return {
        node.name
        for node in tree.body
        if isinstance(node, ast.ClassDef)
        and any(_is_dataclass_decorator(item) for item in node.decorator_list)
    }


def _bound_names(target: ast.expr) -> set[str]:
    if isinstance(target, ast.Name):
        return {target.id}
    if isinstance(target, (ast.Tuple, ast.List)):
        names: set[str] = set()
        for item in target.elts:
            names.update(_bound_names(item))
        return names
    return set()


def _module_name(node: ast.AST) -> str | None:
    if isinstance(node, ast.Name):
        return node.id
    if isinstance(node, ast.Attribute):
        parent = _module_name(node.value)
        return None if parent is None else f"{parent}.{node.attr}"
    return None


def _is_generated_import(module: str | None) -> bool:
    return module in {"generated.models", "searchkernel.generated.models"}


def _analyze_file(path: Path, model_names: set[str]) -> list[tuple[Path, int, str]]:
    tree = ast.parse(path.read_text(encoding="utf-8"), filename=str(path))
    imported_models: set[str] = set()
    generated_module_aliases: set[str] = set()
    local_bindings: set[str] = set()

    for node in tree.body:
        if isinstance(node, (ast.ClassDef, ast.FunctionDef, ast.AsyncFunctionDef)):
            local_bindings.add(node.name)
        elif isinstance(node, (ast.Assign, ast.AnnAssign)):
            targets = node.targets if isinstance(node, ast.Assign) else [node.target]
            for target in targets:
                local_bindings.update(_bound_names(target))

        if isinstance(node, ast.Import):
            for alias in node.names:
                if alias.name == "searchkernel.generated.models":
                    generated_module_aliases.add(alias.asname or "searchkernel")
        elif isinstance(node, ast.ImportFrom):
            module = node.module
            if module == "generated" and any(alias.name == "models" for alias in node.names):
                for alias in node.names:
                    if alias.name == "models":
                        generated_module_aliases.add(alias.asname or alias.name)
            if not _is_generated_import(module):
                continue
            for alias in node.names:
                if alias.name == "*":
                    imported_models.update(model_names)
                elif alias.name in model_names:
                    imported_models.add(alias.asname or alias.name)

    generated_names = imported_models - local_bindings
    violations: list[tuple[Path, int, str]] = []
    for node in ast.walk(tree):
        if not isinstance(node, ast.Call) or not node.args:
            continue
        target = _module_name(node.func)
        direct_name = target if isinstance(node.func, ast.Name) else None
        attribute_model = False
        if isinstance(node.func, ast.Attribute) and target:
            prefix, _, name = target.rpartition(".")
            attribute_model = prefix in generated_module_aliases and name in model_names
        if direct_name in generated_names or attribute_model:
            violations.append((path, node.lineno, target or "<call>"))
    return violations


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("package_root", type=Path)
    args = parser.parse_args()

    package_root = args.package_root.resolve()
    generated_path = package_root / "generated" / "models.py"
    if not generated_path.is_file():
        print(f"generated client not found: {generated_path}", file=sys.stderr)
        return 2
    model_names = _generated_model_names(generated_path)
    violations: list[tuple[Path, int, str]] = []
    for path in sorted(package_root.rglob("*.py")):
        if "generated" in path.relative_to(package_root).parts:
            continue
        violations.extend(_analyze_file(path, model_names))

    if violations:
        for path, line, target in violations:
            print(
                f"{path}:{line}: positional construction of generated model {target}; "
                "use keyword arguments",
                file=sys.stderr,
            )
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
