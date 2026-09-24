#!/usr/bin/env python3
"""Validate that a native-base lib directory is self-contained for non-baseline DSOs."""

from __future__ import annotations

import pathlib
import re
import shutil
import subprocess
import sys


# These are part of the GNU/Linux C/C++ runtime baseline for the supported
# x86_64-unknown-linux-gnu native target. Product/dependency DSOs such as
# libxml2, libz, OpenSSL, ICU, etc. are intentionally *not* allowed here:
# if a Vespa DSO needs one of those, the native-base artifact must bundle it.
SYSTEM_BASELINE = {
    "libc.so.6",
    "libdl.so.2",
    "libgcc_s.so.1",
    "libm.so.6",
    "libpthread.so.0",
    "libresolv.so.2",
    "librt.so.1",
    "libstdc++.so.6",
    "libutil.so.1",
}

NEEDED_RE = re.compile(r"\(NEEDED\).*Shared library: \[(?P<name>[^\]]+)\]")


def die(message: str) -> "NoReturn":
    raise SystemExit(message)


def is_elf(path: pathlib.Path) -> bool:
    try:
        with path.open("rb") as stream:
            return stream.read(4) == b"\x7fELF"
    except OSError:
        return False


def needed_libraries(path: pathlib.Path, readelf: str) -> set[str]:
    completed = subprocess.run(
        [readelf, "-d", str(path)],
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if completed.returncode != 0:
        die(
            "could not inspect native runtime dependencies for {}:\n{}".format(
                path, completed.stderr.strip()
            )
        )
    return {
        match.group("name")
        for line in completed.stdout.splitlines()
        if (match := NEEDED_RE.search(line)) is not None
    }


def validate(libdir: pathlib.Path) -> None:
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

    missing: list[tuple[pathlib.Path, str]] = []
    for payload in sorted(payloads, key=lambda item: str(item)):
        for needed in sorted(needed_libraries(payload, readelf)):
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
    if len(sys.argv) != 2:
        print(
            "usage: tools/check-native-runtime-closure.py <lib-directory>",
            file=sys.stderr,
        )
        return 2

    validate(pathlib.Path(sys.argv[1]))
    print("native runtime dependency closure: PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
