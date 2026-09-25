#!/usr/bin/env python3
"""Resolve the exact Cargo-built Search Kernel native adapter library directory."""

from __future__ import annotations

import json
from pathlib import Path
import sys


ADAPTER_LIBRARY = "libsearchkernel_vespa.so.2"
BINDINGS_PACKAGE = "searchkernel-vespa-bindings"


def resolve_adapter_lib(messages_path: Path, target_root: Path) -> Path:
    target_root = target_root.resolve()
    candidates: list[Path] = []

    for raw in messages_path.read_text(encoding="utf-8", errors="replace").splitlines():
        if not raw.startswith("{"):
            continue
        try:
            event = json.loads(raw)
        except json.JSONDecodeError:
            continue
        if event.get("reason") != "build-script-executed":
            continue
        if BINDINGS_PACKAGE not in str(event.get("package_id", "")):
            continue

        out_dir_raw = event.get("out_dir")
        if not isinstance(out_dir_raw, str) or not out_dir_raw:
            raise ValueError("searchkernel-vespa-bindings build event has no out_dir")

        out_dir = Path(out_dir_raw).resolve()
        adapter_lib = (out_dir / "native-adapter" / "lib").resolve()
        try:
            adapter_lib.relative_to(target_root)
        except ValueError as exc:
            raise ValueError(
                f"native adapter library escaped Cargo target root: {adapter_lib}"
            ) from exc

        linked_paths = event.get("linked_paths", [])
        if not isinstance(linked_paths, list):
            raise ValueError(
                "searchkernel-vespa-bindings build event has malformed linked_paths"
            )
        resolved_link_paths = []
        for linked in linked_paths:
            if not isinstance(linked, str):
                continue
            value = linked[7:] if linked.startswith("native=") else linked
            resolved_link_paths.append(Path(value).resolve())
        if adapter_lib not in resolved_link_paths:
            raise ValueError(
                "Cargo build metadata did not publish the expected native adapter link path: "
                f"{adapter_lib}"
            )

        runtime_library = adapter_lib / ADAPTER_LIBRARY
        if not runtime_library.is_file():
            raise ValueError(
                f"native adapter runtime library is missing from Cargo link path: {adapter_lib}"
            )
        candidates.append(adapter_lib)

    unique = list(dict.fromkeys(candidates))
    if len(unique) != 1:
        raise ValueError(
            "expected exactly one Cargo-built searchkernel-vespa adapter library "
            f"directory, found {len(unique)}: {unique}"
        )
    return unique[0]


def main() -> int:
    if len(sys.argv) != 3:
        print(
            "usage: resolve-native-adapter-lib.py <cargo-messages.jsonl> <cargo-target-dir>",
            file=sys.stderr,
        )
        return 2

    try:
        resolved = resolve_adapter_lib(Path(sys.argv[1]), Path(sys.argv[2]))
    except (OSError, ValueError) as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        return 1

    print(resolved)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
