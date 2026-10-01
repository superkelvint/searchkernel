#!/usr/bin/env python3
"""Keep recursive flatc Python object-api imports inside the client package.

flatc currently emits eager imports for every referenced table.  The recursive value
model is recursive (Value -> ValueObject -> ValueEntry -> Value), so those
imports otherwise form a Python import cycle.  This is a mechanical generated
output post-processing step; it does not know any schema fields or protocol
operations.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path


PACKAGE = "searchkernel._wire.v1.generated.SearchKernel"
DIRECT_IMPORT = re.compile(r"^(from (SearchKernel\.[A-Za-z0-9_.]+) import ([A-Za-z0-9_]+))$")
MODULE_IMPORT = re.compile(r"^import (SearchKernel\.[A-Za-z0-9_.]+)$")


def patch(path: Path) -> None:
    lines = path.read_text().splitlines()
    has_lazy_type_import = False
    has_lazy_module_import = False
    result: list[str] = []
    for line in lines:
        indented_module = re.match(r"^(\s*)import SearchKernel\.[A-Za-z0-9_.]+$", line)
        if indented_module:
            result.append(f"{indented_module.group(1)}from {PACKAGE.rsplit('.', 1)[0]} import SearchKernel")
            continue
        direct = DIRECT_IMPORT.match(line)
        if direct:
            _, module, attribute = direct.groups()
            result.append(
                f"from {PACKAGE}._lazy import lazy_type as _lazy_type"
                if not has_lazy_type_import
                else ""
            )
            has_lazy_type_import = True
            module_name = f'{PACKAGE}.{module.removeprefix("SearchKernel.")}'
            result.append(f'_lazy_{attribute} = _lazy_type("{module_name}", "{attribute}")')
            result.append(f'{attribute} = _lazy_{attribute}')
            continue
        module = MODULE_IMPORT.match(line)
        if module:
            result.append(
                f"from {PACKAGE}._lazy import lazy_module as _lazy_module"
                if not has_lazy_module_import
                else ""
            )
            has_lazy_module_import = True
            module_name = f'{PACKAGE}.{module.group(1).removeprefix("SearchKernel.")}'
            result.append(f'_lazy_{module.group(1).rsplit(".", 1)[-1]} = _lazy_module("{module_name}")')
            continue
        result.append(line)
    path.write_text(f"import {PACKAGE} as SearchKernel\n\n" + "\n".join(result) + "\n")


def main() -> None:
    if len(sys.argv) != 2:
        raise SystemExit("usage: patch-python-flatbuffers-object-api.py GENERATED_DIR")
    root = Path(sys.argv[1])
    lazy = root / "SearchKernel" / "_lazy.py"
    lazy.write_text(
        '''"""Runtime support for recursive generated FlatBuffers modules."""\n\n'''
        "from __future__ import annotations\n\n"
        "import importlib\n\n"
        "\n"
        "class _LazyReference:\n"
        "    def __init__(self, module_name, attribute=None):\n"
        "        self._module_name = module_name\n"
        "        self._attribute = attribute\n\n"
        "    def _resolve(self):\n"
        "        value = importlib.import_module(self._module_name)\n"
        "        return getattr(value, self._attribute) if self._attribute else value\n\n"
        "    def __call__(self, *args, **kwargs):\n"
        "        return self._resolve()(*args, **kwargs)\n\n"
        "    def __getattr__(self, name):\n"
        "        return getattr(self._resolve(), name)\n\n"
        "def lazy_type(module_name, attribute):\n"
        "    return _LazyReference(module_name, attribute)\n\n"
        "def lazy_module(module_name):\n"
        "    return _LazyReference(module_name)\n"
    )
    package_code = (
        "\"\"\"Lazy submodule imports for generated FlatBuffers packages.\"\"\"\n\n"
        "import importlib\n\n"
        "def __getattr__(name):\n"
        "    return importlib.import_module(f\"{__name__}.{name}\")\n"
    )
    for package in (root / "SearchKernel", root / "SearchKernel" / "Wire", root / "SearchKernel" / "Protocol"):
        (package / "__init__.py").write_text(package_code)
    for path in root.rglob("*.py"):
        if path.name not in {"__init__.py", "_lazy.py"}:
            patch(path)


if __name__ == "__main__":
    main()
