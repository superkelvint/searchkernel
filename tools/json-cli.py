#!/usr/bin/env python3
"""Small JSON operations for shell orchestration without inline Python."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any


def load_json(path: str) -> Any:
    if path == "-":
        return json.load(sys.stdin)
    with Path(path).open(encoding="utf-8") as stream:
        return json.load(stream)


def get_value(value: Any, key: str) -> Any:
    current = value
    for part in key.split("."):
        if isinstance(current, dict):
            current = current[part]
        elif isinstance(current, list):
            current = current[int(part)]
        else:
            raise KeyError(key)
    return current


def main() -> int:
    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(dest="command", required=True)

    get_parser = subparsers.add_parser("get")
    get_parser.add_argument("path")
    get_parser.add_argument("key")

    canonicalize_parser = subparsers.add_parser("canonicalize")
    canonicalize_parser.add_argument("path", nargs="?", default="-")

    args = parser.parse_args()
    if args.command == "get":
        value = get_value(load_json(args.path), args.key)
        if isinstance(value, (dict, list)):
            print(json.dumps(value, sort_keys=True, separators=(",", ":")))
        elif isinstance(value, bool):
            print("true" if value else "false")
        elif value is None:
            print("null")
        else:
            print(value)
        return 0

    if args.command == "canonicalize":
        print(json.dumps(load_json(args.path), sort_keys=True, separators=(",", ":")))
        return 0

    raise AssertionError(args.command)


if __name__ == "__main__":
    raise SystemExit(main())
