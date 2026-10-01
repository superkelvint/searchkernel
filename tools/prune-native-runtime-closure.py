#!/usr/bin/env python3
"""Remove producer candidates outside the runtime closure of adapter roots."""

from __future__ import annotations

import argparse
import json
import pathlib
import shutil
import sys

from native_runtime_closure import (
    SYSTEM_BASELINE,
    is_elf,
    ldd_resolutions,
    needed_libraries,
    resolve_origin_dependency,
)


def prune(libdir: pathlib.Path, roots: list[pathlib.Path]) -> int:
    root = libdir.resolve(strict=True)
    readelf = shutil.which("readelf")
    ldd = shutil.which("ldd")
    if readelf is None or ldd is None:
        raise RuntimeError("native runtime pruning requires readelf and ldd")
    queue = [path.resolve(strict=True) for path in roots]
    keep: set[pathlib.Path] = set()
    inspected: set[pathlib.Path] = set()
    while queue:
        payload = queue.pop(0)
        if payload in inspected:
            continue
        inspected.add(payload)
        resolutions = ldd_resolutions(payload, ldd, [root])
        for needed in sorted(needed_libraries(payload, readelf)):
            if needed in SYSTEM_BASELINE:
                continue
            if needed.startswith(("$ORIGIN/", "${ORIGIN}/")):
                dependency = resolve_origin_dependency(payload, needed)
                if dependency is None or not dependency.resolve(strict=True).is_relative_to(root):
                    raise RuntimeError(f"unpackaged origin-relative runtime dependency: {payload}: {needed}")
            else:
                if "/" in needed:
                    raise RuntimeError(f"absolute DT_NEEDED entry is not relocatable: {payload}: {needed}")
                dependency = resolutions.get(needed)
                if dependency is None:
                    raise RuntimeError(f"loader did not resolve dependency {needed} required by {payload}")
            resolved = dependency.resolve(strict=True)
            try:
                resolved.relative_to(root)
            except ValueError as exc:
                if needed in SYSTEM_BASELINE:
                    continue
                raise RuntimeError(f"non-system runtime dependency resolves outside bundle: {needed} -> {resolved}") from exc
            if not resolved.is_file() or not is_elf(resolved):
                raise RuntimeError(f"runtime dependency is not an ELF file: {needed} -> {resolved}")
            keep.add(resolved)
            queue.append(resolved)

    removed = 0
    for entry in sorted(libdir.iterdir(), key=lambda item: item.name):
        if entry.name == ".searchkernel-runtime-provenance.json":
            continue
        try:
            resolved = entry.resolve(strict=True)
        except (OSError, RuntimeError) as exc:
            raise RuntimeError(f"broken or escaping runtime entry {entry}: {exc}") from exc
        try:
            resolved.relative_to(root)
        except ValueError as exc:
            raise RuntimeError(f"runtime entry escapes bundle: {entry} -> {resolved}") from exc
        if resolved.is_file() and is_elf(resolved) and resolved not in keep:
            entry.unlink()
            removed += 1
    provenance_path = libdir / ".searchkernel-runtime-provenance.json"
    if provenance_path.exists():
        provenance = json.loads(provenance_path.read_text(encoding="utf-8"))
        provenance["libraries"] = [
            item for item in provenance.get("libraries", [])
            if isinstance(item, dict) and (libdir / item.get("packaged_name", "")).exists()
        ]
        provenance_path.write_text(
            json.dumps(provenance, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )
    return removed


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("libdir", type=pathlib.Path)
    parser.add_argument("--root-elf", type=pathlib.Path, action="append", required=True)
    args = parser.parse_args()
    try:
        removed = prune(args.libdir, args.root_elf)
    except (OSError, RuntimeError) as exc:
        raise SystemExit(str(exc)) from exc
    print(f"pruned {removed} unused native runtime ELF entries")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
