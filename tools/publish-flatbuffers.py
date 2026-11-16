#!/usr/bin/env python3
"""Publish a validated immutable toolchain without replacing legacy files.

Installers serialize provisioning; readers pin the current symlink's real path.
Retain every published generation because existing compiler/linker processes may
still use it. Never migrate a legacy directory by renaming it out from readers.
"""

import argparse
import os
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
CURRENT = ".searchkernel-current"


def publish(candidate: Path, prefix: Path, version: str) -> None:
    candidate = candidate.resolve(strict=True)
    prefix = prefix.absolute()
    if candidate == prefix or prefix in candidate.parents:
        raise ValueError("FlatBuffers candidate must be outside the published prefix")
    subprocess.run(
        ["bash", str(ROOT / "tools/check-flatbuffers-cpp.sh"), str(candidate), version],
        check=True,
    )
    prefix.mkdir(parents=True, exist_ok=True)
    pointer = prefix / CURRENT
    if os.path.lexists(pointer) and not pointer.is_symlink():
        raise ValueError(f"refusing to replace a non-symlink publication pointer: {pointer}")
    descriptor, temporary = tempfile.mkstemp(prefix=f"{CURRENT}.", dir=prefix)
    os.close(descriptor)
    os.unlink(temporary)
    try:
        os.symlink(candidate, temporary)
        os.replace(temporary, pointer)
    finally:
        if os.path.lexists(temporary):
            os.unlink(temporary)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--candidate", required=True, type=Path)
    parser.add_argument("--prefix", required=True, type=Path)
    parser.add_argument("--version", required=True)
    arguments = parser.parse_args()
    publish(arguments.candidate, arguments.prefix, arguments.version)
