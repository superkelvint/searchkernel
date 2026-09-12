#!/usr/bin/env python3
from __future__ import annotations

import argparse
import itertools
import pathlib
import sys


def parse(line: str, source: str, number: int) -> tuple[str, str]:
    line = line.rstrip("\n")
    if "\t" not in line:
        raise SystemExit(f"{source}:{number}: expected <input>\\t<stem>")
    return tuple(line.split("\t", 1))  # type: ignore[return-value]


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("expected", type=pathlib.Path)
    parser.add_argument("actual", type=pathlib.Path)
    parser.add_argument("--max-errors", type=int, default=50)
    args = parser.parse_args()

    mismatches = 0
    checked = 0
    with args.expected.open(encoding="utf-8") as ef, args.actual.open(encoding="utf-8") as af:
        for number, pair in enumerate(itertools.zip_longest(ef, af), 1):
            expected_line, actual_line = pair
            if expected_line is None:
                print(f"extra actual row at {number}: {actual_line.rstrip()!r}", file=sys.stderr)
                mismatches += 1
                break
            if actual_line is None:
                print(f"missing actual row at {number}: {expected_line.rstrip()!r}", file=sys.stderr)
                mismatches += 1
                break

            expected_input, expected_stem = parse(expected_line, str(args.expected), number)
            actual_input, actual_stem = parse(actual_line, str(args.actual), number)
            checked += 1

            if expected_input != actual_input:
                print(
                    f"row {number}: corpus drift: expected input {expected_input!r}, actual {actual_input!r}",
                    file=sys.stderr,
                )
                mismatches += 1
            elif expected_stem != actual_stem:
                print(
                    f"row {number}: {expected_input!r}: Vespa={expected_stem!r} Rust={actual_stem!r}",
                    file=sys.stderr,
                )
                mismatches += 1

            if mismatches >= args.max_errors:
                print(f"stopping after {mismatches} mismatches", file=sys.stderr)
                break

    if mismatches:
        print(f"KStem parity FAILED: {mismatches} mismatch(es), {checked} rows checked", file=sys.stderr)
        raise SystemExit(1)

    print(f"KStem parity passed: {checked} cases")


if __name__ == "__main__":
    main()
