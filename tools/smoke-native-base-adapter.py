#!/usr/bin/env python3
"""Load a packaged Search Kernel adapter and call its stable ABI probe."""

from __future__ import annotations

import argparse
import ctypes
import pathlib
import sys


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("adapter", type=pathlib.Path)
    parser.add_argument("expected_abi", type=int)
    args = parser.parse_args()
    try:
        library = ctypes.CDLL(str(args.adapter), mode=ctypes.RTLD_LOCAL)
    except OSError as exc:
        raise SystemExit(f"dynamic loader could not load packaged Search Kernel adapter: {exc}") from exc
    try:
        abi_version = library.sk_native_abi_version
    except AttributeError as exc:
        raise SystemExit("loaded adapter does not export sk_native_abi_version") from exc
    abi_version.argtypes = []
    abi_version.restype = ctypes.c_uint32
    actual = abi_version()
    if actual != args.expected_abi:
        raise SystemExit(f"adapter ABI version mismatch: expected {args.expected_abi}, got {actual}")
    print(f"native adapter runtime load and ABI smoke: PASS ({actual})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
