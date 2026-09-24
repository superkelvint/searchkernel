#!/usr/bin/env python3
"""Validate that a native-base lib directory is self-contained for non-baseline DSOs."""

from __future__ import annotations

import pathlib
import shutil
import sys

from native_runtime_closure import SYSTEM_BASELINE, is_elf, needed_libraries

def die(message: str) -> "NoReturn":
    raise SystemExit(message)


def validate(libdir: pathlib.Path, extra_elfs: list[pathlib.Path]) -> None:
    if not libdir.is_dir():
        die("native runtime lib directory does not exist: {}".format(libdir))

    readelf = shutil.which("readelf")
    if readelf is None:
        die("native runtime closure validation requires readelf")

    root = libdir.resolve()
    available: set[str] = set()
    payloads: dict[pathlib.Path, set[str]] = {}

    for entry in libdir.iterdir():
        try:
            resolved = entry.resolve(strict=True)
        except OSError as exc:
            die("broken native runtime entry {}: {}".format(entry, exc))

        try:
            resolved.relative_to(root)
        except ValueError:
            die(
                "native runtime entry escapes bundled lib directory: {} -> {}".format(
                    entry, resolved
                )
            )

        if not resolved.is_file() or not is_elf(resolved):
            continue
        available.add(entry.name)
        payloads.setdefault(resolved, set()).add(entry.name)

    if not payloads:
        die("native runtime lib directory contains no ELF shared libraries: {}".format(libdir))

    inspected = set(payloads)
    for extra in extra_elfs:
        try:
            resolved = extra.resolve(strict=True)
        except (OSError, RuntimeError) as exc:
            die("invalid extra native ELF {}: {}".format(extra, exc))
        if not resolved.is_file() or not is_elf(resolved):
            die("extra native runtime input is not an ELF file: {}".format(extra))
        inspected.add(resolved)

    missing: list[tuple[pathlib.Path, str]] = []
    for payload in sorted(inspected, key=lambda item: str(item)):
        try:
            needed_names = needed_libraries(payload, readelf)
        except RuntimeError as exc:
            die(str(exc))
        for needed in sorted(needed_names):
            if needed in SYSTEM_BASELINE or needed in available:
                continue
            missing.append((payload, needed))

    if missing:
        lines = [
            "{}: missing bundled runtime dependency {}".format(path.name, needed)
            for path, needed in missing
        ]
        die(
            "native runtime dependency closure is incomplete:\n  "
            + "\n  ".join(lines)
        )


def main() -> int:
    if len(sys.argv) < 2:
        print(
            "usage: tools/check-native-runtime-closure.py <lib-directory> [extra-elf ...]",
            file=sys.stderr,
        )
        return 2

    validate(
        pathlib.Path(sys.argv[1]),
        [pathlib.Path(value) for value in sys.argv[2:]],
    )
    print("native runtime dependency closure: PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
