#!/usr/bin/env python3
"""Rewrite only absolute producer roots in harvested CMake metadata."""

from __future__ import annotations

import argparse
import pathlib

from native_base_metadata import normalize_absolute_root


def normalize_tree(root: pathlib.Path, vespa_root: str, deps_root: str) -> None:
    for name in ("flags.make", "link.txt"):
        for path in root.rglob(name):
            text = path.read_text(encoding="utf-8")
            text = normalize_absolute_root(text, vespa_root, "/work/vespa")
            text = normalize_absolute_root(text, deps_root, "/opt/vespa-deps")
            path.write_text(text, encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("metadata_root", type=pathlib.Path)
    parser.add_argument("vespa_root")
    parser.add_argument("deps_root")
    args = parser.parse_args()
    normalize_tree(args.metadata_root, args.vespa_root, args.deps_root)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
