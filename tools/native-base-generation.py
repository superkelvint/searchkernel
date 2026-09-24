#!/usr/bin/env python3
"""Select native-base release generations from tag names on stdin."""

from __future__ import annotations

import argparse
import re
import sys


def versions(pattern: re.Pattern[str]) -> list[int]:
    found: list[int] = []
    for raw in sys.stdin:
        match = pattern.fullmatch(raw.strip())
        if match:
            found.append(int(match.group("generation")))
    return found


def main() -> int:
    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(dest="command", required=True)

    latest = subparsers.add_parser("latest")
    latest.add_argument("--build-env", required=True)
    latest.add_argument("--target", required=True)
    latest.add_argument("--commit", required=True)

    next_parser = subparsers.add_parser("next")
    next_parser.add_argument("--target", required=True)
    next_parser.add_argument("--commit", required=True)

    args = parser.parse_args()
    if args.command == "latest":
        pattern = re.compile(
            r"vespa-base-v(?P<generation>[0-9]+)-"
            + re.escape(args.build_env)
            + "-"
            + re.escape(args.target)
            + "-"
            + re.escape(args.commit)
        )
        found = versions(pattern)
        if not found:
            return 1
        print(max(found))
        return 0

    pattern = re.compile(
        r"vespa-base-v(?P<generation>[0-9]+)-(?:host|ci)-"
        + re.escape(args.target)
        + "-"
        + re.escape(args.commit)
    )
    found = versions(pattern)
    print(max(found, default=0) + 1)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
