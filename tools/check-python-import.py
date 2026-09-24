#!/usr/bin/env python3
"""Exit successfully when a Python module can be imported."""

from __future__ import annotations

import argparse
import importlib


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("module")
    args = parser.parse_args()
    importlib.import_module(args.module)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
