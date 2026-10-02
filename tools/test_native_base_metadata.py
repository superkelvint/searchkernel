#!/usr/bin/env python3
"""Regression coverage for relocatable harvested CMake paths."""

from __future__ import annotations

import pathlib
import subprocess
import sys
import tempfile


ROOT = pathlib.Path(__file__).resolve().parents[1]
NORMALIZER = ROOT / "tools" / "normalize-native-base-metadata.py"
STATIC_COPY = ROOT / "tools" / "copy-native-base-static-link-inputs.py"


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="searchkernel-native-static.") as raw:
        root = pathlib.Path(raw)
        vespa = root / "vespa"
        deps = root / "deps"
        link_cwd = root / "build" / "searchcore" / "tests"
        link_dir = link_cwd / "CMakeFiles" / "fixture.dir"
        packaged_vespa = root / "packaged-vespa"
        packaged_deps = root / "packaged-deps"
        for directory in (vespa / "searchcore", deps / "lib", link_dir):
            directory.mkdir(parents=True, exist_ok=True)
        (vespa / "searchcore" / "absolute.a").write_bytes(b"absolute archive")
        (vespa / "searchcore" / "relative.a").write_bytes(b"relative archive")
        link = link_dir / "link.txt"
        link.write_text(
            "c++ /work/vespa/searchcore/absolute.a ../../../vespa/searchcore/relative.a\n",
            encoding="utf-8",
        )
        subprocess.run(
            [
                sys.executable, str(STATIC_COPY), str(link), str(vespa), str(deps),
                str(packaged_vespa), str(packaged_deps), "--producer-root",
                f"/work/vespa={vespa}",
            ],
            check=True,
        )
        if (packaged_vespa / "searchcore" / "absolute.a").read_bytes() != b"absolute archive":
            raise SystemExit("absolute static archive path was not normalized for the mounted producer")
        if (packaged_vespa / "searchcore" / "relative.a").read_bytes() != b"relative archive":
            raise SystemExit("relative ../../../vespa static archive path was not preserved")

    with tempfile.TemporaryDirectory(prefix="searchkernel-native-metadata.") as raw:
        root = pathlib.Path(raw)
        (root / "flags.make").write_text(
            "CXX_INCLUDES = -I/vespa/searchcore -isystem /producer/vespa-deps/include\n",
            encoding="utf-8",
        )
        original = "c++ ../../../vespa/searchcore/lib.a /vespa/searchcore/libabsolute.a\n"
        (root / "link.txt").write_text(original, encoding="utf-8")
        subprocess.run(
            [
                sys.executable,
                str(NORMALIZER),
                str(root),
                "/vespa",
                "/producer/vespa-deps",
            ],
            check=True,
        )
        flags = (root / "flags.make").read_text(encoding="utf-8")
        link = (root / "link.txt").read_text(encoding="utf-8")
        if "-I/work/vespa/searchcore" not in flags:
            raise SystemExit("absolute mounted Vespa include path was not normalized")
        if "-isystem /opt/vespa-deps/include" not in flags:
            raise SystemExit("absolute dependency include path was not normalized")
        if "../../../vespa/searchcore/lib.a" not in link:
            raise SystemExit("relative ../../../vespa CMake path was rewritten")
        if "../../../work/vespa/" in link:
            raise SystemExit("relative CMake path was corrupted into ../../../work/vespa")
        if "/work/vespa/searchcore/libabsolute.a" not in link:
            raise SystemExit("absolute mounted Vespa path was not normalized")
    print("native-base CMake path normalization regression: PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
