#!/usr/bin/env python3
"""Lightweight static checks for repository Python tooling.

This intentionally uses only the Python standard library so it is always
available on self-hosted runners.
"""

from __future__ import annotations

import ast
import builtins
import pathlib
import subprocess
import symtable
import sys
from collections.abc import Iterable


TOOLING_PREFIXES = (
    pathlib.PurePosixPath(".github"),
    pathlib.PurePosixPath("tools"),
    pathlib.PurePosixPath("reference"),
    pathlib.PurePosixPath("clients/php/examples"),
    pathlib.PurePosixPath("clients/php/tests"),
    pathlib.PurePosixPath("clients/typescript/scripts"),
)

SKIP_PARTS = {
    "_wire_generated",
    "generated",
    "node_modules",
    "target",
    ".venv",
    "venv",
}

IMPLICIT_GLOBALS = {
    "__builtins__",
    "__cached__",
    "__file__",
    "__loader__",
    "__name__",
    "__package__",
    "__spec__",
}


def tracked_python_files() -> list[pathlib.Path]:
    completed = subprocess.run(
        ["git", "ls-files", "-z", "--", "*.py"],
        check=True,
        stdout=subprocess.PIPE,
    )
    paths = [pathlib.Path(raw.decode()) for raw in completed.stdout.split(b"\0") if raw]
    return [path for path in paths if should_check(path)]


def should_check(path: pathlib.Path) -> bool:
    if any(part in SKIP_PARTS for part in path.parts):
        return False
    pure = pathlib.PurePosixPath(path.as_posix())
    return any(pure == prefix or prefix in pure.parents for prefix in TOOLING_PREFIXES)


def module_bindings(tree: ast.Module) -> tuple[set[str], set[str], bool]:
    imports: set[str] = set()
    definitions: set[str] = set()
    has_star_import = False

    def bind_target(target: ast.expr) -> None:
        if isinstance(target, ast.Name):
            definitions.add(target.id)
        elif isinstance(target, (ast.Tuple, ast.List)):
            for element in target.elts:
                bind_target(element)

    for node in tree.body:
        if isinstance(node, ast.Import):
            for alias in node.names:
                imports.add(alias.asname or alias.name.split(".", 1)[0])
        elif isinstance(node, ast.ImportFrom):
            for alias in node.names:
                if alias.name == "*":
                    has_star_import = True
                else:
                    imports.add(alias.asname or alias.name)
        elif isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)):
            definitions.add(node.name)
        elif isinstance(node, (ast.Assign, ast.AnnAssign, ast.NamedExpr)):
            targets = node.targets if isinstance(node, ast.Assign) else [node.target]
            for target in targets:
                bind_target(target)
        elif isinstance(node, ast.AugAssign):
            bind_target(node.target)
        elif isinstance(node, (ast.For, ast.AsyncFor)):
            bind_target(node.target)
        elif isinstance(node, (ast.With, ast.AsyncWith)):
            for item in node.items:
                if item.optional_vars is not None:
                    bind_target(item.optional_vars)
        elif isinstance(node, ast.Try):
            for handler in node.handlers:
                if handler.name:
                    definitions.add(handler.name)

    return imports, definitions, has_star_import


def walk_tables(table: symtable.SymbolTable) -> Iterable[symtable.SymbolTable]:
    yield table
    for child in table.get_children():
        yield from walk_tables(child)


def check_file(path: pathlib.Path) -> list[str]:
    source = path.read_text(encoding="utf-8")
    errors: list[str] = []
    try:
        tree = ast.parse(source, filename=str(path))
        table = symtable.symtable(source, str(path), "exec")
    except SyntaxError as exc:
        errors.append(f"{path}:{exc.lineno or 1}: syntax error: {exc.msg}")
        return errors

    imports, definitions, has_star_import = module_bindings(tree)
    shadowed = sorted(imports & definitions)
    for name in shadowed:
        errors.append(
            f"{path}: imported name {name!r} is redefined at module scope"
        )

    if has_star_import:
        return errors

    module_names = set(imports) | set(definitions) | set(IMPLICIT_GLOBALS)
    builtin_names = set(dir(builtins))

    for child in walk_tables(table):
        if child is table:
            continue
        for symbol in child.get_symbols():
            if (
                symbol.is_referenced()
                and symbol.is_global()
                and symbol.get_name() not in module_names
                and symbol.get_name() not in builtin_names
            ):
                errors.append(
                    f"{path}:{child.get_lineno()}: undefined global name "
                    f"{symbol.get_name()!r} referenced in {child.get_name()}"
                )

    return sorted(set(errors))


def main(argv: list[str]) -> int:
    paths = [pathlib.Path(value) for value in argv] if argv else tracked_python_files()
    errors: list[str] = []
    for path in paths:
        errors.extend(check_file(path))

    for error in errors:
        print(error, file=sys.stderr)
    if errors:
        return 1

    print(f"python tooling static checks: PASS ({len(paths)} files)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
