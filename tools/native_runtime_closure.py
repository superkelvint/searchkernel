#!/usr/bin/env python3
"""Shared ELF runtime-closure policy for Search Kernel native-base tooling."""

from __future__ import annotations

import pathlib
import re
import subprocess


# GNU/Linux C/C++ runtime baseline for the supported x86_64-unknown-linux-gnu
# native target. Product/dependency DSOs are deliberately absent: if Vespa
# needs one of those, the native-base artifact must carry it.
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
        raise RuntimeError(
            "could not inspect native runtime dependencies for {}:\n{}".format(
                path, completed.stderr.strip()
            )
        )
    return {
        match.group("name")
        for line in completed.stdout.splitlines()
        if (match := NEEDED_RE.search(line)) is not None
    }
